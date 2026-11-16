//! Ví F-Coin (thay đổi OpenSpec fcoin-wallet-and-marketplace, design R2–R6, R9).
//!
//! - Sổ cái ghi kép: mỗi bút toán (`wallet_txns`) có các dòng (`wallet_entries`) tổng bằng 0. Tài
//!   khoản hệ thống: `issuance` (đối ứng F-Coin phát hành, âm bằng tổng đang lưu hành), `escrow`,
//!   `withdrawals`, `fees`. Ví người dùng có ràng buộc số dư không âm ngay trong cơ sở dữ liệu.
//! - [`post`] khóa các tài khoản liên quan theo thứ tự id rồi mới kiểm khóa idempotency, nên hai
//!   lần gọi song song với cùng khóa chỉ ghi một lần, và hai lần trừ song song không làm âm ví.
//! - Đơn vị là nguyên F-Coin; số tiền nạp, rút = F-Coin × tỷ giá lúc tạo lệnh.
//! - F-Coin không chuyển tự do giữa người dùng: chỉ đi qua nạp, rút, mua app, ký quỹ dự án, và
//!   quản trị viên Tài chính chuyển hoặc cấp (thay đổi production-readiness, design K2).

use axum::http::StatusCode;
use serde::Serialize;
use serde_json::json;
use sqlx::{PgExecutor, Postgres, Transaction};
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

use crate::{
    audit::{self, Actor},
    gateways,
    i18n::Lang,
    notifications::{self, Category, Note},
    orders::{problem, vnd, OrderError, OrderResult},
    store::Db,
    AppState,
};

pub type Tx<'a> = Transaction<'a, Postgres>;

/// Giới hạn mỗi lần nạp (F-Coin).
pub const MIN_TOPUP: i64 = 10;
pub const MAX_TOPUP: i64 = 50_000;
/// Số F-Coin rút tối thiểu.
pub const MIN_WITHDRAWAL: i64 = 10;
/// Phí tối đa (phần vạn).
pub const MAX_FEE_BPS: i64 = 3_000;
/// Tỷ giá tối đa (₫ cho 1 F-Coin).
pub const MAX_RATE: i64 = 1_000_000;
/// Giá app tối đa (F-Coin).
pub const MAX_PRICE: i64 = 1_000_000;
/// Lệnh rút từ số tiền này (₫) cần hai người duyệt, như lệnh hoàn.
pub const TWO_APPROVALS_FROM: i64 = crate::payments::TWO_APPROVALS_FROM;

/// Mã các tài khoản hệ thống.
pub mod system {
    pub const ISSUANCE: &str = "issuance";
    pub const ESCROW: &str = "escrow";
    pub const WITHDRAWALS: &str = "withdrawals";
    pub const FEES: &str = "fees";
}

/// Loại bút toán.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Topup,
    WithdrawalHold,
    WithdrawalRelease,
    WithdrawalPaid,
    Purchase,
    EscrowFund,
    EscrowRelease,
    EscrowRefund,
    /// Quản trị viên chuyển từ ví của mình.
    AdminTransfer,
    /// Quản trị viên cấp từ quỹ nền tảng (tài khoản `issuance`).
    AdminGrant,
}

impl Kind {
    pub const ALL: [Kind; 10] = [
        Kind::Topup,
        Kind::WithdrawalHold,
        Kind::WithdrawalRelease,
        Kind::WithdrawalPaid,
        Kind::Purchase,
        Kind::EscrowFund,
        Kind::EscrowRelease,
        Kind::EscrowRefund,
        Kind::AdminTransfer,
        Kind::AdminGrant,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Topup => "topup",
            Kind::WithdrawalHold => "withdrawal_hold",
            Kind::WithdrawalRelease => "withdrawal_release",
            Kind::WithdrawalPaid => "withdrawal_paid",
            Kind::Purchase => "purchase",
            Kind::EscrowFund => "escrow_fund",
            Kind::EscrowRelease => "escrow_release",
            Kind::EscrowRefund => "escrow_refund",
            Kind::AdminTransfer => "admin_transfer",
            Kind::AdminGrant => "admin_grant",
        }
    }

    pub fn parse(s: &str) -> Option<Kind> {
        Kind::ALL.into_iter().find(|k| k.as_str() == s)
    }
}

/// Khóa i18n mô tả một dòng trong lịch sử của người dùng (mua và bán cùng loại `purchase`).
pub fn line_key(kind: Kind, amount: i64) -> &'static str {
    match (kind, amount >= 0) {
        (Kind::Topup, _) => "wallet.kind.topup",
        (Kind::WithdrawalHold, _) => "wallet.kind.withdrawal",
        (Kind::WithdrawalRelease, _) => "wallet.kind.withdrawal_release",
        (Kind::WithdrawalPaid, _) => "wallet.kind.withdrawal",
        (Kind::Purchase, false) => "wallet.kind.purchase",
        (Kind::Purchase, true) => "wallet.kind.sale",
        (Kind::EscrowFund, _) => "wallet.kind.escrow_fund",
        (Kind::EscrowRelease, _) => "wallet.kind.escrow_release",
        (Kind::EscrowRefund, _) => "wallet.kind.escrow_refund",
        (Kind::AdminTransfer | Kind::AdminGrant, true) => "wallet.kind.transfer_in",
        (Kind::AdminTransfer | Kind::AdminGrant, false) => "wallet.kind.transfer_out",
    }
}

/// Nhóm thống kê của một dòng: (là thu nhập?, khóa i18n của nhóm). Hoàn ký quỹ và hoàn lệnh rút
/// được trừ vào nhóm chi tương ứng, không tính là thu nhập.
pub fn category(kind: Kind, amount: i64) -> (bool, &'static str) {
    match kind {
        Kind::Topup => (true, "wallet.cat.topup"),
        Kind::Purchase if amount > 0 => (true, "wallet.cat.sales"),
        Kind::Purchase => (false, "wallet.cat.apps"),
        Kind::EscrowRelease => (true, "wallet.cat.project_income"),
        Kind::EscrowFund | Kind::EscrowRefund => (false, "wallet.cat.projects"),
        Kind::AdminTransfer | Kind::AdminGrant => (amount > 0, "wallet.cat.transfers"),
        Kind::WithdrawalHold | Kind::WithdrawalRelease | Kind::WithdrawalPaid => {
            (false, "wallet.cat.withdrawals")
        }
    }
}

/// Nhóm số theo hàng nghìn.
pub fn group(n: i64, sep: &str) -> String {
    let digits = n.unsigned_abs().to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push_str(sep);
        }
        out.push(c);
    }
    format!("{}{out}", if n < 0 { "-" } else { "" })
}

/// Số F-Coin theo cách viết của từng ngôn ngữ, ví dụ "1.250 F-Coin".
pub fn coin(n: i64, lang: Lang) -> String {
    let sep = match lang {
        Lang::Vi => ".",
        Lang::Fr => "\u{a0}",
        Lang::En | Lang::Ja => ",",
    };
    format!("{} F-Coin", group(n, sep))
}

/// Như [`coin`] nhưng luôn có dấu (+/−) để hiện trong lịch sử.
pub fn signed_coin(n: i64, lang: Lang) -> String {
    if n > 0 {
        format!("+{}", coin(n, lang))
    } else {
        coin(n, lang).replacen('-', "\u{2212}", 1)
    }
}

/// Phí theo phần vạn, làm tròn xuống (phần lẻ thuộc người nhận).
pub fn fee(amount: i64, bps: i64) -> i64 {
    amount.max(0) * bps.clamp(0, 10_000) / 10_000
}

fn insufficient(short: i64) -> OrderError {
    problem(
        StatusCode::PAYMENT_REQUIRED,
        "insufficient_funds",
        format!("Không đủ F-Coin: còn thiếu {}.", coin(short, Lang::Vi)),
        format!("Not enough F-Coin: {} short.", coin(short, Lang::En)),
    )
}

fn frozen_error() -> OrderError {
    problem(
        StatusCode::LOCKED,
        "wallet_frozen",
        "Ví F-Coin của bạn đang bị khóa. Hãy liên hệ bộ phận hỗ trợ.",
        "Your F-Coin wallet is frozen. Please contact support.",
    )
}

fn not_found(vi: &str, en: &str) -> OrderError {
    problem(StatusCode::NOT_FOUND, "not_found", vi, en)
}

fn bad(code: &'static str, vi: impl Into<String>, en: impl Into<String>) -> OrderError {
    problem(StatusCode::BAD_REQUEST, code, vi, en)
}

fn conflict(code: &'static str, vi: impl Into<String>, en: impl Into<String>) -> OrderError {
    problem(StatusCode::CONFLICT, code, vi, en)
}

// ---------- Sổ cái ----------

/// Một dòng của bút toán: tài khoản và số F-Coin (dương là ghi có).
#[derive(Debug, Clone, Copy)]
pub struct Leg {
    pub account: Uuid,
    pub amount: i64,
}

/// Thông tin chung của một bút toán.
#[derive(Debug, Clone, Copy)]
pub struct Posting<'a> {
    pub kind: Kind,
    /// Khóa idempotency: cùng khóa thì chỉ ghi một lần.
    pub idem: &'a str,
    pub memo: &'a str,
    pub reference: Option<&'a str>,
    pub by: &'a str,
}

#[derive(Debug, Clone)]
pub struct Posted {
    pub txn: Uuid,
    pub code: String,
    /// Bút toán đã có từ trước (cùng khóa idempotency): không ghi thêm.
    pub duplicate: bool,
}

/// Ghi một bút toán trong giao dịch `tx` của người gọi. Bỏ các dòng 0; tổng các dòng phải bằng 0.
/// Ví người dùng có số dư giảm phải không bị khóa và đủ F-Coin.
pub async fn post(tx: &mut Tx<'_>, p: &Posting<'_>, legs: &[Leg]) -> OrderResult<Posted> {
    let legs: Vec<Leg> = legs.iter().copied().filter(|l| l.amount != 0).collect();
    let sum: i64 = legs.iter().map(|l| l.amount).sum();
    if sum != 0 || legs.len() < 2 {
        tracing::error!(kind = p.kind.as_str(), sum, "bút toán không cân");
        return Err(problem(
            StatusCode::INTERNAL_SERVER_ERROR,
            "unbalanced",
            "Bút toán không cân.",
            "Unbalanced posting.",
        ));
    }
    let mut ids: Vec<Uuid> = legs.iter().map(|l| l.account).collect();
    ids.sort();
    ids.dedup();
    let accounts: Vec<(Uuid, String, i64, Option<OffsetDateTime>)> = sqlx::query_as(
        "SELECT id, kind, balance, frozen_at FROM wallet_accounts WHERE id = ANY($1) ORDER BY id FOR UPDATE",
    )
    .bind(&ids)
    .fetch_all(&mut **tx)
    .await?;
    if accounts.len() != ids.len() {
        return Err(not_found("Không có ví này.", "No such wallet."));
    }
    // Kiểm idempotency sau khi đã khóa: lần gọi thứ hai chờ lần đầu xong rồi thấy bút toán đã có.
    if let Some((txn, code)) =
        sqlx::query_as::<_, (Uuid, String)>("SELECT id, code FROM wallet_txns WHERE idem_key = $1")
            .bind(p.idem)
            .fetch_optional(&mut **tx)
            .await?
    {
        return Ok(Posted {
            txn,
            code,
            duplicate: true,
        });
    }
    for (id, kind, balance, frozen_at) in &accounts {
        let net: i64 = legs
            .iter()
            .filter(|l| l.account == *id)
            .map(|l| l.amount)
            .sum();
        if kind == "user" && net < 0 {
            if frozen_at.is_some() {
                return Err(frozen_error());
            }
            if balance + net < 0 {
                return Err(insufficient(-(balance + net)));
            }
        }
    }
    let txn = Uuid::new_v4();
    let n: i64 = sqlx::query_scalar("SELECT nextval('wallet_txn_seq')")
        .fetch_one(&mut **tx)
        .await?;
    let code = format!("GD-{n:06}");
    sqlx::query(
        "INSERT INTO wallet_txns (id, code, kind, memo, reference, idem_key, created_by) VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(txn)
    .bind(&code)
    .bind(p.kind.as_str())
    .bind(p.memo)
    .bind(p.reference)
    .bind(p.idem)
    .bind(p.by)
    .execute(&mut **tx)
    .await?;
    for leg in &legs {
        let balance: i64 = sqlx::query_scalar(
            "UPDATE wallet_accounts SET balance = balance + $2 WHERE id = $1 RETURNING balance",
        )
        .bind(leg.account)
        .bind(leg.amount)
        .fetch_one(&mut **tx)
        .await?;
        sqlx::query(
            "INSERT INTO wallet_entries (txn_id, account_id, amount, balance_after) VALUES ($1, $2, $3, $4)",
        )
        .bind(txn)
        .bind(leg.account)
        .bind(leg.amount)
        .bind(balance)
        .execute(&mut **tx)
        .await?;
    }
    Ok(Posted {
        txn,
        code,
        duplicate: false,
    })
}

/// Ví của người dùng (tạo khi cần).
pub async fn user_account(
    tx: &mut Tx<'_>,
    subject: Uuid,
    email: Option<&str>,
) -> sqlx::Result<Uuid> {
    sqlx::query_scalar(
        "INSERT INTO wallet_accounts (id, kind, subject, email) VALUES ($1, 'user', $2, $3)
         ON CONFLICT (subject) DO UPDATE SET email = coalesce(EXCLUDED.email, wallet_accounts.email)
         RETURNING id",
    )
    .bind(Uuid::new_v4())
    .bind(subject)
    .bind(
        email
            .map(|e| e.trim().to_ascii_lowercase())
            .filter(|e| !e.is_empty()),
    )
    .fetch_one(&mut **tx)
    .await
}

/// Tài khoản hệ thống theo mã (xem [`system`]).
pub async fn system_account<'c>(e: impl PgExecutor<'c>, code: &str) -> sqlx::Result<Uuid> {
    sqlx::query_scalar("SELECT id FROM wallet_accounts WHERE code = $1")
        .bind(code)
        .fetch_one(e)
        .await
}

