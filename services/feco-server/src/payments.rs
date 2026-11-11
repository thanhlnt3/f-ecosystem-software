//! Lớp điều phối thanh toán (spec payments, nhóm task 13).
//!
//! - Mỗi lần khách bấm thanh toán một yêu cầu thanh toán của đơn hàng (`payment_requests`) tạo một
//!   giao dịch với cổng (`payment_transactions`), khóa idempotency (yêu cầu, lần thử); liên kết hết
//!   hạn sau 15 phút.
//! - Giao dịch chỉ chuyển sang Thành công khi có xác nhận đã kiểm chữ ký: callback của cổng hoặc
//!   truy vấn chủ động. Trang chuyển hướng phía trình duyệt chỉ kích hoạt một lần truy vấn.
//! - Một yêu cầu chỉ được thu thành công một lần: giao dịch thành công thứ hai được tự hoàn và
//!   quản trị viên được báo.
//! - Hoàn tiền: không vượt số đã thu; từ 10.000.000 ₫ cần hai quản trị viên phê duyệt.
//! - Hóa đơn sau mỗi giao dịch thành công, hóa đơn điều chỉnh khi hoàn tiền (nhà cung cấp hóa đơn
//!   điện tử thật: tasks.md 13.4; hiện là bản nháp nội bộ).
//! - Đối soát hằng ngày với cổng.

use axum::http::StatusCode;
use serde::Serialize;
use serde_json::{json, Value};
use time::{Duration, OffsetDateTime, UtcOffset};
use uuid::Uuid;

use crate::{
    audit,
    mailer::Attachment,
    orders::{self, problem, vnd, OrderError, OrderResult},
    zalopay::{self, OrderStatus, RefundStatus},
    AppState,
};

/// Liên kết thanh toán hết hạn sau 15 phút.
pub const LINK_MINUTES: i64 = 15;
/// Hoàn từ mức này trở lên cần hai quản trị viên.
pub const TWO_APPROVALS_FROM: i64 = 10_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TxStatus {
    Pending,
    Succeeded,
    Failed,
    Expired,
}

impl TxStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            TxStatus::Pending => "pending",
            TxStatus::Succeeded => "succeeded",
            TxStatus::Failed => "failed",
            TxStatus::Expired => "expired",
        }
    }

    pub fn parse(s: &str) -> TxStatus {
        match s {
            "succeeded" => TxStatus::Succeeded,
            "failed" => TxStatus::Failed,
            "expired" => TxStatus::Expired,
            _ => TxStatus::Pending,
        }
    }

    /// Bước chuyển của giao dịch: Chờ thanh toán → Thành công, Thất bại hoặc Hết hạn. Cổng vẫn có
    /// thể xác nhận đã thu tiền sau khi liên kết hết hạn hay bị báo thất bại: tiền đã thu thì ghi
    /// nhận (rồi hoàn nếu thừa), không bỏ qua.
    pub fn can_move(self, to: TxStatus) -> bool {
        use TxStatus::*;
        matches!(
            (self, to),
            (Pending, Succeeded | Failed | Expired) | (Failed | Expired, Succeeded)
        )
    }
}

pub fn method_label(method: &str, vi: bool) -> &'static str {
    match (method, vi) {
        ("vietqr", true) => "QR chuyển khoản (VietQR)",
        ("vietqr", false) => "Bank transfer QR (VietQR)",
        ("domestic_card", true) => "Thẻ ATM nội địa",
        ("domestic_card", false) => "Domestic ATM card",
        ("zalopay_wallet", true) => "Ví điện tử ZaloPay",
        ("zalopay_wallet", false) => "ZaloPay e-wallet",
        ("international_card", true) => "Thẻ quốc tế (Visa, Mastercard, JCB)",
        ("international_card", false) => "International card (Visa, Mastercard, JCB)",
        (_, true) => "Ghi nhận thủ công",
        (_, false) => "Recorded manually",
    }
}

fn gateway(state: &AppState) -> OrderResult<&zalopay::ZaloPay> {
    state.zalopay.as_ref().ok_or_else(|| {
        problem(
            StatusCode::SERVICE_UNAVAILABLE,
            "payments_unavailable",
            "Chưa cấu hình cổng thanh toán.",
            "No payment gateway is configured.",
        )
    })
}

fn upstream(e: zalopay::ZaloPayError) -> OrderError {
    tracing::warn!(error = %e, "lỗi khi gọi cổng thanh toán");
    problem(
        StatusCode::BAD_GATEWAY,
        "gateway_error",
        "Cổng thanh toán đang gặp sự cố, hãy thử lại sau ít phút.",
        "The payment gateway is having trouble; try again in a few minutes.",
    )
}

/// Bỏ mọi khóa trông giống dữ liệu thẻ trước khi lưu tin của cổng (spec "Không lưu dữ liệu thẻ").
pub fn scrub(v: &Value) -> Value {
    const CARD: [&str; 7] = [
        "card",
        "cvv",
        "cvc",
        "pan",
        "expiry",
        "exp_month",
        "exp_year",
    ];
    match v {
        Value::Object(m) => Value::Object(
            m.iter()
                .filter(|(k, _)| {
                    let k = k.to_ascii_lowercase();
                    !CARD.iter().any(|c| k.contains(c)) || k == "card_last4"
                })
                .map(|(k, v)| (k.clone(), scrub(v)))
                .collect(),
        ),
        Value::Array(a) => Value::Array(a.iter().map(scrub).collect()),
        other => other.clone(),
    }
}

async fn event(
    state: &AppState,
    gateway_ref: &str,
    source: &str,
    verified: bool,
    outcome: &str,
    payload: &Value,
) {
    let res = sqlx::query("INSERT INTO payment_events (gateway_ref, source, verified, outcome, payload) VALUES ($1, $2, $3, $4, $5)")
        .bind(gateway_ref)
        .bind(source)
        .bind(verified)
        .bind(outcome)
        .bind(scrub(payload))
        .execute(&state.db)
        .await;
    if let Err(e) = res {
        tracing::error!(error = %e, "không ghi được sự kiện thanh toán");
    }
}

async fn mail(state: &AppState, to: &str, subject: &str, body: &str) {
    if let Err(e) = state.mailer.send(to, subject, body).await {
        tracing::warn!(error = %e, "không gửi được email thanh toán");
    }
}

// ---------- Yêu cầu thanh toán và giao dịch ----------

/// Một khoản phải thu của đơn hàng, kèm thông tin để hiện trang thanh toán.
#[derive(Debug, Clone, Serialize)]
pub struct Charge {
    pub request_id: Uuid,
    pub order_id: Uuid,
    pub order_code: String,
    pub order_title: String,
    pub customer: Uuid,
    pub email: String,
    pub milestone: Option<i32>,
    pub milestone_title: String,
    pub amount_vnd: i64,
    pub status: String,
    pub reason: String,
}

type ChargeRow = (
    Uuid,
    Uuid,
    String,
    String,
    Uuid,
    String,
    Option<i32>,
    Option<String>,
    i64,
    String,
    String,
);

pub async fn charge(state: &AppState, request: Uuid) -> OrderResult<Charge> {
    let row: Option<ChargeRow> = sqlx::query_as(
        "SELECT r.id, o.id, o.code, o.title, o.customer, o.customer_email, m.position, m.title, r.amount_vnd, r.status, r.reason
         FROM payment_requests r JOIN orders o ON o.id = r.order_id LEFT JOIN order_milestones m ON m.id = r.milestone_id
         WHERE r.id = $1 AND r.kind = 'charge'",
    )
    .bind(request)
    .fetch_optional(&state.db)
    .await?;
    let r = row.ok_or_else(|| {
        problem(
            StatusCode::NOT_FOUND,
            "not_found",
            "Không có yêu cầu thanh toán này.",
            "No such payment request.",
        )
    })?;
    Ok(Charge {
        request_id: r.0,
        order_id: r.1,
        order_code: r.2,
        order_title: r.3,
        customer: r.4,
        email: r.5,
        milestone: r.6,
        milestone_title: r.7.unwrap_or_default(),
        amount_vnd: r.8,
        status: r.9,
        reason: r.10,
    })
}

/// Khoản phải thu của chính khách hàng này (của người khác là "không có").
pub async fn charge_of(state: &AppState, customer: Uuid, request: Uuid) -> OrderResult<Charge> {
    let c = charge(state, request).await?;
    if c.customer != customer {
        return Err(problem(
            StatusCode::NOT_FOUND,
            "not_found",
            "Không có yêu cầu thanh toán này.",
            "No such payment request.",
        ));
    }
    Ok(c)
}

