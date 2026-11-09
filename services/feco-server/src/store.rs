//! Truy cập PostgreSQL cho dữ liệu đồng ý, nhật ký truy cập, webhook và xuất dữ liệu
//! (migrations/0001_user_data_consent.sql).

use serde_json::Value;
use sqlx::PgPool;
use time::OffsetDateTime;
use uuid::Uuid;

pub type Db = PgPool;

pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsentAction {
    Grant,
    Deny,
    Revoke,
}

impl ConsentAction {
    fn as_str(self) -> &'static str {
        match self {
            ConsentAction::Grant => "grant",
            ConsentAction::Deny => "deny",
            ConsentAction::Revoke => "revoke",
        }
    }
}

pub async fn record_consent(
    db: &Db,
    subject: Uuid,
    client_id: &str,
    action: ConsentAction,
    granted_scopes: &[String],
    content_version: &str,
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO consent_records (subject, client_id, action, granted_scopes, content_version)
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(subject)
    .bind(client_id)
    .bind(action.as_str())
    .bind(granted_scopes)
    .bind(content_version)
    .execute(db)
    .await
    .map(|_| ())
}

#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct ConsentRecord {
    pub client_id: String,
    pub action: String,
    pub granted_scopes: Vec<String>,
    pub content_version: String,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

pub async fn consent_history(db: &Db, subject: Uuid) -> sqlx::Result<Vec<ConsentRecord>> {
    sqlx::query_as(
        "SELECT client_id, action, granted_scopes, content_version, created_at
         FROM consent_records WHERE subject = $1 ORDER BY created_at, id",
    )
    .bind(subject)
    .fetch_all(db)
    .await
}

/// Các app từng được người dùng cấp quyền (để báo sự kiện khi xóa tài khoản).
pub async fn granted_clients(db: &Db, subject: Uuid) -> sqlx::Result<Vec<String>> {
    sqlx::query_scalar(
        "SELECT DISTINCT client_id FROM consent_records WHERE subject = $1 AND action = 'grant'",
    )
    .bind(subject)
    .fetch_all(db)
    .await
}

pub async fn log_access(
    db: &Db,
    subject: Uuid,
    client_id: &str,
    scopes: &[String],
) -> sqlx::Result<()> {
    sqlx::query("INSERT INTO data_access_log (subject, client_id, scopes) VALUES ($1, $2, $3)")
        .bind(subject)
        .bind(client_id)
        .bind(scopes)
        .execute(db)
        .await
        .map(|_| ())
}

#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct AccessEntry {
    pub client_id: String,
    pub scopes: Vec<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

/// Nhật ký truy cập 90 ngày gần nhất, mới nhất trước; `client_id = None` là mọi app.
pub async fn access_log(
    db: &Db,
    subject: Uuid,
    client_id: Option<&str>,
) -> sqlx::Result<Vec<AccessEntry>> {
    sqlx::query_as(
        "SELECT client_id, scopes, created_at FROM data_access_log
         WHERE subject = $1 AND ($2::text IS NULL OR client_id = $2)
           AND created_at > now() - interval '90 days'
         ORDER BY created_at DESC, id DESC LIMIT 1000",
    )
    .bind(subject)
    .bind(client_id)
    .fetch_all(db)
    .await
}

/// Lần truy cập gần nhất của mỗi app.
pub async fn last_access(db: &Db, subject: Uuid) -> sqlx::Result<Vec<(String, OffsetDateTime)>> {
    sqlx::query_as(
        "SELECT client_id, max(created_at) FROM data_access_log
         WHERE subject = $1 GROUP BY client_id",
    )
    .bind(subject)
    .fetch_all(db)
    .await
}

/// Xóa nhật ký truy cập quá 90 ngày.
pub async fn purge_old_access_log(db: &Db) -> sqlx::Result<u64> {
    sqlx::query("DELETE FROM data_access_log WHERE created_at <= now() - interval '90 days'")
        .execute(db)
        .await
        .map(|r| r.rows_affected())
}

/// Xóa dữ liệu của một người dùng khi tài khoản bị xóa hẳn (spec account-identity,
/// "Xóa tài khoản"). Hóa đơn (nhóm 13) không nằm ở đây và được giữ theo luật.
pub async fn delete_subject_data(db: &Db, subject: Uuid) -> sqlx::Result<()> {
    let mut tx = db.begin().await?;
    for q in [
        "DELETE FROM consent_records WHERE subject = $1",
        "DELETE FROM data_access_log WHERE subject = $1",
        "DELETE FROM data_exports WHERE subject = $1",
    ] {
        sqlx::query(q).bind(subject).execute(&mut *tx).await?;
    }
    tx.commit().await
}

// ---------- Webhook ----------

