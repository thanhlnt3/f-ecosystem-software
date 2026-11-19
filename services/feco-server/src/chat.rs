//! Chat giữa khách hàng và nhà phát triển (thay đổi OpenSpec chat-and-source-scan, design C1–C3,
//! C6): hội thoại trực tiếp và hội thoại dự án, tin nhắn, đã đọc, chặn, báo cáo, giới hạn, tệp
//! đính kèm được quét mã độc, báo giữa các máy chủ qua `LISTEN/NOTIFY`, thông báo gộp 15 phút.
//!
//! Trang web ở `chat_web.rs`, API cho F-Launcher ở `chat_api.rs`.

use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

use axum::http::StatusCode;
use serde_json::json;
use sqlx::PgConnection;
use time::OffsetDateTime;
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::{
    notifications::{self, Category, Note},
    roles,
    scan::{self, Engine},
    store::Db,
    AppState,
};

/// Độ dài tối đa của một tin (ký tự, sau khi bỏ khoảng trắng hai đầu).
pub const MAX_BODY: usize = 5_000;
/// Tệp đính kèm tối đa 20 MB.
pub const MAX_ATTACHMENT: usize = 20 * 1024 * 1024;
/// Hội thoại trực tiếp mới tối đa mỗi người mỗi 24 giờ.
pub const NEW_PER_DAY: i64 = 20;
/// Tin tối đa mỗi người mỗi phút.
pub const PER_MINUTE: i64 = 30;
/// Mỗi hội thoại báo "có tin nhắn mới" tối đa một lần mỗi 15 phút cho mỗi người nhận.
pub const NOTIFY_EVERY_MINUTES: i64 = 15;
/// Kênh `LISTEN/NOTIFY` báo có tin mới.
pub const CHANNEL: &str = "feco_chat";
/// Hội thoại dự án đã hủy hoặc đã hoàn tiền còn nhắn được 30 ngày.
const CLOSED_PROJECT_DAYS: i64 = 30;
/// Engine lỗi thì quét lại tối đa chừng này lần.
const MAX_SCAN_ATTEMPTS: i32 = 5;

// ---------- Lỗi ----------

#[derive(Debug)]
pub enum ChatError {
    NotFound,
    Forbidden,
    Blocked,
    TooLong,
    Empty,
    RateLimited,
    AttachmentTooLarge,
    AttachmentPending,
    AttachmentBlocked,
    NotDeveloper,
    SelfChat,
    EmailUnverified,
    Db(sqlx::Error),
    Io(std::io::Error),
}

impl From<sqlx::Error> for ChatError {
    fn from(e: sqlx::Error) -> Self {
        ChatError::Db(e)
    }
}

impl From<std::io::Error> for ChatError {
    fn from(e: std::io::Error) -> Self {
        ChatError::Io(e)
    }
}

impl ChatError {
    /// Mã lỗi API (design C2, C4), có trong `errors::CODES`.
    pub fn code(&self) -> &'static str {
        match self {
            ChatError::NotFound => "chat_not_found",
            ChatError::Forbidden => "chat_forbidden",
            ChatError::Blocked => "chat_blocked",
            ChatError::TooLong => "chat_too_long",
            ChatError::Empty => "chat_empty",
            ChatError::RateLimited => "chat_rate_limited",
            ChatError::AttachmentTooLarge => "chat_attachment_too_large",
            ChatError::AttachmentPending => "chat_attachment_pending",
            ChatError::AttachmentBlocked => "chat_attachment_blocked",
            ChatError::NotDeveloper => "chat_not_developer",
            ChatError::SelfChat => "chat_self",
            ChatError::EmailUnverified => "email_unverified",
            ChatError::Db(_) | ChatError::Io(_) => "unavailable",
        }
    }

    pub fn status(&self) -> StatusCode {
        match self {
            ChatError::NotFound => StatusCode::NOT_FOUND,
            ChatError::Forbidden | ChatError::AttachmentBlocked | ChatError::EmailUnverified => {
                StatusCode::FORBIDDEN
            }
            ChatError::Blocked | ChatError::AttachmentPending => StatusCode::CONFLICT,
            ChatError::TooLong
            | ChatError::Empty
            | ChatError::NotDeveloper
            | ChatError::SelfChat => StatusCode::UNPROCESSABLE_ENTITY,
            ChatError::RateLimited => StatusCode::TOO_MANY_REQUESTS,
            ChatError::AttachmentTooLarge => StatusCode::PAYLOAD_TOO_LARGE,
            ChatError::Db(_) | ChatError::Io(_) => StatusCode::SERVICE_UNAVAILABLE,
        }
    }

    /// Khóa i18n của thông báo lỗi trên web.
    pub fn key(&self) -> &'static str {
        match self {
            ChatError::NotFound => "chat.err.not_found",
            ChatError::Forbidden => "chat.err.forbidden",
            ChatError::Blocked => "chat.err.blocked",
            ChatError::TooLong => "chat.err.too_long",
            ChatError::Empty => "chat.err.empty",
            ChatError::RateLimited => "chat.err.rate_limited",
            ChatError::AttachmentTooLarge => "chat.err.too_large",
            ChatError::AttachmentPending => "chat.err.pending",
            ChatError::AttachmentBlocked => "chat.err.att_blocked",
            ChatError::NotDeveloper => "chat.err.not_developer",
            ChatError::SelfChat => "chat.err.self",
            ChatError::EmailUnverified => "chat.err.unverified",
            ChatError::Db(_) | ChatError::Io(_) => "error.unavailable",
        }
    }

    fn log(&self) {
        match self {
            ChatError::Db(e) => tracing::error!(error = %e, "lỗi cơ sở dữ liệu của chat"),
            ChatError::Io(e) => tracing::error!(error = %e, "lỗi lưu tệp chat"),
            _ => {}
        }
    }

    /// Phản hồi JSON `{"error": "<mã>", ...}` cho `/v1/chat`.
    pub fn api(&self) -> axum::response::Response {
        self.log();
        let (en, vi) = (
            crate::i18n::Lang::En.t(self.key()),
            crate::i18n::Lang::Vi.t(self.key()),
        );
        crate::api::api_error(self.status(), self.code(), en, vi)
    }

    /// Thông báo lỗi trên web theo ngôn ngữ.
    pub fn text(&self, lang: crate::i18n::Lang) -> &'static str {
        self.log();
        lang.t(self.key())
    }
}

