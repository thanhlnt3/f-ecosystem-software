//! Mã khởi chạy dùng một lần cho app con (spec launcher, "Mở app con và truyền phiên đăng
//! nhập"; design D4).
//!
//! 1. `POST /v1/launch-codes` (chỉ token của F-Launcher): `{"client_id": "<app>"}` → mã sống
//!    60 giây, gắn với người dùng và đúng app đó. Máy chủ chỉ lưu mã băm.
//! 2. Launcher chạy app với biến môi trường `FECO_LAUNCH_CODE`.
//! 3. `POST /v1/launch-codes/exchange` (app gọi): `{"code", "client_id", "redirect_uri",
//!    "code_challenge", "scope"}`. Mã bị "đốt" ngay (dùng một lần, kể cả khi sai app). Nếu hợp
//!    lệ, feco-server tự đi luồng Authorization Code của Hydra thay cho trình duyệt: chấp nhận
//!    đăng nhập với định danh riêng của app (pairwise), chỉ cấp lại những scope người dùng đã
//!    đồng ý trước đó, rồi trả `authorization_code`. App đổi code đó lấy token tại Hydra bằng
//!    PKCE verifier của mình, nên chỉ đúng app giữ verifier mới lấy được token.
//!
//! Mã bị dùng lại, hết hạn hoặc dùng sai app đều bị từ chối và ghi vào nhật ký kiểm toán.

use std::{collections::BTreeMap, sync::Arc, time::Duration};

