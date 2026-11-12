//! Đơn xây app theo yêu cầu (spec custom-build-orders, nhóm task 12).
//!
//! Vòng đời: Nháp → Đã gửi → Đã báo giá → Đang thực hiện → Đã bàn giao → Hoàn tất, cùng Đã hủy và
//! Đang tranh chấp. Mọi lần đổi trạng thái đi qua [`transition`], hàm duy nhất kiểm tra bước chuyển
//! ([`can_move`]) và ghi lịch sử (ai, khi nào). Thanh toán là việc của nhóm 13: ở đây chỉ tạo yêu
//! cầu thanh toán (`payment_requests`) và nhận kết quả qua [`payment_settled`].

use axum::http::StatusCode;
use serde::Serialize;
use time::{Duration, OffsetDateTime, Weekday};
use uuid::Uuid;

use crate::{audit, store::Db, AppState};

/// Phiên bản Điều khoản dịch vụ xây app hiện hành (`/policy/custom-build`).
pub const TOS_VERSION: &str = "2026-11-11";
/// Tổng tệp đính kèm của một yêu cầu.
pub const MAX_ATTACHMENTS_BYTES: u64 = 200 * 1024 * 1024;
/// Đặt cọc (mốc đầu) tối đa 30% tổng giá.
pub const MAX_DEPOSIT_PERCENT: i64 = 30;
/// Hạn hiệu lực mặc định của báo giá, và số lượt sửa miễn phí mặc định của mỗi mốc.
pub const QUOTE_VALID_DAYS: i64 = 7;
pub const FREE_REVISIONS: i32 = 2;
/// Báo giá trong 3 ngày làm việc; quản trị viên phản hồi tranh chấp trong 2 ngày làm việc.
pub const QUOTE_BUSINESS_DAYS: u32 = 3;
pub const DISPUTE_BUSINESS_DAYS: u32 = 2;
/// Nghiệm thu: nhắc sau 7 ngày, tự nghiệm thu sau 14 ngày; mở tranh chấp trong 14 ngày.
pub const REMIND_AFTER_DAYS: i64 = 7;
pub const AUTO_ACCEPT_DAYS: i64 = 14;
pub const DISPUTE_WINDOW_DAYS: i64 = 14;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Draft,
    Submitted,
    Quoted,
    InProgress,
    Delivered,
    Completed,
    Cancelled,
    Disputed,
}

impl Status {
    pub const ALL: [Status; 8] = [
        Status::Draft,
        Status::Submitted,
        Status::Quoted,
        Status::InProgress,
        Status::Delivered,
        Status::Completed,
        Status::Cancelled,
        Status::Disputed,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Status::Draft => "draft",
            Status::Submitted => "submitted",
            Status::Quoted => "quoted",
            Status::InProgress => "in_progress",
            Status::Delivered => "delivered",
            Status::Completed => "completed",
            Status::Cancelled => "cancelled",
            Status::Disputed => "disputed",
        }
    }

    pub fn parse(s: &str) -> Option<Status> {
        Status::ALL.into_iter().find(|x| x.as_str() == s)
    }

    pub fn label(self, vi: bool) -> &'static str {
        match (self, vi) {
            (Status::Draft, true) => "Nháp",
            (Status::Submitted, true) => "Đã gửi",
            (Status::Quoted, true) => "Đã báo giá",
            (Status::InProgress, true) => "Đang thực hiện",
            (Status::Delivered, true) => "Đã bàn giao",
            (Status::Completed, true) => "Hoàn tất",
            (Status::Cancelled, true) => "Đã hủy",
            (Status::Disputed, true) => "Đang tranh chấp",
            (Status::Draft, false) => "Draft",
            (Status::Submitted, false) => "Submitted",
            (Status::Quoted, false) => "Quoted",
            (Status::InProgress, false) => "In progress",
            (Status::Delivered, false) => "Delivered",
            (Status::Completed, false) => "Completed",
            (Status::Cancelled, false) => "Cancelled",
            (Status::Disputed, false) => "In dispute",
        }
    }
}

/// Bước chuyển hợp lệ của đơn (spec "Trạng thái đơn hàng").
pub fn can_move(from: Status, to: Status) -> bool {
    use Status::*;
    matches!(
        (from, to),
        (Draft, Submitted)
            | (Submitted, Quoted)
            // Báo giá mới thay báo giá cũ (thương lượng, báo giá lại).
            | (Quoted, Quoted)
            // Chỉ khi đặt cọc thành công.
            | (Quoted, InProgress)
            | (InProgress, Delivered)
            // Yêu cầu sửa, hoặc nghiệm thu một mốc chưa phải mốc cuối.
            | (Delivered, InProgress)
            // Nghiệm thu mốc cuối.
            | (Delivered, Completed)
            | (InProgress | Delivered | Completed, Disputed)
            | (Disputed, InProgress | Delivered | Completed)
            // Hủy ở mọi trạng thái trước Hoàn tất.
            | (Draft | Submitted | Quoted | InProgress | Delivered | Disputed, Cancelled)
    )
}

/// `n` ngày làm việc (thứ Hai–thứ Sáu) sau `from`. Ngày lễ Việt Nam: QUESTIONS.md 38.
pub fn business_days_after(from: OffsetDateTime, n: u32) -> OffsetDateTime {
    let mut t = from;
    let mut left = n;
    while left > 0 {
        t += Duration::days(1);
        if !matches!(t.weekday(), Weekday::Saturday | Weekday::Sunday) {
            left -= 1;
        }
    }
    t
}

/// Lỗi của một thao tác trên đơn: mã máy đọc được, lời cho người (Việt, Anh).
#[derive(Debug)]
pub enum OrderError {
    Problem {
        status: StatusCode,
        code: &'static str,
        vi: String,
        en: String,
    },
    Db(sqlx::Error),
}

impl From<sqlx::Error> for OrderError {
    fn from(e: sqlx::Error) -> Self {
        OrderError::Db(e)
    }
}

impl std::fmt::Display for OrderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OrderError::Problem { code, vi, .. } => write!(f, "{code}: {vi}"),
            OrderError::Db(e) => write!(f, "db: {e}"),
        }
    }
}

pub type OrderResult<T> = Result<T, OrderError>;

pub(crate) fn problem(
    status: StatusCode,
    code: &'static str,
    vi: impl Into<String>,
    en: impl Into<String>,
) -> OrderError {
    OrderError::Problem {
        status,
        code,
        vi: vi.into(),
        en: en.into(),
    }
}

fn not_found() -> OrderError {
    problem(
        StatusCode::NOT_FOUND,
        "not_found",
        "Không có đơn này.",
        "No such order.",
    )
}

#[derive(Debug, Clone, Serialize)]
pub struct Order {
    pub id: Uuid,
    pub code: String,
    pub customer: Uuid,
    pub customer_email: String,
    pub title: String,
    pub description: String,
    pub platforms: Vec<String>,
    pub wanted_by: Option<time::Date>,
    pub budget_vnd: Option<i64>,
    pub status: Status,
    pub tos_version: Option<String>,
    pub quote_due_at: Option<OffsetDateTime>,
    pub started_at: Option<OffsetDateTime>,
    pub created_at: OffsetDateTime,
}

type OrderRow = (
    Uuid,
    String,
    Uuid,
    String,
    String,
    String,
    Vec<String>,
    Option<time::Date>,
    Option<i64>,
    String,
    Option<String>,
    Option<OffsetDateTime>,
    Option<OffsetDateTime>,
    OffsetDateTime,
);

const ORDER_COLUMNS: &str = "id, code, customer, customer_email, title, description, platforms, wanted_by, budget_vnd, status, tos_version, quote_due_at, started_at, created_at";

fn order_from(r: OrderRow) -> Order {
    Order {
        id: r.0,
        code: r.1,
        customer: r.2,
        customer_email: r.3,
        title: r.4,
        description: r.5,
        platforms: r.6,
        wanted_by: r.7,
        budget_vnd: r.8,
        status: Status::parse(&r.9).unwrap_or(Status::Draft),
        tos_version: r.10,
        quote_due_at: r.11,
        started_at: r.12,
        created_at: r.13,
    }
}

pub async fn get(db: &Db, id: Uuid) -> OrderResult<Order> {
    let row: Option<OrderRow> =
        sqlx::query_as(&format!("SELECT {ORDER_COLUMNS} FROM orders WHERE id = $1"))
            .bind(id)
            .fetch_optional(db)
            .await?;
    row.map(order_from).ok_or_else(not_found)
}

