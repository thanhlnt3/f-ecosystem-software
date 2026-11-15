//! Dự án phần mềm giữa khách hàng và người nhận việc (thay đổi OpenSpec
//! fcoin-wallet-and-marketplace, design R7–R8).
//!
//! Luồng: hồ sơ nhận việc → khách gửi yêu cầu → thương lượng (đề nghị giá, thời hạn qua lại) →
//! chốt giá → (hợp đồng nếu có) → ký quỹ F-Coin → bàn giao → chấp nhận (giải ngân) hoặc yêu cầu
//! sửa → tranh chấp do Quản lý dự án xử lý.
//!
//! Mỗi bước khóa dòng dự án (`FOR UPDATE`) và ghi bút toán ví trong cùng giao dịch Postgres với
//! việc đổi trạng thái, nên không có nửa thao tác.

use axum::http::StatusCode;
use serde::Serialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{
    audit::{self, Actor},
    i18n::Lang,
    notifications::{self, Category, Note},
    orders::{problem, vnd, OrderError, OrderResult},
    roles::{self, PlatformRole},
    store::Db,
    wallet::{self, coin, Kind, Leg, Posting, Tx},
    AppState,
};

/// Bên của dự án.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Side {
    Customer,
    Provider,
}

impl Side {
    pub fn as_str(self) -> &'static str {
        match self {
            Side::Customer => "customer",
            Side::Provider => "provider",
        }
    }

    pub fn parse(s: &str) -> Option<Side> {
        match s {
            "customer" => Some(Side::Customer),
            "provider" => Some(Side::Provider),
            _ => None,
        }
    }

    pub fn other(self) -> Side {
        match self {
            Side::Customer => Side::Provider,
            Side::Provider => Side::Customer,
        }
    }
}

/// Trạng thái dự án (design R7).
pub const STATUSES: [&str; 10] = [
    "requested",
    "negotiating",
    "agreed",
    "in_progress",
    "delivered",
    "completed",
    "disputed",
    "resolved",
    "cancelled",
    "refunded",
];

fn not_found() -> OrderError {
    problem(
        StatusCode::NOT_FOUND,
        "not_found",
        "Không có dự án này.",
        "No such project.",
    )
}

fn bad(code: &'static str, vi: impl Into<String>, en: impl Into<String>) -> OrderError {
    problem(StatusCode::BAD_REQUEST, code, vi, en)
}

fn forbidden(code: &'static str, vi: impl Into<String>, en: impl Into<String>) -> OrderError {
    problem(StatusCode::FORBIDDEN, code, vi, en)
}

fn wrong_state() -> OrderError {
    problem(
        StatusCode::CONFLICT,
        "invalid_state",
        "Dự án không ở bước cho phép thao tác này.",
        "The project isn't at a step that allows this.",
    )
}

fn clip(s: &str, max: usize) -> String {
    s.trim().chars().take(max).collect()
}

// ---------- Hồ sơ nhận việc ----------

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Profile {
    pub subject: Uuid,
    pub email: String,
    /// `individual` (Lập trình viên cá nhân) hoặc `company` (Công ty phát triển phần mềm).
    pub kind: String,
    pub display_name: String,
    pub headline: String,
    pub skills: String,
    pub accepting: bool,
}

const PROFILE_COLUMNS: &str = "subject, email, kind, display_name, headline, skills, accepting";

pub async fn profile(db: &Db, subject: Uuid) -> sqlx::Result<Option<Profile>> {
    sqlx::query_as(&format!(
        "SELECT {PROFILE_COLUMNS} FROM provider_profiles WHERE subject = $1"
    ))
    .bind(subject)
    .fetch_optional(db)
    .await
}

/// Tạo hoặc sửa hồ sơ nhận việc; chỉ Lập trình viên cá nhân và Công ty.
pub async fn save_profile(
    state: &AppState,
    subject: Uuid,
    email: &str,
    display_name: &str,
    headline: &str,
    skills: &str,
    accepting: bool,
) -> OrderResult<()> {
    let role = roles::role_of(&state.db, subject).await?;
    if !role.can_take_projects() {
        return Err(forbidden(
            "role_required",
            "Cần vai trò Lập trình viên cá nhân hoặc Công ty phát triển phần mềm.",
            "You need the Individual developer or Software company role.",
        ));
    }
    let name = clip(display_name, 100);
    if name.chars().count() < 2 {
        return Err(bad(
            "invalid_request",
            "Hãy nhập tên hiển thị.",
            "Enter a display name.",
        ));
    }
    let (headline, skills) = (clip(headline, 300), clip(skills, 300));
    let kind = if role == PlatformRole::DevCompany {
        "company"
    } else {
        "individual"
    };
    let search = crate::catalog::normalize(&format!("{name} {headline} {skills}"));
    sqlx::query(
        "INSERT INTO provider_profiles (subject, email, kind, display_name, headline, skills, accepting, search_text)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
         ON CONFLICT (subject) DO UPDATE SET email = $2, kind = $3, display_name = $4, headline = $5, skills = $6,
             accepting = $7, search_text = $8, updated_at = now()",
    )
    .bind(subject)
    .bind(email.trim().to_ascii_lowercase())
    .bind(kind)
    .bind(&name)
    .bind(&headline)
    .bind(&skills)
    .bind(accepting)
    .bind(&search)
    .execute(&state.db)
    .await?;
    Ok(())
}

/// Hồ sơ đang nhận việc, tìm theo tên, giới thiệu, kỹ năng (không phân biệt dấu). Người không còn
/// quyền nhận việc bị ẩn.
pub async fn providers(db: &Db, q: &str) -> sqlx::Result<Vec<Profile>> {
    let q = crate::catalog::normalize(q.trim());
    sqlx::query_as(&format!(
        "SELECT {PROFILE_COLUMNS} FROM provider_profiles p
         WHERE accepting AND ($1 = '' OR search_text LIKE '%' || $1 || '%')
           AND EXISTS (SELECT 1 FROM role_grants g WHERE g.subject = p.subject AND g.role IN ('developer', 'dev_company'))
         ORDER BY display_name LIMIT 100"
    ))
    .bind(&q)
    .fetch_all(db)
    .await
}

