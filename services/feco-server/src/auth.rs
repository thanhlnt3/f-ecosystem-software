//! Trang đăng ký, đăng nhập, xác minh, khôi phục, cài đặt (spec account-identity).
//!
//! Mỗi trang là một luồng tự phục vụ của Kratos. feco-server dựng trang, nhận biểu
//! mẫu, rồi chuyển tiếp tới Kratos kèm cookie của trình duyệt. Những quy tắc riêng
//! của F-Eco được chèn ở giữa:
//! - đăng nhập sai: làm chậm dần, bắt buộc thử thách chống bot từ lần sai thứ 5
//!   trong 15 phút, và gửi email cảnh báo cho chủ tài khoản;
//! - đăng ký bằng email đã có tài khoản: hiện cùng thông báo như đăng ký mới, và gửi
//!   email báo cho chủ tài khoản, để không lộ việc email đã tồn tại;
//! - đăng nhập lại khi tài khoản đang chờ xóa: hủy yêu cầu xóa.

use std::{sync::Arc, time::Instant};

use askama::Template;
use axum::{
    body::Bytes,
    extract::{Path, Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{Html, IntoResponse, Response},
};
use serde::Deserialize;
use serde_json::Value;

use crate::{
    account, challenge,
    i18n::{self, Lang},
    kratos::{ClientInfo, FlowKind, InitOptions, KratosResponse, Whoami},
    mailer::templates,
    views::{flow_view, ids, FlowView, Msg},
    AppState,
};

#[derive(Debug, Deserialize, Default)]
pub struct FlowQuery {
    pub flow: Option<String>,
    pub return_to: Option<String>,
    pub aal: Option<String>,
    pub refresh: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct ErrorQuery {
    pub id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct HomeQuery {
    pub notice: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct LangQuery {
    pub next: Option<String>,
}

pub struct ChallengeBox {
    pub field: &'static str,
    pub value: &'static str,
    pub label: &'static str,
}

#[derive(Template)]
#[template(path = "flow.html")]
struct FlowPage {
    i18n: Lang,
    view: FlowView,
    challenge: Option<ChallengeBox>,
    links: Vec<(String, String)>,
}

#[derive(Template)]
#[template(path = "message.html")]
struct MessagePage {
    i18n: Lang,
    title: String,
    messages: Vec<Msg>,
    links: Vec<(String, String)>,
}

pub struct HomeUser {
    pub email: String,
    pub name: String,
    pub verified: bool,
}

#[derive(Template)]
#[template(path = "home.html")]
struct HomePage {
    i18n: Lang,
    user: Option<HomeUser>,
    notices: Vec<Msg>,
}

fn render(t: impl Template, lang: Lang) -> Response {
    match t.render() {
        Ok(html) => Html(html).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "lỗi dựng trang");
            (StatusCode::INTERNAL_SERVER_ERROR, lang.t("error.render")).into_response()
        }
    }
}

fn cookie(headers: &HeaderMap) -> Option<&str> {
    headers.get(header::COOKIE).and_then(|v| v.to_str().ok())
}

fn redirect(to: &str, set_cookies: &[HeaderValue]) -> Response {
    let mut res = StatusCode::SEE_OTHER.into_response();
    if let Ok(v) = HeaderValue::from_str(to) {
        res.headers_mut().insert(header::LOCATION, v);
    }
    for c in set_cookies {
        res.headers_mut().append(header::SET_COOKIE, c.clone());
    }
    res
}

fn with_cookies(mut res: Response, set_cookies: &[HeaderValue]) -> Response {
    for c in set_cookies {
        res.headers_mut().append(header::SET_COOKIE, c.clone());
    }
    res
}

fn links_for(lang: Lang, kind: FlowKind) -> Vec<(String, String)> {
    let l = |h: &str, k: &str| (h.to_owned(), lang.t(k).to_owned());
    match kind {
        FlowKind::Login => vec![
            l("/registration", "link.no_account"),
            l("/recovery", "link.forgot"),
        ],
        FlowKind::Registration => vec![l("/login", "link.have_account")],
        FlowKind::Recovery | FlowKind::Verification => vec![l("/login", "link.back_login")],
        FlowKind::Settings => vec![
            l("/", "link.back_home"),
            l("/sessions", "link.sessions"),
            l("/apps", "link.apps"),
            l("/account/export", "link.export"),
            l("/account/delete", "link.delete_account"),
        ],
    }
}

fn error_page(lang: Lang, text: &str) -> Response {
    render(
        MessagePage {
            i18n: lang,
            title: lang.t("error.title").into(),
            messages: vec![Msg::error(text)],
            links: vec![("/".into(), lang.t("link.back_home").into())],
        },
        lang,
    )
}

fn unavailable(lang: Lang, e: impl std::fmt::Display) -> Response {
    tracing::error!(error = %e, "không gọi được Kratos");
    let mut res = error_page(lang, lang.t("error.unavailable"));
    *res.status_mut() = StatusCode::SERVICE_UNAVAILABLE;
    res
}

fn flow_page(
    state: &AppState,
    lang: Lang,
    kind: FlowKind,
    flow: &Value,
    challenge_required: bool,
) -> Response {
    let challenge = challenge_required.then(|| ChallengeBox {
        field: challenge::FIELD,
        value: state.challenge.test_value(),
        label: state.challenge.label(lang),
    });
    render(
        FlowPage {
            i18n: lang,
            view: flow_view(lang, kind, flow),
            challenge,
            links: links_for(lang, kind),
        },
        lang,
    )
}

/// Thông báo sau khi gửi biểu mẫu đăng ký. Giống hệt nhau cho email mới và email đã có
/// tài khoản (spec account-identity, "Email đã được dùng").
fn registration_sent_page(lang: Lang, email: &str) -> Response {
    render(
        MessagePage {
            i18n: lang,
            title: lang.t("reg.sent.title").into(),
            messages: vec![Msg::info(lang.t("reg.sent.body").replace("{email}", email))],
            links: vec![("/login".into(), lang.t("home.login").into())],
        },
        lang,
    )
}

fn init_options(q: &FlowQuery) -> InitOptions {
    InitOptions {
        return_to: q.return_to.clone(),
        aal: q.aal.clone().filter(|a| a == "aal2"),
        refresh: q.refresh.unwrap_or(false),
    }
}

/// GET trang của một luồng. Chưa có luồng thì để Kratos tạo luồng mới.
pub async fn show_flow(
    kind: FlowKind,
    State(state): State<Arc<AppState>>,
    lang: Lang,
    Query(q): Query<FlowQuery>,
    headers: HeaderMap,
) -> Response {
    let Some(id) = q.flow.clone() else {
        return redirect(
            state
                .kratos
                .browser_init_url(kind, &init_options(&q))
                .as_str(),
            &[],
        );
    };
    match state.kratos.get_flow(kind, &id, cookie(&headers)).await {
        Err(e) => unavailable(lang, e),
        Ok(r) if r.status == StatusCode::OK => {
            let challenge = kind == FlowKind::Login
                && login_identifier_from_flow(&r.body).is_some_and(|id| {
                    state
                        .throttle
                        .status(&id, Instant::now())
                        .challenge_required
                });
            flow_page(&state, lang, kind, &r.body, challenge)
        }
        // Luồng hết hạn, không tồn tại, hoặc thiếu cookie CSRF: bắt đầu luồng mới.
        Ok(r) if matches!(r.status.as_u16(), 403 | 404 | 410) => redirect(
            state
                .kratos
                .browser_init_url(kind, &InitOptions::default())
                .as_str(),
            &[],
        ),
        Ok(r) => {
            tracing::warn!(status = %r.status, kind = kind.path(), "Kratos trả mã không mong đợi khi lấy luồng");
            error_page(lang, lang.t("error.form"))
        }
    }
}

fn login_identifier_from_flow(flow: &Value) -> Option<String> {
    flow["ui"]["nodes"]
        .as_array()?
        .iter()
        .find(|n| n["attributes"]["name"] == "identifier")
        .and_then(|n| n["attributes"]["value"].as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}

fn field<'a>(form: &'a [(String, String)], name: &str) -> Option<&'a str> {
    form.iter()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.as_str())
}

