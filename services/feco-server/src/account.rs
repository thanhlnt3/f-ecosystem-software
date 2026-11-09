//! Trang tài khoản do chính feco-server xử lý (spec account-identity):
//! - "Phiên đăng nhập": danh sách thiết bị, đăng xuất từng thiết bị hoặc mọi thiết bị khác;
//! - "Xóa tài khoản": chờ 30 ngày, đăng nhập lại để hủy, chặn khi còn đơn hàng.

use std::{
    sync::Arc,
    time::{Duration, SystemTime},
};

use askama::Template;
use axum::{
    body::Bytes,
    extract::{Path, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{Html, IntoResponse, Response},
};
use serde_json::{json, Value};

use crate::{
    csrf,
    i18n::Lang,
    kratos::{FlowKind, InitOptions, KratosClient, KratosError, Whoami},
    views::Msg,
    AppState,
};

/// Thời gian chờ trước khi xóa hẳn tài khoản.
pub const DELETION_GRACE: Duration = Duration::from_secs(30 * 24 * 3600);
/// Phiên phải được xác thực trong khoảng này mới được xóa tài khoản (khớp
/// `privileged_session_max_age` trong cấu hình Kratos).
pub const PRIVILEGED_MAX_AGE: Duration = Duration::from_secs(15 * 60);
const META_KEY: &str = "deletion_requested_at";

pub struct DeviceRow {
    pub id: String,
    pub current: bool,
    pub device: String,
    pub ip: String,
    pub location: String,
    pub signed_in_at: String,
}

#[derive(Template)]
#[template(path = "sessions.html")]
struct SessionsPage {
    i18n: Lang,
    rows: Vec<DeviceRow>,
    messages: Vec<Msg>,
    csrf: String,
}

#[derive(Template)]
#[template(path = "delete.html")]
struct DeletePage {
    i18n: Lang,
    messages: Vec<Msg>,
    csrf: String,
}

#[derive(Template)]
#[template(path = "message.html")]
struct MessagePage {
    i18n: Lang,
    title: String,
    messages: Vec<Msg>,
    links: Vec<(String, String)>,
}

fn html(t: impl Template, lang: Lang) -> Response {
    match t.render() {
        Ok(h) => Html(h).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "lỗi dựng trang");
            (StatusCode::INTERNAL_SERVER_ERROR, lang.t("error.render")).into_response()
        }
    }
}

fn cookie(headers: &HeaderMap) -> Option<&str> {
    headers.get(header::COOKIE).and_then(|v| v.to_str().ok())
}

fn see_other(to: &str) -> Response {
    let mut res = StatusCode::SEE_OTHER.into_response();
    if let Ok(v) = HeaderValue::from_str(to) {
        res.headers_mut().insert(header::LOCATION, v);
    }
    res
}

fn with_cookie(mut res: Response, c: Option<HeaderValue>) -> Response {
    if let Some(c) = c {
        res.headers_mut().append(header::SET_COOKIE, c);
    }
    res
}

fn unavailable(lang: Lang, e: KratosError) -> Response {
    tracing::error!(error = %e, "không gọi được Kratos");
    let mut res = html(
        MessagePage {
            i18n: lang,
            title: lang.t("error.title").into(),
            messages: vec![Msg::error(lang.t("error.unavailable"))],
            links: vec![("/".into(), lang.t("link.back_home").into())],
        },
        lang,
    );
    *res.status_mut() = StatusCode::SERVICE_UNAVAILABLE;
    res
}

/// Phiên hiện tại, hoặc phản hồi chuyển hướng (đăng nhập / bước hai) nếu chưa có.
async fn require_session(
    state: &AppState,
    lang: Lang,
    headers: &HeaderMap,
    return_path: &str,
) -> Result<Value, Box<Response>> {
    match state.kratos.whoami(cookie(headers)).await {
        Ok(Whoami::Session(s)) => Ok(s),
        Ok(Whoami::NeedsSecondFactor(to)) => Err(Box::new(see_other(&to))),
        Ok(Whoami::Anonymous) => Err(Box::new(see_other(
            state
                .kratos
                .browser_init_url(
                    FlowKind::Login,
                    &InitOptions {
                        return_to: Some(format!(
                            "{}{}",
                            state.public_url,
                            return_path.trim_start_matches('/')
                        )),
                        ..Default::default()
                    },
                )
                .as_str(),
        ))),
        Err(e) => Err(Box::new(unavailable(lang, e))),
    }
}

