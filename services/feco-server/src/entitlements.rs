//! Quyền sử dụng của người dùng với từng app (spec app-sdk, "Kiểm tra quyền sử dụng").
//!
//! - `GET /v1/entitlements`: app chỉ thấy quyền của người dùng hiện tại với **chính app đó**
//!   (lấy client ID từ access token), và token phải mang scope `purchases`.
//! - Cấp hay thu hồi quyền gửi webhook `entitlement.changed` cho app. Nhóm 12–13 (đơn xây app,
//!   thanh toán) sẽ cấp quyền; tạm thời dùng `feco-catalog grant`.

use std::sync::Arc;

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use feco_core::Scope;
use serde::Serialize;
use serde_json::json;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{
    api::{api_error, bearer},
    store::{self, Db},
    AppState,
};

pub const KINDS: [&str; 3] = ["owner", "license", "subscription"];

#[derive(Debug, Clone, Serialize)]
pub struct Entitlement {
    pub kind: String,
    pub source: String,
    #[serde(with = "time::serde::rfc3339")]
    pub granted_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339::option")]
    pub expires_at: Option<OffsetDateTime>,
}

/// Quyền còn hiệu lực của một người dùng với một app.
pub async fn active(db: &Db, client_id: &str, subject: Uuid) -> sqlx::Result<Vec<Entitlement>> {
    let rows: Vec<(String, String, OffsetDateTime, Option<OffsetDateTime>)> = sqlx::query_as(
        "SELECT kind, source, granted_at, expires_at FROM entitlements
         WHERE client_id = $1 AND subject = $2 AND revoked_at IS NULL
           AND (expires_at IS NULL OR expires_at > now())
         ORDER BY kind",
    )
    .bind(client_id)
    .bind(subject)
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| Entitlement {
            kind: r.0,
            source: r.1,
            granted_at: r.2,
            expires_at: r.3,
        })
        .collect())
}

async fn notify(
    state: &AppState,
    client_id: &str,
    subject: Uuid,
    kind: &str,
    action: &str,
) -> sqlx::Result<()> {
    let mut payload =
        crate::connected::event_payload(state, "entitlement.changed", client_id, subject);
    payload["kind"] = json!(kind);
    payload["action"] = json!(action);
    store::enqueue_event(&state.db, client_id, "entitlement.changed", &payload).await?;
    Ok(())
}

/// Cấp (hoặc gia hạn) một quyền.
pub async fn grant(
    state: &AppState,
    client_id: &str,
    subject: Uuid,
    kind: &str,
    source: &str,
    expires_at: Option<OffsetDateTime>,
) -> sqlx::Result<bool> {
    if !KINDS.contains(&kind) {
        return Ok(false);
    }
    sqlx::query(
        "INSERT INTO entitlements (client_id, subject, kind, source, expires_at) VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT (client_id, subject, kind) DO UPDATE SET source = $4, expires_at = $5,
             granted_at = now(), revoked_at = NULL",
    )
    .bind(client_id)
    .bind(subject)
    .bind(kind)
    .bind(source)
    .bind(expires_at)
    .execute(&state.db)
    .await?;
    notify(state, client_id, subject, kind, "granted").await?;
    Ok(true)
}

/// Thu hồi một quyền.
pub async fn revoke(
    state: &AppState,
    client_id: &str,
    subject: Uuid,
    kind: &str,
) -> sqlx::Result<bool> {
    let n = sqlx::query(
        "UPDATE entitlements SET revoked_at = now()
         WHERE client_id = $1 AND subject = $2 AND kind = $3 AND revoked_at IS NULL",
    )
    .bind(client_id)
    .bind(subject)
    .bind(kind)
    .execute(&state.db)
    .await?
    .rows_affected();
    if n > 0 {
        notify(state, client_id, subject, kind, "revoked").await?;
    }
    Ok(n > 0)
}