pub type ChatResult<T> = Result<T, ChatError>;

// ---------- Thời gian thực ----------

/// Sự kiện tin mới trong tiến trình này và những người đang mở hội thoại (để không báo).
pub struct Hub {
    pub events: broadcast::Sender<Uuid>,
    viewers: Mutex<HashMap<(Uuid, Uuid), usize>>,
    listening: AtomicBool,
}

impl Default for Hub {
    fn default() -> Self {
        Hub {
            events: broadcast::channel(1024).0,
            viewers: Mutex::new(HashMap::new()),
            listening: AtomicBool::new(false),
        }
    }
}

impl Hub {
    /// Người dùng đang mở hội thoại trên web (có kết nối SSE đang xem hội thoại đó). Chỉ biết
    /// kết nối tới máy chủ này.
    pub fn is_viewing(&self, conversation: Uuid, subject: Uuid) -> bool {
        self.viewers
            .lock()
            .map(|v| v.get(&(conversation, subject)).copied().unwrap_or(0) > 0)
            .unwrap_or(false)
    }

    fn add_viewer(&self, key: (Uuid, Uuid)) {
        if let Ok(mut v) = self.viewers.lock() {
            *v.entry(key).or_default() += 1;
        }
    }

    fn remove_viewer(&self, key: (Uuid, Uuid)) {
        if let Ok(mut v) = self.viewers.lock() {
            if let Some(n) = v.get_mut(&key) {
                *n = n.saturating_sub(1);
                if *n == 0 {
                    v.remove(&key);
                }
            }
        }
    }
}

/// Đánh dấu "đang xem" trong lúc kết nối SSE còn mở.
pub struct ViewGuard {
    state: Arc<AppState>,
    key: (Uuid, Uuid),
}

impl ViewGuard {
    pub fn new(state: Arc<AppState>, conversation: Uuid, subject: Uuid) -> Self {
        let key = (conversation, subject);
        state.chat.add_viewer(key);
        ViewGuard { state, key }
    }
}

impl Drop for ViewGuard {
    fn drop(&mut self) {
        self.state.chat.remove_viewer(self.key);
    }
}

/// Tác vụ nghe kênh `feco_chat` và phát lại qua `AppState.chat.events`; gọi nhiều lần chỉ chạy
/// một tác vụ. Mất kết nối thì nối lại sau 2 giây.
pub fn start_listener(state: &Arc<AppState>) {
    if state.chat.listening.swap(true, Ordering::SeqCst) {
        return;
    }
    let s = state.clone();
    tokio::spawn(async move {
        loop {
            if let Err(e) = listen(&s).await {
                tracing::warn!(error = %e, "mất kết nối LISTEN feco_chat, nối lại sau 2 giây");
            }
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    });
}

async fn listen(state: &AppState) -> sqlx::Result<()> {
    let mut l = sqlx::postgres::PgListener::connect_with(&state.db).await?;
    l.listen(CHANNEL).await?;
    loop {
        let n = l.recv().await?;
        if let Ok(id) = n.payload().parse::<Uuid>() {
            let _ = state.chat.events.send(id);
        }
    }
}

// ---------- Hội thoại ----------

/// Phía của người tham gia hội thoại.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Customer,
    Developer,
}

impl Role {
    pub fn as_str(self) -> &'static str {
        match self {
            Role::Customer => "customer",
            Role::Developer => "developer",
        }
    }
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Conversation {
    pub id: Uuid,
    pub kind: String,
    pub customer: Uuid,
    pub developer: Uuid,
    pub project_id: Option<Uuid>,
    pub app_id: Option<String>,
    pub customer_name: String,
    pub developer_name: String,
    pub blocked_at: Option<OffsetDateTime>,
    pub created_at: OffsetDateTime,
    pub last_message_at: OffsetDateTime,
    pub customer_read: i64,
    pub developer_read: i64,
}

const COLUMNS: &str = "c.id, c.kind, c.customer, c.developer, c.project_id, c.app_id, c.customer_name,
    c.developer_name, c.blocked_at, c.created_at, c.last_message_at, c.customer_read, c.developer_read";

impl Conversation {
    pub fn role_of(&self, subject: Uuid) -> Option<Role> {
        if subject == self.customer {
            Some(Role::Customer)
        } else if subject == self.developer {
            Some(Role::Developer)
        } else {
            None
        }
    }

    pub fn peer_name(&self, role: Role) -> &str {
        match role {
            Role::Customer => &self.developer_name,
            Role::Developer => &self.customer_name,
        }
    }

    pub fn read_of(&self, role: Role) -> i64 {
        match role {
            Role::Customer => self.customer_read,
            Role::Developer => self.developer_read,
        }
    }

    pub fn is_direct(&self) -> bool {
        self.kind == "direct"
    }
}

pub async fn get(db: &Db, id: Uuid) -> ChatResult<Conversation> {
    sqlx::query_as(&format!(
        "SELECT {COLUMNS} FROM conversations c WHERE c.id = $1"
    ))
    .bind(id)
    .fetch_optional(db)
    .await?
    .ok_or(ChatError::NotFound)
}

/// Hội thoại và phía của người xem; người ngoài nhận `chat_forbidden`.
pub async fn for_participant(db: &Db, id: Uuid, subject: Uuid) -> ChatResult<(Conversation, Role)> {
    let c = get(db, id).await?;
    let role = c.role_of(subject).ok_or(ChatError::Forbidden)?;
    Ok((c, role))
}

pub async fn of_project(db: &Db, project: Uuid) -> sqlx::Result<Option<Conversation>> {
    sqlx::query_as(&format!(
        "SELECT {COLUMNS} FROM conversations c WHERE c.project_id = $1"
    ))
    .bind(project)
    .fetch_optional(db)
    .await
}