/// POST biểu mẫu của một luồng.
pub async fn submit_flow(
    kind: FlowKind,
    State(state): State<Arc<AppState>>,
    lang: Lang,
    Query(q): Query<FlowQuery>,
    client: ClientInfo,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let Some(id) = q.flow else {
        return redirect(&format!("/{}", kind.path()), &[]);
    };
    let mut form: Vec<(String, String)> = form_urlencoded::parse(&body).into_owned().collect();
    let challenge_answer = field(&form, challenge::FIELD).map(str::to_owned);
    // Trường của F-Eco không được chuyển tới Kratos.
    form.retain(|(k, _)| !k.starts_with("feco_"));
    let cookie = cookie(&headers);

    // --- Đăng nhập bằng mật khẩu: làm chậm và thử thách chống bot trước khi chuyển tới Kratos.
    let identifier = (kind == FlowKind::Login)
        .then(|| field(&form, "identifier").map(str::to_owned))
        .flatten();
    if let Some(ident) = &identifier {
        let status = state.throttle.status(ident, Instant::now());
        if status.challenge_required && !state.challenge.verify(challenge_answer.as_deref()).await {
            return match state.kratos.get_flow(kind, &id, cookie).await {
                Ok(r) if r.status == StatusCode::OK => {
                    let mut flow = r.body;
                    if let Some(msgs) = flow["ui"]["messages"].as_array_mut() {
                        msgs.push(serde_json::json!({
                            "id": 0, "type": "error", "text": lang.t("challenge.needed")
                        }));
                    }
                    flow_page(&state, lang, kind, &flow, true)
                }
                Ok(_) => redirect(
                    state
                        .kratos
                        .browser_init_url(kind, &InitOptions::default())
                        .as_str(),
                    &[],
                ),
                Err(e) => unavailable(lang, e),
            };
        }
        if !status.delay.is_zero() {
            tokio::time::sleep(status.delay).await;
        }
    }

    let r = match state.kratos.submit(kind, &id, cookie, &form, &client).await {
        Ok(r) => r,
        Err(e) => return unavailable(lang, e),
    };
    handle_submit_response(&state, lang, kind, &form, identifier.as_deref(), r).await
}