async fn ensure_not_frozen(tx: &mut Tx<'_>, account: Uuid) -> OrderResult<()> {
    let frozen: Option<OffsetDateTime> =
        sqlx::query_scalar("SELECT frozen_at FROM wallet_accounts WHERE id = $1")
            .bind(account)
            .fetch_one(&mut **tx)
            .await?;
    if frozen.is_some() {
        return Err(frozen_error());
    }
    Ok(())
}

// ---------- Tỷ giá và phí ----------

/// Tỷ giá hiện hành: số đồng cho 1 F-Coin.
pub async fn rate<'c>(e: impl PgExecutor<'c>) -> sqlx::Result<i64> {
    Ok(
        sqlx::query_scalar("SELECT vnd_per_coin FROM coin_rates ORDER BY id DESC LIMIT 1")
            .fetch_optional(e)
            .await?
            .unwrap_or(1000),
    )
}

#[derive(Debug, Clone, Serialize)]
pub struct RateRow {
    pub vnd_per_coin: i64,
    pub set_by: String,
    pub reason: String,
    pub set_at: OffsetDateTime,
}

pub async fn rate_history(db: &Db, limit: i64) -> sqlx::Result<Vec<RateRow>> {
    let rows: Vec<(i64, String, String, OffsetDateTime)> = sqlx::query_as(
        "SELECT vnd_per_coin, set_by, reason, set_at FROM coin_rates ORDER BY id DESC LIMIT $1",
    )
    .bind(limit)
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| RateRow {
            vnd_per_coin: r.0,
            set_by: r.1,
            reason: r.2,
            set_at: r.3,
        })
        .collect())
}

/// Đặt tỷ giá mới (vai trò con Tài chính, kiểm ở tầng web). Chỉ áp dụng cho lệnh tạo sau đó.
pub async fn set_rate(
    state: &AppState,
    admin: &str,
    vnd_per_coin: i64,
    reason: &str,
) -> OrderResult<()> {
    let reason = reason.trim();
    if reason.is_empty() {
        return Err(bad("reason_required", "Hãy ghi lý do.", "Give a reason."));
    }
    if !(1..=MAX_RATE).contains(&vnd_per_coin) {
        return Err(bad(
            "invalid_rate",
            "Tỷ giá phải từ 1 đến 1.000.000 ₫ cho 1 F-Coin.",
            "The rate must be between 1 and 1,000,000 VND per F-Coin.",
        ));
    }
    let before = rate(&state.db).await?;
    if !rate_change_allowed(before, vnd_per_coin) {
        return Err(bad(
            "rate_change_too_large",
            "Mỗi lần chỉ đổi tỷ giá tối đa 20% so với tỷ giá hiện hành.",
            "Each change may move the rate by at most 20% of the current rate.",
        ));
    }
    sqlx::query("INSERT INTO coin_rates (vnd_per_coin, set_by, reason) VALUES ($1, $2, $3)")
        .bind(vnd_per_coin)
        .bind(admin)
        .bind(reason)
        .execute(&state.db)
        .await?;
    audit::record(
        &state.db,
        Actor::Admin(admin.to_owned()),
        "wallet.rate_set",
        None,
        None,
        json!({"from": before, "to": vnd_per_coin, "reason": reason}),
    )
    .await;
    Ok(())
}

/// Một lần đổi tỷ giá không vượt quá 20% tỷ giá hiện hành (một quản trị viên không đẩy tỷ giá đi
/// xa trong một bước rồi rút tiền theo tỷ giá đó).
pub fn rate_change_allowed(current: i64, new: i64) -> bool {
    let current = current.max(1) as i128;
    let diff = (new as i128 - current).abs();
    diff * 5 <= current
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct Fees {
    pub app_bps: i64,
    pub project_bps: i64,
}

pub async fn fees<'c>(e: impl PgExecutor<'c>) -> sqlx::Result<Fees> {
    let rows: Vec<(String, i64)> = sqlx::query_as("SELECT key, value FROM wallet_settings")
        .fetch_all(e)
        .await?;
    let get = |k: &str| rows.iter().find(|r| r.0 == k).map(|r| r.1).unwrap_or(0);
    Ok(Fees {
        app_bps: get("app_fee_bps"),
        project_bps: get("project_fee_bps"),
    })
}

pub async fn set_fees(state: &AppState, admin: &str, new: Fees) -> OrderResult<()> {
    for v in [new.app_bps, new.project_bps] {
        if !(0..=MAX_FEE_BPS).contains(&v) {
            return Err(bad(
                "invalid_fee",
                "Phí phải từ 0 đến 3.000 phần vạn (30%).",
                "Fees must be between 0 and 3,000 basis points (30%).",
            ));
        }
    }
    let before = fees(&state.db).await?;
    let mut tx = state.db.begin().await?;
    for (k, v) in [
        ("app_fee_bps", new.app_bps),
        ("project_fee_bps", new.project_bps),
    ] {
        sqlx::query(
            "UPDATE wallet_settings SET value = $2, updated_by = $3, updated_at = now() WHERE key = $1",
        )
        .bind(k)
        .bind(v)
        .bind(admin)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    audit::record(
        &state.db,
        Actor::Admin(admin.to_owned()),
        "wallet.fees_set",
        None,
        None,
        json!({"from": before, "to": new}),
    )
    .await;
    Ok(())
}

// ---------- Số dư ----------

#[derive(Debug, Clone, Default, Serialize)]
pub struct Summary {
    pub balance: i64,
    /// Lý do khóa nếu ví đang bị khóa.
    pub frozen: Option<String>,
    pub rate: i64,
}

pub async fn summary(db: &Db, subject: Uuid) -> sqlx::Result<Summary> {
    let row: Option<(i64, Option<OffsetDateTime>, Option<String>)> = sqlx::query_as(
        "SELECT balance, frozen_at, frozen_reason FROM wallet_accounts WHERE subject = $1",
    )
    .bind(subject)
    .fetch_optional(db)
    .await?;
    let (balance, frozen) = match row {
        Some((b, Some(_), reason)) => (b, Some(reason.unwrap_or_default())),
        Some((b, None, _)) => (b, None),
        None => (0, None),
    };
    Ok(Summary {
        balance,
        frozen,
        rate: rate(db).await?,
    })
}

// ---------- Nạp ----------

#[derive(Debug, Clone)]
pub struct TopupStart {
    pub topup: Uuid,
    pub request: Uuid,
    pub code: String,
    pub amount_vnd: i64,
}

/// Tạo lệnh nạp và khoản phải thu tương ứng (design R4). Người dùng được chuyển tới trang thanh
/// toán sẵn có `/payments/{request}/pay`.
pub async fn create_topup(
    state: &AppState,
    subject: Uuid,
    email: &str,
    coins: i64,
) -> OrderResult<TopupStart> {
    if !gateways::any(state) {
        return Err(problem(
            StatusCode::SERVICE_UNAVAILABLE,
            "payments_unavailable",
            "Chưa cấu hình cổng thanh toán.",
            "No payment gateway is configured.",
        ));
    }
    if !(MIN_TOPUP..=MAX_TOPUP).contains(&coins) {
        return Err(bad(
            "invalid_amount",
            "Số F-Coin nạp phải từ 10 đến 50.000.",
            "You can top up between 10 and 50,000 F-Coin.",
        ));
    }
    let mut tx = state.db.begin().await?;
    let account = user_account(&mut tx, subject, Some(email)).await?;
    ensure_not_frozen(&mut tx, account).await?;
    let per_coin = rate(&mut *tx).await?;
    let amount_vnd = coins * per_coin;
    let n: i64 = sqlx::query_scalar("SELECT nextval('topup_seq')")
        .fetch_one(&mut *tx)
        .await?;
    let code = format!("NAP-{n:06}");
    let topup = Uuid::new_v4();
    let request = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO wallet_topups (id, code, subject, email, coins, vnd_per_coin, amount_vnd) VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(topup)
    .bind(&code)
    .bind(subject)
    .bind(email.trim().to_ascii_lowercase())
    .bind(coins)
    .bind(per_coin)
    .bind(amount_vnd)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "INSERT INTO payment_requests (id, order_id, topup_id, kind, amount_vnd, reason) VALUES ($1, NULL, $2, 'charge', $3, $4)",
    )
    .bind(request)
    .bind(topup)
    .bind(amount_vnd)
    .bind(format!("Nạp {coins} F-Coin"))
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    audit::record(
        &state.db,
        Actor::User(subject),
        "wallet.topup_create",
        Some(&code),
        None,
        json!({"coins": coins, "vnd_per_coin": per_coin, "amount_vnd": amount_vnd}),
    )
    .await;
    Ok(TopupStart {
        topup,
        request,
        code,
        amount_vnd,
    })
}