/// Một dòng của danh sách hội thoại.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Summary {
    #[sqlx(flatten)]
    pub conversation: Conversation,
    /// Tên dự án hoặc tên app (nếu có).
    pub context_title: Option<String>,
    pub last_body: Option<String>,
    pub unread: i64,
}

impl Summary {
    /// Tiêu đề: tên dự án, tên app, hoặc tên bên kia.
    pub fn title(&self, role: Role) -> String {
        self.context_title
            .clone()
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| self.conversation.peer_name(role).to_owned())
    }

    /// 120 ký tự đầu của tin cuối.
    pub fn preview(&self) -> Option<String> {
        self.last_body
            .as_ref()
            .map(|b| b.chars().take(120).collect())
    }
}

/// Hội thoại của một người, mới nhất trước, tối đa 100; `kind` lọc `direct` hoặc `project`.
pub async fn list(db: &Db, subject: Uuid, kind: Option<&str>) -> sqlx::Result<Vec<Summary>> {
    sqlx::query_as(&format!(
        "SELECT {COLUMNS}, COALESCE(p.title, a.name) AS context_title,
            (SELECT m.body FROM chat_messages m WHERE m.conversation_id = c.id ORDER BY m.id DESC LIMIT 1) AS last_body,
            (SELECT count(*) FROM chat_messages m WHERE m.conversation_id = c.id
               AND m.id > (CASE WHEN c.customer = $1 THEN c.customer_read ELSE c.developer_read END)
               AND m.side <> (CASE WHEN c.customer = $1 THEN 'customer' ELSE 'developer' END)) AS unread
         FROM conversations c
         LEFT JOIN projects p ON p.id = c.project_id
         LEFT JOIN catalog_apps a ON a.id = c.app_id
         WHERE (c.customer = $1 OR c.developer = $1) AND ($2::text IS NULL OR c.kind = $2)
         ORDER BY c.last_message_at DESC LIMIT 100"
    ))
    .bind(subject)
    .bind(kind)
    .fetch_all(db)
    .await
}

/// Một hội thoại của người tham gia, kèm tiêu đề và số chưa đọc.
pub async fn summary(db: &Db, id: Uuid, subject: Uuid) -> sqlx::Result<Option<Summary>> {
    sqlx::query_as(&format!(
        "SELECT {COLUMNS}, COALESCE(p.title, a.name) AS context_title,
            (SELECT m.body FROM chat_messages m WHERE m.conversation_id = c.id ORDER BY m.id DESC LIMIT 1) AS last_body,
            (SELECT count(*) FROM chat_messages m WHERE m.conversation_id = c.id
               AND m.id > (CASE WHEN c.customer = $2 THEN c.customer_read ELSE c.developer_read END)
               AND m.side <> (CASE WHEN c.customer = $2 THEN 'customer' ELSE 'developer' END)) AS unread
         FROM conversations c
         LEFT JOIN projects p ON p.id = c.project_id
         LEFT JOIN catalog_apps a ON a.id = c.app_id
         WHERE c.id = $1 AND (c.customer = $2 OR c.developer = $2)"
    ))
    .bind(id)
    .bind(subject)
    .fetch_optional(db)
    .await
}

/// Tổng số tin chưa đọc của một người (tin của bên kia, quản trị và hệ thống).
pub async fn unread_total(db: &Db, subject: Uuid) -> sqlx::Result<i64> {
    sqlx::query_scalar(
        "SELECT count(*) FROM conversations c JOIN chat_messages m ON m.conversation_id = c.id
         WHERE (c.customer = $1 OR c.developer = $1)
           AND m.id > (CASE WHEN c.customer = $1 THEN c.customer_read ELSE c.developer_read END)
           AND m.side <> (CASE WHEN c.customer = $1 THEN 'customer' ELSE 'developer' END)",
    )
    .bind(subject)
    .fetch_one(db)
    .await
}

// ---------- Tên hiển thị ----------

fn identity_name(identity: &serde_json::Value) -> Option<String> {
    let t = &identity["traits"];
    t["name"]
        .as_str()
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .map(str::to_owned)
        .or_else(|| {
            t["email"]
                .as_str()
                .and_then(|e| e.split('@').next())
                .filter(|n| !n.is_empty())
                .map(str::to_owned)
        })
}

fn email_verified(identity: &serde_json::Value) -> bool {
    identity["verifiable_addresses"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|a| a["verified"].as_bool() == Some(true))
}

/// Tên hiển thị của nhà phát triển: hồ sơ nhận việc, tài khoản nhà phát triển, rồi tên trong hồ
/// sơ F-Eco. Không dùng email đầy đủ.
pub async fn developer_name(state: &AppState, subject: Uuid) -> String {
    let profile: Option<String> = sqlx::query_scalar(
        "SELECT display_name FROM provider_profiles WHERE subject = $1 AND display_name <> ''",
    )
    .bind(subject)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();
    if let Some(n) = profile {
        return n;
    }
    let account: Option<String> = sqlx::query_scalar(
        "SELECT a.display_name FROM developer_members m JOIN developer_accounts a ON a.id = m.account_id
         WHERE m.subject = $1 AND m.status = 'active' AND a.display_name <> '' LIMIT 1",
    )
    .bind(subject)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();
    if let Some(n) = account {
        return n;
    }
    person_name(state, subject).await
}

/// Tên trong hồ sơ F-Eco (hoặc phần trước @ của email).
pub async fn person_name(state: &AppState, subject: Uuid) -> String {
    match state.kratos.get_identity(&subject.to_string()).await {
        Ok(Some(i)) => identity_name(&i).unwrap_or_else(|| "F-Eco".into()),
        _ => "F-Eco".into(),
    }
}

// ---------- Hội thoại trực tiếp ----------

/// Người được nhắn trực tiếp.
#[derive(Debug, Clone)]
pub enum Target {
    Developer(Uuid),
    App(String),
}