/// Địa chỉ Kratos muốn đưa trình duyệt tới sau khi đăng nhập (`return_to`), nếu nó
/// nằm trong chính feco-server.
fn post_login_target(state: &AppState, body: &Value) -> Option<String> {
    body["continue_with"]
        .as_array()?
        .iter()
        .find(|c| c["action"] == "redirect_browser_to")
        .and_then(|c| c["redirect_browser_to"].as_str())
        .filter(|to| to.starts_with(state.public_url.as_str()))
        .map(str::to_owned)
}

async fn handle_submit_response(
    state: &Arc<AppState>,
    lang: Lang,
    kind: FlowKind,
    form: &[(String, String)],
    identifier: Option<&str>,
    r: KratosResponse,
) -> Response {
    let msg_ids = r.message_ids();
    match (r.status.as_u16(), kind) {
        (200, FlowKind::Login) => {
            if let Some(ident) = identifier {
                state.throttle.reset(ident);
            }
            let mut target = post_login_target(state, &r.body).unwrap_or_else(|| "/".into());
            // Spec: đăng nhập lại trong 30 ngày chờ thì hủy yêu cầu xóa tài khoản.
            if let Some(id) = r.body["session"]["identity"]["id"].as_str() {
                match account::cancel_deletion_if_pending(&state.kratos, id).await {
                    Ok(true) => target = "/?notice=deletion_cancelled".into(),
                    Ok(false) => {}
                    Err(e) => {
                        tracing::error!(error = %e, "không kiểm tra được yêu cầu xóa tài khoản")
                    }
                }
            }
            redirect(&target, &r.set_cookies)
        }
        (200, FlowKind::Registration) => {
            let email = field(form, "traits.email").unwrap_or_default();
            with_cookies(registration_sent_page(lang, email), &r.set_cookies)
        }
        (200, _) => with_cookies(flow_page(state, lang, kind, &r.body, false), &r.set_cookies),
        (400, FlowKind::Registration) if msg_ids.contains(&ids::DUPLICATE_IDENTIFIER) => {
            let email = field(form, "traits.email").unwrap_or_default().to_owned();
            notify_registration_attempt(state, email.clone());
            with_cookies(registration_sent_page(lang, &email), &r.set_cookies)
        }
        (400, FlowKind::Login) => {
            let mut challenge = false;
            if let (Some(ident), true) = (identifier, msg_ids.contains(&ids::CREDENTIALS_INVALID)) {
                let n = state.throttle.record_failure(ident, Instant::now());
                challenge = n >= state.throttle.policy().challenge_after;
                if state.throttle.crossed_threshold(n) {
                    notify_failed_logins(state, ident.to_owned());
                }
            }
            with_cookies(
                flow_page(state, lang, kind, &r.body, challenge),
                &r.set_cookies,
            )
        }
        (400, _) => with_cookies(flow_page(state, lang, kind, &r.body, false), &r.set_cookies),
        (410, _) => redirect(
            state
                .kratos
                .browser_init_url(kind, &InitOptions::default())
                .as_str(),
            &r.set_cookies,
        ),
        (422, _) => match r.body["redirect_browser_to"].as_str() {
            Some(to) => redirect(to, &r.set_cookies),
            None => error_page(lang, lang.t("error.submit")),
        },
        (status, _) => {
            tracing::warn!(status, kind = kind.path(), body = %r.body, "Kratos trả mã không mong đợi");
            with_cookies(error_page(lang, lang.t("error.submit")), &r.set_cookies)
        }
    }
}