/// Đăng ký địa chỉ nhận sự kiện của app; trả về khóa ký (chỉ hiện một lần cho nhà phát triển).
pub async fn register_webhook(db: &Db, client_id: &str, url: &str) -> sqlx::Result<String> {
    let mut buf = [0u8; 32];
    getrandom::fill(&mut buf).expect("nguồn ngẫu nhiên");
    let secret: String = buf.iter().map(|b| format!("{b:02x}")).collect();
    sqlx::query(
        "INSERT INTO app_webhooks (client_id, url, secret) VALUES ($1, $2, $3)
         ON CONFLICT (client_id) DO UPDATE SET url = EXCLUDED.url, secret = EXCLUDED.secret",
    )
    .bind(client_id)
    .bind(url)
    .bind(&secret)
    .execute(db)
    .await?;
    Ok(secret)
}

/// Đưa sự kiện vào hàng đợi nếu app có đăng ký webhook. Trả về `true` khi đã đưa vào.
pub async fn enqueue_event(
    db: &Db,
    client_id: &str,
    event_type: &str,
    payload: &Value,
) -> sqlx::Result<bool> {
    let r = sqlx::query(
        "INSERT INTO webhook_outbox (client_id, event_type, payload)
         SELECT client_id, $2, $3 FROM app_webhooks WHERE client_id = $1",
    )
    .bind(client_id)
    .bind(event_type)
    .bind(payload)
    .execute(db)
    .await?;
    Ok(r.rows_affected() > 0)
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PendingDelivery {
    pub id: i64,
    pub event_type: String,
    pub payload: Value,
    pub attempts: i32,
    pub created_at: OffsetDateTime,
    pub url: String,
    pub secret: String,
}

/// Lấy các webhook đến hạn gửi, khóa dòng để nhiều bản feco-server không gửi trùng.
pub async fn due_deliveries(
    tx: &mut sqlx::PgConnection,
    limit: i64,
) -> sqlx::Result<Vec<PendingDelivery>> {
    sqlx::query_as(
        "SELECT o.id, o.event_type, o.payload, o.attempts, o.created_at, w.url, w.secret
         FROM webhook_outbox o JOIN app_webhooks w USING (client_id)
         WHERE o.delivered_at IS NULL AND o.failed_at IS NULL AND o.next_attempt_at <= now()
         ORDER BY o.next_attempt_at LIMIT $1
         FOR UPDATE OF o SKIP LOCKED",
    )
    .bind(limit)
    .fetch_all(tx)
    .await
}

// ---------- Xuất dữ liệu ----------

pub async fn create_export(db: &Db, subject: Uuid) -> sqlx::Result<Uuid> {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO data_exports (id, subject, status) VALUES ($1, $2, 'pending')")
        .bind(id)
        .bind(subject)
        .execute(db)
        .await?;
    Ok(id)
}

pub async fn finish_export(db: &Db, id: Uuid, file: &[u8], token: &str) -> sqlx::Result<()> {
    sqlx::query(
        "UPDATE data_exports SET status = 'ready', file = $2, download_token = $3,
             ready_at = now(), expires_at = now() + interval '7 days'
         WHERE id = $1",
    )
    .bind(id)
    .bind(file)
    .bind(token)
    .execute(db)
    .await
    .map(|_| ())
}

pub async fn fail_export(db: &Db, id: Uuid) -> sqlx::Result<()> {
    sqlx::query("UPDATE data_exports SET status = 'failed' WHERE id = $1")
        .bind(id)
        .execute(db)
        .await
        .map(|_| ())
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ExportStatus {
    pub status: String,
    pub created_at: OffsetDateTime,
    pub expires_at: Option<OffsetDateTime>,
}

pub async fn latest_export(db: &Db, subject: Uuid) -> sqlx::Result<Option<ExportStatus>> {
    sqlx::query_as(
        "SELECT status, created_at, expires_at FROM data_exports
         WHERE subject = $1 ORDER BY created_at DESC LIMIT 1",
    )
    .bind(subject)
    .fetch_optional(db)
    .await
}

/// File xuất theo token, chỉ khi còn hạn và đúng chủ.
pub async fn export_file(db: &Db, subject: Uuid, token: &str) -> sqlx::Result<Option<Vec<u8>>> {
    sqlx::query_scalar(
        "SELECT file FROM data_exports
         WHERE subject = $1 AND download_token = $2 AND status = 'ready' AND expires_at > now()",
    )
    .bind(subject)
    .bind(token)
    .fetch_optional(db)
    .await
    .map(Option::flatten)
}

/// Xóa file xuất đã hết hạn.
pub async fn purge_expired_exports(db: &Db) -> sqlx::Result<u64> {
    sqlx::query("DELETE FROM data_exports WHERE expires_at <= now()")
        .execute(db)
        .await
        .map(|r| r.rows_affected())
}