pub async fn by_code(db: &Db, code: &str) -> OrderResult<Order> {
    let row: Option<OrderRow> = sqlx::query_as(&format!(
        "SELECT {ORDER_COLUMNS} FROM orders WHERE code = $1"
    ))
    .bind(code.trim().to_uppercase())
    .fetch_optional(db)
    .await?;
    row.map(order_from).ok_or_else(not_found)
}

/// Đơn của một khách hàng: đơn của người khác là "không có".
pub async fn of_customer(db: &Db, customer: Uuid, id: Uuid) -> OrderResult<Order> {
    let o = get(db, id).await?;
    if o.customer != customer {
        return Err(not_found());
    }
    Ok(o)
}

pub async fn list_for(db: &Db, customer: Uuid) -> OrderResult<Vec<Order>> {
    let rows: Vec<OrderRow> = sqlx::query_as(&format!(
        "SELECT {ORDER_COLUMNS} FROM orders WHERE customer = $1 ORDER BY created_at DESC"
    ))
    .bind(customer)
    .fetch_all(db)
    .await?;
    Ok(rows.into_iter().map(order_from).collect())
}

/// Đơn theo trạng thái (cho đội F-Eco), mới nhất trước.
pub async fn list_by_status(db: &Db, status: Option<Status>) -> OrderResult<Vec<Order>> {
    let rows: Vec<OrderRow> = match status {
        Some(s) => {
            sqlx::query_as(&format!(
                "SELECT {ORDER_COLUMNS} FROM orders WHERE status = $1 ORDER BY created_at DESC"
            ))
            .bind(s.as_str())
            .fetch_all(db)
            .await?
        }
        None => {
            sqlx::query_as(&format!(
                "SELECT {ORDER_COLUMNS} FROM orders ORDER BY created_at DESC LIMIT 200"
            ))
            .fetch_all(db)
            .await?
        }
    };
    Ok(rows.into_iter().map(order_from).collect())
}

/// Đổi trạng thái: chỉ bước chuyển hợp lệ, ghi lịch sử (người làm, thời điểm, ghi chú).
pub async fn transition(
    tx: &mut sqlx::PgConnection,
    order: Uuid,
    to: Status,
    actor: &str,
    note: &str,
) -> OrderResult<Status> {
    let current: Option<String> =
        sqlx::query_scalar("SELECT status FROM orders WHERE id = $1 FOR UPDATE")
            .bind(order)
            .fetch_optional(&mut *tx)
            .await?;
    let from = current
        .as_deref()
        .and_then(Status::parse)
        .ok_or_else(not_found)?;
    if !can_move(from, to) {
        return Err(problem(
            StatusCode::CONFLICT,
            "invalid_transition",
            format!(
                "Không chuyển được đơn từ \"{}\" sang \"{}\".",
                from.label(true),
                to.label(true)
            ),
            format!(
                "An order can't go from \"{}\" to \"{}\".",
                from.label(false),
                to.label(false)
            ),
        ));
    }
    if to == Status::Disputed {
        sqlx::query("UPDATE orders SET status_before_dispute = status WHERE id = $1")
            .bind(order)
            .execute(&mut *tx)
            .await?;
    }
    sqlx::query("UPDATE orders SET status = $2, updated_at = now() WHERE id = $1")
        .bind(order)
        .bind(to.as_str())
        .execute(&mut *tx)
        .await?;
    sqlx::query("INSERT INTO order_events (order_id, from_status, to_status, actor, note) VALUES ($1, $2, $3, $4, $5)")
        .bind(order)
        .bind(from.as_str())
        .bind(to.as_str())
        .bind(actor)
        .bind(note)
        .execute(&mut *tx)
        .await?;
    Ok(from)
}

#[derive(Debug, Clone, Serialize)]
pub struct Event {
    pub from: Option<String>,
    pub to: String,
    pub actor: String,
    pub note: String,
    pub at: OffsetDateTime,
}

/// Toàn bộ lịch sử trạng thái của đơn (khách hàng xem được).
pub async fn history(db: &Db, order: Uuid) -> OrderResult<Vec<Event>> {
    let rows: Vec<(Option<String>, String, String, String, OffsetDateTime)> =
        sqlx::query_as("SELECT from_status, to_status, actor, note, at FROM order_events WHERE order_id = $1 ORDER BY id").bind(order).fetch_all(db).await?;
    Ok(rows
        .into_iter()
        .map(|r| Event {
            from: r.0,
            to: r.1,
            actor: r.2,
            note: r.3,
            at: r.4,
        })
        .collect())
}

/// Tên người làm trong lịch sử: khách hàng, đội F-Eco (tên), hệ thống.
pub fn customer_actor(customer: Uuid) -> String {
    format!("customer:{customer}")
}

pub fn team_actor(name: &str) -> String {
    format!("team:{name}")
}

pub const SYSTEM_ACTOR: &str = "system";

#[derive(Debug, Clone, Default)]
pub struct NewOrder {
    pub title: String,
    pub description: String,
    pub platforms: Vec<String>,
    pub wanted_by: Option<time::Date>,
    pub budget_vnd: Option<i64>,
}

/// Một đơn nháp mới của khách hàng.
pub async fn create_draft(
    state: &AppState,
    customer: Uuid,
    email: &str,
    new: &NewOrder,
) -> OrderResult<Order> {
    let title = new.title.trim();
    let description = new.description.trim();
    if title.is_empty() || title.chars().count() > 120 {
        return Err(problem(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "Tên dự án cần từ 1 đến 120 ký tự.",
            "The project name needs 1 to 120 characters.",
        ));
    }
    if description.is_empty() {
        return Err(problem(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "Hãy mô tả app cần xây.",
            "Describe the app to build.",
        ));
    }
    if new
        .platforms
        .iter()
        .any(|p| !matches!(p.as_str(), "windows" | "macos"))
        || new.platforms.is_empty()
    {
        return Err(problem(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "Chọn nền tảng: Windows, macOS hoặc cả hai.",
            "Pick the platforms: Windows, macOS or both.",
        ));
    }
    let id = Uuid::new_v4();
    let seq: i64 = sqlx::query_scalar("SELECT nextval('order_code_seq')")
        .fetch_one(&state.db)
        .await?;
    let code = format!("DH-{seq:06}");
    let mut tx = state.db.begin().await?;
    sqlx::query(
        "INSERT INTO orders (id, code, customer, customer_email, title, description, platforms, wanted_by, budget_vnd)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
    )
    .bind(id)
    .bind(&code)
    .bind(customer)
    .bind(email)
    .bind(title)
    .bind(description)
    .bind(&new.platforms)
    .bind(new.wanted_by)
    .bind(new.budget_vnd)
    .execute(&mut *tx)
    .await?;
    sqlx::query("INSERT INTO order_events (order_id, from_status, to_status, actor) VALUES ($1, NULL, 'draft', $2)")
        .bind(id)
        .bind(customer_actor(customer))
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    get(&state.db, id).await
}

/// Tổng dung lượng tệp đính kèm của yêu cầu.
pub async fn attachments_size(db: &Db, order: Uuid) -> OrderResult<u64> {
    let n: Option<i64> = sqlx::query_scalar(
        "SELECT sum(size)::bigint FROM order_files WHERE order_id = $1 AND message_id IS NULL",
    )
    .bind(order)
    .fetch_one(db)
    .await?;
    Ok(n.unwrap_or(0).max(0) as u64)
}

pub fn too_large() -> OrderError {
    problem(
        StatusCode::PAYLOAD_TOO_LARGE,
        "attachments_too_large",
        "Tổng tệp đính kèm vượt giới hạn 200 MB.",
        "The attachments are larger than the 200 MB limit.",
    )
}

/// Ghi nhận một tệp đã lưu cho đơn (hoặc cho một tin nhắn). Tệp của yêu cầu: tổng tối đa 200 MB.
pub async fn add_file(
    db: &Db,
    order: Uuid,
    message: Option<i64>,
    name: &str,
    path: &std::path::Path,
    size: u64,
    sha256: &str,
) -> OrderResult<Uuid> {
    if message.is_none()
        && attachments_size(db, order).await?.saturating_add(size) > MAX_ATTACHMENTS_BYTES
    {
        return Err(too_large());
    }
    let id = Uuid::new_v4();
    let name: String = name
        .chars()
        .filter(|c| !matches!(c, '/' | '\\') && !c.is_control())
        .take(200)
        .collect();
    sqlx::query("INSERT INTO order_files (id, order_id, message_id, name, size, sha256, path) VALUES ($1, $2, $3, $4, $5, $6, $7)")
        .bind(id)
        .bind(order)
        .bind(message)
        .bind(if name.is_empty() { "tep" } else { name.as_str() })
        .bind(size as i64)
        .bind(sha256)
        .bind(path.to_string_lossy().into_owned())
        .execute(db)
        .await?;
    Ok(id)
}