/// Gửi email cảnh báo cho chủ tài khoản khi chạm ngưỡng đăng nhập sai. Chạy nền để
/// thời gian phản hồi không cho biết tài khoản có tồn tại hay không.
fn notify_failed_logins(state: &Arc<AppState>, identifier: String) {
    let state = state.clone();
    tokio::spawn(async move {
        match state.kratos.identity_exists(&identifier).await {
            Ok(true) => {
                let (subject, body) = templates::failed_logins(state.public_url.as_str());
                if let Err(e) = state.mailer.send(&identifier, &subject, &body).await {
                    tracing::error!(error = %e, "không gửi được email cảnh báo đăng nhập sai");
                }
            }
            Ok(false) => {}
            Err(e) => tracing::error!(error = %e, "không kiểm tra được tài khoản"),
        }
    });
}

fn notify_registration_attempt(state: &Arc<AppState>, email: String) {
    let state = state.clone();
    tokio::spawn(async move {
        let (subject, body) = templates::registration_attempt(state.public_url.as_str());
        if let Err(e) = state.mailer.send(&email, &subject, &body).await {
            tracing::error!(error = %e, "không gửi được email báo đăng ký trùng");
        }
    });
}

/// Trang chủ: cho biết đã đăng nhập chưa và email đã xác minh chưa.
pub async fn home(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    Query(q): Query<HomeQuery>,
    headers: HeaderMap,
) -> Response {
    let notices = match q.notice.as_deref() {
        Some("deletion_cancelled") => vec![Msg::success(lang.t("home.deletion_cancelled"))],
        _ => vec![],
    };
    match state.kratos.whoami(cookie(&headers)).await {
        Err(e) => unavailable(lang, e),
        Ok(Whoami::NeedsSecondFactor(to)) => redirect(&to, &[]),
        Ok(Whoami::Anonymous) => render(
            HomePage {
                i18n: lang,
                user: None,
                notices,
            },
            lang,
        ),
        Ok(Whoami::Session(session)) => {
            let identity = &session["identity"];
            let email = identity["traits"]["email"]
                .as_str()
                .unwrap_or_default()
                .to_owned();
            let verified = identity["verifiable_addresses"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|a| a["value"].as_str() == Some(email.as_str()) && a["verified"] == true);
            render(
                HomePage {
                    i18n: lang,
                    user: Some(HomeUser {
                        name: identity["traits"]["name"]
                            .as_str()
                            .unwrap_or_default()
                            .to_owned(),
                        email,
                        verified,
                    }),
                    notices,
                },
                lang,
            )
        }
    }
}