/// Cổng xác nhận đã thu tiền của một lệnh nạp: cộng F-Coin đúng một lần (idempotency `topup:<id>`).
pub async fn topup_settled(
    state: &AppState,
    request: Uuid,
    now: OffsetDateTime,
) -> OrderResult<()> {
    let mut tx = state.db.begin().await?;
    let row: Option<(Uuid, String, Uuid, String, i64, String)> = sqlx::query_as(
        "SELECT w.id, w.code, w.subject, w.email, w.coins, w.status FROM wallet_topups w
         JOIN payment_requests r ON r.topup_id = w.id WHERE r.id = $1 FOR UPDATE OF w",
    )
    .bind(request)
    .fetch_optional(&mut *tx)
    .await?;
    let Some((id, code, subject, email, coins, status)) = row else {
        return Err(not_found("Không có lệnh nạp này.", "No such top-up."));
    };
    if status != "pending" {
        tx.commit().await?;
        return Ok(());
    }
    let user = user_account(&mut tx, subject, Some(&email)).await?;
    let issuance = system_account(&mut *tx, system::ISSUANCE).await?;
    let memo = format!("Nạp {coins} F-Coin ({code})");
    let posted = post(
        &mut tx,
        &Posting {
            kind: Kind::Topup,
            idem: &format!("topup:{id}"),
            memo: &memo,
            reference: Some(&code),
            by: "system",
        },
        &[
            Leg {
                account: issuance,
                amount: -coins,
            },
            Leg {
                account: user,
                amount: coins,
            },
        ],
    )
    .await?;
    sqlx::query(
        "UPDATE wallet_topups SET status = 'succeeded', txn_id = $2, settled_at = $3 WHERE id = $1",
    )
    .bind(id)
    .bind(posted.txn)
    .bind(now)
    .execute(&mut *tx)
    .await?;
    sqlx::query("UPDATE payment_requests SET status = 'succeeded', settled_at = $2 WHERE id = $1 AND status = 'pending'")
        .bind(request)
        .bind(now)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    let note = Note::vi(
        format!("Đã nạp {}", coin(coins, Lang::Vi)),
        format!(
            "Ví F-Coin của bạn vừa được cộng {} (lệnh {code}, giao dịch {}).",
            coin(coins, Lang::Vi),
            posted.code
        ),
        "/wallet",
    )
    .en(
        format!("{} added", coin(coins, Lang::En)),
        format!(
            "{} was added to your F-Coin wallet (top-up {code}, transaction {}).",
            coin(coins, Lang::En),
            posted.code
        ),
    );
    notifications::notify(state, subject, Some(&email), Category::Payment, &note).await;
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
pub struct TopupRow {
    pub id: Uuid,
    pub code: String,
    pub coins: i64,
    pub amount_vnd: i64,
    pub status: String,
    pub created_at: OffsetDateTime,
    /// Khoản phải thu (để thanh toán tiếp khi còn chờ).
    pub request: Option<Uuid>,
}

pub async fn topups_of(db: &Db, subject: Uuid, limit: i64) -> sqlx::Result<Vec<TopupRow>> {
    type Row = (Uuid, String, i64, i64, String, OffsetDateTime, Option<Uuid>);
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT w.id, w.code, w.coins, w.amount_vnd, w.status, w.created_at, r.id FROM wallet_topups w
         LEFT JOIN payment_requests r ON r.topup_id = w.id
         WHERE w.subject = $1 ORDER BY w.created_at DESC LIMIT $2",
    )
    .bind(subject)
    .bind(limit)
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| TopupRow {
            id: r.0,
            code: r.1,
            coins: r.2,
            amount_vnd: r.3,
            status: r.4,
            created_at: r.5,
            request: r.6,
        })
        .collect())
}

/// Thông tin hiển thị của lệnh nạp gắn với một khoản phải thu (trang thanh toán).
pub async fn topup_of_request(db: &Db, request: Uuid) -> sqlx::Result<Option<(String, i64)>> {
    sqlx::query_as(
        "SELECT w.code, w.coins FROM wallet_topups w JOIN payment_requests r ON r.topup_id = w.id WHERE r.id = $1",
    )
    .bind(request)
    .fetch_optional(db)
    .await
}

// ---------- Rút ----------

/// Một lần nạp còn hoàn được (cách rút "hoàn về giao dịch nạp").
#[derive(Debug, Clone, Serialize)]
pub struct Source {
    pub transaction: Uuid,
    pub topup_code: String,
    pub provider: String,
    pub amount_vnd: i64,
    pub refundable_vnd: i64,
    pub settled_at: Option<OffsetDateTime>,
}

const SOURCES: &str = "SELECT t.id, w.code, t.provider, t.amount_vnd,
        t.amount_vnd
          - coalesce((SELECT sum(f.amount_vnd) FROM refunds f WHERE f.transaction_id = t.id AND f.status <> 'failed'), 0)::bigint
          - coalesce((SELECT sum(d.amount_vnd) FROM wallet_withdrawals d WHERE d.source_tx = t.id AND d.status IN ('pending', 'approved')), 0)::bigint,
        t.settled_at
     FROM payment_transactions t
     JOIN payment_requests r ON r.id = t.request_id
     JOIN wallet_topups w ON w.id = r.topup_id
     WHERE w.subject = $1 AND t.status = 'succeeded' AND t.provider <> 'manual'";

type SourceRow = (Uuid, String, String, i64, i64, Option<OffsetDateTime>);

fn source_from(r: SourceRow) -> Source {
    Source {
        transaction: r.0,
        topup_code: r.1,
        provider: r.2,
        amount_vnd: r.3,
        refundable_vnd: r.4,
        settled_at: r.5,
    }
}

pub async fn refundable_sources(db: &Db, subject: Uuid) -> sqlx::Result<Vec<Source>> {
    let rows: Vec<SourceRow> =
        sqlx::query_as(&format!("{SOURCES} ORDER BY t.settled_at DESC LIMIT 20"))
            .bind(subject)
            .fetch_all(db)
            .await?;
    Ok(rows
        .into_iter()
        .map(source_from)
        .filter(|s| s.refundable_vnd > 0)
        .collect())
}

/// Cách nhận tiền khi rút.
#[derive(Debug, Clone)]
pub enum Payout {
    /// Hoàn về một giao dịch nạp (giao dịch cổng thanh toán).
    Source(Uuid),
    Bank {
        bank: String,
        holder: String,
        number: String,
    },
}

#[derive(Debug, Clone, Serialize)]
pub struct Withdrawal {
    pub id: Uuid,
    pub code: String,
    pub subject: Uuid,
    pub email: String,
    pub coins: i64,
    pub vnd_per_coin: i64,
    pub amount_vnd: i64,
    pub method: String,
    pub source_tx: Option<Uuid>,
    pub bank_name: Option<String>,
    pub account_holder: Option<String>,
    pub account_last4: Option<String>,
    pub note: String,
    pub status: String,
    pub approvals: Vec<String>,
    pub decision_note: Option<String>,
    pub payout_ref: Option<String>,
    pub created_at: OffsetDateTime,
    /// Số người duyệt tính lúc tạo theo tổng 24 giờ (None: lệnh cũ).
    pub approvals_required: Option<i16>,
}

impl Withdrawal {
    pub fn approvals_needed(&self) -> usize {
        let stored = self.approvals_required.unwrap_or(1).clamp(1, 2) as usize;
        approvals_needed(self.amount_vnd).max(stored)
    }

    /// Lệnh rút này của chính quản trị viên `admin` (email).
    fn owned_by(&self, admin: &str) -> bool {
        self.email.trim().eq_ignore_ascii_case(admin.trim())
    }
}

fn own_withdrawal_error() -> OrderError {
    problem(
        StatusCode::FORBIDDEN,
        "own_withdrawal",
        "Quản trị viên không xử lý lệnh rút của chính mình.",
        "Administrators can't act on their own withdrawals.",
    )
}

/// Số người duyệt cần cho một lệnh rút.
pub fn approvals_needed(amount_vnd: i64) -> usize {
    if amount_vnd >= TWO_APPROVALS_FROM {
        2
    } else {
        1
    }
}

const W_COLUMNS: &str =
    "id, code, subject, email, coins, vnd_per_coin, amount_vnd, method, source_tx, bank_name,
    account_holder, account_last4, note, status, approvals, decision_note, payout_ref, created_at,
    approvals_required";

#[derive(sqlx::FromRow)]
struct WRow {
    id: Uuid,
    code: String,
    subject: Uuid,
    email: String,
    coins: i64,
    vnd_per_coin: i64,
    amount_vnd: i64,
    method: String,
    source_tx: Option<Uuid>,
    bank_name: Option<String>,
    account_holder: Option<String>,
    account_last4: Option<String>,
    note: String,
    status: String,
    approvals: Vec<String>,
    decision_note: Option<String>,
    payout_ref: Option<String>,
    created_at: OffsetDateTime,
    approvals_required: Option<i16>,
}

impl From<WRow> for Withdrawal {
    fn from(r: WRow) -> Self {
        Withdrawal {
            id: r.id,
            code: r.code,
            subject: r.subject,
            email: r.email,
            coins: r.coins,
            vnd_per_coin: r.vnd_per_coin,
            amount_vnd: r.amount_vnd,
            method: r.method,
            source_tx: r.source_tx,
            bank_name: r.bank_name,
            account_holder: r.account_holder,
            account_last4: r.account_last4,
            note: r.note,
            status: r.status,
            approvals: r.approvals,
            decision_note: r.decision_note,
            payout_ref: r.payout_ref,
            created_at: r.created_at,
            approvals_required: r.approvals_required,
        }
    }
}

pub async fn withdrawal(db: &Db, id: Uuid) -> OrderResult<Withdrawal> {
    let r: Option<WRow> = sqlx::query_as(&format!(
        "SELECT {W_COLUMNS} FROM wallet_withdrawals WHERE id = $1"
    ))
    .bind(id)
    .fetch_optional(db)
    .await?;
    r.map(Withdrawal::from)
        .ok_or_else(|| not_found("Không có lệnh rút này.", "No such withdrawal."))
}

pub async fn withdrawals_of(db: &Db, subject: Uuid, limit: i64) -> sqlx::Result<Vec<Withdrawal>> {
    let rows: Vec<WRow> = sqlx::query_as(&format!(
        "SELECT {W_COLUMNS} FROM wallet_withdrawals WHERE subject = $1 ORDER BY created_at DESC LIMIT $2"
    ))
    .bind(subject)
    .bind(limit)
    .fetch_all(db)
    .await?;
    Ok(rows.into_iter().map(Withdrawal::from).collect())
}

/// Lệnh rút cần quản trị viên xử lý (chờ duyệt, đã duyệt chờ chuyển, đang hoàn), cũ nhất trước.
pub async fn open_withdrawals(db: &Db) -> sqlx::Result<Vec<Withdrawal>> {
    let rows: Vec<WRow> = sqlx::query_as(&format!(
        "SELECT {W_COLUMNS} FROM wallet_withdrawals WHERE status IN ('pending', 'approved', 'processing') ORDER BY created_at"
    ))
    .fetch_all(db)
    .await?;
    Ok(rows.into_iter().map(Withdrawal::from).collect())
}

async fn lock_withdrawal(tx: &mut Tx<'_>, id: Uuid) -> OrderResult<Withdrawal> {
    let r: Option<WRow> = sqlx::query_as(&format!(
        "SELECT {W_COLUMNS} FROM wallet_withdrawals WHERE id = $1 FOR UPDATE"
    ))
    .bind(id)
    .fetch_optional(&mut **tx)
    .await?;
    r.map(Withdrawal::from)
        .ok_or_else(|| not_found("Không có lệnh rút này.", "No such withdrawal."))
}

fn account_aad(id: Uuid) -> String {
    format!("withdrawal-account:{id}")
}