/// Mô tả ngắn gọn thiết bị từ User-Agent (trình duyệt và hệ điều hành).
pub fn describe_user_agent(ua: &str) -> String {
    let browser = [
        ("Edg/", "Edge"),
        ("OPR/", "Opera"),
        ("Firefox/", "Firefox"),
        ("Chrome/", "Chrome"),
        ("Safari/", "Safari"),
        ("F-Launcher", "F-Launcher"),
    ]
    .iter()
    .find(|(k, _)| ua.contains(k))
    .map(|(_, v)| *v);
    let os = [
        ("Windows", "Windows"),
        ("Android", "Android"),
        ("iPhone", "iOS"),
        ("iPad", "iPadOS"),
        ("Mac OS X", "macOS"),
        ("Linux", "Linux"),
    ]
    .iter()
    .find(|(k, _)| ua.contains(k))
    .map(|(_, v)| *v);
    match (browser, os) {
        (Some(b), Some(o)) => format!("{b} · {o}"),
        (Some(b), None) => b.to_owned(),
        (None, Some(o)) => o.to_owned(),
        (None, None) if !ua.is_empty() => ua.chars().take(60).collect(),
        (None, None) => String::new(),
    }
}

fn device_row(lang: Lang, s: &Value, current: bool) -> DeviceRow {
    let unknown = lang.t("sessions.unknown").to_owned();
    let device = s["devices"].as_array().and_then(|d| d.last());
    let field = |k: &str| {
        device
            .and_then(|d| d[k].as_str())
            .filter(|v| !v.is_empty())
            .map(str::to_owned)
    };
    DeviceRow {
        id: s["id"].as_str().unwrap_or_default().to_owned(),
        current,
        device: field("user_agent")
            .map(|ua| describe_user_agent(&ua))
            .filter(|d| !d.is_empty())
            .unwrap_or_else(|| unknown.clone()),
        // Bỏ số cổng của địa chỉ IP (Kratos ghi dạng ip:port).
        ip: field("ip_address")
            .map(|ip| match ip.rsplit_once(':') {
                Some((host, port))
                    if port.chars().all(|c| c.is_ascii_digit()) && !host.ends_with(':') =>
                {
                    host.trim_matches(['[', ']']).to_owned()
                }
                _ => ip,
            })
            .unwrap_or_else(|| unknown.clone()),
        location: field("location").unwrap_or_else(|| unknown.clone()),
        signed_in_at: s["authenticated_at"]
            .as_str()
            .map(|t| t.chars().take(19).collect::<String>().replace('T', " ") + " UTC")
            .unwrap_or(unknown),
    }
}

async fn sessions_page(
    state: &AppState,
    lang: Lang,
    headers: &HeaderMap,
    messages: Vec<Msg>,
) -> Response {
    let current = match require_session(state, lang, headers, "/sessions").await {
        Ok(s) => s,
        Err(r) => return *r,
    };
    let others = match state.kratos.other_sessions(cookie(headers)).await {
        Ok(o) => o,
        Err(e) => return unavailable(lang, e),
    };
    let current_id = current["id"].as_str().unwrap_or_default();
    let mut rows = vec![device_row(lang, &current, true)];
    rows.extend(
        others
            .iter()
            .filter(|s| s["id"].as_str() != Some(current_id))
            .map(|s| device_row(lang, s, false)),
    );
    let (token, set) = csrf::issue(headers, state.public_url.scheme() == "https");
    with_cookie(
        html(
            SessionsPage {
                i18n: lang,
                rows,
                messages,
                csrf: token,
            },
            lang,
        ),
        set,
    )
}

/// GET /sessions
pub async fn sessions(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    headers: HeaderMap,
) -> Response {
    sessions_page(&state, lang, &headers, vec![]).await
}

fn form_field(body: &Bytes, name: &str) -> Option<String> {
    form_urlencoded::parse(body)
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.into_owned())
}

/// POST /sessions/{id}/revoke — đăng xuất một thiết bị khác.
pub async fn revoke(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    Path(id): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if !csrf::verify(&headers, form_field(&body, csrf::FIELD).as_deref()) {
        return sessions_page(
            &state,
            lang,
            &headers,
            vec![Msg::error(lang.t("error.csrf"))],
        )
        .await;
    }
    match state.kratos.revoke_session(cookie(&headers), &id).await {
        Ok(true) => {
            sessions_page(
                &state,
                lang,
                &headers,
                vec![Msg::success(lang.t("sessions.revoked"))],
            )
            .await
        }
        Ok(false) => {
            sessions_page(
                &state,
                lang,
                &headers,
                vec![Msg::error(lang.t("error.submit"))],
            )
            .await
        }
        Err(e) => unavailable(lang, e),
    }
}