pub async fn logout(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    headers: HeaderMap,
) -> Response {
    match state.kratos.logout_url(cookie(&headers)).await {
        Ok(Some(url)) => redirect(&url, &[]),
        Ok(None) => redirect("/", &[]),
        Err(e) => unavailable(lang, e),
    }
}

pub async fn error(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    Query(q): Query<ErrorQuery>,
) -> Response {
    let Some(id) = q.id else {
        return error_page(lang, lang.t("error.generic"));
    };
    match state.kratos.error(&id).await {
        Ok(e) => {
            tracing::warn!(error = %e, "lỗi tự phục vụ của Kratos");
            let reason = e["error"]["reason"]
                .as_str()
                .or_else(|| e["error"]["message"].as_str())
                .map(str::to_owned)
                .unwrap_or_else(|| lang.t("error.generic").to_owned());
            error_page(lang, &reason)
        }
        Err(err) => unavailable(lang, err),
    }
}

/// GET /lang/{vi|en}?next=/đường-dẫn — đổi ngôn ngữ giao diện (lưu cookie 1 năm).
pub async fn set_lang(Path(code): Path<String>, Query(q): Query<LangQuery>) -> Response {
    let lang = Lang::parse(&code).unwrap_or_default();
    // Chỉ quay về đường dẫn nội bộ, tránh chuyển hướng mở.
    let next = q
        .next
        .filter(|n| n.starts_with('/') && !n.starts_with("//"))
        .unwrap_or_else(|| "/".into());
    let cookie = format!(
        "{}={}; Path=/; Max-Age=31536000; SameSite=Lax",
        i18n::COOKIE,
        lang.code()
    );
    redirect(
        &next,
        &[HeaderValue::from_str(&cookie).expect("cookie hợp lệ")],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn identifier_is_read_from_login_flow() {
        let flow = json!({"ui": {"nodes": [
            {"attributes": {"name": "identifier", "value": "lan@example.com"}},
            {"attributes": {"name": "password"}}
        ]}});
        assert_eq!(
            login_identifier_from_flow(&flow).as_deref(),
            Some("lan@example.com")
        );
        assert_eq!(login_identifier_from_flow(&json!({})), None);
    }

    #[test]
    fn only_aal2_is_passed_through() {
        let q = FlowQuery {
            aal: Some("aal3".into()),
            ..Default::default()
        };
        assert_eq!(init_options(&q).aal, None);
        let q = FlowQuery {
            aal: Some("aal2".into()),
            ..Default::default()
        };
        assert_eq!(init_options(&q).aal.as_deref(), Some("aal2"));
    }

    #[tokio::test]
    async fn set_lang_rejects_open_redirects() {
        let res = set_lang(
            Path("en".into()),
            Query(LangQuery {
                next: Some("//evil.example".into()),
            }),
        )
        .await;
        assert_eq!(res.headers()["location"], "/");
        assert!(res.headers()["set-cookie"]
            .to_str()
            .unwrap()
            .starts_with("feco_lang=en"));
        let res = set_lang(
            Path("vi".into()),
            Query(LangQuery {
                next: Some("/settings".into()),
            }),
        )
        .await;
        assert_eq!(res.headers()["location"], "/settings");
    }
}