#[derive(Debug, Clone, Serialize)]
pub struct Transaction {
    pub id: Uuid,
    pub request_id: Uuid,
    pub attempt: i32,
    pub provider: String,
    pub method: String,
    pub gateway_ref: String,
    pub amount_vnd: i64,
    pub status: TxStatus,
    pub order_url: Option<String>,
    pub gateway_trans_id: Option<String>,
    pub card_last4: Option<String>,
    pub refunded_vnd: i64,
    pub created_at: OffsetDateTime,
    pub expires_at: OffsetDateTime,
    pub settled_at: Option<OffsetDateTime>,
}

impl Transaction {
    /// Nhãn trạng thái cho người dùng, kể cả "Đã hoàn một phần" và "Đã hoàn tiền".
    pub fn label(&self, vi: bool) -> &'static str {
        match (self.status, vi) {
            (TxStatus::Succeeded, true) if self.refunded_vnd >= self.amount_vnd => "Đã hoàn tiền",
            (TxStatus::Succeeded, false) if self.refunded_vnd >= self.amount_vnd => "Refunded",
            (TxStatus::Succeeded, true) if self.refunded_vnd > 0 => "Đã hoàn một phần",
            (TxStatus::Succeeded, false) if self.refunded_vnd > 0 => "Partially refunded",
            (TxStatus::Succeeded, true) => "Thành công",
            (TxStatus::Succeeded, false) => "Succeeded",
            (TxStatus::Pending, true) => "Chờ thanh toán",
            (TxStatus::Pending, false) => "Awaiting payment",
            (TxStatus::Failed, true) => "Thất bại",
            (TxStatus::Failed, false) => "Failed",
            (TxStatus::Expired, true) => "Hết hạn",
            (TxStatus::Expired, false) => "Expired",
        }
    }
}

type TxRow = (
    Uuid,
    Uuid,
    i32,
    String,
    String,
    String,
    i64,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    i64,
    OffsetDateTime,
    OffsetDateTime,
    Option<OffsetDateTime>,
);

const TX_COLUMNS: &str = "t.id, t.request_id, t.attempt, t.provider, t.method, t.gateway_ref, t.amount_vnd, t.status, t.order_url, t.gateway_trans_id, t.card_last4, t.refunded_vnd, t.created_at, t.expires_at, t.settled_at";

fn tx_from(r: TxRow) -> Transaction {
    Transaction {
        id: r.0,
        request_id: r.1,
        attempt: r.2,
        provider: r.3,
        method: r.4,
        gateway_ref: r.5,
        amount_vnd: r.6,
        status: TxStatus::parse(&r.7),
        order_url: r.8,
        gateway_trans_id: r.9,
        card_last4: r.10,
        refunded_vnd: r.11,
        created_at: r.12,
        expires_at: r.13,
        settled_at: r.14,
    }
}

pub async fn transaction(state: &AppState, id: Uuid) -> OrderResult<Transaction> {
    let r: Option<TxRow> = sqlx::query_as(&format!(
        "SELECT {TX_COLUMNS} FROM payment_transactions t WHERE t.id = $1"
    ))
    .bind(id)
    .fetch_optional(&state.db)
    .await?;
    r.map(tx_from).ok_or_else(|| {
        problem(
            StatusCode::NOT_FOUND,
            "not_found",
            "Không có giao dịch này.",
            "No such transaction.",
        )
    })
}

pub async fn by_ref(state: &AppState, gateway_ref: &str) -> OrderResult<Transaction> {
    let r: Option<TxRow> = sqlx::query_as(&format!(
        "SELECT {TX_COLUMNS} FROM payment_transactions t WHERE t.gateway_ref = $1"
    ))
    .bind(gateway_ref)
    .fetch_optional(&state.db)
    .await?;
    r.map(tx_from).ok_or_else(|| {
        problem(
            StatusCode::NOT_FOUND,
            "not_found",
            "Không có giao dịch này.",
            "No such transaction.",
        )
    })
}

pub async fn transactions_of_request(
    state: &AppState,
    request: Uuid,
) -> OrderResult<Vec<Transaction>> {
    let rows: Vec<TxRow> =
        sqlx::query_as(&format!("SELECT {TX_COLUMNS} FROM payment_transactions t WHERE t.request_id = $1 ORDER BY t.attempt")).bind(request).fetch_all(&state.db).await?;
    Ok(rows.into_iter().map(tx_from).collect())
}

/// Khách hàng của một giao dịch: (id, email, mã đơn).
async fn owner(state: &AppState, tx: Uuid) -> OrderResult<(Uuid, String, String)> {
    Ok(sqlx::query_as(
        "SELECT o.customer, o.customer_email, o.code FROM payment_transactions t
         JOIN payment_requests r ON r.id = t.request_id JOIN orders o ON o.id = r.order_id WHERE t.id = $1",
    )
    .bind(tx)
    .fetch_one(&state.db)
    .await?)
}

#[derive(Debug, Clone)]
pub struct PayLink {
    pub transaction: Uuid,
    pub gateway_ref: String,
    pub order_url: String,
}

/// Bắt đầu (hoặc tiếp tục) thanh toán một khoản bằng phương thức khách chọn. Liên kết còn hạn của
/// cùng phương thức được dùng lại (idempotent); hết hạn hoặc đổi phương thức thì tạo lần thử mới.
pub async fn start(
    state: &AppState,
    customer: Uuid,
    request: Uuid,
    method: &str,
    now: OffsetDateTime,
) -> OrderResult<PayLink> {
    let zp = gateway(state)?;
    if !zalopay::METHODS.contains(&method) {
        return Err(problem(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "Hãy chọn một phương thức thanh toán.",
            "Choose a payment method.",
        ));
    }
    let c = charge_of(state, customer, request).await?;
    let mut db = state.db.begin().await?;
    let status: String =
        sqlx::query_scalar("SELECT status FROM payment_requests WHERE id = $1 FOR UPDATE")
            .bind(request)
            .fetch_one(&mut *db)
            .await?;
    match status.as_str() {
        "pending" => {}
        "succeeded" => {
            return Err(problem(
                StatusCode::CONFLICT,
                "already_paid",
                "Khoản này đã được thanh toán.",
                "This has already been paid.",
            ))
        }
        _ => {
            return Err(problem(
                StatusCode::CONFLICT,
                "not_payable",
                "Khoản này không còn cần thanh toán.",
                "This no longer needs paying.",
            ))
        }
    }
    let reusable: Option<(Uuid, String, Option<String>)> = sqlx::query_as(
        "SELECT id, gateway_ref, order_url FROM payment_transactions
         WHERE request_id = $1 AND status = 'pending' AND method = $2 AND expires_at > $3 AND order_url IS NOT NULL
         ORDER BY attempt DESC LIMIT 1",
    )
    .bind(request)
    .bind(method)
    .bind(now)
    .fetch_optional(&mut *db)
    .await?;
    if let Some((id, gateway_ref, Some(order_url))) = reusable {
        db.commit().await?;
        return Ok(PayLink {
            transaction: id,
            gateway_ref,
            order_url,
        });
    }
    let attempt: i32 = sqlx::query_scalar(
        "SELECT coalesce(max(attempt), 0) + 1 FROM payment_transactions WHERE request_id = $1",
    )
    .bind(request)
    .fetch_one(&mut *db)
    .await?;
    let id = Uuid::new_v4();
    let gateway_ref = format!(
        "{}_{}",
        zalopay::yymmdd(now),
        &id.simple().to_string()[..20]
    );
    sqlx::query(
        "INSERT INTO payment_transactions (id, request_id, attempt, provider, method, gateway_ref, amount_vnd, created_at, expires_at)
         VALUES ($1, $2, $3, 'zalopay', $4, $5, $6, $7, $8)",
    )
    .bind(id)
    .bind(request)
    .bind(attempt)
    .bind(method)
    .bind(&gateway_ref)
    .bind(c.amount_vnd)
    .bind(now)
    .bind(now + Duration::minutes(LINK_MINUTES))
    .execute(&mut *db)
    .await?;
    db.commit().await?;
    let description = match c.milestone {
        Some(p) => format!(
            "F-Eco - Đơn {} - mốc {p}: {}",
            c.order_code, c.milestone_title
        ),
        None => format!("F-Eco - Đơn {} - {}", c.order_code, c.reason),
    };
    let description: String = description.chars().take(200).collect();
    let redirect = format!("{}payments/return", state.public_url);
    let callback = format!("{}v1/payments/zalopay/callback", state.public_url);
    // Không gửi email hay tên khách cho cổng: chỉ một mã băm cố định theo người dùng.
    let app_user = format!("feco-{}", audit::hash_identifier(&customer.to_string()));
    let created = zp
        .create(&zalopay::NewOrder {
            app_trans_id: &gateway_ref,
            app_user: &app_user,
            amount: c.amount_vnd,
            description: &description,
            method,
            redirect_url: &redirect,
            callback_url: &callback,
            expire_seconds: LINK_MINUTES * 60,
            now,
        })
        .await;
    tracing::info!(
        gateway_ref,
        method,
        amount = c.amount_vnd,
        "tạo giao dịch với cổng thanh toán"
    );
    match created {
        Ok(created) => {
            sqlx::query("UPDATE payment_transactions SET order_url = $2 WHERE id = $1")
                .bind(id)
                .bind(&created.order_url)
                .execute(&state.db)
                .await?;
            Ok(PayLink {
                transaction: id,
                gateway_ref,
                order_url: created.order_url,
            })
        }
        Err(e) => {
            sqlx::query(
                "UPDATE payment_transactions SET status = 'failed', settled_at = $2 WHERE id = $1",
            )
            .bind(id)
            .bind(now)
            .execute(&state.db)
            .await?;
            Err(upstream(e))
        }
    }
}