/// GET /v1/entitlements
pub async fn api_list(State(state): State<Arc<AppState>>, headers: HeaderMap) -> Response {
    let invalid = || {
        api_error(
            StatusCode::UNAUTHORIZED,
            "invalid_token",
            "The access token is missing, expired or revoked.",
            "Access token bị thiếu, đã hết hạn hoặc đã bị thu hồi.",
        )
    };
    let unavailable = |e: String| {
        tracing::error!(error = %e, "không đọc được quyền sử dụng");
        api_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "temporarily_unavailable",
            "The service is temporarily unavailable.",
            "Dịch vụ đang tạm gián đoạn.",
        )
    };
    let Some(token) = bearer(&headers) else {
        return invalid();
    };
    let info = match state.hydra.introspect(token).await {
        Ok(Some(i)) => i,
        Ok(None) => return invalid(),
        Err(e) => return unavailable(e.to_string()),
    };
    let (Some(subject), Some(client_id)) = (
        info["sub"].as_str().and_then(|s| s.parse::<Uuid>().ok()),
        info["client_id"].as_str(),
    ) else {
        return invalid();
    };
    let has_scope = info["scope"]
        .as_str()
        .unwrap_or_default()
        .split(' ')
        .any(|s| s == Scope::Purchases.as_str());
    if !has_scope {
        return api_error(
            StatusCode::FORBIDDEN,
            "insufficient_scope",
            "This endpoint requires the `purchases` scope.",
            "Endpoint này cần scope `purchases`.",
        );
    }
    match active(&state.db, client_id, subject).await {
        Ok(list) => Json(json!({
            "sub": state.pairwise.subject(client_id, &subject.to_string()),
            "entitlements": list,
        }))
        .into_response(),
        Err(e) => unavailable(e.to_string()),
    }
}

/// Một app trong danh mục, nhìn từ phía một người dùng (để quyết định cấp mã khởi chạy).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchAccess {
    pub private_to: Option<Uuid>,
    pub price_coin: i64,
    /// Người dùng là chủ app hoặc thành viên tài khoản nhà phát triển của app.
    pub developer: bool,
    /// Người dùng đã mua app bằng F-Coin hoặc có quyền sử dụng còn hiệu lực.
    pub owns: bool,
}

impl LaunchAccess {
    /// App riêng tư của người khác, hoặc app có giá chưa mua, thì không được mở.
    pub fn allows(&self, subject: Uuid) -> bool {
        if self.developer {
            return true;
        }
        if self.private_to.is_some_and(|p| p != subject) {
            return false;
        }
        self.price_coin <= 0 || self.owns
    }
}

/// Người dùng `subject` có được mở app có client ID này không (spec launcher, "Mở app con và
/// truyền phiên đăng nhập"). App không có trong danh mục (app thử nghiệm, app của nền tảng) không
/// bị giới hạn.
pub async fn may_launch(db: &Db, client_id: &str, subject: Uuid) -> sqlx::Result<bool> {
    let rows: Vec<(Option<Uuid>, i64, bool, bool)> = sqlx::query_as(
        "SELECT a.private_to, a.price_coin,
                COALESCE(a.owner_subject = $2, false) OR EXISTS (
                    SELECT 1 FROM developer_apps d
                    JOIN developer_members m ON m.account_id = d.account_id
                    WHERE d.app_id = a.id AND m.subject = $2 AND m.status = 'active'),
                EXISTS (SELECT 1 FROM app_purchases p WHERE p.app_id = a.id AND p.buyer = $2)
                OR EXISTS (SELECT 1 FROM entitlements e
                    WHERE e.client_id = a.client_id AND e.subject = $2 AND e.revoked_at IS NULL
                      AND (e.expires_at IS NULL OR e.expires_at > now()))
         FROM catalog_apps a WHERE a.client_id = $1",
    )
    .bind(client_id)
    .bind(subject)
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .all(|(private_to, price_coin, developer, owns)| {
            LaunchAccess {
                private_to,
                price_coin,
                developer,
                owns,
            }
            .allows(subject)
        }))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// App riêng tư của người khác và app có giá chưa mua không được cấp mã khởi chạy; nhà phát
    /// triển của app luôn được.
    #[test]
    fn launch_access() {
        let me = Uuid::new_v4();
        let other = Uuid::new_v4();
        let free = LaunchAccess {
            private_to: None,
            price_coin: 0,
            developer: false,
            owns: false,
        };
        assert!(free.allows(me));
        let private = LaunchAccess {
            private_to: Some(other),
            ..free.clone()
        };
        assert!(!private.allows(me));
        assert!(private.allows(other));
        assert!(LaunchAccess {
            developer: true,
            ..private.clone()
        }
        .allows(me));
        let paid = LaunchAccess {
            price_coin: 49,
            ..free.clone()
        };
        assert!(!paid.allows(me));
        assert!(LaunchAccess {
            owns: true,
            ..paid.clone()
        }
        .allows(me));
        assert!(LaunchAccess {
            developer: true,
            ..paid
        }
        .allows(me));
        // App riêng tư có giá: khách hàng của đơn vẫn cần quyền sử dụng.
        let private_paid = LaunchAccess {
            private_to: Some(me),
            price_coin: 10,
            developer: false,
            owns: false,
        };
        assert!(!private_paid.allows(me));
    }
}