/// Tạo lệnh rút và giữ F-Coin ngay (design R5).
pub async fn create_withdrawal(
    state: &AppState,
    subject: Uuid,
    email: &str,
    coins: i64,
    payout: Payout,
    note: &str,
) -> OrderResult<Uuid> {
    if !state.withdrawals_enabled {
        return Err(problem(
            StatusCode::SERVICE_UNAVAILABLE,
            "withdrawals_disabled",
            "Tạm thời chưa nhận lệnh rút F-Coin.",
            "F-Coin withdrawals are temporarily unavailable.",
        ));
    }
    if coins < MIN_WITHDRAWAL {
        return Err(bad(
            "invalid_amount",
            "Số F-Coin rút tối thiểu là 10.",
            "The minimum withdrawal is 10 F-Coin.",
        ));
    }
    let id = Uuid::new_v4();
    let mut tx = state.db.begin().await?;
    let user = user_account(&mut tx, subject, Some(email)).await?;
    let per_coin = rate(&mut *tx).await?;
    let amount_vnd = coins.checked_mul(per_coin).ok_or_else(|| {
        bad(
            "invalid_amount",
            "Số F-Coin quá lớn.",
            "The amount is too large.",
        )
    })?;
    let (method, source_tx, bank, holder, last4, enc) = match &payout {
        Payout::Source(t) => {
            // Khóa giao dịch nạp: hai lệnh rút song song về cùng một nguồn được xét lần lượt.
            sqlx::query("SELECT id FROM payment_transactions WHERE id = $1 FOR UPDATE")
                .bind(t)
                .fetch_optional(&mut *tx)
                .await?;
            let row: Option<SourceRow> = sqlx::query_as(&format!("{SOURCES} AND t.id = $2"))
                .bind(subject)
                .bind(t)
                .fetch_optional(&mut *tx)
                .await?;
            let Some(src) = row.map(source_from) else {
                return Err(not_found(
                    "Không có giao dịch nạp này.",
                    "No such top-up transaction.",
                ));
            };
            if src.refundable_vnd < amount_vnd {
                return Err(conflict(
                    "source_insufficient",
                    format!(
                        "Giao dịch nạp {} chỉ còn hoàn được {}.",
                        src.topup_code,
                        vnd(src.refundable_vnd.max(0))
                    ),
                    format!(
                        "Top-up {} can only refund {} more.",
                        src.topup_code,
                        vnd(src.refundable_vnd.max(0))
                    ),
                ));
            }
            ("source", Some(*t), None, None, None, None)
        }
        Payout::Bank {
            bank,
            holder,
            number,
        } => {
            let number: String = number.chars().filter(|c| !c.is_whitespace()).collect();
            let (bank, holder) = (bank.trim(), holder.trim());
            if bank.chars().count() < 2
                || holder.chars().count() < 2
                || bank.chars().count() > 100
                || holder.chars().count() > 100
                || !(6..=20).contains(&number.len())
                || !number.chars().all(|c| c.is_ascii_digit())
            {
                return Err(bad(
                    "invalid_bank_account",
                    "Hãy nhập tên ngân hàng, tên chủ tài khoản và số tài khoản (6–20 chữ số).",
                    "Enter the bank name, account holder and account number (6–20 digits).",
                ));
            }
            let last4 = number[number.len() - 4..].to_owned();
            let enc = state.data_key.encrypt_text(&number, &account_aad(id));
            (
                "bank",
                None,
                Some(bank.to_owned()),
                Some(holder.to_owned()),
                Some(last4),
                Some(enc),
            )
        }
    };
    let n: i64 = sqlx::query_scalar("SELECT nextval('withdrawal_seq')")
        .fetch_one(&mut *tx)
        .await?;
    let code = format!("RUT-{n:06}");
    // Tổng các lệnh rút còn hiệu lực trong 24 giờ, cộng lệnh này: chia nhỏ không né được quy tắc
    // hai người duyệt. Ví đã khóa (FOR UPDATE ở `user_account`) nên hai lệnh song song xét lần lượt.
    sqlx::query("SELECT id FROM wallet_accounts WHERE id = $1 FOR UPDATE")
        .bind(user)
        .execute(&mut *tx)
        .await?;
    let recent: i64 = sqlx::query_scalar(
        "SELECT coalesce(sum(amount_vnd), 0)::bigint FROM wallet_withdrawals
         WHERE subject = $1 AND created_at > now() - interval '24 hours'
           AND status NOT IN ('cancelled', 'rejected', 'failed')",
    )
    .bind(subject)
    .fetch_one(&mut *tx)
    .await?;
    let required = approvals_needed(recent.saturating_add(amount_vnd)) as i16;
    sqlx::query(
        "INSERT INTO wallet_withdrawals (id, code, subject, email, coins, vnd_per_coin, amount_vnd, method, source_tx,
             bank_name, account_holder, account_last4, account_enc, note, approvals_required)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)",
    )
    .bind(id)
    .bind(&code)
    .bind(subject)
    .bind(email.trim().to_ascii_lowercase())
    .bind(coins)
    .bind(per_coin)
    .bind(amount_vnd)
    .bind(method)
    .bind(source_tx)
    .bind(&bank)
    .bind(&holder)
    .bind(&last4)
    .bind(&enc)
    .bind(note.trim().chars().take(500).collect::<String>())
    .bind(required)
    .execute(&mut *tx)
    .await?;
    let pending = system_account(&mut *tx, system::WITHDRAWALS).await?;
    let memo = format!("Rút {coins} F-Coin ({code})");
    post(
        &mut tx,
        &Posting {
            kind: Kind::WithdrawalHold,
            idem: &format!("withdrawal:{id}:hold"),
            memo: &memo,
            reference: Some(&code),
            by: &subject.to_string(),
        },
        &[
            Leg {
                account: user,
                amount: -coins,
            },
            Leg {
                account: pending,
                amount: coins,
            },
        ],
    )
    .await?;
    tx.commit().await?;
    audit::record(
        &state.db,
        Actor::User(subject),
        "wallet.withdrawal_create",
        Some(&code),
        None,
        json!({"coins": coins, "amount_vnd": amount_vnd, "method": method}),
    )
    .await;
    crate::review::admin_alert(
        state,
        "withdrawal_request",
        &format!(
            "Lệnh rút {code}: {} ({}) chờ duyệt",
            coin(coins, Lang::Vi),
            vnd(amount_vnd)
        ),
        json!({"withdrawal": id, "coins": coins, "amount_vnd": amount_vnd, "method": method}),
    )
    .await;
    Ok(id)
}