/// Kết quả ghi nhận một xác nhận "đã thu tiền".
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Settled {
    /// Lần đầu: đơn hàng được cập nhật.
    Recorded,
    /// Đã ghi nhận trước đó (webhook lặp lại).
    Duplicate,
    /// Khoản này đã được thu bằng giao dịch khác (hoặc đơn đã hủy): đã tạo lệnh hoàn tự động.
    Refunded(Uuid),
    /// Số tiền cổng báo khác số tiền giao dịch: không ghi nhận, quản trị viên được báo.
    AmountMismatch,
}

/// Ghi nhận giao dịch thành công từ một nguồn đã xác minh (callback có chữ ký, truy vấn chủ động,
/// đối soát). Idempotent: gọi lại bao nhiêu lần cũng chỉ ghi nhận một lần.
pub async fn settle(
    state: &AppState,
    gateway_ref: &str,
    gateway_trans_id: &str,
    amount: i64,
    channel: Option<i64>,
    source: &str,
    now: OffsetDateTime,
) -> OrderResult<Settled> {
    let t = by_ref(state, gateway_ref).await?;
    tracing::info!(gateway_ref, source, amount, "cổng xác nhận đã thu tiền");
    if amount != t.amount_vnd {
        crate::review::admin_alert(
            state,
            "payment_amount_mismatch",
            &format!("Cổng báo số tiền {} khác giao dịch {} ({})", vnd(amount), gateway_ref, vnd(t.amount_vnd)),
            json!({"transaction": gateway_ref, "gateway_amount": amount, "amount": t.amount_vnd, "source": source}),
        )
        .await;
        return Ok(Settled::AmountMismatch);
    }
    let mut db = state.db.begin().await?;
    // Khóa yêu cầu thanh toán trước: hai giao dịch của cùng một khoản được xử lý lần lượt.
    let request_status: String =
        sqlx::query_scalar("SELECT status FROM payment_requests WHERE id = $1 FOR UPDATE")
            .bind(t.request_id)
            .fetch_one(&mut *db)
            .await?;
    let current: String =
        sqlx::query_scalar("SELECT status FROM payment_transactions WHERE id = $1 FOR UPDATE")
            .bind(t.id)
            .fetch_one(&mut *db)
            .await?;
    if !TxStatus::parse(&current).can_move(TxStatus::Succeeded) {
        db.commit().await?;
        return Ok(Settled::Duplicate);
    }
    sqlx::query("UPDATE payment_transactions SET status = 'succeeded', gateway_trans_id = $2, channel = $3, settled_at = $4 WHERE id = $1")
        .bind(t.id)
        .bind(gateway_trans_id)
        .bind(channel.map(|c| c as i32))
        .bind(now)
        .execute(&mut *db)
        .await?;
    let other_paid: bool =
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM payment_transactions WHERE request_id = $1 AND status = 'succeeded' AND id <> $2)")
            .bind(t.request_id)
            .bind(t.id)
            .fetch_one(&mut *db)
            .await?;
    db.commit().await?;
    issue_invoice(state, t.id).await;
    if other_paid || request_status != "pending" && request_status != "succeeded" {
        // Thu hai lần, hoặc khách trả sau khi đơn đã hủy: hoàn ngay giao dịch này.
        let why = if other_paid {
            "Mốc đã được thanh toán bằng giao dịch khác"
        } else {
            "Khoản này không còn cần thanh toán (đơn đã hủy)"
        };
        let refund =
            request_refund(state, t.id, t.amount_vnd, why, Requester::System, None).await?;
        crate::review::admin_alert(
            state,
            "payment_double_charge",
            &format!(
                "Giao dịch {gateway_ref} thu thừa {}: đã tự tạo lệnh hoàn",
                vnd(t.amount_vnd)
            ),
            json!({"transaction": gateway_ref, "request": t.request_id, "reason": why}),
        )
        .await;
        return Ok(Settled::Refunded(refund));
    }
    if request_status == "pending" {
        orders::payment_settled(state, t.request_id, true, now).await?;
    }
    let (_, email, code) = owner(state, t.id).await?;
    mail(
        state,
        &email,
        &format!("Đã nhận thanh toán đơn {code}"),
        &format!("F-Eco đã nhận {} cho đơn {code} (mã giao dịch {gateway_ref}). Hóa đơn được gửi trong email riêng.\n", vnd(t.amount_vnd)),
    )
    .await;
    Ok(Settled::Recorded)
}

/// Phản hồi cho callback của ZaloPay: `return_code` 1 đã nhận, -1 sai chữ ký, 0 lỗi tạm (cổng gửi lại).
pub async fn handle_callback(state: &AppState, body: &Value, now: OffsetDateTime) -> Value {
    let (Some(data), Some(mac)) = (body["data"].as_str(), body["mac"].as_str()) else {
        return json!({"return_code": -1, "return_message": "invalid body"});
    };
    let Some(zp) = state.zalopay.as_ref() else {
        return json!({"return_code": 0, "return_message": "payments disabled"});
    };
    let parsed: Value = serde_json::from_str(data).unwrap_or(Value::Null);
    let gateway_ref = parsed["app_trans_id"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    let Some(d) = zp.verify_callback(data, mac) else {
        event(
            state,
            &gateway_ref,
            "callback",
            false,
            "invalid_mac",
            &parsed,
        )
        .await;
        tracing::warn!(gateway_ref, "callback ZaloPay sai chữ ký");
        return json!({"return_code": -1, "return_message": "mac not equal"});
    };
    match settle(
        state,
        &d.app_trans_id,
        &d.zp_trans_id.to_string(),
        d.amount,
        Some(d.channel),
        "callback",
        now,
    )
    .await
    {
        Ok(outcome) => {
            event(
                state,
                &d.app_trans_id,
                "callback",
                true,
                &format!("{outcome:?}"),
                &parsed,
            )
            .await;
            json!({"return_code": 1, "return_message": "success"})
        }
        Err(OrderError::Problem {
            status: StatusCode::NOT_FOUND,
            ..
        }) => {
            event(
                state,
                &d.app_trans_id,
                "callback",
                true,
                "unknown_transaction",
                &parsed,
            )
            .await;
            json!({"return_code": 2, "return_message": "unknown transaction"})
        }
        Err(e) => {
            tracing::error!(error = %e, "không xử lý được callback ZaloPay");
            json!({"return_code": 0, "return_message": "retry"})
        }
    }
}

/// Truy vấn chủ động một giao dịch rồi cập nhật theo kết quả của cổng. Giao dịch quá hạn mà cổng
/// chưa thu được tiền thì chuyển "Hết hạn".
pub async fn confirm(
    state: &AppState,
    gateway_ref: &str,
    source: &str,
    now: OffsetDateTime,
) -> OrderResult<TxStatus> {
    let t = by_ref(state, gateway_ref).await?;
    if t.status == TxStatus::Succeeded || t.provider != "zalopay" {
        return Ok(t.status);
    }
    let result = gateway(state)?.query(gateway_ref).await.map_err(upstream)?;
    event(
        state,
        gateway_ref,
        source,
        true,
        &format!("{result:?}"),
        &json!({}),
    )
    .await;
    match result {
        OrderStatus::Paid {
            zp_trans_id,
            amount,
        } => {
            settle(state, gateway_ref, &zp_trans_id, amount, None, source, now).await?;
        }
        OrderStatus::Failed if t.status == TxStatus::Pending && now > t.expires_at => {
            set_status(state, t.id, TxStatus::Expired, now).await?
        }
        OrderStatus::Failed if t.status == TxStatus::Pending => {
            set_status(state, t.id, TxStatus::Failed, now).await?
        }
        OrderStatus::Pending { processing: false }
            if t.status == TxStatus::Pending && now > t.expires_at =>
        {
            set_status(state, t.id, TxStatus::Expired, now).await?
        }
        _ => {}
    }
    Ok(transaction(state, t.id).await?.status)
}

async fn set_status(
    state: &AppState,
    id: Uuid,
    to: TxStatus,
    now: OffsetDateTime,
) -> OrderResult<()> {
    sqlx::query("UPDATE payment_transactions SET status = $2, settled_at = $3 WHERE id = $1 AND status = 'pending'")
        .bind(id)
        .bind(to.as_str())
        .bind(now)
        .execute(&state.db)
        .await?;
    Ok(())
}

/// Ghi nhận thanh toán thủ công (chỉ dev và staging, khi chưa nối cổng): tạo một giao dịch
/// "manual" thành công cho khoản phải thu.
pub async fn record_manual(
    state: &AppState,
    request: Uuid,
    now: OffsetDateTime,
) -> OrderResult<Settled> {
    if state.environment.is_production() {
        return Err(problem(
            StatusCode::FORBIDDEN,
            "forbidden",
            "Production chỉ nhận xác nhận từ cổng thanh toán.",
            "Production only accepts gateway confirmations.",
        ));
    }
    let c = charge(state, request).await?;
    let id = Uuid::new_v4();
    let gateway_ref = format!("manual_{}", id.simple());
    let attempt: i32 = sqlx::query_scalar(
        "SELECT coalesce(max(attempt), 0) + 1 FROM payment_transactions WHERE request_id = $1",
    )
    .bind(request)
    .fetch_one(&state.db)
    .await?;
    sqlx::query(
        "INSERT INTO payment_transactions (id, request_id, attempt, provider, method, gateway_ref, amount_vnd, created_at, expires_at)
         VALUES ($1, $2, $3, 'manual', 'manual', $4, $5, $6, $6)",
    )
    .bind(id)
    .bind(request)
    .bind(attempt)
    .bind(&gateway_ref)
    .bind(c.amount_vnd)
    .bind(now)
    .execute(&state.db)
    .await?;
    event(state, &gateway_ref, "manual", true, "recorded", &json!({})).await;
    settle(
        state,
        &gateway_ref,
        &gateway_ref,
        c.amount_vnd,
        None,
        "manual",
        now,
    )
    .await
}

// ---------- Hoàn tiền ----------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Requester {
    /// Quản trị viên (tên); lệnh của họ tính là một phê duyệt.
    Admin(String),
    /// Hệ thống: hủy đơn (cần quản trị viên duyệt).
    System,
}

