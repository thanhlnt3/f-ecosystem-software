//! Điểm nhận token của F-Eco (thay đổi oidc-login-and-token-reuse, design T1–T3).
//!
//! `POST /oauth2/token` chuyển nguyên yêu cầu tới Hydra và trả nguyên phản hồi. Mọi xử lý OAuth
//! (kiểm client, PKCE, xoay vòng, thu hồi chuỗi khi token bị dùng lại) vẫn do Hydra làm. F-Eco
//! chỉ ghi băm SHA-256 của refresh token Hydra cấp, để khi Hydra từ chối một refresh token đã
//! biết thì gửi cảnh báo bảo mật cho người dùng (spec account-identity, "Refresh token bị dùng
//! lại"). Thân yêu cầu và phản hồi chứa client secret và token nên không bao giờ được ghi log.

use std::sync::Arc;

use axum::{
    body::{Body, Bytes},
    extract::State,
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{
    audit::{self, Actor},
    kratos::ClientInfo,
    notifications::{self, Category, Note},
    AppState,
};

/// Thân yêu cầu lớn nhất được chuyển tiếp.
pub const MAX_BODY: usize = 64 * 1024;
/// Hạn refresh token của Hydra (`ttl.refresh_token`, 720 giờ).
const REFRESH_TTL_HOURS: i64 = 720;

pub fn hash(token: &str) -> String {
    Sha256::digest(token.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn form_value(body: &[u8], name: &str) -> Option<String> {
    form_urlencoded::parse(body)
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.into_owned())
}

/// POST /oauth2/token
pub async fn token(
    State(state): State<Arc<AppState>>,
    client: ClientInfo,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if body.len() > MAX_BODY {
        return oauth_error(StatusCode::PAYLOAD_TOO_LARGE, "invalid_request");
    }
    let url = state
        .hydra_public
        .join("oauth2/token")
        .expect("đường dẫn hợp lệ");
    let mut req = state.http.post(url).body(body.clone());
    for name in [header::CONTENT_TYPE, header::AUTHORIZATION, header::ACCEPT] {
        if let Some(v) = headers.get(&name) {
            req = req.header(name.as_str(), v.as_bytes());
        }
    }
    let res = match req.send().await {
        Ok(r) => r,
        Err(e) => {
            tracing::error!(error = %e, "không gọi được điểm nhận token của Hydra");
            return oauth_error(StatusCode::SERVICE_UNAVAILABLE, "temporarily_unavailable");
        }
    };
    let status = StatusCode::from_u16(res.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let mut out_headers = HeaderMap::new();
    for name in [header::CONTENT_TYPE, header::CACHE_CONTROL, header::PRAGMA] {
        if let Some(v) = res
            .headers()
            .get(name.as_str())
            .and_then(|v| HeaderValue::from_bytes(v.as_bytes()).ok())
        {
            out_headers.insert(name, v);
        }
    }
    let res_body = match res.bytes().await {
        Ok(b) => b,
        Err(e) => {
            tracing::error!(error = %e, "không đọc được phản hồi token của Hydra");
            return oauth_error(StatusCode::BAD_GATEWAY, "temporarily_unavailable");
        }
    };

    let grant = form_value(&body, "grant_type").unwrap_or_default();
    let presented = (grant == "refresh_token")
        .then(|| form_value(&body, "refresh_token"))
        .flatten();
    if status.is_success() {
        if let Ok(v) = serde_json::from_slice::<Value>(&res_body) {
            if let Err(e) = record_issued(&state, presented.as_deref(), &v).await {
                tracing::error!(error = %e, "không ghi được refresh token mới");
            }
        }
    } else if let Some(presented) = &presented {
        let invalid_grant = serde_json::from_slice::<Value>(&res_body)
            .is_ok_and(|v| v["error"] == "invalid_grant" || v["error"] == "token_inactive");
        if invalid_grant {
            if let Err(e) = on_rejected(&state, presented, &client).await {
                tracing::error!(error = %e, "không kiểm tra được refresh token bị từ chối");
            }
        }
    }

    let mut response = Response::new(Body::from(res_body));
    *response.status_mut() = status;
    *response.headers_mut() = out_headers;
    response
}

fn oauth_error(status: StatusCode, error: &str) -> Response {
    (
        status,
        [(header::CACHE_CONTROL, "no-store")],
        axum::Json(json!({"error": error})),
    )
        .into_response()
}

/// Hydra vừa cấp token: ghi refresh token mới vào chuỗi của token đã gửi lên, hoặc chuỗi mới.
async fn record_issued(
    state: &AppState,
    presented: Option<&str>,
    tokens: &Value,
) -> Result<(), String> {
    let Some(new_refresh) = tokens["refresh_token"].as_str() else {
        return Ok(());
    };
    let mut tx = state.db.begin().await.map_err(|e| e.to_string())?;
    let mut family: Option<Uuid> = None;
    if let Some(old) = presented {
        family = sqlx::query_scalar(
            "UPDATE oauth_refresh_tokens SET rotated_at = now()
             WHERE token_hash = $1 AND rotated_at IS NULL RETURNING family",
        )
        .bind(hash(old))
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    }
    let family = match family {
        Some(f) => f,
        None => {
            // Lần đầu (đổi mã ủy quyền), hoặc token cấp trước khi có điểm nhận này.
            let Some(access) = tokens["access_token"].as_str() else {
                return Ok(());
            };
            let Some(info) = state
                .hydra
                .introspect(access)
                .await
                .map_err(|e| e.to_string())?
            else {
                return Ok(());
            };
            let (Some(subject), Some(client_id)) = (
                info["sub"].as_str().and_then(|s| s.parse::<Uuid>().ok()),
                info["client_id"].as_str(),
            ) else {
                return Ok(());
            };
            sqlx::query_scalar(
                "INSERT INTO oauth_token_families (subject, client_id) VALUES ($1, $2) RETURNING id",
            )
            .bind(subject)
            .bind(client_id)
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| e.to_string())?
        }
    };
    sqlx::query(
        "INSERT INTO oauth_refresh_tokens (token_hash, family) VALUES ($1, $2)
         ON CONFLICT (token_hash) DO NOTHING",
    )
    .bind(hash(new_refresh))
    .bind(family)
    .execute(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;
    tx.commit().await.map_err(|e| e.to_string())
}

/// Hydra từ chối một refresh token (`invalid_grant`). Token đã biết, còn hạn, chuỗi chưa bị
/// thu hồi chủ động và chưa cảnh báo: đây là dùng lại. Trả `true` khi đã gửi cảnh báo.
pub async fn on_rejected(
    state: &AppState,
    presented: &str,
    client: &ClientInfo,
) -> Result<bool, String> {
    let row: Option<(Uuid, Uuid, String, bool)> = sqlx::query_as(
        "UPDATE oauth_token_families f SET reuse_detected_at = now()
         FROM oauth_refresh_tokens t
         WHERE t.token_hash = $1 AND t.family = f.id
           AND t.issued_at > now() - make_interval(hours => $2)
           AND f.revoked_at IS NULL AND f.reuse_detected_at IS NULL
         RETURNING f.id, f.subject, f.client_id, t.rotated_at IS NOT NULL",
    )
    .bind(hash(presented))
    .bind(REFRESH_TTL_HOURS as i32)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| e.to_string())?;
    let Some((family, subject, client_id, rotated)) = row else {
        return Ok(false);
    };
    let app = match state.hydra.get_client(&client_id).await {
        Ok(Some(c)) => c["client_name"]
            .as_str()
            .filter(|n| !n.is_empty())
            .unwrap_or(&client_id)
            .to_owned(),
        _ => client_id.clone(),
    };
    let when = time::OffsetDateTime::now_utc()
        .to_offset(time::macros::offset!(+7))
        .format(time::macros::format_description!(
            "[hour]:[minute] [day]/[month]/[year] (UTC+7)"
        ))
        .unwrap_or_default();
    let device = crate::account::describe_user_agent(client.user_agent.as_deref().unwrap_or(""));
    let device = if device.is_empty() {
        "?".to_owned()
    } else {
        device
    };
    let ip = client.ip.clone().unwrap_or_else(|| "?".into());
    let note = Note::vi(
        format!("Phiên đăng nhập {app} có dấu hiệu bị sao chép"),
        format!(
            "Lúc {when}, một refresh token cũ của {app} được dùng lại (thiết bị: {device}, IP: {ip}). \
             Có thể ai đó đã sao chép phiên đăng nhập của bạn, nên F-Eco đã đăng xuất phiên đó; \
             thiết bị của bạn cần đăng nhập lại. Nếu bạn không nhận ra việc này, hãy đổi mật khẩu \
             và xem các thiết bị đang đăng nhập."
        ),
        "/sessions",
    )
    .en(
        format!("Your {app} session may have been copied"),
        format!(
            "At {when}, an old refresh token of {app} was used again (device: {device}, IP: {ip}). \
             Someone may have copied your session, so F-Eco signed that session out; your device \
             needs to sign in again. If you don't recognise this, change your password and review \
             your signed-in devices."
        ),
    );
    notifications::notify(state, subject, None, Category::Security, &note).await;
    audit::record(
        &state.db,
        Actor::System,
        "auth.refresh_token_reused",
        Some(&subject.to_string()),
        client.ip.as_deref(),
        json!({"client_id": client_id, "family": family, "rotated": rotated}),
    )
    .await;
    tracing::warn!(%subject, client_id, "refresh token bị dùng lại");
    Ok(true)
}

/// Người dùng gỡ quyền app: token bị từ chối sau đó không phải dùng lại.
pub async fn revoke_families(
    db: &sqlx::PgPool,
    subject: Uuid,
    client_id: Option<&str>,
) -> sqlx::Result<()> {
    sqlx::query(
        "UPDATE oauth_token_families SET revoked_at = now()
         WHERE subject = $1 AND ($2::text IS NULL OR client_id = $2) AND revoked_at IS NULL",
    )
    .bind(subject)
    .bind(client_id)
    .execute(db)
    .await
    .map(|_| ())
}

/// Xóa refresh token cấp quá 31 ngày (đã hết hạn ở Hydra) và chuỗi không còn token nào.
pub async fn purge_old(db: &sqlx::PgPool) -> sqlx::Result<u64> {
    let tokens = sqlx::query(
        "DELETE FROM oauth_refresh_tokens WHERE issued_at < now() - interval '31 days'",
    )
    .execute(db)
    .await?
    .rows_affected();
    sqlx::query(
        "DELETE FROM oauth_token_families f WHERE created_at < now() - interval '31 days'
         AND NOT EXISTS (SELECT 1 FROM oauth_refresh_tokens t WHERE t.family = f.id)",
    )
    .execute(db)
    .await?;
    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_is_sha256_hex() {
        assert_eq!(
            hash("abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn reads_form_fields() {
        let body = b"grant_type=refresh_token&refresh_token=ory_rt_a%2Bb&client_id=x";
        assert_eq!(
            form_value(body, "grant_type").as_deref(),
            Some("refresh_token")
        );
        assert_eq!(
            form_value(body, "refresh_token").as_deref(),
            Some("ory_rt_a+b")
        );
        assert_eq!(form_value(body, "scope"), None);
    }
}