/// Trả F-Coin của lệnh rút về ví và đặt trạng thái kết thúc (rejected, cancelled, failed).
async fn release(
    tx: &mut Tx<'_>,
    w: &Withdrawal,
    status: &str,
    note: Option<&str>,
    by: &str,
    now: OffsetDateTime,
) -> OrderResult<()> {
    let user = user_account(tx, w.subject, None).await?;
    let pending = system_account(&mut **tx, system::WITHDRAWALS).await?;
    let memo = format!("Hoàn lệnh rút {}", w.code);
    post(
        tx,
        &Posting {
            kind: Kind::WithdrawalRelease,
            idem: &format!("withdrawal:{}:release", w.id),
            memo: &memo,
            reference: Some(&w.code),
            by,
        },
        &[
            Leg {
                account: pending,
                amount: -w.coins,
            },
            Leg {
                account: user,
                amount: w.coins,
            },
        ],
    )
    .await?;
    sqlx::query(
        "UPDATE wallet_withdrawals SET status = $2, decision_note = coalesce($3, decision_note), decided_at = $4 WHERE id = $1",
    )
    .bind(w.id)
    .bind(status)
    .bind(note)
    .bind(now)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Lệnh rút đã trả tiền: F-Coin bị hủy (về `issuance`).
async fn burn(
    tx: &mut Tx<'_>,
    w: &Withdrawal,
    payout_ref: &str,
    by: &str,
    now: OffsetDateTime,
) -> OrderResult<()> {
    let pending = system_account(&mut **tx, system::WITHDRAWALS).await?;
    let issuance = system_account(&mut **tx, system::ISSUANCE).await?;
    let memo = format!("Đã trả lệnh rút {}", w.code);
    post(
        tx,
        &Posting {
            kind: Kind::WithdrawalPaid,
            idem: &format!("withdrawal:{}:paid", w.id),
            memo: &memo,
            reference: Some(&w.code),
            by,
        },
        &[
            Leg {
                account: pending,
                amount: -w.coins,
            },
            Leg {
                account: issuance,
                amount: w.coins,
            },
        ],
    )
    .await?;
    sqlx::query("UPDATE wallet_withdrawals SET status = 'paid', payout_ref = $2, paid_at = $3 WHERE id = $1")
        .bind(w.id)
        .bind(payout_ref)
        .bind(now)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

async fn notify_withdrawal(state: &AppState, w: &Withdrawal, outcome: &str, note: &str) {
    let (vi_title, vi_body, en_title, en_body) = match outcome {
        "paid" => (
            format!("Đã trả lệnh rút {}", w.code),
            format!(
                "F-Eco đã chuyển {} cho lệnh rút {} ({}).",
                vnd(w.amount_vnd),
                w.code,
                coin(w.coins, Lang::Vi)
            ),
            format!("Withdrawal {} paid", w.code),
            format!(
                "F-Eco has paid {} for withdrawal {} ({}).",
                vnd(w.amount_vnd),
                w.code,
                coin(w.coins, Lang::En)
            ),
        ),
        "rejected" => (
            format!("Lệnh rút {} không được duyệt", w.code),
            format!(
                "{} đã về lại ví của bạn. Lý do: {note}",
                coin(w.coins, Lang::Vi)
            ),
            format!("Withdrawal {} declined", w.code),
            format!(
                "{} is back in your wallet. Reason: {note}",
                coin(w.coins, Lang::En)
            ),
        ),
        _ => (
            format!("Lệnh rút {} chưa thực hiện được", w.code),
            format!(
                "Cổng thanh toán không thực hiện được lệnh hoàn, {} đã về lại ví của bạn. Bạn có thể tạo lệnh rút khác.",
                coin(w.coins, Lang::Vi)
            ),
            format!("Withdrawal {} could not be completed", w.code),
            format!(
                "The payment gateway could not process the refund; {} is back in your wallet. You can create another withdrawal.",
                coin(w.coins, Lang::En)
            ),
        ),
    };
    let n = Note::vi(vi_title, vi_body, "/wallet#withdrawals").en(en_title, en_body);
    notifications::notify(state, w.subject, Some(&w.email), Category::Payment, &n).await;
}

/// Người dùng hủy lệnh rút khi chưa đủ duyệt.
pub async fn cancel_withdrawal(state: &AppState, subject: Uuid, id: Uuid) -> OrderResult<()> {
    let mut tx = state.db.begin().await?;
    let w = lock_withdrawal(&mut tx, id).await?;
    if w.subject != subject {
        return Err(not_found("Không có lệnh rút này.", "No such withdrawal."));
    }
    if w.status != "pending" {
        return Err(conflict(
            "not_pending",
            "Lệnh rút đã được xử lý, không hủy được nữa.",
            "This withdrawal is already being processed and can't be cancelled.",
        ));
    }
    release(
        &mut tx,
        &w,
        "cancelled",
        None,
        &subject.to_string(),
        OffsetDateTime::now_utc(),
    )
    .await?;
    tx.commit().await?;
    audit::record(
        &state.db,
        Actor::User(subject),
        "wallet.withdrawal_cancel",
        Some(&w.code),
        None,
        json!({}),
    )
    .await;
    Ok(())
}

/// Quản trị viên Tài chính duyệt lệnh rút. Đủ người duyệt thì: hoàn về giao dịch nạp được gửi
/// tới cổng ngay; chuyển khoản thì chờ Tài chính bấm "Đã chuyển".
pub async fn approve_withdrawal(
    state: &AppState,
    id: Uuid,
    admin: &str,
) -> OrderResult<Withdrawal> {
    let admin = admin.trim();
    let mut tx = state.db.begin().await?;
    let w = lock_withdrawal(&mut tx, id).await?;
    if w.status != "pending" {
        return Err(conflict(
            "not_pending",
            "Lệnh rút không ở trạng thái chờ duyệt.",
            "The withdrawal isn't awaiting approval.",
        ));
    }
    if admin.is_empty() || w.approvals.iter().any(|a| a == admin) {
        return Err(problem(
            StatusCode::FORBIDDEN,
            "second_approver_required",
            "Cần một quản trị viên khác duyệt.",
            "Another administrator must approve this.",
        ));
    }
    if w.owned_by(admin) {
        return Err(own_withdrawal_error());
    }
    // Ví đã bị khóa sau khi tạo lệnh: không duyệt.
    let account = user_account(&mut tx, w.subject, None).await?;
    ensure_not_frozen(&mut tx, account).await?;
    let mut approvals = w.approvals.clone();
    approvals.push(admin.to_owned());
    let done = approvals.len() >= w.approvals_needed();
    sqlx::query(
        "UPDATE wallet_withdrawals SET approvals = $2, status = $3, decided_at = CASE WHEN $3 = 'approved' THEN now() ELSE decided_at END WHERE id = $1",
    )
    .bind(id)
    .bind(&approvals)
    .bind(if done { "approved" } else { "pending" })
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    audit::record(
        &state.db,
        Actor::Admin(admin.to_owned()),
        "wallet.withdrawal_approve",
        Some(&w.code),
        None,
        json!({"approvals": approvals, "complete": done}),
    )
    .await;
    if done && w.method == "source" {
        start_source_refund(state, id, &approvals).await?;
    }
    withdrawal(&state.db, id).await
}

/// Tạo lệnh hoàn (loại `withdrawal`) cho giao dịch nạp và gửi tới cổng ngay.
async fn start_source_refund(state: &AppState, id: Uuid, approvals: &[String]) -> OrderResult<()> {
    let refund = Uuid::new_v4();
    let mut tx = state.db.begin().await?;
    let w = lock_withdrawal(&mut tx, id).await?;
    if w.status != "approved" {
        tx.commit().await?;
        return Ok(());
    }
    let Some(source) = w.source_tx else {
        return Ok(());
    };
    sqlx::query(
        "INSERT INTO refunds (id, transaction_id, amount_vnd, reason, kind, status, requested_by, approvals, withdrawal_id)
         VALUES ($1, $2, $3, $4, 'withdrawal', 'pending_approval', 'system', $5, $6)",
    )
    .bind(refund)
    .bind(source)
    .bind(w.amount_vnd)
    .bind(format!("Rút F-Coin {}", w.code))
    .bind(approvals)
    .bind(id)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "UPDATE wallet_withdrawals SET status = 'processing', refund_id = $2 WHERE id = $1",
    )
    .bind(id)
    .bind(refund)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    if let Err(e) = crate::payments::execute_refund(state, refund, OffsetDateTime::now_utc()).await
    {
        tracing::warn!(error = %e, "gửi lệnh hoàn của lệnh rút thất bại");
    }
    Ok(())
}

/// Kết quả lệnh hoàn của một lệnh rút (gọi từ `payments::finish_refund`).
pub async fn refund_finished(
    state: &AppState,
    refund: Uuid,
    succeeded: bool,
    gateway_ref: Option<&str>,
    why: &str,
    now: OffsetDateTime,
) -> OrderResult<()> {
    let id: Option<Uuid> =
        sqlx::query_scalar("SELECT id FROM wallet_withdrawals WHERE refund_id = $1")
            .bind(refund)
            .fetch_optional(&state.db)
            .await?;
    let Some(id) = id else {
        return Ok(());
    };
    let mut tx = state.db.begin().await?;
    let w = lock_withdrawal(&mut tx, id).await?;
    if w.status != "processing" {
        tx.commit().await?;
        return Ok(());
    }
    if succeeded {
        burn(&mut tx, &w, gateway_ref.unwrap_or("gateway"), "system", now).await?;
    } else {
        release(&mut tx, &w, "failed", Some(why), "system", now).await?;
    }
    tx.commit().await?;
    if !succeeded {
        crate::review::admin_alert(
            state,
            "withdrawal_failed",
            &format!("Lệnh rút {} hoàn về giao dịch nạp thất bại: {why}", w.code),
            json!({"withdrawal": id, "refund": refund}),
        )
        .await;
    }
    notify_withdrawal(state, &w, if succeeded { "paid" } else { "failed" }, why).await;
    Ok(())
}

/// Từ chối lệnh rút (đang chờ duyệt, hoặc đã duyệt chờ chuyển khoản): F-Coin về lại ví.
pub async fn reject_withdrawal(
    state: &AppState,
    id: Uuid,
    admin: &str,
    reason: &str,
) -> OrderResult<()> {
    let reason = reason.trim();
    if reason.is_empty() {
        return Err(bad("reason_required", "Hãy ghi lý do.", "Give a reason."));
    }
    let mut tx = state.db.begin().await?;
    let w = lock_withdrawal(&mut tx, id).await?;
    if !(w.status == "pending" || (w.status == "approved" && w.method == "bank")) {
        return Err(conflict(
            "not_pending",
            "Lệnh rút này không từ chối được nữa.",
            "This withdrawal can no longer be declined.",
        ));
    }
    release(
        &mut tx,
        &w,
        "rejected",
        Some(reason),
        admin,
        OffsetDateTime::now_utc(),
    )
    .await?;
    tx.commit().await?;
    audit::record(
        &state.db,
        Actor::Admin(admin.to_owned()),
        "wallet.withdrawal_reject",
        Some(&w.code),
        None,
        json!({"reason": reason}),
    )
    .await;
    notify_withdrawal(state, &w, "rejected", reason).await;
    Ok(())
}

/// Tài chính đã chuyển khoản cho lệnh rút ngân hàng đã duyệt.
pub async fn mark_paid(
    state: &AppState,
    id: Uuid,
    admin: &str,
    reference: &str,
) -> OrderResult<()> {
    let reference = reference.trim();
    if reference.is_empty() {
        return Err(bad(
            "reference_required",
            "Hãy nhập mã chuyển khoản.",
            "Enter the transfer reference.",
        ));
    }
    let mut tx = state.db.begin().await?;
    let w = lock_withdrawal(&mut tx, id).await?;
    if w.owned_by(admin) {
        return Err(own_withdrawal_error());
    }
    let account = user_account(&mut tx, w.subject, None).await?;
    ensure_not_frozen(&mut tx, account).await?;
    if w.method != "bank" || w.status != "approved" {
        return Err(conflict(
            "not_approved",
            "Chỉ lệnh rút chuyển khoản đã duyệt mới đánh dấu đã chuyển được.",
            "Only an approved bank withdrawal can be marked as paid.",
        ));
    }
    burn(&mut tx, &w, reference, admin, OffsetDateTime::now_utc()).await?;
    tx.commit().await?;
    audit::record(
        &state.db,
        Actor::Admin(admin.to_owned()),
        "wallet.withdrawal_paid",
        Some(&w.code),
        None,
        json!({"reference": reference}),
    )
    .await;
    notify_withdrawal(state, &w, "paid", "").await;
    Ok(())
}

/// Số tài khoản đầy đủ của lệnh rút ngân hàng (Tài chính bấm "Hiện"; ghi kiểm toán).
pub async fn reveal_account(state: &AppState, id: Uuid, admin: &str) -> OrderResult<String> {
    let row: Option<(String, Option<String>)> =
        sqlx::query_as("SELECT code, account_enc FROM wallet_withdrawals WHERE id = $1")
            .bind(id)
            .fetch_optional(&state.db)
            .await?;
    let Some((code, Some(enc))) = row else {
        return Err(not_found("Không có lệnh rút này.", "No such withdrawal."));
    };
    let number = state
        .data_key
        .decrypt_text(&enc, &account_aad(id))
        .map_err(|_| {
            problem(
                StatusCode::INTERNAL_SERVER_ERROR,
                "decrypt_failed",
                "Không giải mã được số tài khoản.",
                "Couldn't decrypt the account number.",
            )
        })?;
    audit::record(
        &state.db,
        Actor::Admin(admin.to_owned()),
        "wallet.withdrawal_reveal",
        Some(&code),
        None,
        json!({}),
    )
    .await;
    Ok(number)
}

// ---------- Mua app ----------

#[derive(Debug, Clone, Serialize)]
pub struct Bought {
    pub code: String,
    pub price: i64,
    pub balance: i64,
}

/// Mua một app có giá bằng F-Coin (design R6). `expected` là giá người dùng đã thấy.
pub async fn purchase(
    state: &AppState,
    subject: Uuid,
    email: &str,
    app_id: &str,
    expected: Option<i64>,
) -> OrderResult<Bought> {
    let row: Option<(String, String, i64, Option<Uuid>)> = sqlx::query_as(
        "SELECT client_id, name, price_coin, owner_subject FROM catalog_apps
         WHERE id = $1 AND status = 'listed' AND (private_to IS NULL OR private_to = $2)",
    )
    .bind(app_id)
    .bind(subject)
    .fetch_optional(&state.db)
    .await?;
    let Some((client_id, name, price, owner)) = row else {
        return Err(not_found(
            "Không có app này trong danh mục.",
            "No app with this id is listed.",
        ));
    };
    if price <= 0 {
        return Err(bad(
            "free_app",
            "App này miễn phí, không cần mua.",
            "This app is free; there's nothing to buy.",
        ));
    }
    if expected.is_some_and(|e| e != price) {
        return Err(conflict(
            "price_changed",
            format!(
                "Giá app vừa đổi thành {}. Hãy xem lại trước khi mua.",
                coin(price, Lang::Vi)
            ),
            format!(
                "The price just changed to {}. Please check before buying.",
                coin(price, Lang::En)
            ),
        ));
    }
    if owner == Some(subject) {
        return Err(bad(
            "own_app",
            "Đây là app của bạn.",
            "This is your own app.",
        ));
    }
    let mut tx = state.db.begin().await?;
    let owned: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM app_purchases WHERE app_id = $1 AND buyer = $2)",
    )
    .bind(app_id)
    .bind(subject)
    .fetch_one(&mut *tx)
    .await?;
    if owned {
        return Err(conflict(
            "already_owned",
            "Bạn đã mua app này.",
            "You already own this app.",
        ));
    }
    let buyer = user_account(&mut tx, subject, Some(email)).await?;
    let fees_account = system_account(&mut *tx, system::FEES).await?;
    let (seller, fee_coin) = match owner {
        Some(s) => {
            let bps = fees(&mut *tx).await?.app_bps;
            (Some(user_account(&mut tx, s, None).await?), fee(price, bps))
        }
        // App của F-Eco: toàn bộ vào tài khoản phí của nền tảng.
        None => (None, price),
    };
    let memo = format!("Mua app {name}");
    let mut legs = vec![
        Leg {
            account: buyer,
            amount: -price,
        },
        Leg {
            account: fees_account,
            amount: fee_coin,
        },
    ];
    if let Some(seller) = seller {
        legs.push(Leg {
            account: seller,
            amount: price - fee_coin,
        });
    }
    let posted = post(
        &mut tx,
        &Posting {
            kind: Kind::Purchase,
            idem: &format!("purchase:{subject}:{app_id}"),
            memo: &memo,
            reference: Some(app_id),
            by: &subject.to_string(),
        },
        &legs,
    )
    .await?;
    sqlx::query(
        "INSERT INTO app_purchases (id, app_id, buyer, price_coin, fee_coin, seller, txn_id) VALUES ($1, $2, $3, $4, $5, $6, $7)
         ON CONFLICT (app_id, buyer) DO NOTHING",
    )
    .bind(Uuid::new_v4())
    .bind(app_id)
    .bind(subject)
    .bind(price)
    .bind(fee_coin)
    .bind(owner)
    .bind(posted.txn)
    .execute(&mut *tx)
    .await?;
    let balance: i64 = sqlx::query_scalar("SELECT balance FROM wallet_accounts WHERE id = $1")
        .bind(buyer)
        .fetch_one(&mut *tx)
        .await?;
    tx.commit().await?;
    if let Err(e) = crate::entitlements::grant(
        state,
        &client_id,
        subject,
        "license",
        &format!("fcoin:{}", posted.code),
        None,
    )
    .await
    {
        tracing::error!(error = %e, app_id, "không cấp được quyền sử dụng sau khi mua");
    }
    audit::record(
        &state.db,
        Actor::User(subject),
        "wallet.purchase",
        Some(app_id),
        None,
        json!({"price": price, "fee": fee_coin, "txn": posted.code}),
    )
    .await;
    if let Some(s) = owner {
        let net = price - fee_coin;
        let note = Note::vi(
            format!("Bán được app {name}"),
            format!(
                "Một người dùng vừa mua {name}. Bạn nhận {}.",
                coin(net, Lang::Vi)
            ),
            "/wallet",
        )
        .en(
            format!("{name} sold"),
            format!(
                "Someone just bought {name}. You received {}.",
                coin(net, Lang::En)
            ),
        );
        notifications::notify(state, s, None, Category::Payment, &note).await;
    }
    Ok(Bought {
        code: posted.code,
        price,
        balance,
    })
}