#[derive(Debug, Clone, Serialize)]
pub struct Refund {
    pub id: Uuid,
    pub transaction_id: Uuid,
    pub amount_vnd: i64,
    pub reason: String,
    pub kind: String,
    pub status: String,
    pub requested_by: String,
    pub approvals: Vec<String>,
    pub failure: Option<String>,
    pub created_at: OffsetDateTime,
}

impl Refund {
    pub fn approvals_needed(&self) -> usize {
        if self.kind == "double_charge" {
            0
        } else if self.amount_vnd >= TWO_APPROVALS_FROM {
            2
        } else {
            1
        }
    }

    pub fn label(&self, vi: bool) -> &'static str {
        match (self.status.as_str(), vi) {
            ("pending_approval", true) => "Chờ phê duyệt",
            ("pending_approval", false) => "Awaiting approval",
            ("processing", true) => "Đang hoàn",
            ("processing", false) => "Processing",
            ("succeeded", true) => "Đã hoàn",
            ("succeeded", false) => "Refunded",
            (_, true) => "Hoàn thất bại",
            (_, false) => "Refund failed",
        }
    }
}

type RefundRow = (
    Uuid,
    Uuid,
    i64,
    String,
    String,
    String,
    String,
    Vec<String>,
    Option<String>,
    OffsetDateTime,
);

const REFUND_COLUMNS: &str = "id, transaction_id, amount_vnd, reason, kind, status, requested_by, approvals, failure, created_at";

fn refund_from(r: RefundRow) -> Refund {
    Refund {
        id: r.0,
        transaction_id: r.1,
        amount_vnd: r.2,
        reason: r.3,
        kind: r.4,
        status: r.5,
        requested_by: r.6,
        approvals: r.7,
        failure: r.8,
        created_at: r.9,
    }
}

pub async fn refund(state: &AppState, id: Uuid) -> OrderResult<Refund> {
    let r: Option<RefundRow> = sqlx::query_as(&format!(
        "SELECT {REFUND_COLUMNS} FROM refunds WHERE id = $1"
    ))
    .bind(id)
    .fetch_optional(&state.db)
    .await?;
    r.map(refund_from).ok_or_else(|| {
        problem(
            StatusCode::NOT_FOUND,
            "not_found",
            "Không có lệnh hoàn này.",
            "No such refund.",
        )
    })
}

/// Lệnh hoàn đang chờ phê duyệt hoặc đang xử lý.
pub async fn open_refunds(state: &AppState) -> OrderResult<Vec<Refund>> {
    let rows: Vec<RefundRow> =
        sqlx::query_as(&format!("SELECT {REFUND_COLUMNS} FROM refunds WHERE status IN ('pending_approval', 'processing') ORDER BY created_at")).fetch_all(&state.db).await?;
    Ok(rows.into_iter().map(refund_from).collect())
}

/// Tạo lệnh hoàn cho một giao dịch thành công. Tổng hoàn không vượt số đã thu. Lệnh của quản trị
/// viên dưới 10.000.000 ₫ chạy ngay; từ 10.000.000 ₫ (hoặc lệnh của hệ thống) chờ phê duyệt.
pub async fn request_refund(
    state: &AppState,
    transaction: Uuid,
    amount: i64,
    reason: &str,
    by: Requester,
    order_request: Option<Uuid>,
) -> OrderResult<Uuid> {
    let reason = reason.trim();
    if amount <= 0 {
        return Err(problem(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "Số tiền hoàn phải lớn hơn 0.",
            "The refund amount must be above 0.",
        ));
    }
    if reason.is_empty() {
        return Err(problem(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "Hãy ghi lý do hoàn tiền.",
            "Give a reason for the refund.",
        ));
    }
    let id = Uuid::new_v4();
    let mut db = state.db.begin().await?;
    let row: Option<(String, i64)> = sqlx::query_as(
        "SELECT status, amount_vnd FROM payment_transactions WHERE id = $1 FOR UPDATE",
    )
    .bind(transaction)
    .fetch_optional(&mut *db)
    .await?;
    let (status, paid) = row.ok_or_else(|| {
        problem(
            StatusCode::NOT_FOUND,
            "not_found",
            "Không có giao dịch này.",
            "No such transaction.",
        )
    })?;
    if status != "succeeded" {
        return Err(problem(
            StatusCode::CONFLICT,
            "not_refundable",
            "Chỉ hoàn được giao dịch thành công.",
            "Only a succeeded transaction can be refunded.",
        ));
    }
    let committed: i64 = sqlx::query_scalar("SELECT coalesce(sum(amount_vnd), 0)::bigint FROM refunds WHERE transaction_id = $1 AND status <> 'failed'")
        .bind(transaction)
        .fetch_one(&mut *db)
        .await?;
    if committed + amount > paid {
        return Err(problem(
            StatusCode::CONFLICT,
            "refund_exceeds_payment",
            format!(
                "Tổng hoàn {} vượt số đã thu {}.",
                vnd(committed + amount),
                vnd(paid)
            ),
            format!(
                "Refunds totalling {} would exceed the {} collected.",
                vnd(committed + amount),
                vnd(paid)
            ),
        ));
    }
    let (kind, requested_by, approvals) = match (&by, order_request) {
        (Requester::System, None) => ("double_charge", "system".to_owned(), Vec::new()),
        (Requester::System, Some(_)) => ("order", "system".to_owned(), Vec::new()),
        (Requester::Admin(name), r) => (
            if r.is_some() { "order" } else { "manual" },
            name.clone(),
            vec![name.clone()],
        ),
    };
    sqlx::query(
        "INSERT INTO refunds (id, transaction_id, order_request_id, amount_vnd, reason, kind, status, requested_by, approvals)
         VALUES ($1, $2, $3, $4, $5, $6, 'pending_approval', $7, $8)",
    )
    .bind(id)
    .bind(transaction)
    .bind(order_request)
    .bind(amount)
    .bind(reason)
    .bind(kind)
    .bind(&requested_by)
    .bind(&approvals)
    .execute(&mut *db)
    .await?;
    db.commit().await?;
    audit::record(
        &state.db,
        audit_actor(&by),
        "payment.refund_request",
        Some(&id.to_string()),
        None,
        json!({"transaction": transaction, "amount": amount, "reason": reason}),
    )
    .await;
    let r = refund(state, id).await?;
    if r.approvals.len() >= r.approvals_needed() {
        execute_refund(state, id, OffsetDateTime::now_utc()).await?;
    } else {
        crate::review::admin_alert(
            state,
            "refund_approval",
            &format!(
                "Lệnh hoàn {} cần phê duyệt ({} người)",
                vnd(amount),
                r.approvals_needed()
            ),
            json!({"refund": id, "amount": amount, "reason": reason, "requested_by": requested_by}),
        )
        .await;
    }
    Ok(id)
}