#[derive(Debug, Clone, Serialize)]
pub struct FileInfo {
    pub id: Uuid,
    pub name: String,
    pub size: i64,
    pub message_id: Option<i64>,
}

pub async fn files(db: &Db, order: Uuid) -> OrderResult<Vec<FileInfo>> {
    let rows: Vec<(Uuid, String, i64, Option<i64>)> = sqlx::query_as(
        "SELECT id, name, size, message_id FROM order_files WHERE order_id = $1 ORDER BY at",
    )
    .bind(order)
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| FileInfo {
            id: r.0,
            name: r.1,
            size: r.2,
            message_id: r.3,
        })
        .collect())
}

/// Gửi yêu cầu: cần chấp nhận Điều khoản dịch vụ xây app (ghi phiên bản, thời điểm). Đơn sang "Đã
/// gửi", khách nhận email xác nhận kèm mã đơn, đội F-Eco nhận việc báo giá trong 3 ngày làm việc.
pub async fn submit(
    state: &AppState,
    customer: Uuid,
    order: Uuid,
    accept_tos: bool,
    now: OffsetDateTime,
) -> OrderResult<Order> {
    let o = of_customer(&state.db, customer, order).await?;
    if !accept_tos {
        return Err(problem(
            StatusCode::BAD_REQUEST,
            "tos_required",
            "Bạn cần chấp nhận Điều khoản dịch vụ xây app trước khi gửi.",
            "Accept the custom-build Terms of Service before submitting.",
        ));
    }
    let due = business_days_after(now, QUOTE_BUSINESS_DAYS);
    let mut tx = state.db.begin().await?;
    transition(
        &mut tx,
        order,
        Status::Submitted,
        &customer_actor(customer),
        "",
    )
    .await?;
    sqlx::query(
        "UPDATE orders SET tos_version = $2, tos_accepted_at = $3, quote_due_at = $4 WHERE id = $1",
    )
    .bind(order)
    .bind(TOS_VERSION)
    .bind(now)
    .bind(due)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    audit::record(
        &state.db,
        audit::Actor::User(customer),
        "order.submit",
        Some(&o.code),
        None,
        serde_json::json!({"tos": TOS_VERSION}),
    )
    .await;
    mail(
        state,
        &o,
        &format!("Đã nhận yêu cầu {}: {}", o.code, o.title),
        &format!(
            "F-Eco đã nhận yêu cầu xây app của bạn.\n\nMã đơn: {}\nTên dự án: {}\n\nĐội F-Eco sẽ gửi báo giá trong 3 ngày làm việc. Theo dõi đơn tại {}orders/{}\n",
            o.code, o.title, state.public_url, o.id
        ),
    )
    .await;
    crate::review::admin_alert(
        state,
        "order_submitted",
        &format!("Đơn {} cần báo giá trước {}", o.code, date(due)),
        serde_json::json!({"order": o.code, "title": o.title, "quote_due": date(due)}),
    )
    .await;
    get(&state.db, order).await
}

pub(crate) fn date(t: OffsetDateTime) -> String {
    t.format(&time::macros::format_description!("[day]/[month]/[year]"))
        .unwrap_or_default()
}

pub(crate) fn vnd(n: i64) -> String {
    let digits = n.abs().to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push('.');
        }
        out.push(c);
    }
    format!("{}{out} ₫", if n < 0 { "-" } else { "" })
}

/// Thông báo về đơn hàng (loại bắt buộc): email và trung tâm thông báo, liên kết tới trang đơn.
async fn mail(state: &AppState, o: &Order, subject: &str, body: &str) {
    let note = crate::notifications::Note::vi(subject, body, format!("/orders/{}", o.id));
    crate::notifications::notify(
        state,
        o.customer,
        Some(&o.customer_email),
        crate::notifications::Category::Order,
        &note,
    )
    .await;
}

// ---------- Báo giá ----------