/// App người dùng đã mua bằng F-Coin.
pub async fn owned_apps(db: &Db, subject: Uuid) -> sqlx::Result<Vec<String>> {
    sqlx::query_scalar("SELECT app_id FROM app_purchases WHERE buyer = $1 ORDER BY app_id")
        .bind(subject)
        .fetch_all(db)
        .await
}

pub async fn owns(db: &Db, subject: Uuid, app_id: &str) -> sqlx::Result<bool> {
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM app_purchases WHERE app_id = $1 AND buyer = $2)",
    )
    .bind(app_id)
    .bind(subject)
    .fetch_one(db)
    .await
}

/// Giá các app có giá đang niêm yết.
pub async fn priced_apps(db: &Db) -> sqlx::Result<Vec<(String, i64)>> {
    sqlx::query_as(
        "SELECT id, price_coin FROM catalog_apps WHERE status = 'listed' AND price_coin > 0 ORDER BY id",
    )
    .fetch_all(db)
    .await
}

/// Nhà phát triển đặt giá app (F-Coin), chép sang danh mục nếu app đã niêm yết.
pub async fn set_app_price(
    state: &AppState,
    app_id: &str,
    by: Uuid,
    price: i64,
) -> OrderResult<()> {
    if !(0..=MAX_PRICE).contains(&price) {
        return Err(bad(
            "invalid_price",
            "Giá phải từ 0 đến 1.000.000 F-Coin.",
            "The price must be between 0 and 1,000,000 F-Coin.",
        ));
    }
    let mut tx = state.db.begin().await?;
    sqlx::query("UPDATE developer_apps SET price_coin = $2 WHERE app_id = $1")
        .bind(app_id)
        .bind(price)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "UPDATE catalog_apps SET price_coin = $2, price_vnd = $2 * (SELECT vnd_per_coin FROM coin_rates ORDER BY id DESC LIMIT 1), updated_at = now() WHERE id = $1",
    )
    .bind(app_id)
    .bind(price)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    state.catalog.clear();
    audit::record(
        &state.db,
        Actor::User(by),
        "wallet.app_price",
        Some(app_id),
        None,
        json!({"price_coin": price}),
    )
    .await;
    Ok(())
}

// ---------- Lịch sử và thống kê ----------

#[derive(Debug, Clone, Serialize)]
pub struct Line {
    pub code: String,
    pub at: OffsetDateTime,
    pub kind: Kind,
    pub amount: i64,
    pub balance_after: i64,
    pub memo: String,
    pub reference: Option<String>,
}

type LineRow = (
    String,
    OffsetDateTime,
    String,
    i64,
    i64,
    String,
    Option<String>,
);

fn line_from(r: LineRow) -> Option<Line> {
    Some(Line {
        code: r.0,
        at: r.1,
        kind: Kind::parse(&r.2)?,
        amount: r.3,
        balance_after: r.4,
        memo: r.5,
        reference: r.6,
    })
}

/// Lịch sử giao dịch của một người dùng (mới nhất trước), lọc theo loại, phân trang từ 1.
pub async fn history(
    db: &Db,
    subject: Uuid,
    kind: Option<Kind>,
    page: i64,
    per_page: i64,
) -> sqlx::Result<(Vec<Line>, bool)> {
    let page = page.max(1);
    let rows: Vec<LineRow> = sqlx::query_as(
        "SELECT t.code, e.created_at, t.kind, e.amount, e.balance_after, t.memo, t.reference
         FROM wallet_entries e JOIN wallet_txns t ON t.id = e.txn_id JOIN wallet_accounts a ON a.id = e.account_id
         WHERE a.subject = $1 AND ($2::text IS NULL OR t.kind = $2)
         ORDER BY e.id DESC LIMIT $3 OFFSET $4",
    )
    .bind(subject)
    .bind(kind.map(Kind::as_str))
    .bind(per_page + 1)
    .bind((page - 1) * per_page)
    .fetch_all(db)
    .await?;
    let more = rows.len() as i64 > per_page;
    Ok((
        rows.into_iter()
            .take(per_page as usize)
            .filter_map(line_from)
            .collect(),
        more,
    ))
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct MonthStat {
    pub year: i32,
    pub month: u8,
    pub spend: i64,
    pub income: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Stats {
    /// 12 tháng gần nhất, cũ nhất trước.
    pub months: Vec<MonthStat>,
    /// Tổng chi và thu theo nhóm trong 12 tháng (khóa i18n, số F-Coin), bỏ nhóm bằng 0.
    pub spend: Vec<(&'static str, i64)>,
    pub income: Vec<(&'static str, i64)>,
}

/// Gộp các dòng (loại, số F-Coin, thời điểm) thành thống kê 12 tháng tính tới `now` (giờ Việt Nam).
pub fn aggregate(rows: &[(Kind, i64, OffsetDateTime)], now: OffsetDateTime) -> Stats {
    let offset = time::UtcOffset::from_hms(7, 0, 0).expect("UTC+7");
    let now = now.to_offset(offset);
    let mut months: Vec<MonthStat> = Vec::with_capacity(12);
    let (mut y, mut m) = (now.year(), u8::from(now.month()));
    for _ in 0..12 {
        months.push(MonthStat {
            year: y,
            month: m,
            ..Default::default()
        });
        if m == 1 {
            y -= 1;
            m = 12;
        } else {
            m -= 1;
        }
    }
    months.reverse();
    let order_spend = [
        "wallet.cat.apps",
        "wallet.cat.projects",
        "wallet.cat.withdrawals",
    ];
    let order_income = [
        "wallet.cat.topup",
        "wallet.cat.sales",
        "wallet.cat.project_income",
    ];
    let mut spend = [0i64; 3];
    let mut income = [0i64; 3];
    for (kind, amount, at) in rows {
        let at = at.to_offset(offset);
        let Some(slot) = months
            .iter_mut()
            .find(|s| s.year == at.year() && s.month == u8::from(at.month()))
        else {
            continue;
        };
        let (is_income, key) = category(*kind, *amount);
        if is_income {
            slot.income += amount;
            if let Some(i) = order_income.iter().position(|k| *k == key) {
                income[i] += amount;
            }
        } else {
            slot.spend -= amount;
            if let Some(i) = order_spend.iter().position(|k| *k == key) {
                spend[i] -= amount;
            }
        }
    }
    Stats {
        months,
        spend: order_spend
            .into_iter()
            .zip(spend)
            .filter(|(_, v)| *v != 0)
            .collect(),
        income: order_income
            .into_iter()
            .zip(income)
            .filter(|(_, v)| *v != 0)
            .collect(),
    }
}

pub async fn stats(db: &Db, subject: Uuid, now: OffsetDateTime) -> sqlx::Result<Stats> {
    let since = now - Duration::days(400);
    let rows: Vec<(String, i64, OffsetDateTime)> = sqlx::query_as(
        "SELECT t.kind, e.amount, e.created_at FROM wallet_entries e
         JOIN wallet_txns t ON t.id = e.txn_id JOIN wallet_accounts a ON a.id = e.account_id
         WHERE a.subject = $1 AND e.created_at >= $2",
    )
    .bind(subject)
    .bind(since)
    .fetch_all(db)
    .await?;
    let rows: Vec<(Kind, i64, OffsetDateTime)> = rows
        .into_iter()
        .filter_map(|(k, a, t)| Some((Kind::parse(&k)?, a, t)))
        .collect();
    Ok(aggregate(&rows, now))
}

// ---------- Quản trị ----------

#[derive(Debug, Clone, Default, Serialize)]
pub struct DayStat {
    pub label: String,
    pub topup: i64,
    pub withdraw: i64,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Overview {
    pub circulation: i64,
    pub escrow: i64,
    pub withdrawals_pending: i64,
    pub fees: i64,
    pub open_withdrawals: i64,
    pub topup_coin: i64,
    pub topup_vnd: i64,
    pub withdraw_coin: i64,
    pub withdraw_vnd: i64,
    /// 30 ngày gần nhất (giờ Việt Nam), cũ nhất trước: F-Coin nạp và rút mỗi ngày.
    pub days: Vec<DayStat>,
    pub rate: i64,
    pub fee_settings: Fees,
}

pub async fn overview(db: &Db, now: OffsetDateTime) -> sqlx::Result<Overview> {
    let circulation: i64 = sqlx::query_scalar(
        "SELECT coalesce(sum(balance), 0)::bigint FROM wallet_accounts WHERE kind = 'user'",
    )
    .fetch_one(db)
    .await?;
    let systems: Vec<(String, i64)> =
        sqlx::query_as("SELECT code, balance FROM wallet_accounts WHERE kind = 'system'")
            .fetch_all(db)
            .await?;
    let sys = |c: &str| systems.iter().find(|s| s.0 == c).map(|s| s.1).unwrap_or(0);
    let open: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM wallet_withdrawals WHERE status IN ('pending', 'approved', 'processing')",
    )
    .fetch_one(db)
    .await?;
    let since = now - Duration::days(30);
    let (topup_coin, topup_vnd): (i64, i64) = sqlx::query_as(
        "SELECT coalesce(sum(coins), 0)::bigint, coalesce(sum(amount_vnd), 0)::bigint FROM wallet_topups
         WHERE status = 'succeeded' AND settled_at >= $1",
    )
    .bind(since)
    .fetch_one(db)
    .await?;
    let (withdraw_coin, withdraw_vnd): (i64, i64) = sqlx::query_as(
        "SELECT coalesce(sum(coins), 0)::bigint, coalesce(sum(amount_vnd), 0)::bigint FROM wallet_withdrawals
         WHERE status = 'paid' AND paid_at >= $1",
    )
    .bind(since)
    .fetch_one(db)
    .await?;
    let per_day: Vec<(time::Date, i64, i64)> = sqlx::query_as(
        "SELECT d, coalesce(sum(t), 0)::bigint, coalesce(sum(w), 0)::bigint FROM (
             SELECT (settled_at AT TIME ZONE 'UTC' + interval '7 hours')::date AS d, coins AS t, 0::bigint AS w
             FROM wallet_topups WHERE status = 'succeeded' AND settled_at >= $1
             UNION ALL
             SELECT (paid_at AT TIME ZONE 'UTC' + interval '7 hours')::date, 0::bigint, coins
             FROM wallet_withdrawals WHERE status = 'paid' AND paid_at >= $1
         ) x GROUP BY d",
    )
    .bind(since)
    .fetch_all(db)
    .await?;
    let today = now
        .to_offset(time::UtcOffset::from_hms(7, 0, 0).expect("UTC+7"))
        .date();
    let days = (0..30)
        .rev()
        .map(|i| {
            let d = today - Duration::days(i);
            let (t, w) = per_day
                .iter()
                .find(|r| r.0 == d)
                .map(|r| (r.1, r.2))
                .unwrap_or((0, 0));
            DayStat {
                label: format!("{:02}/{:02}", d.day(), u8::from(d.month())),
                topup: t,
                withdraw: w,
            }
        })
        .collect();
    Ok(Overview {
        circulation,
        escrow: sys(system::ESCROW),
        withdrawals_pending: sys(system::WITHDRAWALS),
        fees: sys(system::FEES),
        open_withdrawals: open,
        topup_coin,
        topup_vnd,
        withdraw_coin,
        withdraw_vnd,
        days,
        rate: rate(db).await?,
        fee_settings: fees(db).await?,
    })
}

#[derive(Debug, Clone, Serialize)]
pub struct AdminLine {
    pub code: String,
    pub at: OffsetDateTime,
    pub kind: Kind,
    /// Email của ví, hoặc mã tài khoản hệ thống.
    pub account: String,
    pub amount: i64,
    pub memo: String,
}

/// Tra giao dịch theo mã (GD-…) hoặc email của ví; 50 dòng mới nhất.
pub async fn search(db: &Db, q: &str) -> sqlx::Result<Vec<AdminLine>> {
    let q = q.trim();
    if q.is_empty() {
        return Ok(Vec::new());
    }
    let rows: Vec<(String, OffsetDateTime, String, String, i64, String)> = sqlx::query_as(
        "SELECT t.code, e.created_at, t.kind, coalesce(a.email, a.code, ''), e.amount, t.memo
         FROM wallet_entries e JOIN wallet_txns t ON t.id = e.txn_id JOIN wallet_accounts a ON a.id = e.account_id
         WHERE upper(t.code) = upper($1) OR upper(coalesce(t.reference, '')) = upper($1)
            OR (a.kind = 'user' AND lower(a.email) = lower($1))
         ORDER BY e.id DESC LIMIT 50",
    )
    .bind(q)
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .filter_map(|r| {
            Some(AdminLine {
                code: r.0,
                at: r.1,
                kind: Kind::parse(&r.2)?,
                account: r.3,
                amount: r.4,
                memo: r.5,
            })
        })
        .collect())
}

/// Khóa hoặc mở ví theo email (ví phải đã tồn tại).
pub async fn set_frozen(
    state: &AppState,
    admin: &str,
    email: &str,
    freeze: bool,
    reason: &str,
) -> OrderResult<()> {
    let reason = reason.trim();
    if reason.is_empty() {
        return Err(bad("reason_required", "Hãy ghi lý do.", "Give a reason."));
    }
    let email = email.trim().to_ascii_lowercase();
    let changed = sqlx::query(
        "UPDATE wallet_accounts SET frozen_at = CASE WHEN $2 THEN now() ELSE NULL END,
             frozen_reason = CASE WHEN $2 THEN $3 ELSE NULL END, frozen_by = $4
         WHERE kind = 'user' AND lower(email) = $1",
    )
    .bind(&email)
    .bind(freeze)
    .bind(reason)
    .bind(admin)
    .execute(&state.db)
    .await?
    .rows_affected();
    if changed == 0 {
        return Err(not_found(
            "Không có ví F-Coin của email này.",
            "No F-Coin wallet for this email.",
        ));
    }
    audit::record(
        &state.db,
        Actor::Admin(admin.to_owned()),
        if freeze {
            "wallet.freeze"
        } else {
            "wallet.unfreeze"
        },
        Some(&email),
        None,
        json!({"reason": reason}),
    )
    .await;
    Ok(())
}

/// Kết quả kiểm tra sổ cái.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct LedgerCheck {
    pub accounts: i64,
    pub txns: i64,
    /// Tổng số dư mọi tài khoản (phải bằng 0).
    pub total: i64,
    /// Tài khoản có số dư khác tổng các dòng.
    pub mismatched: Vec<Uuid>,
    /// Bút toán có tổng các dòng khác 0.
    pub unbalanced: Vec<String>,
}

impl LedgerCheck {
    pub fn ok(&self) -> bool {
        self.total == 0 && self.mismatched.is_empty() && self.unbalanced.is_empty()
    }
}

pub async fn check_ledger(db: &Db) -> sqlx::Result<LedgerCheck> {
    let (accounts, total): (i64, i64) =
        sqlx::query_as("SELECT count(*), coalesce(sum(balance), 0)::bigint FROM wallet_accounts")
            .fetch_one(db)
            .await?;
    let txns: i64 = sqlx::query_scalar("SELECT count(*) FROM wallet_txns")
        .fetch_one(db)
        .await?;
    let mismatched: Vec<Uuid> = sqlx::query_scalar(
        "SELECT a.id FROM wallet_accounts a
         WHERE a.balance <> coalesce((SELECT sum(e.amount) FROM wallet_entries e WHERE e.account_id = a.id), 0)",
    )
    .fetch_all(db)
    .await?;
    let unbalanced: Vec<String> = sqlx::query_scalar(
        "SELECT t.code FROM wallet_txns t JOIN wallet_entries e ON e.txn_id = t.id
         GROUP BY t.code HAVING sum(e.amount) <> 0",
    )
    .fetch_all(db)
    .await?;
    Ok(LedgerCheck {
        accounts,
        txns,
        total,
        mismatched,
        unbalanced,
    })
}

// ---------- Quản trị viên chuyển F-Coin (production-readiness, design K2) ----------

/// Số F-Coin tối đa mỗi lần chuyển.
pub const MAX_ADMIN_TRANSFER: i64 = 1_000_000;
/// Cấp từ quỹ nền tảng từ mức này (đồng) trở lên cần quản trị viên Tài chính thứ hai duyệt.
pub const GRANT_SECOND_APPROVAL_FROM: i64 = 1_000_000;

/// Nguồn F-Coin khi quản trị viên chuyển.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferSource {
    /// Ví của chính quản trị viên.
    Own,
    /// Quỹ nền tảng: phát hành thêm F-Coin.
    Platform,
}