fn audit_actor(by: &Requester) -> audit::Actor {
    match by {
        Requester::Admin(n) => audit::Actor::Admin(n.clone()),
        Requester::System => audit::Actor::System,
    }
}

/// Quản trị viên phê duyệt một lệnh hoàn. Người đã duyệt (hoặc người tạo lệnh) không duyệt lần hai.
pub async fn approve_refund(state: &AppState, id: Uuid, admin: &str) -> OrderResult<Refund> {
    let admin = admin.trim();
    let mut db = state.db.begin().await?;
    let row: Option<(String, Vec<String>)> =
        sqlx::query_as("SELECT status, approvals FROM refunds WHERE id = $1 FOR UPDATE")
            .bind(id)
            .fetch_optional(&mut *db)
            .await?;
    let (status, approvals) = row.ok_or_else(|| {
        problem(
            StatusCode::NOT_FOUND,
            "not_found",
            "Không có lệnh hoàn này.",
            "No such refund.",
        )
    })?;
    if status != "pending_approval" {
        return Err(problem(
            StatusCode::CONFLICT,
            "not_pending",
            "Lệnh hoàn không ở trạng thái chờ phê duyệt.",
            "The refund isn't awaiting approval.",
        ));
    }
    if admin.is_empty() || approvals.iter().any(|a| a == admin) {
        return Err(problem(
            StatusCode::FORBIDDEN,
            "second_approver_required",
            "Cần một quản trị viên khác phê duyệt.",
            "Another administrator must approve this.",
        ));
    }
    sqlx::query("UPDATE refunds SET approvals = array_append(approvals, $2) WHERE id = $1")
        .bind(id)
        .bind(admin)
        .execute(&mut *db)
        .await?;
    db.commit().await?;
    audit::record(
        &state.db,
        audit::Actor::Admin(admin.to_owned()),
        "payment.refund_approve",
        Some(&id.to_string()),
        None,
        json!({}),
    )
    .await;
    let r = refund(state, id).await?;
    if r.approvals.len() >= r.approvals_needed() {
        execute_refund(state, id, OffsetDateTime::now_utc()).await?;
    }
    refund(state, id).await
}

/// Gửi lệnh hoàn tới cổng, về đúng giao dịch (và phương thức) ban đầu.
async fn execute_refund(state: &AppState, id: Uuid, now: OffsetDateTime) -> OrderResult<()> {
    let r = refund(state, id).await?;
    let t = transaction(state, r.transaction_id).await?;
    if t.provider == "manual" {
        sqlx::query("UPDATE refunds SET status = 'processing', gateway_ref = $2 WHERE id = $1")
            .bind(id)
            .bind(format!("manual_{}", id.simple()))
            .execute(&state.db)
            .await?;
        return finish_refund(state, id, RefundStatus::Succeeded, now).await;
    }
    let zp = gateway(state)?;
    let m_refund_id = format!(
        "{}_{}_{}",
        zalopay::yymmdd(now),
        zp.app_id(),
        &id.simple().to_string()[..16]
    );
    sqlx::query("UPDATE refunds SET status = 'processing', gateway_ref = $2 WHERE id = $1")
        .bind(id)
        .bind(&m_refund_id)
        .execute(&state.db)
        .await?;
    let description: String = format!("F-Eco hoan tien: {}", r.reason)
        .chars()
        .take(100)
        .collect();
    let zp_trans_id = t.gateway_trans_id.clone().unwrap_or_default();
    match zp
        .refund(&m_refund_id, &zp_trans_id, r.amount_vnd, &description, now)
        .await
    {
        Ok(RefundStatus::Failed(why)) => {
            finish_refund(state, id, RefundStatus::Failed(why), now).await
        }
        Ok(_) => {
            // ZaloPay chỉ xác nhận đã nhận lệnh: hỏi ngay kết quả, chưa xong thì worker hỏi tiếp.
            match zp.query_refund(&m_refund_id, now).await {
                Ok(RefundStatus::Processing) | Err(_) => Ok(()),
                Ok(status) => finish_refund(state, id, status, now).await,
            }
        }
        Err(e) => {
            tracing::warn!(error = %e, "gửi lệnh hoàn thất bại, sẽ thử truy vấn lại");
            Ok(())
        }
    }
}

/// Kết thúc lệnh hoàn: cập nhật giao dịch, yêu cầu hoàn của đơn, gửi email và hóa đơn điều chỉnh.
async fn finish_refund(
    state: &AppState,
    id: Uuid,
    status: RefundStatus,
    now: OffsetDateTime,
) -> OrderResult<()> {
    let r = refund(state, id).await?;
    if r.status != "processing" {
        return Ok(());
    }
    let (_, email, code) = owner(state, r.transaction_id).await?;
    match status {
        RefundStatus::Processing => Ok(()),
        RefundStatus::Failed(why) => {
            sqlx::query(
                "UPDATE refunds SET status = 'failed', failure = $2, settled_at = $3 WHERE id = $1",
            )
            .bind(id)
            .bind(&why)
            .bind(now)
            .execute(&state.db)
            .await?;
            crate::review::admin_alert(
                state,
                "refund_failed",
                &format!("Hoàn {} cho đơn {code} thất bại: {why}", vnd(r.amount_vnd)),
                json!({"refund": id}),
            )
            .await;
            Ok(())
        }
        RefundStatus::Succeeded => {
            let mut db = state.db.begin().await?;
            sqlx::query("UPDATE refunds SET status = 'succeeded', settled_at = $2 WHERE id = $1")
                .bind(id)
                .bind(now)
                .execute(&mut *db)
                .await?;
            sqlx::query(
                "UPDATE payment_transactions SET refunded_vnd = refunded_vnd + $2 WHERE id = $1",
            )
            .bind(r.transaction_id)
            .bind(r.amount_vnd)
            .execute(&mut *db)
            .await?;
            sqlx::query("UPDATE payment_requests SET status = 'succeeded', settled_at = $2 WHERE id = (SELECT order_request_id FROM refunds WHERE id = $1) AND status = 'pending'")
                .bind(id)
                .bind(now)
                .execute(&mut *db)
                .await?;
            db.commit().await?;
            mail(
                state,
                &email,
                &format!("Hoàn tiền đơn {code}"),
                &format!("F-Eco đã hoàn {} cho đơn {code} về phương thức thanh toán ban đầu. Lý do: {}.\nHóa đơn điều chỉnh được gửi trong email riêng.\n", vnd(r.amount_vnd), r.reason),
            )
            .await;
            adjustment_invoice(state, id).await;
            Ok(())
        }
    }
}

/// Lệnh hoàn từ đơn hàng (hủy đơn, tranh chấp): tìm giao dịch đã thu của mốc rồi tạo lệnh hoàn.
pub async fn from_order_request(
    state: &AppState,
    request: Uuid,
    by: Requester,
) -> OrderResult<Option<Uuid>> {
    let row: Option<(i64, String, Option<Uuid>)> =
        sqlx::query_as("SELECT amount_vnd, reason, milestone_id FROM payment_requests WHERE id = $1 AND kind = 'refund'").bind(request).fetch_optional(&state.db).await?;
    let Some((amount, reason, Some(milestone))) = row else {
        return Ok(None);
    };
    let tx: Option<Uuid> = sqlx::query_scalar(
        "SELECT t.id FROM payment_transactions t JOIN payment_requests r ON r.id = t.request_id
         WHERE r.milestone_id = $1 AND r.kind = 'charge' AND t.status = 'succeeded' ORDER BY t.settled_at LIMIT 1",
    )
    .bind(milestone)
    .fetch_optional(&state.db)
    .await?;
    let Some(tx) = tx else {
        tracing::warn!(%request, "yêu cầu hoàn của đơn không có giao dịch đã thu");
        return Ok(None);
    };
    request_refund(state, tx, amount, &reason, by, Some(request))
        .await
        .map(Some)
}