use axum::{
    extract::State,
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use base64::Engine;
use serde::Deserialize;
use serde_json::json;
use sha2::Digest;
use uuid::Uuid;

use crate::{
    api::{api_error, bearer},
    apps::{self, AppInfo},
    audit::{self, Actor},
    kratos::ClientInfo,
    oauth, AppState,
};

/// Thời gian sống của mã khởi chạy (spec: 60 giây).
pub const TTL_SECS: u64 = 60;
/// Số mã một người dùng xin được mỗi phút.
const ISSUE_PER_MIN: u32 = 30;

#[derive(Debug, Deserialize)]
pub struct IssueRequest {
    pub client_id: String,
}

#[derive(Debug, Deserialize)]
pub struct ExchangeRequest {
    pub code: String,
    pub client_id: String,
    pub redirect_uri: String,
    pub code_challenge: String,
    #[serde(default)]
    pub scope: Option<String>,
}

fn random_code() -> String {
    let mut buf = [0u8; 32];
    getrandom::fill(&mut buf).expect("nguồn ngẫu nhiên");
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(buf)
}

fn code_hash(code: &str) -> String {
    sha2::Sha256::digest(code.trim().as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn unavailable() -> Response {
    api_error(
        StatusCode::SERVICE_UNAVAILABLE,
        "temporarily_unavailable",
        "The service is temporarily unavailable.",
        "Dịch vụ đang tạm gián đoạn.",
    )
}

fn invalid_grant() -> Response {
    api_error(
        StatusCode::BAD_REQUEST,
        "invalid_grant",
        "The launch code is invalid, expired or already used.",
        "Mã khởi chạy không hợp lệ, đã hết hạn hoặc đã được dùng.",
    )
}

/// POST /v1/launch-codes
pub async fn issue(
    State(state): State<Arc<AppState>>,
    client: ClientInfo,
    headers: HeaderMap,
    Json(req): Json<IssueRequest>,
) -> Response {
    let Some(launcher) = state.launcher_client_id.as_deref() else {
        return api_error(
            StatusCode::NOT_FOUND,
            "not_enabled",
            "Launch codes are not enabled on this server.",
            "Máy chủ chưa bật mã khởi chạy.",
        );
    };
    let Some(token) = bearer(&headers) else {
        return api_error(
            StatusCode::UNAUTHORIZED,
            "invalid_token",
            "The access token is missing, expired or revoked.",
            "Access token bị thiếu, đã hết hạn hoặc đã bị thu hồi.",
        );
    };
    let info = match state.hydra.introspect(token).await {
        Ok(Some(i)) => i,
        Ok(None) => {
            return api_error(
                StatusCode::UNAUTHORIZED,
                "invalid_token",
                "The access token is missing, expired or revoked.",
                "Access token bị thiếu, đã hết hạn hoặc đã bị thu hồi.",
            )
        }
        Err(e) => {
            tracing::error!(error = %e, "không kiểm tra được token");
            return unavailable();
        }
    };
    let subject = info["sub"].as_str().and_then(|s| s.parse::<Uuid>().ok());
    let (Some(subject), Some(caller)) = (subject, info["client_id"].as_str()) else {
        return api_error(
            StatusCode::UNAUTHORIZED,
            "invalid_token",
            "The access token is missing, expired or revoked.",
            "Access token bị thiếu, đã hết hạn hoặc đã bị thu hồi.",
        );
    };
    if caller != launcher {
        audit::record(
            &state.db,
            Actor::App(caller.to_owned()),
            "launch_code.denied_not_launcher",
            Some(&req.client_id),
            client.ip.as_deref(),
            json!({}),
        )
        .await;
        return api_error(
            StatusCode::FORBIDDEN,
            "launcher_only",
            "Only F-Launcher can request launch codes.",
            "Chỉ F-Launcher được xin mã khởi chạy.",
        );
    }
    if let Err(wait) = state.ratelimit.check(
        &format!("launch:{subject}"),
        ISSUE_PER_MIN,
        Duration::from_secs(60),
        std::time::Instant::now(),
    ) {
        return crate::security::too_many("/v1/launch-codes", wait);
    }
    if req.client_id == launcher {
        return api_error(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "Launch codes are for child apps only.",
            "Mã khởi chạy chỉ dành cho app con.",
        );
    }
    match state.hydra.get_client(&req.client_id).await {
        Ok(Some(_)) => {}
        Ok(None) => {
            return api_error(
                StatusCode::NOT_FOUND,
                "unknown_client",
                "No app has this client ID.",
                "Không có app nào có client ID này.",
            )
        }
        Err(e) => {
            tracing::error!(error = %e, "không đọc được app");
            return unavailable();
        }
    }
    let code = random_code();
    let res = sqlx::query(
        "INSERT INTO launch_codes (code_hash, subject, client_id, expires_at)
         VALUES ($1, $2, $3, now() + make_interval(secs => $4))",
    )
    .bind(code_hash(&code))
    .bind(subject)
    .bind(&req.client_id)
    .bind(TTL_SECS as f64)
    .execute(&state.db)
    .await;
    if let Err(e) = res {
        tracing::error!(error = %e, "không lưu được mã khởi chạy");
        return unavailable();
    }
    audit::record(
        &state.db,
        Actor::User(subject),
        "launch_code.issue",
        Some(&req.client_id),
        client.ip.as_deref(),
        json!({}),
    )
    .await;
    Json(json!({ "code": code, "expires_in": TTL_SECS })).into_response()
}

/// POST /v1/launch-codes/exchange
pub async fn exchange(
    State(state): State<Arc<AppState>>,
    client: ClientInfo,
    Json(req): Json<ExchangeRequest>,
) -> Response {
    let hash = code_hash(&req.code);
    // Đốt mã trước mọi kiểm tra khác: một mã chỉ có một lần thử.
    let claimed: Result<Option<(Uuid, String)>, _> = sqlx::query_as(
        "UPDATE launch_codes SET used_at = now()
         WHERE code_hash = $1 AND used_at IS NULL AND expires_at > now()
         RETURNING subject, client_id",
    )
    .bind(&hash)
    .fetch_optional(&state.db)
    .await;
    let (subject, owner) = match claimed {
        Ok(Some(row)) => row,
        Ok(None) => {
            let known: Option<(String, bool)> = sqlx::query_as(
                "SELECT client_id, used_at IS NOT NULL FROM launch_codes WHERE code_hash = $1",
            )
            .bind(&hash)
            .fetch_optional(&state.db)
            .await
            .unwrap_or(None);
            let action = match &known {
                Some((_, true)) => "launch_code.reuse",
                Some((_, false)) => "launch_code.expired",
                None => "launch_code.unknown",
            };
            audit::record(
                &state.db,
                Actor::App(req.client_id.clone()),
                action,
                known.as_ref().map(|(c, _)| c.as_str()),
                client.ip.as_deref(),
                json!({}),
            )
            .await;
            return invalid_grant();
        }
        Err(e) => {
            tracing::error!(error = %e, "không đọc được mã khởi chạy");
            return unavailable();
        }
    };
    if owner != req.client_id {
        audit::record(
            &state.db,
            Actor::App(req.client_id.clone()),
            "launch_code.client_mismatch",
            Some(&owner),
            client.ip.as_deref(),
            json!({}),
        )
        .await;
        return invalid_grant();
    }
    match authorize_headless(&state, subject, &req).await {
        Ok(code) => {
            audit::record(
                &state.db,
                Actor::User(subject),
                "launch_code.exchange",
                Some(&owner),
                client.ip.as_deref(),
                json!({}),
            )
            .await;
            Json(json!({ "authorization_code": code, "redirect_uri": req.redirect_uri }))
                .into_response()
        }
        Err(Headless::ConsentRequired) => api_error(
            StatusCode::FORBIDDEN,
            "consent_required",
            "The user has not granted these scopes to the app yet; use the interactive sign-in.",
            "Người dùng chưa đồng ý các scope này cho app; hãy dùng luồng đăng nhập qua trình duyệt.",
        ),
        Err(Headless::Rejected(e)) => api_error(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            &format!("The authorization request was rejected: {e}"),
            &format!("Yêu cầu ủy quyền bị từ chối: {e}"),
        ),
        Err(Headless::Upstream(e)) => {
            tracing::error!(error = %e, "không đi được luồng ủy quyền với Hydra");
            unavailable()
        }
    }
}

enum Headless {
    ConsentRequired,
    Rejected(String),
    Upstream(String),
}

/// Cookie của Hydra (chống CSRF giữa các bước) trong một lần đi luồng.
#[derive(Default)]
struct Jar(BTreeMap<String, String>);

impl Jar {
    fn take(&mut self, res: &reqwest::Response) {
        for v in res.headers().get_all(header::SET_COOKIE) {
            if let Some((name, value)) = v
                .to_str()
                .ok()
                .and_then(|s| s.split(';').next())
                .and_then(|s| s.split_once('='))
            {
                self.0
                    .insert(name.trim().to_owned(), value.trim().to_owned());
            }
        }
    }

    fn header(&self) -> String {
        self.0
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join("; ")
    }
}

fn query_param(u: &url::Url, key: &str) -> Option<String> {
    u.query_pairs()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.into_owned())
}

async fn step(state: &AppState, jar: &mut Jar, url: &str) -> Result<url::Url, Headless> {
    let res = state
        .http
        .get(url)
        .header(header::COOKIE, jar.header())
        .send()
        .await
        .map_err(|e| Headless::Upstream(e.to_string()))?;
    jar.take(&res);
    let location = res
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| {
            Headless::Upstream(format!("Hydra trả {} không chuyển hướng", res.status()))
        })?;
    let next = url::Url::parse(location)
        .or_else(|_| url::Url::parse(url).and_then(|base| base.join(location)))
        .map_err(|e| Headless::Upstream(e.to_string()))?;
    // Hydra báo lỗi bằng cách chuyển về redirect_uri kèm `error`.
    if let Some(error) = query_param(&next, "error") {
        let description = query_param(&next, "error_description").unwrap_or_default();
        return Err(Headless::Rejected(format!("{error}: {description}")));
    }
    Ok(next)
}

async fn authorize_headless(
    state: &AppState,
    subject: Uuid,
    req: &ExchangeRequest,
) -> Result<String, Headless> {
    let upstream = |e: crate::hydra::HydraError| Headless::Upstream(e.to_string());
    let csrf_state = random_code();
    let mut auth = state
        .hydra_public
        .join("oauth2/auth")
        .map_err(|e| Headless::Upstream(e.to_string()))?;
    auth.query_pairs_mut()
        .append_pair("client_id", &req.client_id)
        .append_pair("response_type", "code")
        .append_pair("scope", req.scope.as_deref().unwrap_or("openid"))
        .append_pair("redirect_uri", &req.redirect_uri)
        .append_pair("state", &csrf_state)
        .append_pair("code_challenge", &req.code_challenge)
        .append_pair("code_challenge_method", "S256");
    let mut jar = Jar::default();

    // 1. Hydra chuyển sang trang đăng nhập của feco-server kèm login_challenge.
    let to_login = step(state, &mut jar, auth.as_str()).await?;
    let login_challenge = query_param(&to_login, "login_challenge")
        .ok_or_else(|| Headless::Upstream(format!("thiếu login_challenge: {to_login}")))?;
    let login = state
        .hydra
        .login_request(&login_challenge)
        .await
        .map_err(upstream)?;
    if login["client"]["client_id"].as_str() != Some(req.client_id.as_str()) {
        return Err(Headless::Rejected("client_id không khớp".into()));
    }
    let pairwise = state.pairwise.subject(&req.client_id, &subject.to_string());
    let next = state
        .hydra
        .accept_login(&login_challenge, &subject.to_string(), &pairwise)
        .await
        .map_err(upstream)?;

    // 2. Hydra chuyển sang màn hình đồng ý kèm consent_challenge.
    let to_consent = step(state, &mut jar, &next).await?;
    let consent_challenge = query_param(&to_consent, "consent_challenge")
        .ok_or_else(|| Headless::Upstream(format!("thiếu consent_challenge: {to_consent}")))?;
    let request = state
        .hydra
        .consent_request(&consent_challenge)
        .await
        .map_err(upstream)?;
    let app = AppInfo::from_client(&request["client"]);
    let requested = oauth::requested_scopes(&request);
    let previous = oauth::previously_granted(state, &subject.to_string(), &app.client_id).await;
    // Không hiện được màn hình đồng ý ở đây: chỉ cấp lại đúng những gì người dùng đã đồng ý.
    if !apps::scopes_to_ask(&app, &requested, &previous).is_empty() {
        let _ = state.hydra.reject_consent(&consent_challenge).await;
        return Err(Headless::ConsentRequired);
    }
    let grant = apps::grant(&app, &requested, &previous, &Default::default());
    let next = oauth::accept_and_record(
        state,
        &consent_challenge,
        &request,
        &app,
        &grant,
        "launch-code",
    )
    .await
    .map_err(Headless::Upstream)?;

    // 3. Hydra chuyển về redirect_uri của app kèm authorization code.
    let callback = step(state, &mut jar, &next).await?;
    if query_param(&callback, "state").as_deref() != Some(csrf_state.as_str()) {
        return Err(Headless::Upstream("state không khớp".into()));
    }
    query_param(&callback, "code")
        .ok_or_else(|| Headless::Upstream(format!("thiếu code: {callback}")))
}

/// Xóa mã khởi chạy đã hết hạn quá 1 ngày (chạy cùng các tác vụ dọn dẹp định kỳ).
pub async fn purge_expired(db: &crate::store::Db) -> Result<u64, sqlx::Error> {
    let res = sqlx::query("DELETE FROM launch_codes WHERE expires_at < now() - interval '1 day'")
        .execute(db)
        .await?;
    Ok(res.rows_affected())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_are_random_and_only_hash_is_stored() {
        let a = random_code();
        let b = random_code();
        assert_ne!(a, b);
        assert_eq!(a.len(), 43);
        assert_eq!(code_hash(&a).len(), 64);
        assert_ne!(code_hash(&a), a);
        assert_eq!(code_hash(&format!(" {a}\n")), code_hash(&a));
    }

    #[test]
    fn jar_keeps_latest_cookie_values() {
        let mut jar = Jar::default();
        jar.0.insert("a".into(), "1".into());
        jar.0.insert("b".into(), "2".into());
        assert_eq!(jar.header(), "a=1; b=2");
    }
}