/// POST /sessions/revoke-others — đăng xuất mọi thiết bị khác.
pub async fn revoke_others(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if !csrf::verify(&headers, form_field(&body, csrf::FIELD).as_deref()) {
        return sessions_page(
            &state,
            lang,
            &headers,
            vec![Msg::error(lang.t("error.csrf"))],
        )
        .await;
    }
    match state.kratos.revoke_other_sessions(cookie(&headers)).await {
        Ok(true) => {
            sessions_page(
                &state,
                lang,
                &headers,
                vec![Msg::success(lang.t("sessions.revoked_others"))],
            )
            .await
        }
        Ok(false) => {
            sessions_page(
                &state,
                lang,
                &headers,
                vec![Msg::error(lang.t("error.submit"))],
            )
            .await
        }
        Err(e) => unavailable(lang, e),
    }
}

/// Phiên được xác thực đủ gần để làm thao tác nhạy cảm.
pub fn is_privileged(session: &Value, now: SystemTime) -> bool {
    session["authenticated_at"]
        .as_str()
        .and_then(|t| humantime::parse_rfc3339(t).ok())
        .and_then(|at| now.duration_since(at).ok())
        .is_some_and(|age| age <= PRIVILEGED_MAX_AGE)
}

/// Người dùng còn đơn xây app chưa hoàn tất không. Đơn hàng có từ nhóm task 12;
/// tới lúc đó hàm này luôn trả `false`.
async fn has_open_orders(_state: &AppState, _identity_id: &str) -> bool {
    false
}

async fn require_privileged(
    state: &AppState,
    lang: Lang,
    headers: &HeaderMap,
) -> Result<Value, Box<Response>> {
    let session = require_session(state, lang, headers, "/account/delete").await?;
    if is_privileged(&session, SystemTime::now()) {
        return Ok(session);
    }
    // Phiên đã lâu: bắt đăng nhập lại rồi quay về trang này.
    Err(Box::new(see_other(
        state
            .kratos
            .browser_init_url(
                FlowKind::Login,
                &InitOptions {
                    return_to: Some(format!("{}account/delete", state.public_url)),
                    refresh: true,
                    ..Default::default()
                },
            )
            .as_str(),
    )))
}

/// GET /account/delete
pub async fn delete_form(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    headers: HeaderMap,
) -> Response {
    if let Err(r) = require_privileged(&state, lang, &headers).await {
        return *r;
    }
    let (token, set) = csrf::issue(&headers, state.public_url.scheme() == "https");
    with_cookie(
        html(
            DeletePage {
                i18n: lang,
                messages: vec![],
                csrf: token,
            },
            lang,
        ),
        set,
    )
}