// ---------- Việc theo thời gian ----------

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct DueReport {
    pub checked: usize,
    pub refunds_checked: usize,
}

/// Truy vấn chủ động giao dịch "Chờ thanh toán" đã quá 15 phút (spec "Liên kết hết hạn") và lệnh
/// hoàn đang xử lý. Chạy mỗi phút.
pub async fn process_due(state: &AppState, now: OffsetDateTime) -> OrderResult<DueReport> {
    let mut report = DueReport::default();
    if state.zalopay.is_none() {
        return Ok(report);
    }
    let due: Vec<String> = sqlx::query_scalar("SELECT gateway_ref FROM payment_transactions WHERE status = 'pending' AND provider = 'zalopay' AND expires_at <= $1 ORDER BY expires_at LIMIT 200")
        .bind(now)
        .fetch_all(&state.db)
        .await?;
    for gateway_ref in due {
        if let Err(e) = confirm(state, &gateway_ref, "query", now).await {
            tracing::warn!(error = %e, gateway_ref, "không truy vấn được giao dịch");
        }
        report.checked += 1;
    }
    let refunds: Vec<(Uuid, String)> =
        sqlx::query_as("SELECT id, gateway_ref FROM refunds WHERE status = 'processing' AND gateway_ref IS NOT NULL").fetch_all(&state.db).await?;
    let zp = gateway(state)?;
    for (id, m_refund_id) in refunds {
        match zp.query_refund(&m_refund_id, now).await {
            Ok(status) => finish_refund(state, id, status, now).await?,
            Err(e) => tracing::warn!(error = %e, "không truy vấn được lệnh hoàn"),
        }
        report.refunds_checked += 1;
    }
    Ok(report)
}

// ---------- Đối soát ----------

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Mismatch {
    pub gateway_ref: String,
    /// missing_webhook: cổng đã thu, F-Eco chưa ghi nhận; local_only: F-Eco ghi thành công, cổng
    /// không có; amount: số tiền khác nhau.
    pub kind: String,
    pub local_amount: i64,
    pub gateway_amount: i64,
}

fn day_bounds(day: time::Date) -> (OffsetDateTime, OffsetDateTime) {
    let vn = UtcOffset::from_hms(7, 0, 0).expect("GMT+7");
    let start = day.midnight().assume_offset(vn);
    (start, start + Duration::days(1))
}

/// Đối soát các giao dịch ZaloPay tạo trong một ngày (giờ Việt Nam) với kết quả truy vấn của cổng.
/// Chênh lệch được sửa nếu có thể (cổng đã thu → ghi nhận) và báo quản trị viên.
pub async fn reconcile(
    state: &AppState,
    day: time::Date,
    now: OffsetDateTime,
) -> OrderResult<Vec<Mismatch>> {
    let zp = gateway(state)?;
    let (from, to) = day_bounds(day);
    let rows: Vec<(String, i64, String)> = sqlx::query_as(
        "SELECT gateway_ref, amount_vnd, status FROM payment_transactions WHERE provider = 'zalopay' AND created_at >= $1 AND created_at < $2 ORDER BY created_at",
    )
    .bind(from)
    .bind(to)
    .fetch_all(&state.db)
    .await?;
    let mut mismatches = Vec::new();
    for (gateway_ref, amount, status) in &rows {
        let result = zp.query(gateway_ref).await.map_err(upstream)?;
        let local_paid = status == "succeeded";
        match result {
            OrderStatus::Paid {
                zp_trans_id,
                amount: paid,
            } => {
                if paid != *amount {
                    mismatches.push(Mismatch {
                        gateway_ref: gateway_ref.clone(),
                        kind: "amount".into(),
                        local_amount: *amount,
                        gateway_amount: paid,
                    });
                } else if !local_paid {
                    settle(
                        state,
                        gateway_ref,
                        &zp_trans_id,
                        paid,
                        None,
                        "reconcile",
                        now,
                    )
                    .await?;
                    mismatches.push(Mismatch {
                        gateway_ref: gateway_ref.clone(),
                        kind: "missing_webhook".into(),
                        local_amount: *amount,
                        gateway_amount: paid,
                    });
                }
            }
            _ if local_paid => mismatches.push(Mismatch {
                gateway_ref: gateway_ref.clone(),
                kind: "local_only".into(),
                local_amount: *amount,
                gateway_amount: 0,
            }),
            _ => {}
        }
    }
    sqlx::query(
        "INSERT INTO reconciliation_runs (day, checked, mismatches, ran_at) VALUES ($1, $2, $3, $4)
         ON CONFLICT (day) DO UPDATE SET checked = $2, mismatches = $3, ran_at = $4",
    )
    .bind(day)
    .bind(rows.len() as i32)
    .bind(json!(mismatches))
    .bind(now)
    .execute(&state.db)
    .await?;
    if !mismatches.is_empty() {
        let lines: Vec<String> = mismatches
            .iter()
            .map(|m| {
                format!(
                    "{} {}: F-Eco {} / cổng {}",
                    m.gateway_ref,
                    m.kind,
                    vnd(m.local_amount),
                    vnd(m.gateway_amount)
                )
            })
            .collect();
        crate::review::admin_alert(
            state,
            "reconciliation",
            &format!("Đối soát ngày {day}: {} chênh lệch", mismatches.len()),
            json!({"day": day.to_string(), "checked": rows.len(), "mismatches": lines}),
        )
        .await;
    }
    Ok(mismatches)
}

/// Đối soát ngày hôm qua (giờ Việt Nam) nếu chưa chạy. Gọi từ worker mỗi giờ.
pub async fn reconcile_if_due(state: &AppState, now: OffsetDateTime) -> OrderResult<Option<usize>> {
    if state.zalopay.is_none() {
        return Ok(None);
    }
    let vn = now.to_offset(UtcOffset::from_hms(7, 0, 0).expect("GMT+7"));
    // Chạy sau 2 giờ sáng, khi cổng đã chốt số liệu ngày trước.
    if vn.hour() < 2 {
        return Ok(None);
    }
    let yesterday = vn.date().previous_day().expect("có ngày hôm trước");
    let done: bool =
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM reconciliation_runs WHERE day = $1)")
            .bind(yesterday)
            .fetch_one(&state.db)
            .await?;
    if done {
        return Ok(None);
    }
    Ok(Some(reconcile(state, yesterday, now).await?.len()))
}

// ---------- Thông tin xuất hóa đơn và hóa đơn ----------

#[derive(Debug, Clone, Serialize, Default)]
pub struct BillingProfile {
    pub kind: String,
    pub name: String,
    pub tax_code: Option<String>,
    pub address: String,
    pub email: String,
}

pub async fn billing_profile(
    state: &AppState,
    subject: Uuid,
) -> OrderResult<Option<BillingProfile>> {
    let row: Option<(String, String, Option<String>, String, String)> = sqlx::query_as(
        "SELECT kind, name, tax_code, address, email FROM billing_profiles WHERE subject = $1",
    )
    .bind(subject)
    .fetch_optional(&state.db)
    .await?;
    Ok(row.map(|r| BillingProfile {
        kind: r.0,
        name: r.1,
        tax_code: r.2,
        address: r.3,
        email: r.4,
    }))
}

/// Mã số thuế Việt Nam: 10 chữ số, hoặc 10 chữ số kèm `-` và 3 chữ số (đơn vị phụ thuộc).
pub fn valid_tax_code(s: &str) -> bool {
    let (main, branch) = s.split_once('-').map_or((s, None), |(a, b)| (a, Some(b)));
    main.len() == 10
        && main.bytes().all(|b| b.is_ascii_digit())
        && branch.is_none_or(|b| b.len() == 3 && b.bytes().all(|c| c.is_ascii_digit()))
}