#[derive(Debug, Clone, serde::Deserialize)]
pub struct MilestoneInput {
    pub title: String,
    pub deliverable: String,
    pub amount_vnd: i64,
    pub days: i32,
    #[serde(default)]
    pub free_revisions: Option<i32>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct QuoteInput {
    pub scope: String,
    pub exclusions: String,
    pub ownership: String,
    #[serde(default)]
    pub valid_days: Option<i64>,
    pub milestones: Vec<MilestoneInput>,
}

/// Kiểm tra báo giá: đủ phần, có mốc, đặt cọc (mốc đầu) không quá 30% tổng giá.
pub fn check_quote(q: &QuoteInput) -> OrderResult<i64> {
    let bad = |vi: &str, en: &str| problem(StatusCode::BAD_REQUEST, "invalid_quote", vi, en);
    if q.scope.trim().is_empty() || q.exclusions.trim().is_empty() || q.ownership.trim().is_empty()
    {
        return Err(bad(
            "Báo giá cần phạm vi, những gì không bao gồm và quyền sở hữu mã nguồn.",
            "A quote needs its scope, what it leaves out and who owns the source.",
        ));
    }
    if q.milestones.is_empty() {
        return Err(bad(
            "Báo giá cần ít nhất một mốc.",
            "A quote needs at least one milestone.",
        ));
    }
    if q.milestones.iter().any(|m| {
        m.amount_vnd <= 0
            || m.days <= 0
            || m.title.trim().is_empty()
            || m.free_revisions.is_some_and(|r| r < 0)
    }) {
        return Err(bad(
            "Mỗi mốc cần tên, số tiền và thời hạn dương.",
            "Each milestone needs a name, a positive amount and a positive duration.",
        ));
    }
    let total: i64 = q.milestones.iter().map(|m| m.amount_vnd).sum();
    let deposit = q.milestones[0].amount_vnd;
    if q.milestones.len() > 1 && deposit * 100 > total * MAX_DEPOSIT_PERCENT {
        return Err(bad(
            &format!("Đặt cọc {} vượt 30% tổng giá {}.", vnd(deposit), vnd(total)),
            &format!(
                "The deposit {} is more than 30% of the total {}.",
                vnd(deposit),
                vnd(total)
            ),
        ));
    }
    if q.milestones.len() == 1 {
        return Err(bad(
            "Cần ít nhất hai mốc: mốc đầu là đặt cọc, không quá 30% tổng giá.",
            "At least two milestones are needed: the first is the deposit, at most 30% of the total.",
        ));
    }
    Ok(total)
}

/// Gửi báo giá (đội F-Eco). Báo giá cũ còn mở bị thay; đơn sang (hoặc ở lại) "Đã báo giá".
pub async fn create_quote(
    state: &AppState,
    order: Uuid,
    team: &str,
    q: &QuoteInput,
    now: OffsetDateTime,
) -> OrderResult<Uuid> {
    let total = check_quote(q)?;
    let o = get(&state.db, order).await?;
    let valid_until = now + Duration::days(q.valid_days.unwrap_or(QUOTE_VALID_DAYS).clamp(1, 60));
    let mut tx = state.db.begin().await?;
    transition(&mut tx, order, Status::Quoted, &team_actor(team), "").await?;
    sqlx::query(
        "UPDATE order_quotes SET status = 'superseded' WHERE order_id = $1 AND status = 'open'",
    )
    .bind(order)
    .execute(&mut *tx)
    .await?;
    let number: i32 = sqlx::query_scalar(
        "SELECT coalesce(max(number), 0) + 1 FROM order_quotes WHERE order_id = $1",
    )
    .bind(order)
    .fetch_one(&mut *tx)
    .await?;
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO order_quotes (id, order_id, number, scope, exclusions, ownership, valid_until, created_by) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)")
        .bind(id)
        .bind(order)
        .bind(number)
        .bind(q.scope.trim())
        .bind(q.exclusions.trim())
        .bind(q.ownership.trim())
        .bind(valid_until)
        .bind(team)
        .execute(&mut *tx)
        .await?;
    for (i, m) in q.milestones.iter().enumerate() {
        sqlx::query(
            "INSERT INTO order_milestones (id, quote_id, order_id, position, title, deliverable, amount_vnd, days, free_revisions)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
        )
        .bind(Uuid::new_v4())
        .bind(id)
        .bind(order)
        .bind(i as i32 + 1)
        .bind(m.title.trim())
        .bind(m.deliverable.trim())
        .bind(m.amount_vnd)
        .bind(m.days)
        .bind(m.free_revisions.unwrap_or(FREE_REVISIONS))
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    let note = crate::notifications::Note::vi(
        format!("Báo giá cho đơn {}", o.code),
        format!(
            "Đội F-Eco đã gửi báo giá số {number} cho \"{}\": tổng {} gồm {} mốc, hiệu lực tới {}.",
            o.title,
            vnd(total),
            q.milestones.len(),
            date(valid_until),
        ),
        format!("/orders/{}#quote", o.id),
    )
    .en(
        format!("Quote for order {}", o.code),
        format!(
            "The F-Eco team sent quote no. {number} for \"{}\": {} in {} milestones, valid until {}.",
            o.title,
            vnd(total),
            q.milestones.len(),
            date(valid_until),
        ),
    );
    crate::notifications::notify(
        state,
        o.customer,
        Some(&o.customer_email),
        crate::notifications::Category::Order,
        &note,
    )
    .await;
    Ok(id)
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Milestone {
    pub id: Uuid,
    pub position: i32,
    pub title: String,
    pub deliverable: String,
    pub amount_vnd: i64,
    pub days: i32,
    pub free_revisions: i32,
    pub revisions_used: i32,
    pub status: String,
    pub paid: bool,
    pub due_at: Option<OffsetDateTime>,
    pub delivered_at: Option<OffsetDateTime>,
    pub accepted_at: Option<OffsetDateTime>,
    pub app_id: Option<String>,
    pub app_version: Option<String>,
    pub notes: String,
    pub check_report: String,
}

impl Milestone {
    pub fn revisions_left(&self) -> i32 {
        (self.free_revisions - self.revisions_used).max(0)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Quote {
    pub id: Uuid,
    pub number: i32,
    pub scope: String,
    pub exclusions: String,
    pub ownership: String,
    pub valid_until: OffsetDateTime,
    pub status: String,
    pub milestones: Vec<Milestone>,
}

impl Quote {
    pub fn total(&self) -> i64 {
        self.milestones.iter().map(|m| m.amount_vnd).sum()
    }

    /// Hết hạn: không chấp nhận được nữa (spec "Báo giá hết hạn").
    pub fn expired(&self, now: OffsetDateTime) -> bool {
        self.status == "open" && now > self.valid_until
    }
}

async fn milestones_of(db: &Db, quote: Uuid) -> OrderResult<Vec<Milestone>> {
    Ok(sqlx::query_as::<_, Milestone>(
        "SELECT id, position, title, deliverable, amount_vnd, days, free_revisions, revisions_used, status, paid,
                due_at, delivered_at, accepted_at, app_id, app_version, notes, check_report
         FROM order_milestones WHERE quote_id = $1 ORDER BY position",
    )
    .bind(quote)
    .fetch_all(db)
    .await?)
}

/// Báo giá đang dùng: báo giá đã chấp nhận, không có thì báo giá mở mới nhất.
pub async fn current_quote(db: &Db, order: Uuid) -> OrderResult<Option<Quote>> {
    let row: Option<(Uuid, i32, String, String, String, OffsetDateTime, String)> = sqlx::query_as(
        "SELECT id, number, scope, exclusions, ownership, valid_until, status FROM order_quotes
         WHERE order_id = $1 AND status IN ('accepted', 'open') ORDER BY (status = 'accepted') DESC, number DESC LIMIT 1",
    )
    .bind(order)
    .fetch_optional(db)
    .await?;
    let Some(r) = row else { return Ok(None) };
    let milestones = milestones_of(db, r.0).await?;
    Ok(Some(Quote {
        id: r.0,
        number: r.1,
        scope: r.2,
        exclusions: r.3,
        ownership: r.4,
        valid_until: r.5,
        status: r.6,
        milestones,
    }))
}

/// Tạo yêu cầu thanh toán (cổng thanh toán, nhóm 13, xử lý).
async fn payment_request(
    tx: &mut sqlx::PgConnection,
    order: Uuid,
    milestone: Option<Uuid>,
    kind: &str,
    amount: i64,
    reason: &str,
) -> OrderResult<Uuid> {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO payment_requests (id, order_id, milestone_id, kind, amount_vnd, reason) VALUES ($1, $2, $3, $4, $5, $6)")
        .bind(id)
        .bind(order)
        .bind(milestone)
        .bind(kind)
        .bind(amount)
        .bind(reason)
        .execute(&mut *tx)
        .await?;
    Ok(id)
}

#[derive(Debug, Clone, Serialize)]
pub struct PaymentRequest {
    pub id: Uuid,
    pub milestone_id: Option<Uuid>,
    pub kind: String,
    pub amount_vnd: i64,
    pub status: String,
    pub reason: String,
}

pub async fn payment_requests(db: &Db, order: Uuid) -> OrderResult<Vec<PaymentRequest>> {
    let rows: Vec<(Uuid, Option<Uuid>, String, i64, String, String)> =
        sqlx::query_as("SELECT id, milestone_id, kind, amount_vnd, status, reason FROM payment_requests WHERE order_id = $1 ORDER BY created_at")
            .bind(order)
            .fetch_all(db)
            .await?;
    Ok(rows
        .into_iter()
        .map(|r| PaymentRequest {
            id: r.0,
            milestone_id: r.1,
            kind: r.2,
            amount_vnd: r.3,
            status: r.4,
            reason: r.5,
        })
        .collect())
}

/// Khách chấp nhận báo giá còn hiệu lực: tạo yêu cầu thanh toán mốc đặt cọc. Đơn bắt đầu thực hiện
/// khi đặt cọc thành công ([`payment_settled`]).
pub async fn accept_quote(
    state: &AppState,
    customer: Uuid,
    order: Uuid,
    now: OffsetDateTime,
) -> OrderResult<Uuid> {
    let o = of_customer(&state.db, customer, order).await?;
    if o.status != Status::Quoted {
        return Err(problem(
            StatusCode::CONFLICT,
            "invalid_transition",
            "Đơn không ở trạng thái Đã báo giá.",
            "The order isn't quoted.",
        ));
    }
    let q = current_quote(&state.db, order)
        .await?
        .filter(|q| q.status == "open")
        .ok_or_else(|| {
            problem(
                StatusCode::CONFLICT,
                "no_open_quote",
                "Không có báo giá đang mở.",
                "There is no open quote.",
            )
        })?;
    if q.expired(now) {
        return Err(problem(
            StatusCode::CONFLICT,
            "quote_expired",
            "Báo giá đã hết hạn. Hãy yêu cầu báo giá lại.",
            "The quote has expired. Ask for a new quote.",
        ));
    }
    let deposit = q.milestones.first().ok_or_else(|| {
        problem(
            StatusCode::CONFLICT,
            "no_open_quote",
            "Báo giá không có mốc.",
            "The quote has no milestones.",
        )
    })?;
    let mut tx = state.db.begin().await?;
    sqlx::query("UPDATE order_quotes SET status = 'accepted', accepted_at = $2 WHERE id = $1")
        .bind(q.id)
        .bind(now)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE order_milestones SET status = 'awaiting_payment' WHERE id = $1")
        .bind(deposit.id)
        .execute(&mut *tx)
        .await?;
    let request = payment_request(
        &mut tx,
        order,
        Some(deposit.id),
        "charge",
        deposit.amount_vnd,
        "Đặt cọc",
    )
    .await?;
    sqlx::query("INSERT INTO order_events (order_id, from_status, to_status, actor, note) VALUES ($1, 'quoted', 'quoted', $2, $3)")
        .bind(order)
        .bind(customer_actor(customer))
        .bind(format!("Chấp nhận báo giá số {}; chờ đặt cọc {}", q.number, vnd(deposit.amount_vnd)))
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    audit::record(
        &state.db,
        audit::Actor::User(customer),
        "order.quote_accept",
        Some(&o.code),
        None,
        serde_json::json!({"quote": q.number}),
    )
    .await;
    Ok(request)
}

/// Thương lượng (spec "Thương lượng") hoặc yêu cầu báo giá lại sau khi hết hạn: đơn vẫn "Đã báo
/// giá", đề nghị được lưu trong đơn và đội F-Eco nhận việc gửi báo giá mới.
pub async fn ask_adjustment(
    state: &AppState,
    customer: Uuid,
    order: Uuid,
    note: &str,
) -> OrderResult<()> {
    let o = of_customer(&state.db, customer, order).await?;
    if o.status != Status::Quoted {
        return Err(problem(
            StatusCode::CONFLICT,
            "invalid_transition",
            "Đơn không ở trạng thái Đã báo giá.",
            "The order isn't quoted.",
        ));
    }
    let note = if note.trim().is_empty() {
        "Yêu cầu báo giá lại"
    } else {
        note.trim()
    };
    sqlx::query("INSERT INTO order_messages (order_id, author, kind, body) VALUES ($1, 'customer', 'adjustment', $2)").bind(order).bind(note).execute(&state.db).await?;
    crate::review::admin_alert(
        state,
        "order_adjustment",
        &format!("Đơn {} đề nghị điều chỉnh báo giá", o.code),
        serde_json::json!({"order": o.code, "note": note}),
    )
    .await;
    Ok(())
}

/// Kết quả thanh toán từ cổng thanh toán (nhóm 13; tạm thời qua `feco-catalog order-paid`).
/// Đặt cọc thành công: đơn "Đang thực hiện", thời hạn các mốc tính từ hôm nay, khách nhận email
/// kèm lịch các mốc. Mốc sau: mốc đó bắt đầu thực hiện.
pub async fn payment_settled(
    state: &AppState,
    request: Uuid,
    succeeded: bool,
    now: OffsetDateTime,
) -> OrderResult<()> {
    let row: Option<(Uuid, Option<Uuid>, String, String)> = sqlx::query_as(
        "SELECT order_id, milestone_id, kind, status FROM payment_requests WHERE id = $1",
    )
    .bind(request)
    .fetch_optional(&state.db)
    .await?;
    let (order, milestone, kind, status) = row.ok_or_else(|| {
        problem(
            StatusCode::NOT_FOUND,
            "not_found",
            "Không có yêu cầu thanh toán này.",
            "No such payment request.",
        )
    })?;
    if status != "pending" {
        return Ok(());
    }
    let mut tx = state.db.begin().await?;
    sqlx::query("UPDATE payment_requests SET status = $2, settled_at = $3 WHERE id = $1")
        .bind(request)
        .bind(if succeeded { "succeeded" } else { "failed" })
        .bind(now)
        .execute(&mut *tx)
        .await?;
    if !succeeded || kind != "charge" {
        tx.commit().await?;
        return Ok(());
    }
    let Some(milestone) = milestone else {
        tx.commit().await?;
        return Ok(());
    };
    let position: i32 = sqlx::query_scalar("SELECT position FROM order_milestones WHERE id = $1")
        .bind(milestone)
        .fetch_one(&mut *tx)
        .await?;
    sqlx::query("UPDATE order_milestones SET paid = true, status = 'in_progress' WHERE id = $1")
        .bind(milestone)
        .execute(&mut *tx)
        .await?;
    let o = get(&state.db, order).await?;
    if position == 1 && o.status == Status::Quoted {
        transition(
            &mut tx,
            order,
            Status::InProgress,
            SYSTEM_ACTOR,
            "Đặt cọc thành công",
        )
        .await?;
        sqlx::query("UPDATE orders SET started_at = $2 WHERE id = $1")
            .bind(order)
            .bind(now)
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            "UPDATE order_milestones SET due_at = $2 + make_interval(days => days)
             WHERE order_id = $1 AND quote_id = (SELECT quote_id FROM order_milestones WHERE id = $3)",
        )
        .bind(order)
        .bind(now)
        .bind(milestone)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    if position == 1 {
        let q = current_quote(&state.db, order).await?;
        let schedule = q
            .map(|q| {
                q.milestones
                    .iter()
                    .map(|m| {
                        format!(
                            "- Mốc {}: {} ({}), hạn {}",
                            m.position,
                            m.title,
                            vnd(m.amount_vnd),
                            m.due_at.map(date).unwrap_or_default()
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default();
        mail(
            state,
            &o,
            &format!("Đơn {} bắt đầu thực hiện", o.code),
            &format!("Đã nhận tiền đặt cọc. Đơn \"{}\" bắt đầu thực hiện hôm nay.\n\nLịch các mốc:\n{schedule}\n", o.title),
        )
        .await;
    }
    Ok(())
}

async fn milestone_at(db: &Db, order: Uuid, position: i32) -> OrderResult<Milestone> {
    let q = current_quote(db, order)
        .await?
        .filter(|q| q.status == "accepted")
        .ok_or_else(|| {
            problem(
                StatusCode::CONFLICT,
                "no_accepted_quote",
                "Đơn chưa có báo giá đã chấp nhận.",
                "The order has no accepted quote.",
            )
        })?;
    q.milestones
        .into_iter()
        .find(|m| m.position == position)
        .ok_or_else(|| {
            problem(
                StatusCode::NOT_FOUND,
                "not_found",
                "Không có mốc này.",
                "No such milestone.",
            )
        })
}

#[derive(Debug, Clone, Default)]
pub struct Delivery {
    pub app_id: String,
    pub app_version: String,
    pub notes: String,
    pub check_report: String,
}

/// Đội F-Eco bàn giao một mốc: bản build qua F-Launcher (app riêng tư), ghi chú thay đổi, kết quả
/// kiểm tra theo spec. Đơn sang "Đã bàn giao"; khách có 7 ngày để nghiệm thu hoặc yêu cầu sửa.
pub async fn deliver(
    state: &AppState,
    order: Uuid,
    position: i32,
    team: &str,
    d: &Delivery,
    now: OffsetDateTime,
) -> OrderResult<()> {
    let o = get(&state.db, order).await?;
    let m = milestone_at(&state.db, order, position).await?;
    if m.status != "in_progress" || !m.paid {
        return Err(problem(
            StatusCode::CONFLICT,
            "milestone_not_in_progress",
            "Mốc này chưa ở trạng thái đang thực hiện (đã thanh toán).",
            "This milestone isn't in progress (paid).",
        ));
    }
    if d.app_id.trim().is_empty()
        || d.app_version.trim().is_empty()
        || d.check_report.trim().is_empty()
    {
        return Err(problem(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "Bàn giao cần bản build (app, phiên bản) và kết quả kiểm tra theo spec.",
            "A delivery needs the build (app, version) and the checks against the spec.",
        ));
    }
    // Bản build là app riêng tư trong danh mục, đã phát hành phiên bản này (qua kiểm duyệt).
    let app = d.app_id.trim();
    let row: Option<(String, Option<Uuid>)> =
        sqlx::query_as("SELECT client_id, private_to FROM catalog_apps WHERE id = $1")
            .bind(app)
            .fetch_optional(&state.db)
            .await?;
    let (client_id, private_to) = row.ok_or_else(|| {
        problem(
            StatusCode::BAD_REQUEST,
            "app_not_in_catalog",
            format!("App {app} chưa có trong danh mục."),
            format!("App {app} isn't in the catalog."),
        )
    })?;
    if private_to.is_some_and(|p| p != o.customer) {
        return Err(problem(
            StatusCode::CONFLICT,
            "app_private_elsewhere",
            "App này là app riêng tư của khách hàng khác.",
            "This app is private to another customer.",
        ));
    }
    let released: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM catalog_versions WHERE app_id = $1 AND version = $2 AND status = 'released')")
        .bind(app)
        .bind(d.app_version.trim())
        .fetch_one(&state.db)
        .await?;
    if !released {
        return Err(problem(
            StatusCode::BAD_REQUEST,
            "build_not_released",
            "Phiên bản này chưa được phát hành trong danh mục (cần qua kiểm duyệt).",
            "This version hasn't been released in the catalog (it must pass review).",
        ));
    }
    crate::catalog::set_private(&state.db, &state.catalog, app, Some(o.customer)).await?;
    crate::entitlements::grant(
        state,
        &client_id,
        o.customer,
        "license",
        &format!("order:{}", o.code),
        None,
    )
    .await?;
    let mut tx = state.db.begin().await?;
    transition(
        &mut tx,
        order,
        Status::Delivered,
        &team_actor(team),
        &format!("Bàn giao mốc {position}"),
    )
    .await?;
    sqlx::query(
        "UPDATE order_milestones SET status = 'delivered', delivered_at = $2, reminded_at = NULL, app_id = $3, app_version = $4, notes = $5, check_report = $6
         WHERE id = $1",
    )
    .bind(m.id)
    .bind(now)
    .bind(d.app_id.trim())
    .bind(d.app_version.trim())
    .bind(d.notes.trim())
    .bind(d.check_report.trim())
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    mail(
        state,
        &o,
        &format!("Đơn {}: bàn giao mốc {position}", o.code),
        &format!(
            "Mốc {position} \"{}\" đã được bàn giao. Mở F-Launcher để chạy thử app \"{}\" {} (chỉ bạn thấy).\n\nGhi chú: {}\n\nHãy nghiệm thu hoặc yêu cầu sửa trong 7 ngày tại {}orders/{}\n",
            m.title,
            d.app_id.trim(),
            d.app_version.trim(),
            d.notes.trim(),
            state.public_url,
            o.id
        ),
    )
    .await;
    Ok(())
}

/// Nghiệm thu mốc (khách, hoặc hệ thống sau 14 ngày): mốc cuối thì đơn "Hoàn tất", không thì tạo
/// yêu cầu thanh toán mốc kế tiếp (spec "Nghiệm thu mốc").
pub async fn accept_milestone(
    state: &AppState,
    order: Uuid,
    position: i32,
    actor: &str,
    now: OffsetDateTime,
) -> OrderResult<Option<Uuid>> {
    let o = get(&state.db, order).await?;
    let m = milestone_at(&state.db, order, position).await?;
    if m.status != "delivered" {
        return Err(problem(
            StatusCode::CONFLICT,
            "milestone_not_delivered",
            "Mốc này chưa được bàn giao.",
            "This milestone hasn't been delivered.",
        ));
    }
    let q = current_quote(&state.db, order)
        .await?
        .ok_or_else(not_found)?;
    let next = q
        .milestones
        .iter()
        .find(|x| x.position == position + 1)
        .cloned();
    let mut tx = state.db.begin().await?;
    sqlx::query("UPDATE order_milestones SET status = 'accepted', accepted_at = $2 WHERE id = $1")
        .bind(m.id)
        .bind(now)
        .execute(&mut *tx)
        .await?;
    let request = match &next {
        None => {
            transition(
                &mut tx,
                order,
                Status::Completed,
                actor,
                &format!("Nghiệm thu mốc cuối ({position})"),
            )
            .await?;
            None
        }
        Some(n) => {
            transition(
                &mut tx,
                order,
                Status::InProgress,
                actor,
                &format!("Nghiệm thu mốc {position}"),
            )
            .await?;
            sqlx::query("UPDATE order_milestones SET status = 'awaiting_payment' WHERE id = $1")
                .bind(n.id)
                .execute(&mut *tx)
                .await?;
            Some(
                payment_request(
                    &mut tx,
                    order,
                    Some(n.id),
                    "charge",
                    n.amount_vnd,
                    &format!("Mốc {}", n.position),
                )
                .await?,
            )
        }
    };
    tx.commit().await?;
    let what = match &next {
        None => format!(
            "Bạn đã nghiệm thu mốc cuối. Đơn \"{}\" đã hoàn tất; tải bàn giao cuối tại {}orders/{}",
            o.title, state.public_url, o.id
        ),
        Some(n) => format!(
            "Mốc {position} đã được nghiệm thu. Yêu cầu thanh toán mốc {} ({}) đã được tạo.",
            n.position,
            vnd(n.amount_vnd)
        ),
    };
    mail(
        state,
        &o,
        &format!("Đơn {}: nghiệm thu mốc {position}", o.code),
        &what,
    )
    .await;
    Ok(request)
}

/// Yêu cầu sửa mốc đã bàn giao: còn lượt miễn phí thì mốc quay lại thực hiện; hết lượt thì cần báo
/// giá bổ sung (spec "Yêu cầu sửa trong giới hạn", "Hết lượt sửa miễn phí").
pub async fn request_revision(
    state: &AppState,
    customer: Uuid,
    order: Uuid,
    position: i32,
    description: &str,
) -> OrderResult<i32> {
    let o = of_customer(&state.db, customer, order).await?;
    let m = milestone_at(&state.db, order, position).await?;
    if m.status != "delivered" {
        return Err(problem(
            StatusCode::CONFLICT,
            "milestone_not_delivered",
            "Chỉ yêu cầu sửa được mốc đang chờ nghiệm thu.",
            "Only a delivered milestone can be revised.",
        ));
    }
    if description.trim().is_empty() {
        return Err(problem(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "Hãy mô tả lỗi cần sửa.",
            "Describe what to fix.",
        ));
    }
    if m.revisions_left() == 0 {
        return Err(problem(
            StatusCode::CONFLICT,
            "revision_needs_quote",
            "Mốc này đã dùng hết lượt sửa miễn phí: yêu cầu sửa thêm cần báo giá bổ sung.",
            "This milestone has used its free revisions: more changes need an additional quote.",
        ));
    }
    let mut tx = state.db.begin().await?;
    transition(
        &mut tx,
        order,
        Status::InProgress,
        &customer_actor(customer),
        &format!("Yêu cầu sửa mốc {position}"),
    )
    .await?;
    sqlx::query("UPDATE order_milestones SET status = 'in_progress', revisions_used = revisions_used + 1, delivered_at = NULL, reminded_at = NULL WHERE id = $1")
        .bind(m.id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("INSERT INTO order_messages (order_id, author, kind, body) VALUES ($1, 'customer', 'revision', $2)")
        .bind(order)
        .bind(format!("Mốc {position}: {}", description.trim()))
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    crate::review::admin_alert(
        state,
        "order_revision",
        &format!("Đơn {} yêu cầu sửa mốc {position}", o.code),
        serde_json::json!({"order": o.code, "milestone": position}),
    )
    .await;
    Ok(m.revisions_left() - 1)
}

/// Mốc chờ nghiệm thu: đơn, mốc, lúc bàn giao, lúc đã nhắc, email khách, mã đơn, trạng thái đơn.
type DueRow = (
    Uuid,
    i32,
    OffsetDateTime,
    Option<OffsetDateTime>,
    String,
    String,
    String,
);

/// Việc theo thời gian (worker mỗi giờ, và test với thời gian mô phỏng): nhắc nghiệm thu sau 7
/// ngày, tự nghiệm thu sau 14 ngày. Trả số mốc đã tự nghiệm thu.
pub async fn process_due(state: &AppState, now: OffsetDateTime) -> OrderResult<usize> {
    let rows: Vec<DueRow> = sqlx::query_as(
        "SELECT m.order_id, m.position, m.delivered_at, m.reminded_at, o.customer_email, o.code, o.status
         FROM order_milestones m JOIN orders o ON o.id = m.order_id
         WHERE m.status = 'delivered' AND m.delivered_at IS NOT NULL",
    )
    .fetch_all(&state.db)
    .await?;
    let mut accepted = 0;
    for (order, position, delivered, reminded, email, code, status) in rows {
        if status != Status::Delivered.as_str() {
            continue;
        }
        let age = now - delivered;
        if age >= Duration::days(AUTO_ACCEPT_DAYS) {
            accept_milestone(state, order, position, SYSTEM_ACTOR, now).await?;
            accepted += 1;
        } else if age >= Duration::days(REMIND_AFTER_DAYS) && reminded.is_none() {
            sqlx::query("UPDATE order_milestones SET reminded_at = $3 WHERE order_id = $1 AND position = $2 AND status = 'delivered'")
                .bind(order)
                .bind(position)
                .bind(now)
                .execute(&state.db)
                .await?;
            let o = get(&state.db, order).await?;
            debug_assert_eq!(o.customer_email, email);
            mail(
                state,
                &o,
                &format!("Nhắc nghiệm thu mốc {position} của đơn {code}"),
                &format!("Mốc {position} đã bàn giao được 7 ngày. Hãy nghiệm thu hoặc yêu cầu sửa; sau 14 ngày mốc sẽ tự được nghiệm thu.\n"),
            )
            .await;
        }
    }
    Ok(accepted)
}

// ---------- Tin nhắn ----------

#[derive(Debug, Clone, Serialize)]
pub struct Message {
    pub id: i64,
    pub author: String,
    pub author_name: String,
    pub kind: String,
    pub body: String,
    pub at: OffsetDateTime,
}

pub async fn messages(db: &Db, order: Uuid) -> OrderResult<Vec<Message>> {
    let rows: Vec<(i64, String, String, String, String, OffsetDateTime)> =
        sqlx::query_as("SELECT id, author, author_name, kind, body, at FROM order_messages WHERE order_id = $1 ORDER BY id").bind(order).fetch_all(db).await?;
    Ok(rows
        .into_iter()
        .map(|r| Message {
            id: r.0,
            author: r.1,
            author_name: r.2,
            kind: r.3,
            body: r.4,
            at: r.5,
        })
        .collect())
}

/// Tin nhắn trong đơn; người bên kia được báo (khách qua email, đội F-Eco qua việc cần làm).
pub async fn post_message(
    state: &AppState,
    order: Uuid,
    from_team: Option<&str>,
    body: &str,
) -> OrderResult<i64> {
    let o = get(&state.db, order).await?;
    let body = body.trim();
    if body.is_empty() || body.chars().count() > 10_000 {
        return Err(problem(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "Tin nhắn cần từ 1 đến 10.000 ký tự.",
            "A message needs 1 to 10,000 characters.",
        ));
    }
    let (author, name) = match from_team {
        Some(n) => ("team", n),
        None => ("customer", ""),
    };
    let id: i64 = sqlx::query_scalar("INSERT INTO order_messages (order_id, author, author_name, body) VALUES ($1, $2, $3, $4) RETURNING id")
        .bind(order)
        .bind(author)
        .bind(name)
        .bind(body)
        .fetch_one(&state.db)
        .await?;
    match from_team {
        Some(_) => {
            let note = crate::notifications::Note::vi(
                format!("Tin nhắn mới trong đơn {}", o.code),
                format!("Bạn có tin nhắn mới trong đơn #{}.", o.code),
                format!("/orders/{}#thread", o.id),
            )
            .en(
                format!("New message in order {}", o.code),
                format!("You have a new message in order #{}.", o.code),
            );
            crate::notifications::notify(
                state,
                o.customer,
                Some(&o.customer_email),
                crate::notifications::Category::Order,
                &note,
            )
            .await
        }
        None => {
            crate::review::admin_alert(
                state,
                "order_message",
                &format!("Tin nhắn mới trong đơn {}", o.code),
                serde_json::json!({"order": o.code}),
            )
            .await
        }
    }
    Ok(id)
}

// ---------- Hủy đơn, tranh chấp ----------

/// Hủy đơn (khách) trước "Hoàn tất". Trước khi đặt cọc: không mất phí. Sau đặt cọc: hoàn các khoản
/// đã trả cho mốc chưa bàn giao, giữ tiền đặt cọc khi công việc đã bắt đầu (spec "Hủy đơn").
pub async fn cancel(
    state: &AppState,
    customer: Uuid,
    order: Uuid,
    reason: &str,
) -> OrderResult<Vec<Uuid>> {
    let o = of_customer(&state.db, customer, order).await?;
    let quote = current_quote(&state.db, order).await?;
    let mut tx = state.db.begin().await?;
    transition(
        &mut tx,
        order,
        Status::Cancelled,
        &customer_actor(customer),
        reason.trim(),
    )
    .await?;
    sqlx::query("UPDATE payment_requests SET status = 'cancelled' WHERE order_id = $1 AND status = 'pending'").bind(order).execute(&mut *tx).await?;
    let mut refunds = Vec::new();
    if let Some(q) = quote.filter(|q| q.status == "accepted") {
        let started = o.started_at.is_some();
        for m in &q.milestones {
            let delivered = matches!(m.status.as_str(), "delivered" | "accepted");
            let deposit_kept = m.position == 1 && started;
            if m.paid && !delivered && !deposit_kept {
                refunds.push(
                    payment_request(
                        &mut tx,
                        order,
                        Some(m.id),
                        "refund",
                        m.amount_vnd,
                        &format!("Hủy đơn: hoàn mốc {}", m.position),
                    )
                    .await?,
                );
            }
        }
    }
    tx.commit().await?;
    // Lệnh hoàn với cổng thanh toán; hủy đơn là việc của khách nên cần quản trị viên duyệt.
    for r in &refunds {
        if let Err(e) =
            crate::payments::from_order_request(state, *r, crate::payments::Requester::System).await
        {
            tracing::error!(error = %e, "không tạo được lệnh hoàn khi hủy đơn");
        }
    }
    let note = if refunds.is_empty() {
        String::new()
    } else {
        format!(" Đã tạo {} yêu cầu hoàn tiền.", refunds.len())
    };
    mail(
        state,
        &o,
        &format!("Đơn {} đã hủy", o.code),
        &format!("Đơn \"{}\" đã được hủy.{note}\n", o.title),
    )
    .await;
    Ok(refunds)
}

/// Mở tranh chấp cho một mốc trong 14 ngày sau khi bàn giao. Quản trị viên phản hồi trong 2 ngày
/// làm việc (spec "Mở tranh chấp").
pub async fn open_dispute(
    state: &AppState,
    customer: Uuid,
    order: Uuid,
    position: i32,
    reason: &str,
    now: OffsetDateTime,
) -> OrderResult<Uuid> {
    let o = of_customer(&state.db, customer, order).await?;
    let m = milestone_at(&state.db, order, position).await?;
    let delivered = m.delivered_at.ok_or_else(|| {
        problem(
            StatusCode::CONFLICT,
            "milestone_not_delivered",
            "Mốc này chưa được bàn giao.",
            "This milestone hasn't been delivered.",
        )
    })?;
    if now - delivered > Duration::days(DISPUTE_WINDOW_DAYS) {
        return Err(problem(
            StatusCode::CONFLICT,
            "dispute_window_closed",
            "Đã quá 14 ngày kể từ khi bàn giao mốc này.",
            "More than 14 days have passed since this milestone was delivered.",
        ));
    }
    if reason.trim().is_empty() {
        return Err(problem(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "Hãy nêu lý do tranh chấp.",
            "Say why you dispute it.",
        ));
    }
    let respond_by = business_days_after(now, DISPUTE_BUSINESS_DAYS);
    let id = Uuid::new_v4();
    let mut tx = state.db.begin().await?;
    transition(
        &mut tx,
        order,
        Status::Disputed,
        &customer_actor(customer),
        &format!("Tranh chấp mốc {position}"),
    )
    .await?;
    sqlx::query("INSERT INTO order_disputes (id, order_id, milestone_id, reason, opened_at, respond_by) VALUES ($1, $2, $3, $4, $5, $6)")
        .bind(id)
        .bind(order)
        .bind(m.id)
        .bind(reason.trim())
        .bind(now)
        .bind(respond_by)
        .execute(&mut *tx)
        .await?;
    sqlx::query("INSERT INTO order_messages (order_id, author, kind, body) VALUES ($1, 'customer', 'dispute', $2)")
        .bind(order)
        .bind(format!("Tranh chấp mốc {position}: {}", reason.trim()))
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    crate::review::admin_alert(
        state,
        "order_dispute",
        &format!("Tranh chấp đơn {} mốc {position}: phản hồi trước {}", o.code, date(respond_by)),
        serde_json::json!({"order": o.code, "milestone": position, "reason": reason.trim(), "respond_by": date(respond_by)}),
    )
    .await;
    Ok(id)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resolution {
    /// Tiếp tục sửa: mốc quay lại thực hiện.
    Continue,
    /// Hoàn một phần tiền của mốc.
    PartialRefund(i64),
    /// Hoàn toàn bộ tiền của mốc.
    FullRefund,
}

/// Quản trị viên giải quyết tranh chấp: tiếp tục sửa, hoàn một phần hoặc hoàn toàn bộ mốc.
pub async fn resolve_dispute(
    state: &AppState,
    order: Uuid,
    admin: &str,
    resolution: Resolution,
) -> OrderResult<Option<Uuid>> {
    let o = get(&state.db, order).await?;
    let row: Option<(Uuid, Uuid, i64, i32)> = sqlx::query_as(
        "SELECT d.id, m.id, m.amount_vnd, m.position FROM order_disputes d JOIN order_milestones m ON m.id = d.milestone_id
         WHERE d.order_id = $1 AND d.resolved_at IS NULL ORDER BY d.opened_at DESC LIMIT 1",
    )
    .bind(order)
    .fetch_optional(&state.db)
    .await?;
    let (dispute, milestone, amount, position) = row.ok_or_else(|| {
        problem(
            StatusCode::CONFLICT,
            "no_open_dispute",
            "Đơn không có tranh chấp đang mở.",
            "The order has no open dispute.",
        )
    })?;
    let before: Option<String> =
        sqlx::query_scalar("SELECT status_before_dispute FROM orders WHERE id = $1")
            .bind(order)
            .fetch_one(&state.db)
            .await?;
    let back = before
        .as_deref()
        .and_then(Status::parse)
        .filter(|s| *s != Status::Disputed)
        .unwrap_or(Status::InProgress);
    let mut tx = state.db.begin().await?;
    let (name, refund_vnd, to) = match resolution {
        Resolution::Continue => ("continue", None, Status::InProgress),
        Resolution::PartialRefund(n) if n > 0 && n < amount => ("partial_refund", Some(n), back),
        Resolution::PartialRefund(_) => {
            return Err(problem(
                StatusCode::BAD_REQUEST,
                "invalid_request",
                "Số tiền hoàn một phần phải lớn hơn 0 và nhỏ hơn giá mốc.",
                "A partial refund must be above 0 and below the milestone's price.",
            ))
        }
        Resolution::FullRefund => ("full_refund", Some(amount), back),
    };
    transition(
        &mut tx,
        order,
        to,
        &team_actor(admin),
        &format!("Giải quyết tranh chấp mốc {position}: {name}"),
    )
    .await?;
    if resolution == Resolution::Continue {
        sqlx::query(
            "UPDATE order_milestones SET status = 'in_progress', delivered_at = NULL WHERE id = $1",
        )
        .bind(milestone)
        .execute(&mut *tx)
        .await?;
    }
    if resolution == Resolution::FullRefund {
        sqlx::query("UPDATE order_milestones SET status = 'refunded' WHERE id = $1")
            .bind(milestone)
            .execute(&mut *tx)
            .await?;
    }
    let refund = match refund_vnd {
        Some(n) => Some(
            payment_request(
                &mut tx,
                order,
                Some(milestone),
                "refund",
                n,
                &format!("Tranh chấp mốc {position}"),
            )
            .await?,
        ),
        None => None,
    };
    sqlx::query("UPDATE order_disputes SET resolution = $2, refund_vnd = $3, resolved_by = $4, resolved_at = now() WHERE id = $1")
        .bind(dispute)
        .bind(name)
        .bind(refund_vnd)
        .bind(admin)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    if let Some(r) = refund {
        if let Err(e) = crate::payments::from_order_request(
            state,
            r,
            crate::payments::Requester::Admin(admin.to_owned()),
        )
        .await
        {
            tracing::error!(error = %e, "không tạo được lệnh hoàn khi giải quyết tranh chấp");
        }
    }
    let what = match resolution {
        Resolution::Continue => "Đội F-Eco sẽ tiếp tục sửa mốc này.".to_string(),
        Resolution::PartialRefund(n) => format!("Bạn được hoàn {} cho mốc này.", vnd(n)),
        Resolution::FullRefund => format!("Bạn được hoàn toàn bộ {} của mốc này.", vnd(amount)),
    };
    mail(
        state,
        &o,
        &format!("Kết quả tranh chấp đơn {}", o.code),
        &format!("Tranh chấp mốc {position} đã được giải quyết. {what}\n"),
    )
    .await;
    Ok(refund)
}

// ---------- Bàn giao cuối ----------

/// Tải bàn giao cuối: chỉ khi đơn Hoàn tất và đã thanh toán đủ (spec "Bàn giao cuối và quyền sở hữu").
pub async fn handover_ready(db: &Db, order: &Order) -> OrderResult<bool> {
    if order.status != Status::Completed {
        return Ok(false);
    }
    let unpaid: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM order_milestones m JOIN order_quotes q ON q.id = m.quote_id
         WHERE m.order_id = $1 AND q.status = 'accepted' AND NOT m.paid AND m.status <> 'refunded'",
    )
    .bind(order.id)
    .fetch_one(db)
    .await?;
    let pending: i64 = sqlx::query_scalar("SELECT count(*) FROM payment_requests WHERE order_id = $1 AND kind = 'charge' AND status = 'pending'")
        .bind(order.id)
        .fetch_one(db)
        .await?;
    Ok(unpaid == 0 && pending == 0)
}

/// Thư mục giữ bàn giao cuối của một đơn (ZIP do đội F-Eco đặt vào: mã nguồn, spec, tài liệu, build).
pub fn handover_path(state: &AppState, order: &Order) -> std::path::PathBuf {
    state
        .package_dir
        .join("orders")
        .join(order.id.to_string())
        .join("handover.zip")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Spec "Chuyển trạng thái không hợp lệ" và mọi bước chuyển của máy trạng thái.
    #[test]
    fn only_listed_transitions_are_allowed() {
        use Status::*;
        assert!(
            !can_move(Submitted, Completed),
            "Đã gửi thẳng sang Hoàn tất"
        );
        let allowed = [
            (Draft, Submitted),
            (Submitted, Quoted),
            (Quoted, Quoted),
            (Quoted, InProgress),
            (InProgress, Delivered),
            (Delivered, InProgress),
            (Delivered, Completed),
            (InProgress, Disputed),
            (Delivered, Disputed),
            (Completed, Disputed),
            (Disputed, InProgress),
            (Disputed, Delivered),
            (Disputed, Completed),
            (Draft, Cancelled),
            (Submitted, Cancelled),
            (Quoted, Cancelled),
            (InProgress, Cancelled),
            (Delivered, Cancelled),
            (Disputed, Cancelled),
        ];
        for from in Status::ALL {
            for to in Status::ALL {
                assert_eq!(
                    can_move(from, to),
                    allowed.contains(&(from, to)),
                    "{from:?} -> {to:?}"
                );
            }
        }
        // Đã hủy và Hoàn tất (trừ tranh chấp) là điểm cuối.
        assert!(Status::ALL.iter().all(|s| !can_move(Cancelled, *s)));
        assert!(!can_move(Completed, Cancelled), "không hủy sau Hoàn tất");
        for s in Status::ALL {
            assert_eq!(Status::parse(s.as_str()), Some(s));
        }
    }

    #[test]
    fn deposit_is_at_most_thirty_percent() {
        let m = |amount| MilestoneInput {
            title: "M".into(),
            deliverable: "D".into(),
            amount_vnd: amount,
            days: 10,
            free_revisions: None,
        };
        let quote = |ms: Vec<MilestoneInput>| QuoteInput {
            scope: "S".into(),
            exclusions: "E".into(),
            ownership: "O".into(),
            valid_days: None,
            milestones: ms,
        };
        assert_eq!(
            check_quote(&quote(vec![m(9_000_000), m(12_000_000), m(9_000_000)])).unwrap(),
            30_000_000,
            "30% exactly"
        );
        assert!(
            check_quote(&quote(vec![m(10_000_000), m(12_000_000), m(8_000_000)])).is_err(),
            "33%"
        );
        assert!(
            check_quote(&quote(vec![m(30_000_000)])).is_err(),
            "a single milestone would be a 100% deposit"
        );
        assert!(check_quote(&quote(vec![m(1), m(-5)])).is_err());
        let mut q = quote(vec![m(3), m(7)]);
        q.ownership = " ".into();
        assert!(
            check_quote(&q).is_err(),
            "who owns the source must be stated"
        );
    }

    #[test]
    fn business_days_skip_weekends() {
        let fri = time::macros::datetime!(2026-11-13 10:00 UTC);
        assert_eq!(
            business_days_after(fri, 3).date(),
            time::macros::date!(2026 - 11 - 18)
        );
        assert_eq!(
            business_days_after(fri, 2).date(),
            time::macros::date!(2026 - 11 - 17)
        );
        let mon = time::macros::datetime!(2026-11-16 10:00 UTC);
        assert_eq!(
            business_days_after(mon, 3).date(),
            time::macros::date!(2026 - 11 - 19)
        );
    }

    #[test]
    fn money_reads_as_vietnamese_dong() {
        assert_eq!(vnd(9_000_000), "9.000.000 ₫");
        assert_eq!(vnd(30_000_000), "30.000.000 ₫");
        assert_eq!(vnd(500), "500 ₫");
    }
}