/// Chủ (thành viên `owner` đang hoạt động) của tài khoản nhà phát triển sở hữu app.
pub async fn app_owner(db: &Db, app_id: &str) -> sqlx::Result<Option<Uuid>> {
    sqlx::query_scalar(
        "SELECT m.subject FROM developer_apps d
         JOIN developer_members m ON m.account_id = d.account_id
         WHERE d.app_id = $1 AND d.deleted_at IS NULL AND m.role = 'owner' AND m.status = 'active'
           AND m.subject IS NOT NULL
         ORDER BY m.joined_at NULLS LAST, m.invited_at LIMIT 1",
    )
    .bind(app_id)
    .fetch_optional(db)
    .await
}

/// Người này nhận được hội thoại trực tiếp: vai trò Lập trình viên cá nhân hoặc Công ty phát
/// triển phần mềm, hoặc chủ tài khoản nhà phát triển có app trong F-Store.
pub async fn is_developer(db: &Db, subject: Uuid) -> sqlx::Result<bool> {
    if roles::role_of(db, subject).await?.can_take_projects() {
        return Ok(true);
    }
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM developer_members m
            JOIN developer_apps d ON d.account_id = m.account_id AND d.deleted_at IS NULL
            JOIN catalog_apps a ON a.id = d.app_id
            WHERE m.subject = $1 AND m.role = 'owner' AND m.status = 'active')",
    )
    .bind(subject)
    .fetch_one(db)
    .await
}

/// Tạo hoặc mở hội thoại trực tiếp của `customer` với một nhà phát triển. Trả hội thoại và
/// `true` nếu vừa tạo.
pub async fn start_direct(
    state: &AppState,
    customer: Uuid,
    target: Target,
) -> ChatResult<(Conversation, bool)> {
    let db = &state.db;
    let (developer, app_id) = match target {
        Target::Developer(d) => {
            if d == customer {
                return Err(ChatError::SelfChat);
            }
            if !is_developer(db, d).await? {
                return Err(ChatError::NotDeveloper);
            }
            (d, None)
        }
        Target::App(app) => {
            let app = app.trim().to_owned();
            let exists: bool =
                sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM catalog_apps WHERE id = $1)")
                    .bind(&app)
                    .fetch_one(db)
                    .await?;
            if !exists {
                return Err(ChatError::NotFound);
            }
            let owner = app_owner(db, &app).await?.ok_or(ChatError::NotDeveloper)?;
            if owner == customer {
                return Err(ChatError::SelfChat);
            }
            (owner, Some(app))
        }
    };
    let identity = state
        .kratos
        .get_identity(&customer.to_string())
        .await
        .map_err(|e| ChatError::Io(std::io::Error::other(e.to_string())))?
        .ok_or(ChatError::Forbidden)?;
    if !email_verified(&identity) {
        return Err(ChatError::EmailUnverified);
    }
    let existing: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM conversations WHERE kind = 'direct' AND customer = $1 AND developer = $2",
    )
    .bind(customer)
    .bind(developer)
    .fetch_optional(db)
    .await?;
    if let Some(id) = existing {
        if let Some(app) = &app_id {
            sqlx::query("UPDATE conversations SET app_id = $2 WHERE id = $1")
                .bind(id)
                .bind(app)
                .execute(db)
                .await?;
        }
        return Ok((get(db, id).await?, false));
    }
    let recent: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM conversations
         WHERE kind = 'direct' AND customer = $1 AND created_at > now() - interval '24 hours'",
    )
    .bind(customer)
    .fetch_one(db)
    .await?;
    if recent >= NEW_PER_DAY {
        return Err(ChatError::RateLimited);
    }
    let customer_name = identity_name(&identity).unwrap_or_else(|| "F-Eco".into());
    let developer_name = developer_name(state, developer).await;
    let created: Option<Uuid> = sqlx::query_scalar(
        "INSERT INTO conversations (kind, customer, developer, app_id, customer_name, developer_name)
         VALUES ('direct', $1, $2, $3, $4, $5)
         ON CONFLICT (customer, developer) WHERE kind = 'direct' DO NOTHING
         RETURNING id",
    )
    .bind(customer)
    .bind(developer)
    .bind(&app_id)
    .bind(&customer_name)
    .bind(&developer_name)
    .fetch_optional(db)
    .await?;
    match created {
        Some(id) => Ok((get(db, id).await?, true)),
        None => {
            let id: Uuid = sqlx::query_scalar(
                "SELECT id FROM conversations WHERE kind = 'direct' AND customer = $1 AND developer = $2",
            )
            .bind(customer)
            .bind(developer)
            .fetch_one(db)
            .await?;
            Ok((get(db, id).await?, false))
        }
    }
}

// ---------- Hội thoại dự án ----------

/// Hội thoại của dự án (tạo nếu chưa có), trong giao dịch của dự án.
pub async fn ensure_project(
    conn: &mut PgConnection,
    project: Uuid,
    customer: Uuid,
    provider: Uuid,
    customer_name: &str,
    provider_name: &str,
) -> sqlx::Result<Uuid> {
    sqlx::query(
        "INSERT INTO conversations (kind, customer, developer, project_id, customer_name, developer_name)
         VALUES ('project', $1, $2, $3, $4, $5)
         ON CONFLICT (project_id) DO NOTHING",
    )
    .bind(customer)
    .bind(provider)
    .bind(project)
    .bind(customer_name)
    .bind(provider_name)
    .execute(&mut *conn)
    .await?;
    sqlx::query_scalar("SELECT id FROM conversations WHERE project_id = $1")
        .bind(project)
        .fetch_one(&mut *conn)
        .await
}

/// Ghi một tin trong hội thoại dự án từ luồng dự án (tranh chấp, quyết định của quản trị), trong
/// giao dịch của dự án. `side` là `customer`, `provider`, `admin` hoặc `system`.
pub async fn project_message(
    conn: &mut PgConnection,
    project: Uuid,
    sender: Option<Uuid>,
    side: &str,
    body: &str,
) -> sqlx::Result<(Uuid, i64)> {
    let row: (Uuid, Uuid, String, Uuid, String) = sqlx::query_as(
        "SELECT p.customer, p.provider, p.customer_email, p.provider, COALESCE(pp.display_name, p.provider_email)
         FROM projects p LEFT JOIN provider_profiles pp ON pp.subject = p.provider WHERE p.id = $1",
    )
    .bind(project)
    .fetch_one(&mut *conn)
    .await?;
    let conv = ensure_project(conn, project, row.0, row.1, &row.2, &row.4).await?;
    let side = if side == "provider" {
        "developer"
    } else {
        side
    };
    let body: String = body.trim().chars().take(MAX_BODY).collect();
    let id = insert_message(conn, conv, sender, side, &body).await?;
    Ok((conv, id))
}

