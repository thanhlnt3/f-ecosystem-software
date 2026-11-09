//! Phát hiện đăng nhập bất thường (spec platform-security, "Phát hiện đăng nhập bất
//! thường"): đăng nhập thành công từ thiết bị mới (trình duyệt + hệ điều hành) hoặc từ
//! quốc gia khác với trước thì gửi email cho chủ tài khoản, kèm nút "Không phải tôi"
//! để đăng xuất mọi phiên và đặt lại mật khẩu.
//!
//! Lần đăng nhập đầu tiên của một tài khoản chỉ được ghi nhận, không gửi email.

use std::sync::Arc;

use askama::Template;
use axum::{
    body::Bytes,
    extract::{Path, State},
    http::{header, HeaderMap, StatusCode},
    response::{Html, IntoResponse, Response},
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{
    account::describe_user_agent,
    audit::{self, Actor},
    csrf,
    i18n::Lang,
    mailer::templates,
    views::Msg,
    AppState,
};

/// Thiết bị và quốc gia của phiên vừa đăng nhập (từ dữ liệu thiết bị Kratos ghi lại).
pub fn device_of(session: &Value) -> (String, String, String) {
    let d = session["devices"].as_array().and_then(|d| d.last());
    let field = |k: &str| d.and_then(|d| d[k].as_str()).unwrap_or_default().to_owned();
    let device = describe_user_agent(&field("user_agent"));
    let location = field("location");
    // Kratos ghi vị trí dạng "Thành phố, QG"; phần cuối là mã quốc gia.
    let country = location
        .rsplit(',')
        .next()
        .map(|c| c.trim().to_owned())
        .unwrap_or_default();
    (
        if device.is_empty() {
            "?".into()
        } else {
            device
        },
        country,
        location,
    )
}

fn hash_token(token: &str) -> String {
    Sha256::digest(token.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Ghi nhận thiết bị của lần đăng nhập; gửi email nếu đó là thiết bị hoặc quốc gia mới.
pub async fn on_login(state: &AppState, session: &Value) -> Result<bool, String> {
    let Some(subject) = session["identity"]["id"]
        .as_str()
        .and_then(|s| s.parse::<Uuid>().ok())
    else {
        return Ok(false);
    };
    let (device, country, location) = device_of(session);
    let mut tx = state.db.begin().await.map_err(|e| e.to_string())?;
    let known: i64 = sqlx::query_scalar("SELECT count(*) FROM known_devices WHERE subject = $1")
        .bind(subject)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    // `xmax = 0` chỉ đúng với dòng vừa được chèn mới (không phải dòng cũ được cập nhật).
    let fresh: bool = sqlx::query_scalar(
        "INSERT INTO known_devices (subject, device, country) VALUES ($1, $2, $3)
         ON CONFLICT (subject, device, country) DO UPDATE SET last_seen = now()
         RETURNING (xmax = 0)",
    )
    .bind(subject)
    .bind(&device)
    .bind(&country)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;
    let alert = fresh && known > 0;
    let token = alert.then(|| {
        let mut buf = [0u8; 32];
        getrandom::fill(&mut buf).expect("nguồn ngẫu nhiên");
        buf.iter().map(|b| format!("{b:02x}")).collect::<String>()
    });
    if let Some(t) = &token {
        sqlx::query("INSERT INTO security_alert_tokens (token_hash, subject) VALUES ($1, $2)")
            .bind(hash_token(t))
            .bind(subject)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
    }
    tx.commit().await.map_err(|e| e.to_string())?;

    let Some(token) = token else {
        return Ok(false);
    };
    let email = session["identity"]["traits"]["email"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    let when = time::OffsetDateTime::now_utc()
        .format(time::macros::format_description!(
            "[year]-[month]-[day] [hour]:[minute] UTC"
        ))
        .unwrap_or_default();
    let link = format!("{}security/not-me/{token}", state.public_url);
    let (subject_line, body) = templates::new_device(&device, &location, &when, &link);
    state
        .mailer
        .send(&email, &subject_line, &body)
        .await
        .map_err(|e| e.to_string())?;
    audit::record(
        &state.db,
        Actor::System,
        "security.new_device_alert",
        Some(&subject.to_string()),
        None,
        json!({"device": device, "country": country}),
    )
    .await;
    Ok(true)
}

#[derive(Template)]
#[template(path = "not_me.html")]
struct NotMePage {
    i18n: Lang,
    messages: Vec<Msg>,
    token: String,
    csrf: String,
    done: bool,
}

fn render(t: NotMePage, lang: Lang, set_cookie: Option<header::HeaderValue>) -> Response {
    let mut res = match t.render() {
        Ok(h) => Html(h).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "lỗi dựng trang");
            return (StatusCode::INTERNAL_SERVER_ERROR, lang.t("error.render")).into_response();
        }
    };
    if let Some(c) = set_cookie {
        res.headers_mut().append(header::SET_COOKIE, c);
    }
    res
}

async fn token_subject(state: &AppState, token: &str) -> sqlx::Result<Option<Uuid>> {
    sqlx::query_scalar(
        "SELECT subject FROM security_alert_tokens
         WHERE token_hash = $1 AND used_at IS NULL AND created_at > now() - interval '7 days'",
    )
    .bind(hash_token(token))
    .fetch_optional(&state.db)
    .await
}

/// GET /security/not-me/{token}
pub async fn not_me(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    Path(token): Path<String>,
    headers: HeaderMap,
) -> Response {
    let valid = matches!(token_subject(&state, &token).await, Ok(Some(_)));
    let (csrf_token, set) = csrf::issue(&headers, state.public_url.scheme() == "https");
    let messages = if valid {
        vec![]
    } else {
        vec![Msg::error(lang.t("notme.invalid"))]
    };
    render(
        NotMePage {
            i18n: lang,
            messages,
            token: if valid { token } else { String::new() },
            csrf: csrf_token,
            done: !valid,
        },
        lang,
        set,
    )
}

/// POST /security/not-me/{token}: đăng xuất mọi phiên, hướng dẫn đặt lại mật khẩu.
pub async fn not_me_confirm(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    Path(token): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let form_csrf = form_urlencoded::parse(&body)
        .find(|(k, _)| k == csrf::FIELD)
        .map(|(_, v)| v.into_owned());
    if !csrf::verify(&headers, form_csrf.as_deref()) {
        let (t, set) = csrf::issue(&headers, false);
        return render(
            NotMePage {
                i18n: lang,
                messages: vec![Msg::error(lang.t("error.csrf"))],
                token,
                csrf: t,
                done: false,
            },
            lang,
            set,
        );
    }
    let subject = match token_subject(&state, &token).await {
        Ok(Some(s)) => s,
        Ok(None) => {
            return render(
                NotMePage {
                    i18n: lang,
                    messages: vec![Msg::error(lang.t("notme.invalid"))],
                    token: String::new(),
                    csrf: String::new(),
                    done: true,
                },
                lang,
                None,
            )
        }
        Err(e) => {
            tracing::error!(error = %e, "không đọc được token cảnh báo");
            return (StatusCode::SERVICE_UNAVAILABLE, lang.t("error.unavailable")).into_response();
        }
    };
    if let Err(e) = state
        .kratos
        .revoke_identity_sessions(&subject.to_string())
        .await
    {
        tracing::error!(error = %e, "không đăng xuất được các phiên");
        return (StatusCode::SERVICE_UNAVAILABLE, lang.t("error.unavailable")).into_response();
    }
    let _ = sqlx::query("UPDATE security_alert_tokens SET used_at = now() WHERE token_hash = $1")
        .bind(hash_token(&token))
        .execute(&state.db)
        .await;
    audit::record(
        &state.db,
        Actor::User(subject),
        "security.not_me",
        Some(&subject.to_string()),
        None,
        json!({"sessions_revoked": "all"}),
    )
    .await;
    render(
        NotMePage {
            i18n: lang,
            messages: vec![Msg::success(lang.t("notme.done"))],
            token: String::new(),
            csrf: String::new(),
            done: true,
        },
        lang,
        None,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_and_country_from_kratos_session() {
        let s = json!({"devices": [{"user_agent": "Mozilla/5.0 (X11; Linux x86_64; rv:132.0) Gecko/20100101 Firefox/132.0", "location": "Hà Nội, VN"}]});
        assert_eq!(
            device_of(&s),
            ("Firefox · Linux".into(), "VN".into(), "Hà Nội, VN".into())
        );
        let unknown = device_of(&json!({}));
        assert_eq!(unknown.0, "?");
        assert_eq!(unknown.1, "");
    }

    #[test]
    fn tokens_are_stored_hashed() {
        let h = hash_token("abc");
        assert_eq!(h.len(), 64);
        assert_ne!(h, "abc");
    }
}