pub async fn save_billing_profile(
    state: &AppState,
    subject: Uuid,
    p: &BillingProfile,
) -> OrderResult<()> {
    let bad = |vi: &str, en: &str| problem(StatusCode::BAD_REQUEST, "invalid_request", vi, en);
    let name = p.name.trim();
    if name.is_empty() || name.chars().count() > 200 {
        return Err(bad(
            "Tên trên hóa đơn cần từ 1 đến 200 ký tự.",
            "The invoice name needs 1 to 200 characters.",
        ));
    }
    let tax_code = p
        .tax_code
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty());
    match (p.kind.as_str(), tax_code) {
        ("company", Some(t)) if valid_tax_code(t) => {}
        ("company", _) => {
            return Err(bad(
                "Doanh nghiệp cần mã số thuế hợp lệ (10 chữ số, hoặc 10 chữ số-3 chữ số).",
                "A company needs a valid tax code (10 digits, or 10 digits-3 digits).",
            ))
        }
        ("individual", None) => {}
        ("individual", Some(t)) if valid_tax_code(t) => {}
        ("individual", Some(_)) => {
            return Err(bad(
                "Mã số thuế không hợp lệ.",
                "The tax code is not valid.",
            ))
        }
        _ => {
            return Err(bad(
                "Chọn cá nhân hoặc doanh nghiệp.",
                "Choose individual or company.",
            ))
        }
    }
    sqlx::query(
        "INSERT INTO billing_profiles (subject, kind, name, tax_code, address, email) VALUES ($1, $2, $3, $4, $5, $6)
         ON CONFLICT (subject) DO UPDATE SET kind = $2, name = $3, tax_code = $4, address = $5, email = $6, updated_at = now()",
    )
    .bind(subject)
    .bind(&p.kind)
    .bind(name)
    .bind(tax_code)
    .bind(p.address.trim())
    .bind(p.email.trim())
    .execute(&state.db)
    .await?;
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
pub struct Invoice {
    pub id: Uuid,
    pub number: String,
    pub transaction_id: Uuid,
    pub kind: String,
    pub buyer_name: String,
    pub buyer_tax_code: Option<String>,
    pub amount_vnd: i64,
    pub provider: String,
    pub issued_at: OffsetDateTime,
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

struct InvoiceDoc<'a> {
    number: &'a str,
    kind: &'a str,
    adjusts: Option<&'a str>,
    buyer: &'a BillingProfile,
    line: &'a str,
    amount: i64,
    transaction: &'a str,
    issued: OffsetDateTime,
}

/// Hóa đơn dạng HTML. Bản nháp nội bộ: chưa phải hóa đơn điện tử hợp lệ cho tới khi tích hợp nhà
/// cung cấp được cấp phép (tasks.md 13.4).
fn render_invoice(d: &InvoiceDoc) -> String {
    let title = if d.kind == "adjustment" {
        "HÓA ĐƠN ĐIỀU CHỈNH (giảm)"
    } else {
        "HÓA ĐƠN"
    };
    let adjusts = d
        .adjusts
        .map(|n| format!("<p>Điều chỉnh cho hóa đơn số <b>{}</b></p>", esc(n)))
        .unwrap_or_default();
    let tax = d
        .buyer
        .tax_code
        .as_deref()
        .map(|t| format!("<tr><td>Mã số thuế</td><td>{}</td></tr>", esc(t)))
        .unwrap_or_default();
    format!(
        "<!doctype html><html lang=\"vi\"><head><meta charset=\"utf-8\"><title>{number}</title></head><body>\
         <p style=\"color:#b3261e\"><b>BẢN NHÁP</b>: chưa phải hóa đơn điện tử hợp lệ (đang chờ tích hợp nhà cung cấp hóa đơn điện tử).</p>\
         <h1>{title}</h1><p>Số: <b>{number}</b> · Ngày: {date}</p>{adjusts}\
         <h2>Người bán</h2><p>F-Eco</p>\
         <h2>Người mua</h2><table><tr><td>Tên</td><td>{name}</td></tr>{tax}<tr><td>Địa chỉ</td><td>{address}</td></tr><tr><td>Email</td><td>{email}</td></tr></table>\
         <h2>Nội dung</h2><table border=\"1\" cellpadding=\"6\"><tr><th>Diễn giải</th><th>Thành tiền</th></tr><tr><td>{line}</td><td>{amount}</td></tr>\
         <tr><td><b>Tổng cộng (đã gồm thuế)</b></td><td><b>{amount}</b></td></tr></table>\
         <p>Mã giao dịch: {transaction}</p></body></html>",
        number = esc(d.number),
        date = orders::date(d.issued),
        name = esc(&d.buyer.name),
        address = esc(&d.buyer.address),
        email = esc(&d.buyer.email),
        line = esc(d.line),
        amount = vnd(d.amount),
        transaction = esc(d.transaction),
    )
}

/// Phát hành hóa đơn cho giao dịch thành công (một lần) và gửi qua email. Lỗi không làm hỏng
/// việc ghi nhận thanh toán: quản trị viên được báo để phát hành lại.
pub async fn issue_invoice(state: &AppState, tx_id: Uuid) {
    if let Err(e) = try_issue_invoice(state, tx_id).await {
        tracing::error!(error = %e, %tx_id, "không phát hành được hóa đơn");
        crate::review::admin_alert(
            state,
            "invoice_failed",
            "Không phát hành được hóa đơn",
            json!({"transaction": tx_id}),
        )
        .await;
    }
}

async fn invoice_buyer(
    state: &AppState,
    customer: Uuid,
    email: &str,
) -> OrderResult<BillingProfile> {
    Ok(billing_profile(state, customer)
        .await?
        .unwrap_or(BillingProfile {
            kind: "individual".into(),
            name: email.to_owned(),
            tax_code: None,
            address: String::new(),
            email: email.to_owned(),
        }))
}

async fn try_issue_invoice(state: &AppState, tx_id: Uuid) -> OrderResult<()> {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM invoices WHERE transaction_id = $1 AND kind = 'original')",
    )
    .bind(tx_id)
    .fetch_one(&state.db)
    .await?;
    if exists {
        return Ok(());
    }
    let t = transaction(state, tx_id).await?;
    let c = charge(state, t.request_id).await?;
    let buyer = invoice_buyer(state, c.customer, &c.email).await?;
    let seq: i64 = sqlx::query_scalar("SELECT nextval('invoice_seq')")
        .fetch_one(&state.db)
        .await?;
    let number = format!("FECO-NHAP-{seq:07}");
    let line = match c.milestone {
        Some(p) => format!(
            "Dịch vụ xây app theo yêu cầu, đơn {}, mốc {p}: {}",
            c.order_code, c.milestone_title
        ),
        None => format!("Dịch vụ xây app theo yêu cầu, đơn {}", c.order_code),
    };
    let now = OffsetDateTime::now_utc();
    let doc = render_invoice(&InvoiceDoc {
        number: &number,
        kind: "original",
        adjusts: None,
        buyer: &buyer,
        line: &line,
        amount: t.amount_vnd,
        transaction: &t.gateway_ref,
        issued: now,
    });
    store_and_send(
        state,
        c.customer,
        tx_id,
        None,
        "original",
        None,
        &buyer,
        &number,
        t.amount_vnd,
        &doc,
        &format!("Hóa đơn thanh toán đơn {}", c.order_code),
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn store_and_send(
    state: &AppState,
    subject: Uuid,
    transaction: Uuid,
    refund: Option<Uuid>,
    kind: &str,
    adjusts: Option<Uuid>,
    buyer: &BillingProfile,
    number: &str,
    amount: i64,
    doc: &str,
    mail_subject: &str,
) -> OrderResult<()> {
    let inserted = sqlx::query(
        "INSERT INTO invoices (id, number, subject, transaction_id, refund_id, kind, adjusts, buyer_kind, buyer_name, buyer_tax_code, buyer_address, amount_vnd, provider, document)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, 'draft', $13) ON CONFLICT DO NOTHING",
    )
    .bind(Uuid::new_v4())
    .bind(number)
    .bind(subject)
    .bind(transaction)
    .bind(refund)
    .bind(kind)
    .bind(adjusts)
    .bind(&buyer.kind)
    .bind(&buyer.name)
    .bind(&buyer.tax_code)
    .bind(&buyer.address)
    .bind(amount)
    .bind(doc)
    .execute(&state.db)
    .await?
    .rows_affected();
    if inserted == 0 {
        return Ok(());
    }
    let attachment = Attachment {
        name: format!("{number}.html"),
        content_type: "text/html; charset=utf-8".into(),
        bytes: doc.as_bytes().to_vec(),
    };
    let body = format!(
        "Hóa đơn số {number} ({}) mang tên {}{}, số tiền {}.\nTải lại bất kỳ lúc nào ở Lịch sử thanh toán: {}payments\n",
        if kind == "adjustment" { "điều chỉnh giảm" } else { "thanh toán" },
        buyer.name,
        buyer.tax_code.as_deref().map(|t| format!(", mã số thuế {t}")).unwrap_or_default(),
        vnd(amount),
        state.public_url
    );
    if let Err(e) = state
        .mailer
        .send_with_attachment(&buyer.email, mail_subject, &body, &attachment)
        .await
    {
        tracing::warn!(error = %e, "không gửi được email hóa đơn");
    }
    Ok(())
}

/// Hóa đơn điều chỉnh giảm khi hoàn tiền.
async fn adjustment_invoice(state: &AppState, refund_id: Uuid) {
    let result: OrderResult<()> = async {
        let r = refund(state, refund_id).await?;
        let original: Option<(Uuid, String)> = sqlx::query_as(
            "SELECT id, number FROM invoices WHERE transaction_id = $1 AND kind = 'original'",
        )
        .bind(r.transaction_id)
        .fetch_optional(&state.db)
        .await?;
        let t = transaction(state, r.transaction_id).await?;
        let c = charge(state, t.request_id).await?;
        let buyer = invoice_buyer(state, c.customer, &c.email).await?;
        let seq: i64 = sqlx::query_scalar("SELECT nextval('invoice_seq')")
            .fetch_one(&state.db)
            .await?;
        let number = format!("FECO-NHAP-{seq:07}");
        let line = format!(
            "Điều chỉnh giảm do hoàn tiền đơn {}: {}",
            c.order_code, r.reason
        );
        let doc = render_invoice(&InvoiceDoc {
            number: &number,
            kind: "adjustment",
            adjusts: original.as_ref().map(|o| o.1.as_str()),
            buyer: &buyer,
            line: &line,
            amount: -r.amount_vnd,
            transaction: &t.gateway_ref,
            issued: OffsetDateTime::now_utc(),
        });
        store_and_send(
            state,
            c.customer,
            t.id,
            Some(refund_id),
            "adjustment",
            original.map(|o| o.0),
            &buyer,
            &number,
            -r.amount_vnd,
            &doc,
            &format!("Hóa đơn điều chỉnh đơn {}", c.order_code),
        )
        .await
    }
    .await;
    if let Err(e) = result {
        tracing::error!(error = %e, "không lập được hóa đơn điều chỉnh");
        crate::review::admin_alert(
            state,
            "invoice_failed",
            "Không lập được hóa đơn điều chỉnh",
            json!({"refund": refund_id}),
        )
        .await;
    }
}

// ---------- Lịch sử thanh toán ----------

#[derive(Debug, Clone, Serialize)]
pub struct HistoryItem {
    pub transaction: Transaction,
    pub order_id: Uuid,
    pub order_code: String,
    pub what: String,
    pub refunds: Vec<Refund>,
    pub invoices: Vec<Invoice>,
}

type InvoiceRow = (
    Uuid,
    String,
    Uuid,
    String,
    String,
    Option<String>,
    i64,
    String,
    OffsetDateTime,
);

pub async fn invoices_of(state: &AppState, subject: Uuid) -> OrderResult<Vec<Invoice>> {
    let rows: Vec<InvoiceRow> = sqlx::query_as(
        "SELECT id, number, transaction_id, kind, buyer_name, buyer_tax_code, amount_vnd, provider, issued_at FROM invoices WHERE subject = $1 ORDER BY issued_at",
    )
    .bind(subject)
    .fetch_all(&state.db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| Invoice {
            id: r.0,
            number: r.1,
            transaction_id: r.2,
            kind: r.3,
            buyer_name: r.4,
            buyer_tax_code: r.5,
            amount_vnd: r.6,
            provider: r.7,
            issued_at: r.8,
        })
        .collect())
}