impl TransferSource {
    pub fn parse(s: &str) -> Option<TransferSource> {
        match s {
            "own" => Some(TransferSource::Own),
            "platform" => Some(TransferSource::Platform),
            _ => None,
        }
    }
}

/// Một lần chuyển do quản trị viên gửi.
#[derive(Debug, Clone)]
pub struct TransferRequest<'a> {
    pub admin_subject: Uuid,
    pub admin_email: &'a str,
    pub recipient: Uuid,
    pub recipient_email: &'a str,
    pub coins: i64,
    pub source: TransferSource,
    pub reason: &'a str,
    /// Khóa sinh khi hiện biểu mẫu: gửi lại cùng biểu mẫu không chuyển hai lần.
    pub idem: &'a str,
}

/// Kết quả: đã ghi bút toán (mã GD-…), hoặc yêu cầu cấp đang chờ duyệt (mã CP-…).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransferOutcome {
    Posted(String),
    PendingGrant(String),
}

fn check_transfer(r: &TransferRequest<'_>) -> OrderResult<()> {
    let reason = r.reason.trim();
    if reason.is_empty() || reason.chars().count() > 500 {
        return Err(bad(
            "reason_required",
            "Hãy ghi lý do (tối đa 500 ký tự).",
            "Give a reason (at most 500 characters).",
        ));
    }
    if !(1..=MAX_ADMIN_TRANSFER).contains(&r.coins) {
        return Err(bad(
            "invalid_amount",
            "Số F-Coin phải từ 1 đến 1.000.000.",
            "The amount must be between 1 and 1,000,000 F-Coin.",
        ));
    }
    if r.recipient == r.admin_subject
        || r.recipient_email
            .trim()
            .eq_ignore_ascii_case(r.admin_email.trim())
    {
        return Err(bad(
            "self_transfer",
            "Không chuyển F-Coin cho chính mình.",
            "You can't transfer F-Coin to yourself.",
        ));
    }
    let idem = r.idem.trim();
    if idem.is_empty() || idem.len() > 100 {
        return Err(bad(
            "invalid_form",
            "Biểu mẫu không hợp lệ, hãy tải lại trang.",
            "Invalid form; reload the page.",
        ));
    }
    Ok(())
}

/// Quản trị viên chuyển F-Coin cho người dùng: từ ví của mình, hoặc cấp từ quỹ nền tảng (lớn thì
/// chờ người thứ hai duyệt). Quyền Tài chính kiểm ở tầng web.
pub async fn admin_transfer(
    state: &AppState,
    r: &TransferRequest<'_>,
) -> OrderResult<TransferOutcome> {
    check_transfer(r)?;
    let reason = r.reason.trim();
    let idem = format!("admin-transfer:{}", r.idem.trim());
    let mut tx = state.db.begin().await?;
    let to = user_account(&mut tx, r.recipient, Some(r.recipient_email)).await?;
    ensure_not_frozen(&mut tx, to).await?;
    let (from, kind) = match r.source {
        TransferSource::Own => (
            user_account(&mut tx, r.admin_subject, Some(r.admin_email)).await?,
            Kind::AdminTransfer,
        ),
        TransferSource::Platform => {
            let rate = rate(&mut *tx).await?;
            let amount_vnd = r.coins.checked_mul(rate).ok_or_else(|| {
                bad(
                    "invalid_amount",
                    "Số tiền quá lớn.",
                    "The amount is too large.",
                )
            })?;
            if amount_vnd >= GRANT_SECOND_APPROVAL_FROM {
                let code = request_grant(&mut tx, r, amount_vnd, &idem).await?;
                tx.commit().await?;
                audit::record(
                    &state.db,
                    Actor::Admin(r.admin_email.to_owned()),
                    "wallet.grant_request",
                    Some(&code),
                    None,
                    json!({"to": r.recipient_email, "coins": r.coins, "vnd": amount_vnd, "reason": reason}),
                )
                .await;
                return Ok(TransferOutcome::PendingGrant(code));
            }
            (
                system_account(&mut *tx, system::ISSUANCE).await?,
                Kind::AdminGrant,
            )
        }
    };
    let posted = post(
        &mut tx,
        &Posting {
            kind,
            idem: &idem,
            memo: reason,
            reference: Some(r.admin_email),
            by: r.admin_email,
        },
        &[
            Leg {
                account: from,
                amount: -r.coins,
            },
            Leg {
                account: to,
                amount: r.coins,
            },
        ],
    )
    .await?;
    tx.commit().await?;
    if !posted.duplicate {
        audit::record(
            &state.db,
            Actor::Admin(r.admin_email.to_owned()),
            match kind {
                Kind::AdminGrant => "wallet.admin_grant",
                _ => "wallet.admin_transfer",
            },
            Some(&posted.code),
            None,
            json!({"to": r.recipient_email, "coins": r.coins, "reason": reason}),
        )
        .await;
        notify_received(
            state,
            r.recipient,
            r.recipient_email,
            r.coins,
            reason,
            &posted.code,
        )
        .await;
    }
    Ok(TransferOutcome::Posted(posted.code))
}

async fn request_grant(
    tx: &mut Tx<'_>,
    r: &TransferRequest<'_>,
    amount_vnd: i64,
    idem: &str,
) -> OrderResult<String> {
    if let Some(code) =
        sqlx::query_scalar::<_, String>("SELECT code FROM wallet_grants WHERE idem_key = $1")
            .bind(idem)
            .fetch_optional(&mut **tx)
            .await?
    {
        return Ok(code);
    }
    let n: i64 = sqlx::query_scalar("SELECT nextval('wallet_grant_seq')")
        .fetch_one(&mut **tx)
        .await?;
    let code = format!("CP-{n:06}");
    sqlx::query(
        "INSERT INTO wallet_grants (id, code, recipient, recipient_email, coins, amount_vnd, reason, requested_by, idem_key)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
    )
    .bind(Uuid::new_v4())
    .bind(&code)
    .bind(r.recipient)
    .bind(r.recipient_email.trim().to_ascii_lowercase())
    .bind(r.coins)
    .bind(amount_vnd)
    .bind(r.reason.trim())
    .bind(r.admin_email.trim())
    .bind(idem)
    .execute(&mut **tx)
    .await?;
    Ok(code)
}