/// Ghi tin, cập nhật thời điểm tin cuối, báo các máy chủ (`pg_notify` có hiệu lực khi giao dịch
/// được ghi).
async fn insert_message(
    conn: &mut PgConnection,
    conversation: Uuid,
    sender: Option<Uuid>,
    side: &str,
    body: &str,
) -> sqlx::Result<i64> {
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO chat_messages (conversation_id, sender, side, body) VALUES ($1, $2, $3, $4) RETURNING id",
    )
    .bind(conversation)
    .bind(sender)
    .bind(side)
    .bind(body)
    .fetch_one(&mut *conn)
    .await?;
    sqlx::query("UPDATE conversations SET last_message_at = now() WHERE id = $1")
        .bind(conversation)
        .execute(&mut *conn)
        .await?;
    sqlx::query(
        "UPDATE projects SET updated_at = now() WHERE id = (SELECT project_id FROM conversations WHERE id = $1)",
    )
    .bind(conversation)
    .execute(&mut *conn)
    .await?;
    sqlx::query("SELECT pg_notify($1, $2)")
        .bind(CHANNEL)
        .bind(conversation.to_string())
        .execute(&mut *conn)
        .await?;
    Ok(id)
}

/// Dự án đã hủy hoặc đã hoàn tiền quá 30 ngày thì hội thoại không nhận tin mới.
async fn project_closed(db: &Db, project: Uuid) -> sqlx::Result<bool> {
    sqlx::query_scalar(
        "SELECT status IN ('cancelled', 'refunded')
                AND COALESCE(closed_at, updated_at) < now() - make_interval(days => $2)
         FROM projects WHERE id = $1",
    )
    .bind(project)
    .bind(CLOSED_PROJECT_DAYS as i32)
    .fetch_optional(db)
    .await
    .map(|x| x.unwrap_or(false))
}

// ---------- Tin nhắn ----------

/// Tệp người dùng tải lên kèm tin.
pub struct Upload {
    pub name: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Message {
    pub id: i64,
    pub sender: Option<Uuid>,
    pub side: String,
    pub body: String,
    pub at: OffsetDateTime,
    pub att_id: Option<Uuid>,
    pub att_name: Option<String>,
    pub att_size: Option<i64>,
    pub att_status: Option<String>,
}

const MESSAGE_COLUMNS: &str =
    "m.id, m.sender, m.side, m.body, m.at, a.id AS att_id, a.name AS att_name,
    a.size AS att_size, a.scan_status AS att_status";

/// Tin sau `after` (cũ trước), tối đa `limit`. `after = 0` trả `limit` tin mới nhất.
pub async fn messages(
    db: &Db,
    conversation: Uuid,
    after: i64,
    limit: i64,
) -> sqlx::Result<Vec<Message>> {
    let limit = limit.clamp(1, 200);
    if after <= 0 {
        return sqlx::query_as(&format!(
            "SELECT * FROM (SELECT {MESSAGE_COLUMNS} FROM chat_messages m
               LEFT JOIN chat_attachments a ON a.message_id = m.id
               WHERE m.conversation_id = $1 ORDER BY m.id DESC LIMIT $2) x ORDER BY id"
        ))
        .bind(conversation)
        .bind(limit)
        .fetch_all(db)
        .await;
    }
    sqlx::query_as(&format!(
        "SELECT {MESSAGE_COLUMNS} FROM chat_messages m
         LEFT JOIN chat_attachments a ON a.message_id = m.id
         WHERE m.conversation_id = $1 AND m.id > $2 ORDER BY m.id LIMIT $3"
    ))
    .bind(conversation)
    .bind(after)
    .bind(limit)
    .fetch_all(db)
    .await
}

pub async fn message(db: &Db, id: i64) -> sqlx::Result<Message> {
    sqlx::query_as(&format!(
        "SELECT {MESSAGE_COLUMNS} FROM chat_messages m
         LEFT JOIN chat_attachments a ON a.message_id = m.id WHERE m.id = $1"
    ))
    .bind(id)
    .fetch_one(db)
    .await
}

/// Tên tệp an toàn: bỏ đường dẫn và ký tự điều khiển, tối đa 120 ký tự.
pub fn clean_file_name(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or_default();
    let cleaned: String = base
        .chars()
        .filter(|c| !c.is_control() && !matches!(c, '<' | '>' | ':' | '"' | '|' | '?' | '*'))
        .collect();
    let cleaned = cleaned.trim().trim_matches('.').trim();
    let out: String = cleaned.chars().take(120).collect();
    if out.is_empty() {
        "tep-dinh-kem".into()
    } else {
        out
    }
}

/// Kiểm tra nội dung tin: bỏ khoảng trắng hai đầu; tin chỉ có tệp thì nội dung là tên tệp.
pub fn check_body(body: &str, file_name: Option<&str>) -> ChatResult<String> {
    let body = body.trim();
    let body = if body.is_empty() {
        file_name.unwrap_or_default().to_owned()
    } else {
        body.to_owned()
    };
    if body.is_empty() {
        return Err(ChatError::Empty);
    }
    if body.chars().count() > MAX_BODY {
        return Err(ChatError::TooLong);
    }
    Ok(body)
}