/// Mọi giao dịch, hoàn tiền và hóa đơn của một khách hàng, mới nhất trước.
pub async fn history(state: &AppState, subject: Uuid) -> OrderResult<Vec<HistoryItem>> {
    let rows: Vec<(Uuid, Uuid, String, Option<i32>, String)> = sqlx::query_as(
        "SELECT t.id, o.id, o.code, m.position, r.reason FROM payment_transactions t
         JOIN payment_requests r ON r.id = t.request_id JOIN orders o ON o.id = r.order_id LEFT JOIN order_milestones m ON m.id = r.milestone_id
         WHERE o.customer = $1 ORDER BY t.created_at DESC",
    )
    .bind(subject)
    .fetch_all(&state.db)
    .await?;
    let invoices = invoices_of(state, subject).await?;
    let mut out = Vec::new();
    for (tx, order_id, order_code, position, reason) in rows {
        let refunds: Vec<RefundRow> = sqlx::query_as(&format!(
            "SELECT {REFUND_COLUMNS} FROM refunds WHERE transaction_id = $1 ORDER BY created_at"
        ))
        .bind(tx)
        .fetch_all(&state.db)
        .await?;
        out.push(HistoryItem {
            transaction: transaction(state, tx).await?,
            order_id,
            what: position.map(|p| format!("Mốc {p}")).unwrap_or(reason),
            order_code,
            refunds: refunds.into_iter().map(refund_from).collect(),
            invoices: invoices
                .iter()
                .filter(|i| i.transaction_id == tx)
                .cloned()
                .collect(),
        });
    }
    Ok(out)
}

/// Tài liệu hóa đơn (HTML) của chính khách hàng.
pub async fn invoice_document(
    state: &AppState,
    subject: Uuid,
    id: Uuid,
) -> OrderResult<(String, String)> {
    let row: Option<(String, String)> =
        sqlx::query_as("SELECT number, document FROM invoices WHERE id = $1 AND subject = $2")
            .bind(id)
            .bind(subject)
            .fetch_optional(&state.db)
            .await?;
    row.ok_or_else(|| {
        problem(
            StatusCode::NOT_FOUND,
            "not_found",
            "Không có hóa đơn này.",
            "No such invoice.",
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transaction_states() {
        use TxStatus::*;
        let all = [Pending, Succeeded, Failed, Expired];
        let allowed = [
            (Pending, Succeeded),
            (Pending, Failed),
            (Pending, Expired),
            (Failed, Succeeded),
            (Expired, Succeeded),
        ];
        for a in all {
            for b in all {
                assert_eq!(a.can_move(b), allowed.contains(&(a, b)), "{a:?} -> {b:?}");
            }
        }
        // Đã thành công thì webhook lặp lại không làm gì (Succeeded -> Succeeded không hợp lệ).
        assert!(!Succeeded.can_move(Succeeded));
    }

    #[test]
    fn tax_codes() {
        assert!(valid_tax_code("0101234567"));
        assert!(valid_tax_code("0101234567-001"));
        assert!(!valid_tax_code("010123456"));
        assert!(!valid_tax_code("0101234567-01"));
        assert!(!valid_tax_code("01012345AB"));
    }

    #[test]
    fn card_like_fields_are_dropped_before_storing() {
        let v = json!({"app_trans_id": "x", "card_number": "4111111111111111", "cvv": "987", "nested": {"pan": "1"}, "card_last4": "1111"});
        let s = scrub(&v);
        assert_eq!(
            s,
            json!({"app_trans_id": "x", "nested": {}, "card_last4": "1111"})
        );
    }

    #[test]
    fn refunds_from_ten_million_need_two_approvals() {
        let r = |amount, kind: &str| Refund {
            id: Uuid::nil(),
            transaction_id: Uuid::nil(),
            amount_vnd: amount,
            reason: String::new(),
            kind: kind.into(),
            status: "pending_approval".into(),
            requested_by: String::new(),
            approvals: Vec::new(),
            failure: None,
            created_at: OffsetDateTime::UNIX_EPOCH,
        };
        assert_eq!(r(9_999_999, "manual").approvals_needed(), 1);
        assert_eq!(r(10_000_000, "manual").approvals_needed(), 2);
        assert_eq!(r(15_000_000, "order").approvals_needed(), 2);
        assert_eq!(
            r(15_000_000, "double_charge").approvals_needed(),
            0,
            "thu hai lần: tự hoàn"
        );
    }

    #[test]
    fn invoice_shows_company_and_tax_code() {
        let buyer = BillingProfile {
            kind: "company".into(),
            name: "Công ty TNHH Kho <Vui>".into(),
            tax_code: Some("0101234567".into()),
            address: "Hà Nội".into(),
            email: "a@b.vn".into(),
        };
        let doc = render_invoice(&InvoiceDoc {
            number: "FECO-NHAP-0000001",
            kind: "original",
            adjusts: None,
            buyer: &buyer,
            line: "Mốc 1",
            amount: 9_000_000,
            transaction: "261111_x",
            issued: OffsetDateTime::UNIX_EPOCH,
        });
        assert!(
            doc.contains("Công ty TNHH Kho &lt;Vui&gt;")
                && doc.contains("0101234567")
                && doc.contains("9.000.000 ₫")
        );
        assert!(doc.contains("BẢN NHÁP"));
    }
}
