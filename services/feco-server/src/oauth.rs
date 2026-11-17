//! feco-server là "login & consent provider" của Hydra (spec account-identity, "Đăng
//! nhập một lần cho app con"; spec user-data-consent, "Đồng ý theo từng app và từng scope").
//!
//! - `/oauth/login`: dùng phiên Kratos sẵn có; chưa đăng nhập thì chuyển sang trang
//!   đăng nhập rồi quay lại. ID token mang định danh riêng cho từng app (pairwise).
//! - `/oauth/consent`: màn hình đồng ý theo scope; scope bắt buộc được cấp sẵn, scope
//!   không bắt buộc người dùng tự chọn (mặc định không chọn); scope đã cấp từ trước
//!   không hỏi lại. Mỗi lựa chọn được ghi vào `consent_records`.
//! - `/oauth/logout`: chấp nhận yêu cầu đăng xuất từ app.

use std::{
    collections::BTreeSet,
    sync::Arc,
    time::{Duration, SystemTime},
};

use askama::Template;
use axum::{
    body::Bytes,
    extract::{Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{Html, IntoResponse, Response},
};
use feco_core::Scope;
use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

use crate::{
    apps::{self, AppInfo},
    csrf,
    i18n::Lang,
    kratos::{FlowKind, InitOptions, Whoami},
    store::{self, ConsentAction},
    views::Msg,
    AppState,
};

#[derive(Debug, Deserialize)]
pub struct LoginQuery {
    pub login_challenge: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ConsentQuery {
    pub consent_challenge: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct LogoutQuery {
    pub logout_challenge: Option<String>,
}

pub struct ScopeRow {
    pub name: String,
    pub description: String,
    pub purpose: String,
    pub required: bool,
}

#[derive(Template)]
#[template(path = "consent.html")]
struct ConsentPage {
    i18n: Lang,
    app_name: String,
    developer: String,
    developer_verified: bool,
    policy_uri: String,
    rows: Vec<ScopeRow>,
    challenge: String,
    csrf: String,
    content_version: String,
}

#[derive(Template)]
#[template(path = "message.html")]
struct MessagePage {
    i18n: Lang,
    title: String,
    messages: Vec<Msg>,
    links: Vec<(String, String)>,
}

fn see_other(to: &str) -> Response {
    let mut res = StatusCode::SEE_OTHER.into_response();
    if let Ok(v) = HeaderValue::from_str(to) {
        res.headers_mut().insert(header::LOCATION, v);
    }
    res
}

fn error(lang: Lang, status: StatusCode, text: &str) -> Response {
    let page = MessagePage {
        i18n: lang,
        title: lang.t("error.title").into(),
        messages: vec![Msg::error(text)],
        links: vec![("/".into(), lang.t("link.back_home").into())],
    };
    let mut res = match page.render() {
        Ok(h) => Html(h).into_response(),
        Err(_) => lang.t("error.render").into_response(),
    };
    *res.status_mut() = status;
    res
}

fn upstream(lang: Lang, e: impl std::fmt::Display) -> Response {
    tracing::error!(error = %e, "lỗi khi gọi Hydra/Kratos/CSDL");
    error(
        lang,
        StatusCode::SERVICE_UNAVAILABLE,
        lang.t("error.unavailable"),
    )
}

fn cookie(headers: &HeaderMap) -> Option<&str> {
    headers.get(header::COOKIE).and_then(|v| v.to_str().ok())
}

/// Phiên vừa xác thực trong khoảng này coi như đã "đăng nhập lại" cho `prompt=login`, và là mức
/// tối thiểu của `max_age` (tránh vòng lặp: sau khi đăng nhập lại, trình duyệt quay về đây ngay).
const FRESH_LOGIN: Duration = Duration::from_secs(60);

/// Có phải bắt đăng nhập lại trước khi chấp nhận yêu cầu đăng nhập OAuth không: `prompt=login`
/// mà phiên không vừa xác thực, hoặc phiên cũ hơn `max_age` giây. Không rõ lúc xác thực thì
/// coi như cũ.
pub fn needs_reauth(
    request_url: &str,
    authenticated_at: Option<SystemTime>,
    now: SystemTime,
) -> bool {
    let query = request_url.split_once('?').map(|(_, q)| q).unwrap_or("");
    let query = query.split('#').next().unwrap_or("");
    let mut prompt_login = false;
    let mut max_age: Option<u64> = None;
    for (k, v) in form_urlencoded::parse(query.as_bytes()) {
        match k.as_ref() {
            "prompt" => prompt_login |= v.split(' ').any(|p| p == "login"),
            "max_age" => max_age = v.trim().parse().ok(),
            _ => {}
        }
    }
    if !prompt_login && max_age.is_none() {
        return false;
    }
    let Some(age) = authenticated_at.map(|at| now.duration_since(at).unwrap_or(Duration::ZERO))
    else {
        return true;
    };
    if prompt_login && age > FRESH_LOGIN {
        return true;
    }
    max_age.is_some_and(|m| age > Duration::from_secs(m).max(FRESH_LOGIN))
}

/// `acr` (mức đảm bảo xác thực của phiên Kratos) và `amr` (các phương thức đã dùng) cho Hydra.
fn session_acr_amr(session: &Value) -> (Option<String>, Vec<String>) {
    let acr = session["authenticator_assurance_level"]
        .as_str()
        .map(str::to_owned);
    let mut amr: Vec<String> = Vec::new();
    for m in session["authentication_methods"]
        .as_array()
        .into_iter()
        .flatten()
    {
        if let Some(name) = m["method"].as_str() {
            if !amr.iter().any(|x| x == name) {
                amr.push(name.to_owned());
            }
        }
    }
    (acr, amr)
}

/// GET /oauth/login?login_challenge=...
pub async fn login(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    Query(q): Query<LoginQuery>,
    headers: HeaderMap,
) -> Response {
    let Some(challenge) = q.login_challenge else {
        return error(lang, StatusCode::BAD_REQUEST, lang.t("oauth.bad_request"));
    };
    let request = match state.hydra.login_request(&challenge).await {
        Ok(r) => r,
        Err(e) => return upstream(lang, e),
    };
    let client_id = request["client"]["client_id"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    let back_here = format!(
        "{}oauth/login?login_challenge={}",
        state.public_url,
        form_urlencoded::byte_serialize(challenge.as_bytes()).collect::<String>()
    );
    let session = match state.kratos.whoami(cookie(&headers)).await {
        Ok(Whoami::Session(s)) => s,
        Ok(Whoami::NeedsSecondFactor(_)) => {
            let opts = InitOptions {
                return_to: Some(back_here),
                aal: Some("aal2".into()),
                refresh: false,
            };
            return see_other(
                state
                    .kratos
                    .browser_init_url(FlowKind::Login, &opts)
                    .as_str(),
            );
        }
        Ok(Whoami::Anonymous) => {
            let opts = InitOptions {
                return_to: Some(back_here),
                ..Default::default()
            };
            return see_other(
                state
                    .kratos
                    .browser_init_url(FlowKind::Login, &opts)
                    .as_str(),
            );
        }
        Err(e) => return upstream(lang, e),
    };
    // App yêu cầu đăng nhập lại (`prompt=login`) hoặc phiên quá cũ so với `max_age`: bắt
    // đăng nhập lại (Kratos refresh) rồi quay về đúng yêu cầu này.
    let authenticated_at = session["authenticated_at"]
        .as_str()
        .and_then(|t| humantime::parse_rfc3339(t).ok());
    if needs_reauth(
        request["request_url"].as_str().unwrap_or_default(),
        authenticated_at,
        SystemTime::now(),
    ) {
        let opts = InitOptions {
            return_to: Some(back_here),
            aal: None,
            refresh: true,
        };
        return see_other(
            state
                .kratos
                .browser_init_url(FlowKind::Login, &opts)
                .as_str(),
        );
    }
    let subject = session["identity"]["id"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    // Hydra nhớ người đăng nhập trước (skip): chỉ chấp nhận nếu đúng người đang có phiên.
    if request["skip"] == true && request["subject"].as_str() != Some(subject.as_str()) {
        return match state
            .hydra
            .reject_login(&challenge, "session mismatch")
            .await
        {
            Ok(to) => see_other(&to),
            Err(e) => upstream(lang, e),
        };
    }
    let pairwise = state.pairwise.subject(&client_id, &subject);
    let (acr, amr) = session_acr_amr(&session);
    match state
        .hydra
        .accept_login(&challenge, &subject, &pairwise, acr.as_deref(), &amr)
        .await
    {
        Ok(to) => see_other(&to),
        Err(e) => upstream(lang, e),
    }
}

/// Scope đã cấp trước đây cho app (theo phiên đồng ý còn hiệu lực của Hydra).
pub(crate) async fn previously_granted(
    state: &AppState,
    subject: &str,
    client_id: &str,
) -> BTreeSet<String> {
    match state.hydra.consent_sessions(subject).await {
        Ok(sessions) => sessions
            .iter()
            .filter(|s| s["consent_request"]["client"]["client_id"] == client_id)
            .flat_map(|s| s["grant_scope"].as_array().cloned().unwrap_or_default())
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect(),
        Err(e) => {
            tracing::warn!(error = %e, "không đọc được phiên đồng ý cũ");
            BTreeSet::new()
        }
    }
}

pub(crate) fn requested_scopes(request: &Value) -> Vec<String> {
    request["requested_scope"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_str().map(str::to_owned))
        .collect()
}

fn subject_uuid(request: &Value) -> Option<Uuid> {
    request["subject"].as_str().and_then(|s| s.parse().ok())
}

pub(crate) async fn accept_and_record(
    state: &AppState,
    challenge: &str,
    request: &Value,
    app: &AppInfo,
    grant: &[String],
    content_version: &str,
) -> Result<String, String> {
    let subject = subject_uuid(request).ok_or("subject không hợp lệ")?;
    let to = state
        .hydra
        .accept_consent(
            challenge,
            grant,
            &request["requested_access_token_audience"],
        )
        .await
        .map_err(|e| e.to_string())?;
    store::record_consent(
        &state.db,
        subject,
        &app.client_id,
        ConsentAction::Grant,
        grant,
        content_version,
    )
    .await
    .map_err(|e| e.to_string())?;
    Ok(to)
}

/// GET /oauth/consent?consent_challenge=...
pub async fn consent(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    Query(q): Query<ConsentQuery>,
    headers: HeaderMap,
) -> Response {
    let Some(challenge) = q.consent_challenge else {
        return error(lang, StatusCode::BAD_REQUEST, lang.t("oauth.bad_request"));
    };
    let request = match state.hydra.consent_request(&challenge).await {
        Ok(r) => r,
        Err(e) => return upstream(lang, e),
    };
    let app = AppInfo::from_client(&request["client"]);
    let subject = request["subject"].as_str().unwrap_or_default().to_owned();
    let requested = requested_scopes(&request);
    // F-Launcher (desktop và di động) là app bên thứ nhất của nền tảng: không hỏi đồng ý.
    if state.is_launcher_client(&app.client_id) {
        return match accept_and_record(
            &state,
            &challenge,
            &request,
            &app,
            &requested,
            "first-party-launcher",
        )
        .await
        {
            Ok(to) => see_other(&to),
            Err(e) => upstream(lang, e),
        };
    }
    let previous = previously_granted(&state, &subject, &app.client_id).await;
    let to_ask = apps::scopes_to_ask(&app, &requested, &previous);

    // Không có gì mới cần hỏi: cấp lại đúng những gì đã đồng ý.
    if to_ask.is_empty() {
        let grant = apps::grant(&app, &requested, &previous, &BTreeSet::new());
        return match accept_and_record(
            &state,
            &challenge,
            &request,
            &app,
            &grant,
            "previously-granted",
        )
        .await
        {
            Ok(to) => see_other(&to),
            Err(e) => upstream(lang, e),
        };
    }

    let rows = to_ask
        .iter()
        .map(|s| {
            let d = app.declared(*s).expect("đã lọc theo khai báo");
            ScopeRow {
                name: s.as_str().into(),
                description: match lang {
                    Lang::Vi => s.description_vi().into(),
                    Lang::En | Lang::Ja | Lang::Fr => s.description_en().into(),
                },
                purpose: match lang {
                    Lang::Vi => d.purpose_vi.clone(),
                    Lang::En | Lang::Ja | Lang::Fr => d.purpose_en.clone(),
                },
                required: d.required,
            }
        })
        .collect();
    let (token, set) = csrf::issue(&headers, state.public_url.scheme() == "https");
    let page = ConsentPage {
        i18n: lang,
        content_version: app.content_version(lang, &to_ask),
        app_name: app.name.clone(),
        developer: app.developer.clone(),
        developer_verified: app.developer_verified,
        policy_uri: app.policy_uri.clone(),
        rows,
        challenge,
        csrf: token,
    };
    let mut res = match page.render() {
        Ok(h) => Html(h).into_response(),
        Err(e) => return upstream(lang, e),
    };
    if let Some(c) = set {
        res.headers_mut().append(header::SET_COOKIE, c);
    }
    res
}

/// POST /oauth/consent
pub async fn consent_submit(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let form: Vec<(String, String)> = form_urlencoded::parse(&body).into_owned().collect();
    let get = |k: &str| form.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_str());
    if !csrf::verify(&headers, get(csrf::FIELD)) {
        return error(lang, StatusCode::FORBIDDEN, lang.t("error.csrf"));
    }
    let Some(challenge) = get("consent_challenge").map(str::to_owned) else {
        return error(lang, StatusCode::BAD_REQUEST, lang.t("oauth.bad_request"));
    };
    // Luôn đọc lại yêu cầu từ Hydra; không tin danh sách scope trong biểu mẫu.
    let request = match state.hydra.consent_request(&challenge).await {
        Ok(r) => r,
        Err(e) => return upstream(lang, e),
    };
    let app = AppInfo::from_client(&request["client"]);
    let content_version = get("content_version").unwrap_or_default().to_owned();

    if get("action") != Some("accept") {
        let subject = subject_uuid(&request);
        return match state.hydra.reject_consent(&challenge).await {
            Ok(to) => {
                if let Some(subject) = subject {
                    if let Err(e) = store::record_consent(
                        &state.db,
                        subject,
                        &app.client_id,
                        ConsentAction::Deny,
                        &[],
                        &content_version,
                    )
                    .await
                    {
                        tracing::error!(error = %e, "không ghi được bản ghi từ chối");
                    }
                }
                see_other(&to)
            }
            Err(e) => upstream(lang, e),
        };
    }

    let subject = request["subject"].as_str().unwrap_or_default().to_owned();
    let requested = requested_scopes(&request);
    let previous = previously_granted(&state, &subject, &app.client_id).await;
    let chosen: BTreeSet<Scope> = form
        .iter()
        .filter(|(k, v)| v == "on" && k.starts_with("scope_"))
        .filter_map(|(k, _)| k.trim_start_matches("scope_").parse::<Scope>().ok())
        .collect();
    let grant = apps::grant(&app, &requested, &previous, &chosen);
    match accept_and_record(&state, &challenge, &request, &app, &grant, &content_version).await {
        Ok(to) => see_other(&to),
        Err(e) => upstream(lang, e),
    }
}

/// GET /oauth/logout?logout_challenge=...
pub async fn logout(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    Query(q): Query<LogoutQuery>,
) -> Response {
    let Some(challenge) = q.logout_challenge else {
        return error(lang, StatusCode::BAD_REQUEST, lang.t("oauth.bad_request"));
    };
    match state.hydra.accept_logout(&challenge).await {
        Ok(to) => see_other(&to),
        Err(e) => upstream(lang, e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const AUTH: &str = "https://auth.f-eco.example/oauth2/auth?client_id=app&response_type=code";

    #[test]
    fn oauth_login_honors_prompt_login_and_max_age() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_800_000_000);
        let ago = |s: u64| Some(now - Duration::from_secs(s));
        // Không có prompt/max_age: dùng phiên sẵn có.
        assert!(!needs_reauth(AUTH, ago(86_400), now));
        assert!(!needs_reauth(AUTH, None, now));
        // prompt=login: phiên cũ phải đăng nhập lại; vừa đăng nhập lại thì cho qua (không lặp).
        let p = format!("{AUTH}&prompt=login");
        assert!(needs_reauth(&p, ago(600), now));
        assert!(!needs_reauth(&p, ago(5), now));
        assert!(needs_reauth(&p, None, now));
        assert!(needs_reauth(
            &format!("{AUTH}&prompt=consent%20login"),
            ago(600),
            now
        ));
        assert!(!needs_reauth(
            &format!("{AUTH}&prompt=consent"),
            ago(600),
            now
        ));
        // max_age tính bằng giây.
        let m = format!("{AUTH}&max_age=300");
        assert!(!needs_reauth(&m, ago(200), now));
        assert!(needs_reauth(&m, ago(301), now));
        assert!(needs_reauth(&m, None, now));
        // max_age=0 không gây vòng lặp ngay sau khi đăng nhập lại.
        let z = format!("{AUTH}&max_age=0");
        assert!(!needs_reauth(&z, ago(3), now));
        assert!(needs_reauth(&z, ago(120), now));
    }

    #[test]
    fn oauth_accept_login_passes_session_acr_amr() {
        let session = serde_json::json!({
            "authenticator_assurance_level": "aal2",
            "authentication_methods": [
                {"method": "password"}, {"method": "totp"}, {"method": "password"}
            ],
        });
        let (acr, amr) = session_acr_amr(&session);
        assert_eq!(acr.as_deref(), Some("aal2"));
        assert_eq!(amr, ["password", "totp"]);
        assert_eq!(session_acr_amr(&serde_json::json!({})), (None, vec![]));
    }
}