/// Người tham gia gửi tin. Trả id tin và id tệp đính kèm (nếu có); người gọi chạy
/// [`scan_attachment`] cho tệp.
pub async fn send(
    state: &AppState,
    conversation: Uuid,
    sender: Uuid,
    body: &str,
    upload: Option<Upload>,
) -> ChatResult<(i64, Option<(Uuid, Vec<u8>)>)> {
    let db = &state.db;
    let (c, role) = for_participant(db, conversation, sender).await?;
    if c.is_direct() && c.blocked_at.is_some() && role == Role::Customer {
        return Err(ChatError::Blocked);
    }
    if let Some(p) = c.project_id {
        if project_closed(db, p).await? {
            return Err(ChatError::Forbidden);
        }
    }
    let upload = upload.map(|u| Upload {
        name: clean_file_name(&u.name),
        bytes: u.bytes,
    });
    if upload
        .as_ref()
        .is_some_and(|u| u.bytes.len() > MAX_ATTACHMENT)
    {
        return Err(ChatError::AttachmentTooLarge);
    }
    let body = check_body(body, upload.as_ref().map(|u| u.name.as_str()))?;
    rate_check(db, sender).await?;
    let mut tx = db.begin().await?;
    let id = insert_message(&mut tx, c.id, Some(sender), role.as_str(), &body).await?;
    let mut att = None;
    if let Some(u) = upload {
        let att_id = Uuid::new_v4();
        let dir = state.package_dir.join("chat").join(c.id.to_string());
        tokio::fs::create_dir_all(&dir).await?;
        let path = dir.join(att_id.to_string());
        tokio::fs::write(&path, &u.bytes).await?;
        sqlx::query(
            "INSERT INTO chat_attachments (id, message_id, name, size, sha256, path) VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(att_id)
        .bind(id)
        .bind(&u.name)
        .bind(u.bytes.len() as i64)
        .bind(scan::sha256_hex(&u.bytes))
        .bind(path.to_string_lossy().into_owned())
        .execute(&mut *tx)
        .await?;
        att = Some((att_id, u.bytes));
    }
    tx.commit().await?;
    notify_new(state, &c, id, role.as_str()).await;
    Ok((id, att))
}

/// Quản trị dự án gửi tin trong hội thoại dự án (`side = 'admin'`).
pub async fn send_admin(
    state: &AppState,
    project: Uuid,
    admin: Uuid,
    body: &str,
) -> ChatResult<i64> {
    let body = check_body(body, None)?;
    let mut tx = state.db.begin().await?;
    let exists: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM projects WHERE id = $1)")
        .bind(project)
        .fetch_one(&mut *tx)
        .await?;
    if !exists {
        return Err(ChatError::NotFound);
    }
    let (conv, id) = project_message(&mut tx, project, Some(admin), "admin", &body).await?;
    tx.commit().await?;
    let c = get(&state.db, conv).await?;
    notify_new(state, &c, id, "admin").await;
    Ok(id)
}

async fn rate_check(db: &Db, sender: Uuid) -> ChatResult<()> {
    let n: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM chat_messages WHERE sender = $1 AND at > now() - interval '1 minute'",
    )
    .bind(sender)
    .fetch_one(db)
    .await?;
    if n >= PER_MINUTE {
        Err(ChatError::RateLimited)
    } else {
        Ok(())
    }
}

/// Đánh dấu đã đọc tới tin `up_to` (không có thì tới tin mới nhất).
pub async fn mark_read(
    db: &Db,
    c: &Conversation,
    role: Role,
    up_to: Option<i64>,
) -> sqlx::Result<()> {
    let column = match role {
        Role::Customer => "customer_read",
        Role::Developer => "developer_read",
    };
    sqlx::query(&format!(
        "UPDATE conversations SET {column} = GREATEST({column}, LEAST($2,
            (SELECT COALESCE(max(id), 0) FROM chat_messages WHERE conversation_id = $1)))
         WHERE id = $1"
    ))
    .bind(c.id)
    .bind(up_to.unwrap_or(i64::MAX))
    .execute(db)
    .await?;
    Ok(())
}

// ---------- Thông báo ----------

/// Báo "Tin nhắn mới từ {tên}" cho người nhận chưa đọc, không đang mở hội thoại, và chưa được
/// báo trong 15 phút qua. Thông báo và email không chứa nội dung tin.
pub async fn notify_new(state: &AppState, c: &Conversation, message_id: i64, side: &str) {
    let recipients: &[Role] = match side {
        "customer" => &[Role::Developer],
        "developer" => &[Role::Customer],
        _ => &[Role::Customer, Role::Developer],
    };
    let (name_vi, name_en) = match side {
        "customer" => (c.customer_name.clone(), c.customer_name.clone()),
        "developer" => (c.developer_name.clone(), c.developer_name.clone()),
        "admin" => (
            "Quản trị F-Eco".to_owned(),
            "F-Eco administrator".to_owned(),
        ),
        _ => ("F-Eco".to_owned(), "F-Eco".to_owned()),
    };
    for role in recipients {
        let (subject, read_col, notified_col) = match role {
            Role::Customer => (c.customer, "customer_read", "customer_notified_at"),
            Role::Developer => (c.developer, "developer_read", "developer_notified_at"),
        };
        if state.chat.is_viewing(c.id, subject) {
            continue;
        }
        let claimed = sqlx::query(&format!(
            "UPDATE conversations SET {notified_col} = now()
             WHERE id = $1 AND {read_col} < $2
               AND ({notified_col} IS NULL OR {notified_col} < now() - make_interval(mins => $3))"
        ))
        .bind(c.id)
        .bind(message_id)
        .bind(NOTIFY_EVERY_MINUTES as i32)
        .execute(&state.db)
        .await;
        match claimed {
            Ok(r) if r.rows_affected() == 1 => {}
            Ok(_) => continue,
            Err(e) => {
                tracing::error!(error = %e, "không ghi được lần báo tin nhắn");
                continue;
            }
        }
        let note = Note::vi(
            format!("Tin nhắn mới từ {name_vi}"),
            "Mở hội thoại trên F-Eco để đọc tin.",
            format!("/messages/{}", c.id),
        )
        .en(
            format!("New message from {name_en}"),
            "Open the conversation on F-Eco to read it.",
        );
        notifications::notify(state, subject, None, Category::Messages, &note).await;
    }
}

// ---------- Chặn, báo cáo ----------

