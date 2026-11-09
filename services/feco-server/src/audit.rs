//! Nhật ký kiểm toán (spec platform-security, "Nhật ký kiểm toán"): đăng nhập, đổi thông
//! tin bảo mật, cấp và thu hồi đồng ý, xóa tài khoản, thu hồi thiết bị, xuất dữ liệu,
//! thao tác quản trị (nhóm 14).
//!
//! - Chỉ ghi thêm: cơ sở dữ liệu chặn sửa và xóa (migrations/0002_security.sql); chỉ
//!   bản ghi quá 1 năm mới được tác vụ lưu trữ xóa.
//! - Không chứa bí mật: mọi khóa có tên giống mật khẩu, token, mã, thẻ bị loại khỏi
//!   `details` trước khi ghi.
//! - Ghi kiểu "cố gắng hết sức" cho thao tác của người dùng: lỗi ghi được báo lên nhật ký
//!   hệ thống nhưng không chặn người dùng.

use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::store::Db;

#[derive(Debug, Clone)]
pub enum Actor {
    User(Uuid),
    /// Người dùng chưa xác định (ví dụ lần đăng nhập sai).
    Anonymous,
    App(String),
    System,
}

impl Actor {
    fn parts(&self) -> (&'static str, Option<String>) {
        match self {
            Actor::User(id) => ("user", Some(id.to_string())),
            Actor::Anonymous => ("user", None),
            Actor::App(c) => ("app", Some(c.clone())),
            Actor::System => ("system", None),
        }
    }
}

const SECRET_HINTS: &[&str] = &[
    "password",
    "passwd",
    "secret",
    "token",
    "code",
    "otp",
    "totp",
    "cvv",
    "card",
    "pan",
    "cookie",
    "session_token",
    "authorization",
    "key",
];

fn looks_secret(key: &str) -> bool {
    let k = key.to_ascii_lowercase();
    SECRET_HINTS.iter().any(|h| k.contains(h))
}

/// Loại mọi khóa có tên giống bí mật, đệ quy.
pub fn sanitize(v: &Value) -> Value {
    match v {
        Value::Object(m) => Value::Object(
            m.iter()
                .filter(|(k, _)| !looks_secret(k))
                .map(|(k, v)| (k.clone(), sanitize(v)))
                .collect::<Map<_, _>>(),
        ),
        Value::Array(a) => Value::Array(a.iter().map(sanitize).collect()),
        other => other.clone(),
    }
}

/// Băm định danh (ví dụ email trong lần đăng nhập sai) để điều tra mà không lưu bản rõ.
pub fn hash_identifier(id: &str) -> String {
    let h = Sha256::digest(id.trim().to_lowercase().as_bytes());
    h.iter().take(16).map(|b| format!("{b:02x}")).collect()
}

pub async fn record(
    db: &Db,
    actor: Actor,
    action: &str,
    target: Option<&str>,
    ip: Option<&str>,
    details: Value,
) {
    let (actor_type, actor_id) = actor.parts();
    let res = sqlx::query(
        "INSERT INTO audit_log (actor_type, actor_id, action, target, ip, details)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(actor_type)
    .bind(actor_id)
    .bind(action)
    .bind(target)
    .bind(ip)
    .bind(sanitize(&details))
    .execute(db)
    .await;
    if let Err(e) = res {
        tracing::error!(error = %e, action, "không ghi được nhật ký kiểm toán");
    }
}

/// Xóa bản ghi quá 1 năm (trigger chỉ cho phép xóa đúng các bản ghi này).
pub async fn purge_older_than_one_year(db: &Db) -> sqlx::Result<u64> {
    sqlx::query("DELETE FROM audit_log WHERE at <= now() - interval '1 year'")
        .execute(db)
        .await
        .map(|r| r.rows_affected())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn secrets_are_removed_recursively() {
        let v = json!({
            "method": "password",
            "password": "bi-mat",
            "nested": {"access_token": "t", "client_id": "app", "totp_code": "123456"},
            "list": [{"card_number": "4111", "ok": 1}],
            "csrf_token": "x"
        });
        let s = sanitize(&v);
        assert_eq!(
            s,
            json!({"method": "password", "nested": {"client_id": "app"}, "list": [{"ok": 1}]})
        );
    }

    #[test]
    fn identifiers_are_hashed_case_insensitively() {
        assert_eq!(
            hash_identifier("Lan@Example.com "),
            hash_identifier("lan@example.com")
        );
        assert_eq!(hash_identifier("lan@example.com").len(), 32);
        assert!(!hash_identifier("lan@example.com").contains("lan"));
    }
}