async fn notify_received(
    state: &AppState,
    subject: Uuid,
    email: &str,
    coins: i64,
    reason: &str,
    code: &str,
) {
    let n = Note::vi(
        format!("Bạn nhận được {}", coin(coins, Lang::Vi)),
        format!(
            "F-Eco đã chuyển {} vào ví của bạn (giao dịch {code}). Lý do: {reason}",
            coin(coins, Lang::Vi)
        ),
        "/wallet",
    )
    .en(
        format!("You received {}", coin(coins, Lang::En)),
        format!(
            "F-Eco added {} to your wallet (transaction {code}). Reason: {reason}",
            coin(coins, Lang::En)
        ),
    );
    notifications::notify(state, subject, Some(email), Category::Payment, &n).await;
}

/// Yêu cầu cấp F-Coin từ quỹ nền tảng.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Grant {
    pub id: Uuid,
    pub code: String,
    pub recipient: Uuid,
    pub recipient_email: String,
    pub coins: i64,
    pub amount_vnd: i64,
    pub reason: String,
    pub status: String,
    pub requested_by: String,
    pub created_at: OffsetDateTime,
}

const GRANT_COLUMNS: &str =
    "id, code, recipient, recipient_email, coins, amount_vnd, reason, status, requested_by, created_at";

/// Yêu cầu cấp đang chờ, cũ nhất trước.
pub async fn pending_grants(db: &Db) -> sqlx::Result<Vec<Grant>> {
    sqlx::query_as(&format!(
        "SELECT {GRANT_COLUMNS} FROM wallet_grants WHERE status = 'pending' ORDER BY created_at"
    ))
    .fetch_all(db)
    .await
}

async fn lock_grant(tx: &mut Tx<'_>, id: Uuid, admin: &str) -> OrderResult<Grant> {
    let g: Grant = sqlx::query_as(&format!(
        "SELECT {GRANT_COLUMNS} FROM wallet_grants WHERE id = $1 FOR UPDATE"
    ))
    .bind(id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| not_found("Không có yêu cầu cấp này.", "No such grant request."))?;
    if g.status != "pending" {
        return Err(conflict(
            "not_pending",
            "Yêu cầu cấp đã được xử lý.",
            "The grant request was already decided.",
        ));
    }
    let admin = admin.trim();
    if admin.is_empty()
        || admin.eq_ignore_ascii_case(&g.requested_by)
        || admin.eq_ignore_ascii_case(&g.recipient_email)
    {
        return Err(problem(
            StatusCode::FORBIDDEN,
            "second_approver_required",
            "Cần một quản trị viên Tài chính khác (không phải người tạo hay người nhận) duyệt.",
            "Another Finance administrator (not the requester or recipient) must decide.",
        ));
    }
    Ok(g)
}

/// Quản trị viên Tài chính thứ hai duyệt yêu cầu cấp: ghi bút toán `admin_grant`.
pub async fn approve_grant(state: &AppState, id: Uuid, admin: &str) -> OrderResult<String> {
    let mut tx = state.db.begin().await?;
    let g = lock_grant(&mut tx, id, admin).await?;
    let to = user_account(&mut tx, g.recipient, Some(&g.recipient_email)).await?;
    ensure_not_frozen(&mut tx, to).await?;
    let issuance = system_account(&mut *tx, system::ISSUANCE).await?;
    let posted = post(
        &mut tx,
        &Posting {
            kind: Kind::AdminGrant,
            idem: &format!("grant:{}", g.id),
            memo: &g.reason,
            reference: Some(&g.code),
            by: admin.trim(),
        },
        &[
            Leg {
                account: issuance,
                amount: -g.coins,
            },
            Leg {
                account: to,
                amount: g.coins,
            },
        ],
    )
    .await?;
    sqlx::query(
        "UPDATE wallet_grants SET status = 'approved', decided_by = $2, decided_at = now(), txn_id = $3 WHERE id = $1",
    )
    .bind(g.id)
    .bind(admin.trim())
    .bind(posted.txn)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    audit::record(
        &state.db,
        Actor::Admin(admin.trim().to_owned()),
        "wallet.admin_grant",
        Some(&posted.code),
        None,
        json!({"grant": g.code, "to": g.recipient_email, "coins": g.coins, "requested_by": g.requested_by}),
    )
    .await;
    notify_received(
        state,
        g.recipient,
        &g.recipient_email,
        g.coins,
        &g.reason,
        &posted.code,
    )
    .await;
    Ok(posted.code)
}

/// Từ chối yêu cầu cấp (bắt buộc lý do).
pub async fn reject_grant(state: &AppState, id: Uuid, admin: &str, note: &str) -> OrderResult<()> {
    let note = note.trim();
    if note.is_empty() {
        return Err(bad("reason_required", "Hãy ghi lý do.", "Give a reason."));
    }
    let mut tx = state.db.begin().await?;
    let g = lock_grant(&mut tx, id, admin).await?;
    sqlx::query(
        "UPDATE wallet_grants SET status = 'rejected', decided_by = $2, decided_at = now(), decision_note = $3 WHERE id = $1",
    )
    .bind(g.id)
    .bind(admin.trim())
    .bind(note)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    audit::record(
        &state.db,
        Actor::Admin(admin.trim().to_owned()),
        "wallet.grant_reject",
        Some(&g.code),
        None,
        json!({"note": note}),
    )
    .await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Rà soát trước production: một lần đổi tỷ giá không quá 20%; ngưỡng hai người duyệt tính theo
    /// tổng 24 giờ (giá trị lưu được ưu tiên khi lớn hơn).
    #[test]
    fn rate_steps_and_rolling_approvals() {
        assert!(rate_change_allowed(1000, 1200));
        assert!(rate_change_allowed(1000, 800));
        assert!(!rate_change_allowed(1000, 1201));
        assert!(!rate_change_allowed(1000, 999_999));
        assert!(!rate_change_allowed(1000, 1));
        let w = |amount_vnd: i64, stored: Option<i16>| Withdrawal {
            id: Uuid::nil(),
            code: "RUT-000001".into(),
            subject: Uuid::nil(),
            email: "lan@example.com".into(),
            coins: 1,
            vnd_per_coin: 1000,
            amount_vnd,
            method: "bank".into(),
            source_tx: None,
            bank_name: None,
            account_holder: None,
            account_last4: None,
            note: String::new(),
            status: "pending".into(),
            approvals: vec![],
            decision_note: None,
            payout_ref: None,
            created_at: OffsetDateTime::UNIX_EPOCH,
            approvals_required: stored,
        };
        assert_eq!(w(9_000_000, None).approvals_needed(), 1);
        assert_eq!(w(9_000_000, Some(2)).approvals_needed(), 2);
        assert_eq!(w(12_000_000, Some(1)).approvals_needed(), 2);
        assert!(w(1, None).owned_by(" LAN@example.com "));
        assert!(!w(1, None).owned_by("an@example.com"));
    }

    #[test]
    fn transfer_checks() {
        let base = TransferRequest {
            admin_subject: Uuid::from_u128(1),
            admin_email: "tc@f-eco.vn",
            recipient: Uuid::from_u128(2),
            recipient_email: "lan@example.com",
            coins: 50,
            source: TransferSource::Own,
            reason: "Hỗ trợ",
            idem: "abc",
        };
        assert!(check_transfer(&base).is_ok());
        for bad in [
            TransferRequest {
                reason: "  ",
                ..base.clone()
            },
            TransferRequest {
                coins: 0,
                ..base.clone()
            },
            TransferRequest {
                coins: MAX_ADMIN_TRANSFER + 1,
                ..base.clone()
            },
            TransferRequest {
                recipient: Uuid::from_u128(1),
                ..base.clone()
            },
            TransferRequest {
                recipient_email: "TC@f-eco.vn",
                ..base.clone()
            },
            TransferRequest {
                idem: "",
                ..base.clone()
            },
        ] {
            assert!(check_transfer(&bad).is_err(), "{bad:?}");
        }
        assert_eq!(
            TransferSource::parse("platform"),
            Some(TransferSource::Platform)
        );
        assert_eq!(TransferSource::parse("issuance"), None);
    }
    use time::macros::datetime;

    #[test]
    fn coins_read_per_language() {
        assert_eq!(coin(1250, Lang::Vi), "1.250 F-Coin");
        assert_eq!(coin(1250, Lang::En), "1,250 F-Coin");
        assert_eq!(coin(0, Lang::Vi), "0 F-Coin");
        assert_eq!(coin(1_000_000, Lang::Fr), "1\u{a0}000\u{a0}000 F-Coin");
        assert_eq!(signed_coin(50, Lang::Vi), "+50 F-Coin");
        assert_eq!(signed_coin(-49, Lang::En), "\u{2212}49 F-Coin");
    }

    #[test]
    fn fees_round_down() {
        assert_eq!(fee(49, 1000), 4, "10% của 49 là 4,9 → 4");
        assert_eq!(fee(650, 0), 0);
        assert_eq!(fee(100, 3000), 30);
        assert_eq!(fee(-5, 1000), 0);
    }

    #[test]
    fn large_withdrawals_need_two_approvals() {
        assert_eq!(approvals_needed(9_999_999), 1);
        assert_eq!(approvals_needed(10_000_000), 2);
        assert_eq!(approvals_needed(12_000 * 1000), 2);
    }

    #[test]
    fn kinds_round_trip() {
        for k in Kind::ALL {
            assert_eq!(Kind::parse(k.as_str()), Some(k));
        }
        assert_eq!(line_key(Kind::Purchase, -49), "wallet.kind.purchase");
        assert_eq!(line_key(Kind::Purchase, 49), "wallet.kind.sale");
    }

    /// Scenario "Thống kê chi tiêu": nạp 100, mua hai app tổng 60 trong tháng.
    #[test]
    fn monthly_statistics() {
        let now = datetime!(2026-11-15 10:00 UTC);
        let rows = vec![
            (Kind::Topup, 100, datetime!(2026-11-02 08:00 UTC)),
            (Kind::Purchase, -49, datetime!(2026-11-03 08:00 UTC)),
            (Kind::Purchase, -11, datetime!(2026-11-04 08:00 UTC)),
            // Tháng trước: ký quỹ 650 rồi được hoàn 250, bán app được 45.
            (Kind::EscrowFund, -650, datetime!(2026-10-10 08:00 UTC)),
            (Kind::EscrowRefund, 250, datetime!(2026-10-12 08:00 UTC)),
            (Kind::Purchase, 45, datetime!(2026-10-20 08:00 UTC)),
            // Rút 60 rồi bị từ chối: không tính chi.
            (Kind::WithdrawalHold, -60, datetime!(2026-10-21 08:00 UTC)),
            (Kind::WithdrawalRelease, 60, datetime!(2026-10-22 08:00 UTC)),
            // Ngoài 12 tháng.
            (Kind::Topup, 999, datetime!(2025-10-31 08:00 UTC)),
        ];
        let s = aggregate(&rows, now);
        assert_eq!(s.months.len(), 12);
        let last = s.months.last().unwrap();
        assert_eq!(
            (last.year, last.month, last.income, last.spend),
            (2026, 11, 100, 60)
        );
        let oct = &s.months[10];
        assert_eq!((oct.month, oct.income, oct.spend), (10, 45, 400));
        assert_eq!(s.months[0].month, 12, "bắt đầu từ tháng 12/2025");
        assert_eq!(
            s.spend,
            vec![("wallet.cat.apps", 60), ("wallet.cat.projects", 400)]
        );
        assert_eq!(
            s.income,
            vec![("wallet.cat.topup", 100), ("wallet.cat.sales", 45)]
        );
    }

    #[test]
    fn statistics_use_vietnam_time() {
        // 30/11 23:30 giờ Việt Nam là 30/11 16:30 UTC; 01/12 00:30 giờ VN là 30/11 17:30 UTC.
        let now = datetime!(2026-12-05 00:00 UTC);
        let rows = vec![(Kind::Topup, 10, datetime!(2026-11-30 17:30 UTC))];
        let s = aggregate(&rows, now);
        assert_eq!(s.months.last().unwrap().income, 10, "tính vào tháng 12");
    }
}