/// Nhà phát triển chặn hoặc bỏ chặn hội thoại trực tiếp.
pub async fn set_blocked(
    state: &AppState,
    id: Uuid,
    subject: Uuid,
    blocked: bool,
) -> ChatResult<()> {
    let (c, role) = for_participant(&state.db, id, subject).await?;
    if role != Role::Developer || !c.is_direct() {
        return Err(ChatError::Forbidden);
    }
    sqlx::query(
        "UPDATE conversations SET blocked_at = CASE WHEN $2 THEN COALESCE(blocked_at, now()) END WHERE id = $1",
    )
    .bind(id)
    .bind(blocked)
    .execute(&state.db)
    .await?;
    crate::audit::record(
        &state.db,
        crate::audit::Actor::User(subject),
        if blocked {
            "chat.block"
        } else {
            "chat.unblock"
        },
        Some(&id.to_string()),
        None,
        json!({}),
    )
    .await;
    Ok(())
}

/// Người tham gia báo cáo hội thoại tới quản trị hỗ trợ.
pub async fn report(state: &AppState, id: Uuid, subject: Uuid, reason: &str) -> ChatResult<i64> {
    for_participant(&state.db, id, subject).await?;
    let reason: String = reason.trim().chars().take(1_000).collect();
    if reason.is_empty() {
        return Err(ChatError::Empty);
    }
    let rid: i64 = sqlx::query_scalar(
        "INSERT INTO chat_reports (conversation_id, reporter, reason) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind(id)
    .bind(subject)
    .bind(&reason)
    .fetch_one(&state.db)
    .await?;
    crate::review::admin_alert(
        state,
        "chat_report",
        "Có báo cáo hội thoại mới",
        json!({"conversation": id, "report": rid}),
    )
    .await;
    Ok(rid)
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Report {
    pub id: i64,
    pub conversation_id: Uuid,
    pub reporter: Uuid,
    pub reason: String,
    pub at: OffsetDateTime,
    pub handled_at: Option<OffsetDateTime>,
    pub handled_by: Option<String>,
}

/// Báo cáo chưa xử lý trước, rồi 100 báo cáo gần nhất.
pub async fn reports(db: &Db) -> sqlx::Result<Vec<Report>> {
    sqlx::query_as(
        "SELECT id, conversation_id, reporter, reason, at, handled_at, handled_by FROM chat_reports
         ORDER BY (handled_at IS NULL) DESC, at DESC LIMIT 100",
    )
    .fetch_all(db)
    .await
}

pub async fn report_by_id(db: &Db, id: i64) -> sqlx::Result<Option<Report>> {
    sqlx::query_as(
        "SELECT id, conversation_id, reporter, reason, at, handled_at, handled_by FROM chat_reports WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(db)
    .await
}

pub async fn mark_handled(db: &Db, id: i64, by: &str) -> sqlx::Result<bool> {
    Ok(sqlx::query(
        "UPDATE chat_reports SET handled_at = now(), handled_by = $2 WHERE id = $1 AND handled_at IS NULL",
    )
    .bind(id)
    .bind(by)
    .execute(db)
    .await?
    .rows_affected()
        == 1)
}

// ---------- Tệp đính kèm ----------

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Attachment {
    pub id: Uuid,
    pub message_id: i64,
    pub name: String,
    pub size: i64,
    pub path: String,
    pub scan_status: String,
}

/// Tệp đính kèm thuộc hội thoại.
pub async fn attachment(db: &Db, conversation: Uuid, id: Uuid) -> ChatResult<Attachment> {
    sqlx::query_as(
        "SELECT a.id, a.message_id, a.name, a.size, a.path, a.scan_status FROM chat_attachments a
         JOIN chat_messages m ON m.id = a.message_id WHERE a.id = $1 AND m.conversation_id = $2",
    )
    .bind(id)
    .bind(conversation)
    .fetch_optional(db)
    .await?
    .ok_or(ChatError::NotFound)
}

/// Tệp đã quét sạch mới tải được.
pub fn downloadable(a: &Attachment) -> ChatResult<()> {
    match a.scan_status.as_str() {
        "clean" => Ok(()),
        "infected" => Err(ChatError::AttachmentBlocked),
        _ => Err(ChatError::AttachmentPending),
    }
}

async fn engines(state: &AppState) -> sqlx::Result<Vec<Engine>> {
    let signatures: Vec<(String, String)> =
        sqlx::query_as("SELECT sha256, name FROM malware_signatures")
            .fetch_all(&state.db)
            .await?;
    let mut out = vec![Engine::Signatures(signatures)];
    if let Some(addr) = state.clamd {
        out.push(Engine::Clamd(addr));
    }
    Ok(out)
}

/// Quét một tệp đính kèm bằng các engine của khâu duyệt app. Có mã độc thì chặn tải và báo quản
/// trị (`chat_malware`); engine lỗi thì để `error` và quét lại sau.
pub async fn scan_attachment(state: &AppState, id: Uuid) {
    scan(state, id, None).await
}

/// Quét tệp vừa tải lên từ bộ nhớ (không phụ thuộc tệp trên đĩa, vốn có thể bị phần mềm diệt
/// virus của máy chủ cách ly trước khi kịp đọc lại).
pub async fn scan_upload(state: &AppState, id: Uuid, bytes: Vec<u8>) {
    scan(state, id, Some(bytes)).await
}

async fn scan(state: &AppState, id: Uuid, bytes: Option<Vec<u8>>) {
    let row: Option<(String, String, i64, Uuid)> = sqlx::query_as(
        "SELECT a.path, a.name, a.message_id, m.conversation_id FROM chat_attachments a
         JOIN chat_messages m ON m.id = a.message_id WHERE a.id = $1",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await
    .unwrap_or_else(|e| {
        tracing::error!(error = %e, "không đọc được tệp đính kèm cần quét");
        None
    });
    let Some((path, name, message_id, conversation)) = row else {
        return;
    };
    let outcome: Result<Option<String>, String> = async {
        let bytes = match bytes {
            Some(b) => b,
            None => tokio::fs::read(&path).await.map_err(|e| e.to_string())?,
        };
        let engines = engines(state).await.map_err(|e| e.to_string())?;
        for e in engines {
            if let Some(found) = e
                .scan(&bytes)
                .await
                .map_err(|err| format!("{}: {err}", e.name()))?
            {
                return Ok(Some(format!("{}: {found}", e.name())));
            }
        }
        Ok(None)
    }
    .await;
    let (status, note) = match &outcome {
        Ok(None) => ("clean", None),
        Ok(Some(found)) => ("infected", Some(found.clone())),
        Err(e) => ("error", Some(e.clone())),
    };
    if let Err(e) = sqlx::query(
        "UPDATE chat_attachments SET scan_status = $2, scan_note = $3, scanned_at = now(),
             scan_attempts = scan_attempts + 1 WHERE id = $1",
    )
    .bind(id)
    .bind(status)
    .bind(&note)
    .execute(&state.db)
    .await
    {
        tracing::error!(error = %e, "không ghi được kết quả quét tệp đính kèm");
        return;
    }
    // Trang đang mở nhận sự kiện để cập nhật trạng thái tệp.
    let _ = sqlx::query("SELECT pg_notify($1, $2)")
        .bind(CHANNEL)
        .bind(conversation.to_string())
        .execute(&state.db)
        .await;
    match outcome {
        Ok(Some(found)) => {
            crate::review::admin_alert(
                state,
                "chat_malware",
                &format!("Tệp đính kèm trong chat có mã độc: {found}"),
                json!({"conversation": conversation, "message": message_id, "attachment": id, "name": name, "found": found}),
            )
            .await;
        }
        Err(e) => tracing::warn!(error = %e, "chưa quét được tệp đính kèm, sẽ quét lại"),
        Ok(None) => {}
    }
}

/// Quét lại tệp còn `pending` quá 1 phút (máy chủ khởi động lại giữa chừng) hoặc `error`.
pub async fn rescan_due(state: &AppState) -> sqlx::Result<usize> {
    let ids: Vec<Uuid> = sqlx::query_scalar(
        "SELECT a.id FROM chat_attachments a JOIN chat_messages m ON m.id = a.message_id
         WHERE (a.scan_status = 'pending' AND m.at < now() - interval '1 minute')
            OR (a.scan_status = 'error' AND a.scan_attempts < $1)
         LIMIT 50",
    )
    .bind(MAX_SCAN_ATTEMPTS)
    .fetch_all(&state.db)
    .await?;
    for id in &ids {
        scan_attachment(state, *id).await;
    }
    Ok(ids.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_codes_are_documented_with_design_status() {
        let all = [
            (ChatError::NotFound, 404),
            (ChatError::Forbidden, 403),
            (ChatError::Blocked, 409),
            (ChatError::TooLong, 422),
            (ChatError::Empty, 422),
            (ChatError::RateLimited, 429),
            (ChatError::AttachmentTooLarge, 413),
            (ChatError::AttachmentPending, 409),
            (ChatError::AttachmentBlocked, 403),
            (ChatError::NotDeveloper, 422),
            (ChatError::SelfChat, 422),
            (ChatError::EmailUnverified, 403),
        ];
        for (e, status) in all {
            assert_eq!(e.status().as_u16(), status, "{e:?}");
            let documented = crate::errors::CODES
                .iter()
                .find(|(c, ..)| *c == e.code())
                .unwrap_or_else(|| panic!("{} chưa có trong errors::CODES", e.code()));
            assert_eq!(documented.1, status, "{}", e.code());
            for lang in crate::i18n::Lang::ALL {
                assert!(!lang.t(e.key()).is_empty());
            }
        }
    }

    /// Design C2: 1–5.000 ký tự sau khi bỏ khoảng trắng; tin chỉ có tệp thì nội dung là tên tệp.
    #[test]
    fn body_rules() {
        assert_eq!(check_body("  chào  ", None).unwrap(), "chào");
        assert!(matches!(check_body("   ", None), Err(ChatError::Empty)));
        assert_eq!(check_body(" ", Some("a.pdf")).unwrap(), "a.pdf");
        assert!(check_body(&"ă".repeat(5_000), None).is_ok());
        assert!(matches!(
            check_body(&"ă".repeat(5_001), None),
            Err(ChatError::TooLong)
        ));
    }

    #[test]
    fn file_names_are_cleaned() {
        assert_eq!(clean_file_name("../../etc/passwd"), "passwd");
        assert_eq!(clean_file_name("C:\\Users\\a\\báo cáo.pdf"), "báo cáo.pdf");
        assert_eq!(clean_file_name("a<b>\u{0007}.txt"), "ab.txt");
        assert_eq!(clean_file_name(".."), "tep-dinh-kem");
        assert_eq!(clean_file_name(&"x".repeat(300)).chars().count(), 120);
    }

    #[test]
    fn viewers_are_counted_per_connection() {
        let hub = Hub::default();
        let (c, s) = (Uuid::from_u128(1), Uuid::from_u128(2));
        assert!(!hub.is_viewing(c, s));
        hub.add_viewer((c, s));
        hub.add_viewer((c, s));
        hub.remove_viewer((c, s));
        assert!(hub.is_viewing(c, s));
        hub.remove_viewer((c, s));
        assert!(!hub.is_viewing(c, s));
        assert!(!hub.is_viewing(c, Uuid::from_u128(3)));
    }

    #[test]
    fn roles_and_names() {
        let c = Conversation {
            id: Uuid::from_u128(9),
            kind: "direct".into(),
            customer: Uuid::from_u128(1),
            developer: Uuid::from_u128(2),
            project_id: None,
            app_id: None,
            customer_name: "Khách".into(),
            developer_name: "Lan".into(),
            blocked_at: None,
            created_at: OffsetDateTime::UNIX_EPOCH,
            last_message_at: OffsetDateTime::UNIX_EPOCH,
            customer_read: 3,
            developer_read: 5,
        };
        assert_eq!(c.role_of(Uuid::from_u128(1)), Some(Role::Customer));
        assert_eq!(c.role_of(Uuid::from_u128(2)), Some(Role::Developer));
        assert_eq!(c.role_of(Uuid::from_u128(3)), None);
        assert_eq!(c.peer_name(Role::Customer), "Lan");
        assert_eq!(c.peer_name(Role::Developer), "Khách");
        assert_eq!(c.read_of(Role::Developer), 5);
    }
}