/// POST /account/delete
pub async fn delete_submit(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let session = match require_privileged(&state, lang, &headers).await {
        Ok(s) => s,
        Err(r) => return *r,
    };
    let (token, _) = csrf::issue(&headers, false);
    let page = |msg: Msg| {
        html(
            DeletePage {
                i18n: lang,
                messages: vec![msg],
                csrf: token.clone(),
            },
            lang,
        )
    };
    if !csrf::verify(&headers, form_field(&body, csrf::FIELD).as_deref()) {
        return page(Msg::error(lang.t("error.csrf")));
    }
    if form_field(&body, "confirm").as_deref() != Some("yes") {
        return page(Msg::error(lang.t("delete.need_confirm")));
    }
    let id = session["identity"]["id"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    if has_open_orders(&state, &id).await {
        return page(Msg::error(lang.t("delete.blocked_orders")));
    }
    if let Err(e) = request_deletion(&state.kratos, &id, SystemTime::now()).await {
        return unavailable(lang, e);
    }
    html(
        MessagePage {
            i18n: lang,
            title: lang.t("delete.title").into(),
            messages: vec![Msg::info(lang.t("delete.requested"))],
            links: vec![("/login".into(), lang.t("home.login").into())],
        },
        lang,
    )
}

/// Đánh dấu chờ xóa và đăng xuất mọi thiết bị.
pub async fn request_deletion(
    kratos: &KratosClient,
    id: &str,
    now: SystemTime,
) -> Result<(), KratosError> {
    let identity = kratos
        .get_identity(id)
        .await?
        .ok_or_else(|| KratosError::Body("không tìm thấy danh tính".into()))?;
    let mut meta = match &identity["metadata_admin"] {
        Value::Object(m) => Value::Object(m.clone()),
        _ => json!({}),
    };
    meta[META_KEY] = json!(humantime::format_rfc3339_seconds(now).to_string());
    kratos.set_metadata_admin(id, &meta).await?;
    kratos.revoke_identity_sessions(id).await
}

/// Nếu tài khoản đang chờ xóa thì hủy yêu cầu. Trả về `true` khi đã hủy.
pub async fn cancel_deletion_if_pending(
    kratos: &KratosClient,
    id: &str,
) -> Result<bool, KratosError> {
    let Some(identity) = kratos.get_identity(id).await? else {
        return Ok(false);
    };
    let mut meta = identity["metadata_admin"].clone();
    let pending = meta.get(META_KEY).is_some();
    if pending {
        if let Some(m) = meta.as_object_mut() {
            m.remove(META_KEY);
        }
        kratos.set_metadata_admin(id, &meta).await?;
    }
    Ok(pending)
}

fn deletion_due(identity: &Value, now: SystemTime) -> bool {
    identity["metadata_admin"][META_KEY]
        .as_str()
        .and_then(|t| humantime::parse_rfc3339(t).ok())
        .and_then(|at| now.duration_since(at).ok())
        .is_some_and(|age| age >= DELETION_GRACE)
}

/// Xóa hẳn các tài khoản đã chờ đủ 30 ngày. Trả về số tài khoản đã xóa.
///
/// Phần dữ liệu ngoài Kratos (đồng ý cho app con, dự án F-Builder, đơn hàng; hóa đơn
/// được giữ) sẽ được xóa ở đây khi các nhóm task tương ứng có dữ liệu.
pub async fn purge_due(kratos: &KratosClient, now: SystemTime) -> Result<usize, KratosError> {
    let mut deleted = 0;
    for identity in kratos.list_identities().await? {
        if deletion_due(&identity, now) {
            if let Some(id) = identity["id"].as_str() {
                if kratos.delete_identity(id).await? {
                    tracing::info!(identity = id, "đã xóa tài khoản sau 30 ngày chờ");
                    deleted += 1;
                }
            }
        }
    }
    Ok(deleted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn describes_common_user_agents() {
        let chrome_win = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0 Safari/537.36";
        assert_eq!(describe_user_agent(chrome_win), "Chrome · Windows");
        let safari_mac = "Mozilla/5.0 (Macintosh; Intel Mac OS X 14_5) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.5 Safari/605.1.15";
        assert_eq!(describe_user_agent(safari_mac), "Safari · macOS");
        let edge =
            "Mozilla/5.0 (Windows NT 10.0) AppleWebKit/537.36 Chrome/131.0 Safari/537.36 Edg/131.0";
        assert_eq!(describe_user_agent(edge), "Edge · Windows");
        assert_eq!(describe_user_agent(""), "");
    }

    #[test]
    fn device_row_strips_port_and_marks_current() {
        let s = json!({"id": "s1", "authenticated_at": "2026-11-09T08:30:00.123Z",
            "devices": [{"ip_address": "203.0.113.5:53200", "user_agent": "Mozilla/5.0 (Windows NT 10.0) Chrome/1", "location": "Hà Nội, VN"}]});
        let r = device_row(Lang::Vi, &s, true);
        assert_eq!(r.ip, "203.0.113.5");
        assert_eq!(r.location, "Hà Nội, VN");
        assert_eq!(r.signed_in_at, "2026-11-09 08:30:00 UTC");
        assert!(r.current);
        let unknown = device_row(Lang::En, &json!({"id": "s2"}), false);
        assert_eq!(unknown.ip, "Unknown");
    }

    #[test]
    fn privileged_only_within_fifteen_minutes() {
        let at = humantime::parse_rfc3339("2026-11-09T08:00:00Z").unwrap();
        let s = json!({"authenticated_at": "2026-11-09T08:00:00.517Z"});
        assert!(is_privileged(&s, at + Duration::from_secs(14 * 60)));
        assert!(!is_privileged(&s, at + Duration::from_secs(16 * 60)));
        assert!(!is_privileged(&json!({}), at));
    }

    #[test]
    fn deletion_is_due_only_after_thirty_days() {
        let at = humantime::parse_rfc3339("2026-11-09T08:00:00Z").unwrap();
        let identity = json!({"metadata_admin": {"deletion_requested_at": "2026-11-09T08:00:00Z"}});
        assert!(!deletion_due(
            &identity,
            at + Duration::from_secs(29 * 24 * 3600)
        ));
        assert!(deletion_due(&identity, at + DELETION_GRACE));
        assert!(!deletion_due(
            &json!({"metadata_admin": null}),
            at + DELETION_GRACE * 2
        ));
    }
}