// ---------- Dự án ----------

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Project {
    pub id: Uuid,
    pub code: String,
    pub customer: Uuid,
    pub customer_email: String,
    pub provider: Uuid,
    pub provider_email: String,
    pub title: String,
    pub description: String,
    pub budget_coin: Option<i64>,
    pub wanted_days: Option<i32>,
    pub price_coin: Option<i64>,
    pub days: Option<i32>,
    pub status: String,
    pub escrow_coin: i64,
    pub delivery_note: Option<String>,
    pub delivery_link: Option<String>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

const COLUMNS: &str = "id, code, customer, customer_email, provider, provider_email, title, description,
    budget_coin, wanted_days, price_coin, days, status, escrow_coin, delivery_note, delivery_link, created_at, updated_at";

impl Project {
    pub fn side_of(&self, subject: Uuid) -> Option<Side> {
        if subject == self.customer {
            Some(Side::Customer)
        } else if subject == self.provider {
            Some(Side::Provider)
        } else {
            None
        }
    }

    fn party(&self, side: Side) -> (Uuid, &str) {
        match side {
            Side::Customer => (self.customer, &self.customer_email),
            Side::Provider => (self.provider, &self.provider_email),
        }
    }
}

pub async fn get(db: &Db, id: Uuid) -> OrderResult<Project> {
    sqlx::query_as::<_, Project>(&format!("SELECT {COLUMNS} FROM projects WHERE id = $1"))
        .bind(id)
        .fetch_optional(db)
        .await?
        .ok_or_else(not_found)
}

/// Dự án và bên của người xem; người ngoài dự án nhận "không có".
pub async fn for_party(db: &Db, id: Uuid, subject: Uuid) -> OrderResult<(Project, Side)> {
    let p = get(db, id).await?;
    let side = p.side_of(subject).ok_or_else(not_found)?;
    Ok((p, side))
}

/// Dự án của một người (là khách hoặc người nhận việc), mới cập nhật trước.
pub async fn list_for(db: &Db, subject: Uuid) -> sqlx::Result<Vec<Project>> {
    sqlx::query_as(&format!(
        "SELECT {COLUMNS} FROM projects WHERE customer = $1 OR provider = $1 ORDER BY updated_at DESC LIMIT 200"
    ))
    .bind(subject)
    .fetch_all(db)
    .await
}

/// Mọi dự án cho bảng quản trị: tranh chấp trước, rồi mới cập nhật.
pub async fn list_all(db: &Db, status: Option<&str>) -> sqlx::Result<Vec<Project>> {
    sqlx::query_as(&format!(
        "SELECT {COLUMNS} FROM projects WHERE ($1::text IS NULL OR status = $1)
         ORDER BY (status = 'disputed') DESC, updated_at DESC LIMIT 200"
    ))
    .bind(status)
    .fetch_all(db)
    .await
}

async fn lock(tx: &mut Tx<'_>, id: Uuid) -> OrderResult<Project> {
    sqlx::query_as::<_, Project>(&format!(
        "SELECT {COLUMNS} FROM projects WHERE id = $1 FOR UPDATE"
    ))
    .bind(id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(not_found)
}

async fn lock_party(tx: &mut Tx<'_>, id: Uuid, subject: Uuid) -> OrderResult<(Project, Side)> {
    let p = lock(tx, id).await?;
    let side = p.side_of(subject).ok_or_else(not_found)?;
    Ok((p, side))
}

async fn set_status(
    tx: &mut Tx<'_>,
    p: &Project,
    to: &str,
    actor: &str,
    note: &str,
) -> sqlx::Result<()> {
    sqlx::query("UPDATE projects SET status = $2, updated_at = now() WHERE id = $1")
        .bind(p.id)
        .bind(to)
        .execute(&mut **tx)
        .await?;
    event(tx, p.id, Some(&p.status), to, actor, note).await
}

async fn event(
    tx: &mut Tx<'_>,
    id: Uuid,
    from: Option<&str>,
    to: &str,
    actor: &str,
    note: &str,
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO project_events (project_id, from_status, to_status, actor, note) VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(id)
    .bind(from)
    .bind(to)
    .bind(actor)
    .bind(note)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Báo cho một bên của dự án (loại Đơn hàng, bắt buộc).
async fn tell(
    state: &AppState,
    p: &Project,
    side: Side,
    vi: (String, String),
    en: (String, String),
) {
    let (subject, email) = p.party(side);
    let note = Note::vi(vi.0, vi.1, format!("/projects/{}", p.id)).en(en.0, en.1);
    notifications::notify(state, subject, Some(email), Category::Order, &note).await;
}

/// Khách hàng gửi yêu cầu làm phần mềm cho một người nhận việc.
#[allow(clippy::too_many_arguments)]
pub async fn create(
    state: &AppState,
    customer: Uuid,
    customer_email: &str,
    provider: Uuid,
    title: &str,
    description: &str,
    budget_coin: Option<i64>,
    wanted_days: Option<i32>,
) -> OrderResult<Uuid> {
    if !roles::role_of(&state.db, customer).await?.can_commission() {
        return Err(forbidden(
            "role_required",
            "Cần vai trò Khách hàng cá nhân hoặc Khách hàng doanh nghiệp để gửi yêu cầu.",
            "You need the Individual customer or Business customer role to send a request.",
        ));
    }
    let Some(pro) = profile(&state.db, provider).await?.filter(|p| p.accepting) else {
        return Err(bad(
            "provider_unavailable",
            "Người này hiện không nhận việc.",
            "This provider isn't taking work right now.",
        ));
    };
    if provider == customer
        || !roles::role_of(&state.db, provider)
            .await?
            .can_take_projects()
    {
        return Err(bad(
            "provider_unavailable",
            "Người này hiện không nhận việc.",
            "This provider isn't taking work right now.",
        ));
    }
    let title = clip(title, 200);
    let description = clip(description, 10_000);
    if title.chars().count() < 3 || description.chars().count() < 10 {
        return Err(bad(
            "invalid_request",
            "Hãy nhập tiêu đề (ít nhất 3 ký tự) và mô tả (ít nhất 10 ký tự).",
            "Enter a title (at least 3 characters) and a description (at least 10).",
        ));
    }
    if budget_coin.is_some_and(|b| b <= 0) || wanted_days.is_some_and(|d| d <= 0) {
        return Err(bad(
            "invalid_request",
            "Ngân sách và thời hạn phải lớn hơn 0.",
            "Budget and timeframe must be above 0.",
        ));
    }
    let id = Uuid::new_v4();
    let mut tx = state.db.begin().await?;
    let n: i64 = sqlx::query_scalar("SELECT nextval('project_seq')")
        .fetch_one(&mut *tx)
        .await?;
    let code = format!("DA-{n:06}");
    sqlx::query(
        "INSERT INTO projects (id, code, customer, customer_email, provider, provider_email, title, description, budget_coin, wanted_days)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
    )
    .bind(id)
    .bind(&code)
    .bind(customer)
    .bind(customer_email.trim().to_ascii_lowercase())
    .bind(provider)
    .bind(&pro.email)
    .bind(&title)
    .bind(&description)
    .bind(budget_coin)
    .bind(wanted_days)
    .execute(&mut *tx)
    .await?;
    event(&mut tx, id, None, "requested", customer_email, "").await?;
    tx.commit().await?;
    audit::record(
        &state.db,
        Actor::User(customer),
        "project.create",
        Some(&code),
        None,
        json!({"provider": provider, "budget": budget_coin}),
    )
    .await;
    let p = get(&state.db, id).await?;
    tell(
        state,
        &p,
        Side::Provider,
        (
            format!("Yêu cầu làm phần mềm mới: {title}"),
            format!("Bạn nhận được yêu cầu {code} \"{title}\". Mở dự án để xem và đưa giá."),
        ),
        (
            format!("New software request: {title}"),
            format!("You received request {code} \"{title}\". Open the project to review it and make an offer."),
        ),
    )
    .await;
    Ok(id)
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Offer {
    pub id: Uuid,
    pub side: String,
    pub price_coin: i64,
    pub days: i32,
    pub note: String,
    pub status: String,
    pub created_at: OffsetDateTime,
}

pub async fn offers(db: &Db, project: Uuid) -> sqlx::Result<Vec<Offer>> {
    sqlx::query_as(
        "SELECT id, side, price_coin, days, note, status, created_at FROM project_offers WHERE project_id = $1 ORDER BY created_at",
    )
    .bind(project)
    .fetch_all(db)
    .await
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Message {
    pub side: String,
    pub body: String,
    pub at: OffsetDateTime,
}

pub async fn messages(db: &Db, project: Uuid) -> sqlx::Result<Vec<Message>> {
    sqlx::query_as("SELECT side, body, at FROM project_messages WHERE project_id = $1 ORDER BY id")
        .bind(project)
        .fetch_all(db)
        .await
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Event {
    pub from_status: Option<String>,
    pub to_status: String,
    pub actor: String,
    pub note: String,
    pub at: OffsetDateTime,
}

pub async fn events(db: &Db, project: Uuid) -> sqlx::Result<Vec<Event>> {
    sqlx::query_as(
        "SELECT from_status, to_status, actor, note, at FROM project_events WHERE project_id = $1 ORDER BY id",
    )
    .bind(project)
    .fetch_all(db)
    .await
}

async fn add_message(tx: &mut Tx<'_>, id: Uuid, side: &str, body: &str) -> sqlx::Result<()> {
    sqlx::query("INSERT INTO project_messages (project_id, side, body) VALUES ($1, $2, $3)")
        .bind(id)
        .bind(side)
        .bind(body)
        .execute(&mut **tx)
        .await?;
    sqlx::query("UPDATE projects SET updated_at = now() WHERE id = $1")
        .bind(id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// Tin nhắn trong dự án.
pub async fn message(state: &AppState, id: Uuid, subject: Uuid, body: &str) -> OrderResult<()> {
    let body = clip(body, 5_000);
    if body.is_empty() {
        return Err(bad(
            "invalid_request",
            "Tin nhắn trống.",
            "The message is empty.",
        ));
    }
    let mut tx = state.db.begin().await?;
    let (p, side) = lock_party(&mut tx, id, subject).await?;
    add_message(&mut tx, id, side.as_str(), &body).await?;
    tx.commit().await?;
    tell(
        state,
        &p,
        side.other(),
        (
            format!("Tin nhắn mới ở dự án {}", p.code),
            body.chars().take(300).collect(),
        ),
        (
            format!("New message on project {}", p.code),
            body.chars().take(300).collect(),
        ),
    )
    .await;
    Ok(())
}

/// Đưa đề nghị giá và thời hạn; đề nghị mới thay mọi đề nghị đang mở.
pub async fn offer(
    state: &AppState,
    id: Uuid,
    subject: Uuid,
    price_coin: i64,
    days: i32,
    note: &str,
) -> OrderResult<()> {
    if !(1..=100_000_000).contains(&price_coin) || !(1..=3650).contains(&days) {
        return Err(bad(
            "invalid_request",
            "Giá phải từ 1 F-Coin và thời hạn từ 1 đến 3.650 ngày.",
            "The price must be at least 1 F-Coin and the timeframe 1–3,650 days.",
        ));
    }
    let mut tx = state.db.begin().await?;
    let (p, side) = lock_party(&mut tx, id, subject).await?;
    if !matches!(p.status.as_str(), "requested" | "negotiating") {
        return Err(wrong_state());
    }
    sqlx::query("UPDATE project_offers SET status = 'superseded', decided_at = now() WHERE project_id = $1 AND status = 'open'")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "INSERT INTO project_offers (id, project_id, side, price_coin, days, note) VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(Uuid::new_v4())
    .bind(id)
    .bind(side.as_str())
    .bind(price_coin)
    .bind(days)
    .bind(clip(note, 2_000))
    .execute(&mut *tx)
    .await?;
    let actor = p.party(side).1.to_owned();
    let summary = format!("{} · {days} ngày", coin(price_coin, Lang::Vi));
    if p.status == "requested" {
        set_status(&mut tx, &p, "negotiating", &actor, &summary).await?;
    } else {
        event(
            &mut tx,
            id,
            Some(&p.status),
            "negotiating",
            &actor,
            &summary,
        )
        .await?;
        sqlx::query("UPDATE projects SET updated_at = now() WHERE id = $1")
            .bind(id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    tell(
        state,
        &p,
        side.other(),
        (
            format!("Đề nghị mới cho dự án {}", p.code),
            format!(
                "Bên kia đề nghị {} trong {days} ngày. Mở dự án để chấp nhận hoặc đưa đề nghị khác.",
                coin(price_coin, Lang::Vi)
            ),
        ),
        (
            format!("New offer on project {}", p.code),
            format!(
                "The other side offered {} over {days} days. Open the project to accept or counter.",
                coin(price_coin, Lang::En)
            ),
        ),
    )
    .await;
    Ok(())
}

/// Chấp nhận đề nghị đang mở của bên kia: chốt giá và thời hạn.
pub async fn accept_offer(
    state: &AppState,
    id: Uuid,
    subject: Uuid,
    offer: Uuid,
) -> OrderResult<()> {
    let mut tx = state.db.begin().await?;
    let (p, side) = lock_party(&mut tx, id, subject).await?;
    if p.status != "negotiating" {
        return Err(wrong_state());
    }
    let row: Option<(String, i64, i32, String)> = sqlx::query_as(
        "SELECT side, price_coin, days, status FROM project_offers WHERE id = $1 AND project_id = $2 FOR UPDATE",
    )
    .bind(offer)
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some((offer_side, price, days, status)) = row else {
        return Err(not_found());
    };
    if status != "open" {
        return Err(problem(
            StatusCode::CONFLICT,
            "offer_closed",
            "Đề nghị này không còn mở.",
            "This offer is no longer open.",
        ));
    }
    if offer_side == side.as_str() {
        return Err(forbidden(
            "own_offer",
            "Không thể chấp nhận đề nghị của chính mình.",
            "You can't accept your own offer.",
        ));
    }
    sqlx::query("UPDATE project_offers SET status = 'accepted', decided_at = now() WHERE id = $1")
        .bind(offer)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE projects SET price_coin = $2, days = $3 WHERE id = $1")
        .bind(id)
        .bind(price)
        .bind(days)
        .execute(&mut *tx)
        .await?;
    let actor = p.party(side).1.to_owned();
    set_status(
        &mut tx,
        &p,
        "agreed",
        &actor,
        &format!("{} · {days} ngày", coin(price, Lang::Vi)),
    )
    .await?;
    tx.commit().await?;
    tell(
        state,
        &p,
        side.other(),
        (
            format!("Dự án {} đã chốt giá", p.code),
            format!(
                "Hai bên đã chốt {} trong {days} ngày. Khách hàng ký quỹ để bắt đầu.",
                coin(price, Lang::Vi)
            ),
        ),
        (
            format!("Project {} price agreed", p.code),
            format!(
                "Both sides agreed on {} over {days} days. The customer funds escrow to start.",
                coin(price, Lang::En)
            ),
        ),
    )
    .await;
    Ok(())
}

/// Hủy trước khi ký quỹ (người nhận việc gọi khi chưa đưa giá là "từ chối").
pub async fn cancel(state: &AppState, id: Uuid, subject: Uuid, reason: &str) -> OrderResult<()> {
    let mut tx = state.db.begin().await?;
    let (p, side) = lock_party(&mut tx, id, subject).await?;
    if !matches!(p.status.as_str(), "requested" | "negotiating" | "agreed") {
        return Err(wrong_state());
    }
    sqlx::query("UPDATE project_offers SET status = 'superseded', decided_at = now() WHERE project_id = $1 AND status = 'open'")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "UPDATE project_contracts SET status = 'void' WHERE project_id = $1 AND status = 'draft'",
    )
    .bind(id)
    .execute(&mut *tx)
    .await?;
    sqlx::query("UPDATE projects SET closed_at = now() WHERE id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    let actor = p.party(side).1.to_owned();
    set_status(&mut tx, &p, "cancelled", &actor, &clip(reason, 1_000)).await?;
    tx.commit().await?;
    tell(
        state,
        &p,
        side.other(),
        (
            format!("Dự án {} đã hủy", p.code),
            format!(
                "Bên kia đã hủy dự án \"{}\". {}",
                p.title,
                clip(reason, 500)
            ),
        ),
        (
            format!("Project {} cancelled", p.code),
            format!(
                "The other side cancelled \"{}\". {}",
                p.title,
                clip(reason, 500)
            ),
        ),
    )
    .await;
    Ok(())
}

/// Hợp đồng hiện hành (chưa hủy) của dự án.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Contract {
    pub id: Uuid,
    pub number: String,
    pub creator_side: String,
    pub customer_name: String,
    pub provider_name: String,
    pub body: String,
    pub sha256: String,
    pub status: String,
    pub customer_signed_at: Option<OffsetDateTime>,
    pub provider_signed_at: Option<OffsetDateTime>,
    pub created_at: OffsetDateTime,
}

const CONTRACT_COLUMNS: &str =
    "id, number, creator_side, customer_name, provider_name, body, sha256, status,
    customer_signed_at, provider_signed_at, created_at";

pub async fn contract(db: &Db, project: Uuid) -> sqlx::Result<Option<Contract>> {
    sqlx::query_as(&format!(
        "SELECT {CONTRACT_COLUMNS} FROM project_contracts WHERE project_id = $1 AND status <> 'void'"
    ))
    .bind(project)
    .fetch_optional(db)
    .await
}

/// Khách hàng ký quỹ đúng giá đã chốt (bị chặn khi có hợp đồng chưa ký đủ).
pub async fn fund(state: &AppState, id: Uuid, subject: Uuid) -> OrderResult<()> {
    let mut tx = state.db.begin().await?;
    let (p, side) = lock_party(&mut tx, id, subject).await?;
    if side != Side::Customer {
        return Err(forbidden(
            "customer_only",
            "Chỉ khách hàng ký quỹ.",
            "Only the customer funds escrow.",
        ));
    }
    if p.status != "agreed" {
        return Err(wrong_state());
    }
    let unsigned: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM project_contracts WHERE project_id = $1 AND status = 'draft')",
    )
    .bind(id)
    .fetch_one(&mut *tx)
    .await?;
    if unsigned {
        return Err(problem(
            StatusCode::CONFLICT,
            "contract_unsigned",
            "Hợp đồng của dự án chưa được hai bên ký đủ.",
            "The project contract hasn't been signed by both sides yet.",
        ));
    }
    let price = p.price_coin.unwrap_or(0);
    let customer = wallet::user_account(&mut tx, p.customer, Some(&p.customer_email)).await?;
    let escrow = wallet::system_account(&mut *tx, wallet::system::ESCROW).await?;
    wallet::post(
        &mut tx,
        &Posting {
            kind: Kind::EscrowFund,
            idem: &format!("project:{id}:fund"),
            memo: &format!("Ký quỹ dự án {}", p.code),
            reference: Some(&p.code),
            by: &p.customer_email,
        },
        &[
            Leg {
                account: customer,
                amount: -price,
            },
            Leg {
                account: escrow,
                amount: price,
            },
        ],
    )
    .await?;
    sqlx::query("UPDATE projects SET escrow_coin = $2, funded_at = now() WHERE id = $1")
        .bind(id)
        .bind(price)
        .execute(&mut *tx)
        .await?;
    set_status(
        &mut tx,
        &p,
        "in_progress",
        &p.customer_email,
        &coin(price, Lang::Vi),
    )
    .await?;
    tx.commit().await?;
    tell(
        state,
        &p,
        Side::Provider,
        (
            format!("Dự án {} đã ký quỹ", p.code),
            format!(
                "Khách hàng đã ký quỹ {}. Bạn có thể bắt đầu làm.",
                coin(price, Lang::Vi)
            ),
        ),
        (
            format!("Project {} funded", p.code),
            format!(
                "The customer put {} in escrow. You can start work.",
                coin(price, Lang::En)
            ),
        ),
    )
    .await;
    Ok(())
}

/// Người nhận việc bàn giao.
pub async fn deliver(
    state: &AppState,
    id: Uuid,
    subject: Uuid,
    note: &str,
    link: &str,
) -> OrderResult<()> {
    let note = clip(note, 5_000);
    let link = clip(link, 500);
    if note.is_empty() {
        return Err(bad(
            "invalid_request",
            "Hãy mô tả bản bàn giao.",
            "Describe the delivery.",
        ));
    }
    let mut tx = state.db.begin().await?;
    let (p, side) = lock_party(&mut tx, id, subject).await?;
    if side != Side::Provider {
        return Err(forbidden(
            "provider_only",
            "Chỉ người nhận việc bàn giao.",
            "Only the provider can deliver.",
        ));
    }
    if p.status != "in_progress" {
        return Err(wrong_state());
    }
    sqlx::query("UPDATE projects SET delivery_note = $2, delivery_link = $3, delivered_at = now() WHERE id = $1")
        .bind(id)
        .bind(&note)
        .bind(if link.is_empty() { None } else { Some(&link) })
        .execute(&mut *tx)
        .await?;
    set_status(&mut tx, &p, "delivered", &p.provider_email, "").await?;
    tx.commit().await?;
    tell(
        state,
        &p,
        Side::Customer,
        (
            format!("Dự án {} đã bàn giao", p.code),
            "Người nhận việc đã bàn giao. Hãy kiểm tra rồi chấp nhận hoặc yêu cầu sửa.".to_owned(),
        ),
        (
            format!("Project {} delivered", p.code),
            "The provider has delivered. Review it, then accept or ask for changes.".to_owned(),
        ),
    )
    .await;
    Ok(())
}

/// Chuyển F-Coin từ ký quỹ: `to_provider` cho người nhận việc (trừ phí), phần còn lại hoàn khách.
async fn settle_escrow(
    tx: &mut Tx<'_>,
    p: &Project,
    to_provider: i64,
    by: &str,
) -> OrderResult<i64> {
    let escrow = wallet::system_account(&mut **tx, wallet::system::ESCROW).await?;
    let held = p.escrow_coin;
    let refund = held - to_provider;
    let mut fee = 0;
    if to_provider > 0 {
        let bps = wallet::fees(&mut **tx).await?.project_bps;
        fee = wallet::fee(to_provider, bps);
        let provider = wallet::user_account(tx, p.provider, Some(&p.provider_email)).await?;
        let fees = wallet::system_account(&mut **tx, wallet::system::FEES).await?;
        wallet::post(
            tx,
            &Posting {
                kind: Kind::EscrowRelease,
                idem: &format!("project:{}:release", p.id),
                memo: &format!("Thanh toán dự án {}", p.code),
                reference: Some(&p.code),
                by,
            },
            &[
                Leg {
                    account: escrow,
                    amount: -to_provider,
                },
                Leg {
                    account: provider,
                    amount: to_provider - fee,
                },
                Leg {
                    account: fees,
                    amount: fee,
                },
            ],
        )
        .await?;
    }
    if refund > 0 {
        let customer = wallet::user_account(tx, p.customer, Some(&p.customer_email)).await?;
        wallet::post(
            tx,
            &Posting {
                kind: Kind::EscrowRefund,
                idem: &format!("project:{}:refund", p.id),
                memo: &format!("Hoàn ký quỹ dự án {}", p.code),
                reference: Some(&p.code),
                by,
            },
            &[
                Leg {
                    account: escrow,
                    amount: -refund,
                },
                Leg {
                    account: customer,
                    amount: refund,
                },
            ],
        )
        .await?;
    }
    sqlx::query("UPDATE projects SET escrow_coin = 0, closed_at = now() WHERE id = $1")
        .bind(p.id)
        .execute(&mut **tx)
        .await?;
    Ok(fee)
}

/// Khách chấp nhận bàn giao: giải ngân toàn bộ ký quỹ cho người nhận việc.
pub async fn accept_delivery(state: &AppState, id: Uuid, subject: Uuid) -> OrderResult<()> {
    let mut tx = state.db.begin().await?;
    let (p, side) = lock_party(&mut tx, id, subject).await?;
    if side != Side::Customer {
        return Err(forbidden(
            "customer_only",
            "Chỉ khách hàng chấp nhận bàn giao.",
            "Only the customer can accept the delivery.",
        ));
    }
    if p.status != "delivered" {
        return Err(wrong_state());
    }
    let held = p.escrow_coin;
    let fee = settle_escrow(&mut tx, &p, held, &p.customer_email).await?;
    set_status(&mut tx, &p, "completed", &p.customer_email, "").await?;
    tx.commit().await?;
    audit::record(
        &state.db,
        Actor::User(subject),
        "project.accept",
        Some(&p.code),
        None,
        json!({"released": held, "fee": fee}),
    )
    .await;
    tell(
        state,
        &p,
        Side::Provider,
        (
            format!("Dự án {} hoàn thành", p.code),
            format!(
                "Khách hàng đã chấp nhận bàn giao. Bạn nhận {}.",
                coin(held - fee, Lang::Vi)
            ),
        ),
        (
            format!("Project {} completed", p.code),
            format!(
                "The customer accepted the delivery. You received {}.",
                coin(held - fee, Lang::En)
            ),
        ),
    )
    .await;
    Ok(())
}

/// Khách yêu cầu sửa bản bàn giao: dự án quay lại "Đang làm", F-Coin vẫn trong ký quỹ.
pub async fn request_revision(
    state: &AppState,
    id: Uuid,
    subject: Uuid,
    reason: &str,
) -> OrderResult<()> {
    let reason = clip(reason, 2_000);
    if reason.is_empty() {
        return Err(bad(
            "reason_required",
            "Hãy ghi cần sửa gì.",
            "Say what needs changing.",
        ));
    }
    let mut tx = state.db.begin().await?;
    let (p, side) = lock_party(&mut tx, id, subject).await?;
    if side != Side::Customer {
        return Err(forbidden(
            "customer_only",
            "Chỉ khách hàng yêu cầu sửa.",
            "Only the customer can ask for changes.",
        ));
    }
    if p.status != "delivered" {
        return Err(wrong_state());
    }
    add_message(&mut tx, id, "customer", &reason).await?;
    set_status(&mut tx, &p, "in_progress", &p.customer_email, &reason).await?;
    tx.commit().await?;
    tell(
        state,
        &p,
        Side::Provider,
        (
            format!("Dự án {}: khách yêu cầu sửa", p.code),
            reason.clone(),
        ),
        (format!("Project {}: changes requested", p.code), reason),
    )
    .await;
    Ok(())
}

/// Người nhận việc rút lui khi đang làm: hoàn toàn bộ ký quỹ cho khách.
pub async fn withdraw(state: &AppState, id: Uuid, subject: Uuid, reason: &str) -> OrderResult<()> {
    let mut tx = state.db.begin().await?;
    let (p, side) = lock_party(&mut tx, id, subject).await?;
    if side != Side::Provider {
        return Err(forbidden(
            "provider_only",
            "Chỉ người nhận việc rút lui được.",
            "Only the provider can withdraw.",
        ));
    }
    if p.status != "in_progress" {
        return Err(wrong_state());
    }
    settle_escrow(&mut tx, &p, 0, &p.provider_email).await?;
    set_status(
        &mut tx,
        &p,
        "refunded",
        &p.provider_email,
        &clip(reason, 1_000),
    )
    .await?;
    tx.commit().await?;
    tell(
        state,
        &p,
        Side::Customer,
        (
            format!("Dự án {}: người nhận việc rút lui", p.code),
            format!(
                "{} trong ký quỹ đã về lại ví của bạn. {}",
                coin(p.escrow_coin, Lang::Vi),
                clip(reason, 500)
            ),
        ),
        (
            format!("Project {}: provider withdrew", p.code),
            format!(
                "{} from escrow is back in your wallet. {}",
                coin(p.escrow_coin, Lang::En),
                clip(reason, 500)
            ),
        ),
    )
    .await;
    Ok(())
}

/// Mở tranh chấp khi đang làm hoặc đã bàn giao.
pub async fn dispute(state: &AppState, id: Uuid, subject: Uuid, reason: &str) -> OrderResult<()> {
    let reason = clip(reason, 2_000);
    if reason.is_empty() {
        return Err(bad("reason_required", "Hãy ghi lý do.", "Give a reason."));
    }
    let mut tx = state.db.begin().await?;
    let (p, side) = lock_party(&mut tx, id, subject).await?;
    if !matches!(p.status.as_str(), "in_progress" | "delivered") {
        return Err(wrong_state());
    }
    sqlx::query("UPDATE projects SET status_before_dispute = status WHERE id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    let actor = p.party(side).1.to_owned();
    add_message(&mut tx, id, side.as_str(), &reason).await?;
    set_status(&mut tx, &p, "disputed", &actor, &reason).await?;
    tx.commit().await?;
    crate::review::admin_alert(
        state,
        "project_dispute",
        &format!("Dự án {} có tranh chấp: {reason}", p.code),
        json!({"project": id, "escrow": p.escrow_coin}),
    )
    .await;
    tell(
        state,
        &p,
        side.other(),
        (
            format!("Dự án {} có tranh chấp", p.code),
            format!("Bên kia đã mở tranh chấp: {reason}. Quản lý dự án của F-Eco sẽ liên hệ."),
        ),
        (
            format!("Dispute on project {}", p.code),
            format!("The other side opened a dispute: {reason}. An F-Eco project manager will be in touch."),
        ),
    )
    .await;
    Ok(())
}

/// Quản lý dự án xử lý tranh chấp: `to_provider` F-Coin cho người nhận việc, phần còn lại hoàn khách.
pub async fn resolve(
    state: &AppState,
    id: Uuid,
    admin: &str,
    to_provider: i64,
    note: &str,
) -> OrderResult<()> {
    let note = clip(note, 2_000);
    if note.is_empty() {
        return Err(bad("reason_required", "Hãy ghi ghi chú.", "Add a note."));
    }
    let mut tx = state.db.begin().await?;
    let p = lock(&mut tx, id).await?;
    if p.status != "disputed" {
        return Err(wrong_state());
    }
    if !(0..=p.escrow_coin).contains(&to_provider) {
        return Err(bad(
            "invalid_split",
            format!(
                "Phần của người nhận việc phải từ 0 đến {}.",
                coin(p.escrow_coin, Lang::Vi)
            ),
            format!(
                "The provider's share must be between 0 and {}.",
                coin(p.escrow_coin, Lang::En)
            ),
        ));
    }
    let refund = p.escrow_coin - to_provider;
    settle_escrow(&mut tx, &p, to_provider, admin).await?;
    add_message(&mut tx, id, "admin", &note).await?;
    set_status(
        &mut tx,
        &p,
        "resolved",
        admin,
        &format!(
            "{} → người nhận việc, {} → khách hàng. {note}",
            coin(to_provider, Lang::Vi),
            coin(refund, Lang::Vi)
        ),
    )
    .await?;
    tx.commit().await?;
    audit::record(
        &state.db,
        Actor::Admin(admin.to_owned()),
        "project.resolve",
        Some(&p.code),
        None,
        json!({"to_provider": to_provider, "refund": refund, "note": note}),
    )
    .await;
    for side in [Side::Customer, Side::Provider] {
        tell(
            state,
            &p,
            side,
            (
                format!("Tranh chấp dự án {} đã được xử lý", p.code),
                format!(
                    "Người nhận việc nhận {}, khách hàng được hoàn {}. Ghi chú: {note}",
                    coin(to_provider, Lang::Vi),
                    coin(refund, Lang::Vi)
                ),
            ),
            (
                format!("Dispute on project {} resolved", p.code),
                format!(
                    "The provider receives {}, the customer is refunded {}. Note: {note}",
                    coin(to_provider, Lang::En),
                    coin(refund, Lang::En)
                ),
            ),
        )
        .await;
    }
    Ok(())
}

// ---------- Hợp đồng ----------

pub fn sha256_hex(s: &str) -> String {
    Sha256::digest(s.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Nội dung hợp đồng từ mẫu (design R8).
#[allow(clippy::too_many_arguments)]
pub fn contract_body(
    number: &str,
    p: &Project,
    customer_name: &str,
    provider_name: &str,
    vnd_per_coin: i64,
    extra: &str,
    date: &str,
) -> String {
    let price = p.price_coin.unwrap_or(0);
    let days = p.days.unwrap_or(0);
    let mut s = format!(
        "HỢP ĐỒNG PHÁT TRIỂN PHẦN MỀM\nSố: {number}\nNgày lập: {date}\nDự án: {code} – {title}\n\n\
         BÊN A (Bên đặt hàng): {customer_name}\nBÊN B (Bên thực hiện): {provider_name}\n\n\
         Điều 1. Phạm vi công việc\nBên B phát triển phần mềm theo mô tả của dự án {code} trên F-Eco:\n{desc}\n\n\
         Điều 2. Giá và thanh toán\nGiá trọn gói: {price_coin} ({price_vnd} theo tỷ giá {rate} cho 1 F-Coin tại ngày lập). \
         Bên A ký quỹ toàn bộ giá bằng F-Coin trên F-Eco trước khi Bên B bắt đầu; F-Eco giải ngân cho Bên B khi Bên A chấp nhận bàn giao.\n\n\
         Điều 3. Thời hạn\nBên B bàn giao trong {days} ngày kể từ ngày Bên A ký quỹ.\n\n\
         Điều 4. Bàn giao và nghiệm thu\nBên B bàn giao qua F-Eco kèm mô tả và đường dẫn hoặc mã app. Bên A chấp nhận hoặc yêu cầu sửa kèm lý do. \
         Chấp nhận bàn giao trên F-Eco là nghiệm thu.\n\n\
         Điều 5. Quyền sở hữu mã nguồn\nSau khi được thanh toán đủ, quyền sở hữu mã nguồn và sản phẩm bàn giao thuộc Bên A, trừ thư viện bên thứ ba theo giấy phép của chúng.\n\n\
         Điều 6. Tranh chấp\nHai bên thương lượng trước. Không thống nhất được thì một bên mở tranh chấp trên F-Eco; quyết định chia ký quỹ của F-Eco không thay thế quyền khởi kiện theo pháp luật Việt Nam.\n",
        code = p.code,
        title = p.title,
        desc = p.description,
        price_coin = coin(price, Lang::Vi),
        price_vnd = vnd(price.saturating_mul(vnd_per_coin)),
        rate = vnd(vnd_per_coin),
    );
    let extra = extra.trim();
    if !extra.is_empty() {
        s.push_str(&format!("\nĐiều 7. Điều khoản bổ sung\n{extra}\n"));
    }
    s.push_str("\nHợp đồng được ký điện tử trên F-Eco: mỗi bên đánh dấu đồng ý và nhập họ tên đại diện. F-Eco lưu nội dung, mã băm SHA-256, người ký và thời điểm ký.\n");
    s
}

/// Lập hợp đồng (bên có quyền ký kết hợp đồng, dự án đã chốt giá). Bản nháp cũ bị hủy.
pub async fn create_contract(
    state: &AppState,
    id: Uuid,
    subject: Uuid,
    customer_name: &str,
    provider_name: &str,
    extra: &str,
) -> OrderResult<()> {
    let role = roles::role_of(&state.db, subject).await?;
    if !role.can_sign_contracts() {
        return Err(forbidden(
            "contract_role_required",
            "Cần vai trò Khách hàng doanh nghiệp hoặc Công ty phát triển phần mềm để lập hợp đồng.",
            "You need the Business customer or Software company role to draw up a contract.",
        ));
    }
    let (customer_name, provider_name) = (clip(customer_name, 200), clip(provider_name, 200));
    if customer_name.chars().count() < 2 || provider_name.chars().count() < 2 {
        return Err(bad(
            "invalid_request",
            "Hãy nhập tên đại diện của hai bên.",
            "Enter both parties' representative names.",
        ));
    }
    let mut tx = state.db.begin().await?;
    let (p, side) = lock_party(&mut tx, id, subject).await?;
    if p.status != "agreed" {
        return Err(wrong_state());
    }
    let signed: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM project_contracts WHERE project_id = $1 AND status = 'signed')",
    )
    .bind(id)
    .fetch_one(&mut *tx)
    .await?;
    if signed {
        return Err(problem(
            StatusCode::CONFLICT,
            "contract_signed",
            "Hợp đồng đã được hai bên ký, không sửa được.",
            "The contract is signed by both sides and can't be changed.",
        ));
    }
    sqlx::query(
        "UPDATE project_contracts SET status = 'void' WHERE project_id = $1 AND status = 'draft'",
    )
    .bind(id)
    .execute(&mut *tx)
    .await?;
    let n: i64 = sqlx::query_scalar("SELECT nextval('contract_seq')")
        .fetch_one(&mut *tx)
        .await?;
    let number = format!("HD-{n:06}");
    let rate = wallet::rate(&mut *tx).await?;
    let date = OffsetDateTime::now_utc()
        .to_offset(time::UtcOffset::from_hms(7, 0, 0).expect("UTC+7"))
        .format(&time::macros::format_description!("[day]/[month]/[year]"))
        .unwrap_or_default();
    let body = contract_body(
        &number,
        &p,
        &customer_name,
        &provider_name,
        rate,
        &clip(extra, 5_000),
        &date,
    );
    sqlx::query(
        "INSERT INTO project_contracts (id, number, project_id, created_by, creator_side, customer_name, provider_name, body, sha256)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
    )
    .bind(Uuid::new_v4())
    .bind(&number)
    .bind(id)
    .bind(subject)
    .bind(side.as_str())
    .bind(&customer_name)
    .bind(&provider_name)
    .bind(&body)
    .bind(sha256_hex(&body))
    .execute(&mut *tx)
    .await?;
    let actor = p.party(side).1.to_owned();
    event(
        &mut tx,
        id,
        Some(&p.status),
        &p.status,
        &actor,
        &format!("Lập hợp đồng {number}"),
    )
    .await?;
    tx.commit().await?;
    tell(
        state,
        &p,
        side.other(),
        (
            format!("Hợp đồng {number} cho dự án {}", p.code),
            "Bên kia đã lập hợp đồng. Hãy đọc và ký trên trang dự án.".to_owned(),
        ),
        (
            format!("Contract {number} for project {}", p.code),
            "The other side drew up a contract. Read and sign it on the project page.".to_owned(),
        ),
    )
    .await;
    Ok(())
}

fn same_name(a: &str, b: &str) -> bool {
    let norm = |s: &str| {
        s.split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase()
    };
    norm(a) == norm(b)
}

/// Một bên ký hợp đồng: đánh dấu đồng ý và nhập đúng tên đại diện của bên mình.
pub async fn sign_contract(
    state: &AppState,
    id: Uuid,
    subject: Uuid,
    typed_name: &str,
    agree: bool,
) -> OrderResult<()> {
    let mut tx = state.db.begin().await?;
    let (p, side) = lock_party(&mut tx, id, subject).await?;
    let c: Option<Contract> = sqlx::query_as(&format!(
        "SELECT {CONTRACT_COLUMNS} FROM project_contracts WHERE project_id = $1 AND status = 'draft' FOR UPDATE"
    ))
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(c) = c else {
        return Err(problem(
            StatusCode::CONFLICT,
            "no_contract",
            "Không có hợp đồng chờ ký.",
            "There's no contract awaiting signature.",
        ));
    };
    let (expected, already) = match side {
        Side::Customer => (&c.customer_name, c.customer_signed_at.is_some()),
        Side::Provider => (&c.provider_name, c.provider_signed_at.is_some()),
    };
    if already {
        return Err(problem(
            StatusCode::CONFLICT,
            "already_signed",
            "Bạn đã ký hợp đồng này.",
            "You've already signed this contract.",
        ));
    }
    if !agree || !same_name(typed_name, expected) {
        return Err(bad(
            "signature_invalid",
            format!("Hãy đánh dấu đồng ý và nhập đúng tên \"{expected}\"."),
            format!("Tick the agreement box and type the name \"{expected}\" exactly."),
        ));
    }
    let column = match side {
        Side::Customer => "customer_signed_at",
        Side::Provider => "provider_signed_at",
    };
    sqlx::query(&format!(
        "UPDATE project_contracts SET {column} = now(),
             status = CASE WHEN customer_signed_at IS NOT NULL OR provider_signed_at IS NOT NULL THEN 'signed' ELSE status END
         WHERE id = $1"
    ))
    .bind(c.id)
    .execute(&mut *tx)
    .await?;
    let actor = p.party(side).1.to_owned();
    event(
        &mut tx,
        id,
        Some(&p.status),
        &p.status,
        &actor,
        &format!("Ký hợp đồng {} ({})", c.number, &c.sha256[..12]),
    )
    .await?;
    tx.commit().await?;
    audit::record(
        &state.db,
        Actor::User(subject),
        "project.contract_sign",
        Some(&c.number),
        None,
        json!({"project": p.code, "side": side.as_str(), "sha256": c.sha256}),
    )
    .await;
    tell(
        state,
        &p,
        side.other(),
        (
            format!("Hợp đồng {} đã có chữ ký", c.number),
            "Bên kia đã ký hợp đồng của dự án.".to_owned(),
        ),
        (
            format!("Contract {} signed", c.number),
            "The other side signed the project contract.".to_owned(),
        ),
    )
    .await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Project {
        Project {
            id: Uuid::nil(),
            code: "DA-000001".into(),
            customer: Uuid::from_u128(1),
            customer_email: "kh@example.com".into(),
            provider: Uuid::from_u128(2),
            provider_email: "lan@example.com".into(),
            title: "App bán hàng".into(),
            description: "Ứng dụng quản lý bán hàng".into(),
            budget_coin: Some(500),
            wanted_days: None,
            price_coin: Some(650),
            days: Some(20),
            status: "agreed".into(),
            escrow_coin: 0,
            delivery_note: None,
            delivery_link: None,
            created_at: OffsetDateTime::UNIX_EPOCH,
            updated_at: OffsetDateTime::UNIX_EPOCH,
        }
    }

    #[test]
    fn sides() {
        let p = sample();
        assert_eq!(p.side_of(Uuid::from_u128(1)), Some(Side::Customer));
        assert_eq!(p.side_of(Uuid::from_u128(2)), Some(Side::Provider));
        assert_eq!(p.side_of(Uuid::from_u128(3)), None);
        assert_eq!(Side::Customer.other(), Side::Provider);
        for s in [Side::Customer, Side::Provider] {
            assert_eq!(Side::parse(s.as_str()), Some(s));
        }
    }

    #[test]
    fn contract_template_fills_parties_and_price() {
        let p = sample();
        let body = contract_body(
            "HD-000001",
            &p,
            "Công ty TNHH Minh An",
            "Trần Thị Lan",
            1000,
            "Bảo hành 3 tháng.",
            "15/11/2026",
        );
        assert!(body.contains("Số: HD-000001"));
        assert!(body.contains("BÊN A (Bên đặt hàng): Công ty TNHH Minh An"));
        assert!(body.contains("650 F-Coin (650.000 ₫"));
        assert!(body.contains("trong 20 ngày"));
        assert!(body.contains("Điều 7. Điều khoản bổ sung\nBảo hành 3 tháng."));
        let without = contract_body("HD-000001", &p, "A B", "C D", 1000, "", "15/11/2026");
        assert!(!without.contains("Điều 7"));
        assert_eq!(sha256_hex("abc").len(), 64);
        assert_eq!(
            sha256_hex("abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn signature_name_ignores_case_and_spacing() {
        assert!(same_name("  trần   thị LAN ", "Trần Thị Lan"));
        assert!(!same_name("Tran Thi Lan", "Trần Thị Lan"));
    }
}
