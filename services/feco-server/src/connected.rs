//! Trang "Ứng dụng đã kết nối" (spec user-data-consent, "Xem và thu hồi quyền",
//! "Nhật ký truy cập dữ liệu").

use std::{collections::HashMap, sync::Arc};

use askama::Template;
use axum::{
    body::Bytes,
    extract::{Path, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{Html, IntoResponse, Response},
};
use feco_core::Scope;
use serde_json::{json, Value};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{
    apps::{AppInfo, PROTOCOL_SCOPES},
    csrf,
    i18n::Lang,
    kratos::{FlowKind, InitOptions, Whoami},
    store::{self, ConsentAction},
    views::Msg,
    AppState,
};

pub struct AppRow {
    pub client_id: String,
    pub name: String,
    pub developer: String,
    pub scopes: Vec<String>,
    pub granted_at: String,
    pub last_access: String,
}

pub struct LogRow {
    pub at: String,
    pub scopes: String,
}

#[derive(Template)]
#[template(path = "apps.html")]
struct AppsPage {
    i18n: Lang,
    rows: Vec<AppRow>,
    messages: Vec<Msg>,
    csrf: String,
}

#[derive(Template)]
#[template(path = "app_detail.html")]
struct AppDetailPage {
    i18n: Lang,
    name: String,
    log: Vec<LogRow>,
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

fn see_other(to: &str) -> Response {
    let mut res = StatusCode::SEE_OTHER.into_response();
    if let Ok(v) = HeaderValue::from_str(to) {
        res.headers_mut().insert(header::LOCATION, v);
    }
    res
}

fn unavailable(lang: Lang, e: impl std::fmt::Display) -> Response {
    tracing::error!(error = %e, "lỗi khi gọi Hydra/Kratos/CSDL");
    (StatusCode::SERVICE_UNAVAILABLE, lang.t("error.unavailable")).into_response()
}

pub fn format_time(t: OffsetDateTime) -> String {
    let f = time::macros::format_description!("[year]-[month]-[day] [hour]:[minute] UTC");
    t.to_offset(time::UtcOffset::UTC)
        .format(&f)
        .unwrap_or_default()
}

/// Id danh tính của phiên hiện tại, hoặc phản hồi chuyển hướng tới đăng nhập.
pub async fn current_subject(
    state: &AppState,
    lang: Lang,
    headers: &HeaderMap,
    return_path: &str,
) -> Result<Uuid, Box<Response>> {
    let cookie = headers.get(header::COOKIE).and_then(|v| v.to_str().ok());
    match state.kratos.whoami(cookie).await {
        Ok(Whoami::Session(s)) => s["identity"]["id"]
            .as_str()
            .and_then(|id| id.parse().ok())
            .ok_or_else(|| Box::new(unavailable(lang, "id danh tính không hợp lệ"))),
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

fn data_scope_names(lang: Lang, scopes: &Value) -> Vec<String> {
    scopes
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_str())
        .filter(|s| !PROTOCOL_SCOPES.contains(s))
        .filter_map(|s| s.parse::<Scope>().ok())
        .map(|s| match lang {
            Lang::Vi => s.description_vi().to_owned(),
            Lang::En | Lang::Ja => s.description_en().to_owned(),
        })
        .collect()
}

async fn apps_page(
    state: &AppState,
    lang: Lang,
    headers: &HeaderMap,
    messages: Vec<Msg>,
) -> Response {
    let subject = match current_subject(state, lang, headers, "/apps").await {
        Ok(s) => s,
        Err(r) => return *r,
    };
    let sessions = match state.hydra.consent_sessions(&subject.to_string()).await {
        Ok(s) => s,
        Err(e) => return unavailable(lang, e),
    };
    let last: HashMap<String, OffsetDateTime> = match store::last_access(&state.db, subject).await {
        Ok(v) => v.into_iter().collect(),
        Err(e) => return unavailable(lang, e),
    };
    let never = lang.t("apps.never").to_owned();
    let rows = sessions
        .iter()
        .map(|s| {
            let app = AppInfo::from_client(&s["consent_request"]["client"]);
            AppRow {
                scopes: data_scope_names(lang, &s["grant_scope"]),
                granted_at: s["handled_at"]
                    .as_str()
                    .map(|t| t.chars().take(16).collect::<String>().replace('T', " ") + " UTC")
                    .unwrap_or_default(),
                last_access: last
                    .get(&app.client_id)
                    .map(|t| format_time(*t))
                    .unwrap_or_else(|| never.clone()),
                client_id: app.client_id,
                name: app.name,
                developer: app.developer,
            }
        })
        .collect();
    let (token, set) = csrf::issue(headers, state.public_url.scheme() == "https");
    let mut res = html(
        AppsPage {
            i18n: lang,
            rows,
            messages,
            csrf: token,
        },
        lang,
    );
    if let Some(c) = set {
        res.headers_mut().append(header::SET_COOKIE, c);
    }
    res
}

/// GET /apps
pub async fn list(State(state): State<Arc<AppState>>, lang: Lang, headers: HeaderMap) -> Response {
    apps_page(&state, lang, &headers, vec![]).await
}

/// POST /apps/{client_id}/revoke
pub async fn revoke(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    Path(client_id): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let token = form_urlencoded::parse(&body)
        .find(|(k, _)| k == csrf::FIELD)
        .map(|(_, v)| v.into_owned());
    if !csrf::verify(&headers, token.as_deref()) {
        return apps_page(
            &state,
            lang,
            &headers,
            vec![Msg::error(lang.t("error.csrf"))],
        )
        .await;
    }
    let subject = match current_subject(&state, lang, &headers, "/apps").await {
        Ok(s) => s,
        Err(r) => return *r,
    };
    if let Err(e) = revoke_app(&state, subject, &client_id).await {
        return unavailable(lang, e);
    }
    apps_page(
        &state,
        lang,
        &headers,
        vec![Msg::success(lang.t("apps.revoked"))],
    )
    .await
}

/// Thu hồi quyền của một app: Hydra xóa đồng ý và mọi token của app cho người dùng
/// này; ghi bản ghi thu hồi; báo cho app qua webhook `user.revoked`.
pub async fn revoke_app(state: &AppState, subject: Uuid, client_id: &str) -> Result<(), String> {
    state
        .hydra
        .revoke_consent(&subject.to_string(), Some(client_id))
        .await
        .map_err(|e| e.to_string())?;
    store::record_consent(
        &state.db,
        subject,
        client_id,
        ConsentAction::Revoke,
        &[],
        "",
    )
    .await
    .map_err(|e| e.to_string())?;
    let payload = event_payload(state, "user.revoked", client_id, subject);
    store::enqueue_event(&state.db, client_id, "user.revoked", &payload)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Nội dung sự kiện gửi cho app: chỉ có định danh riêng của app, không có email hay tên.
pub fn event_payload(state: &AppState, event: &str, client_id: &str, subject: Uuid) -> Value {
    json!({
        "event": event,
        "client_id": client_id,
        "sub": state.pairwise.subject(client_id, &subject.to_string()),
        "occurred_at": OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap_or_default(),
    })
}

/// GET /apps/{client_id}: nhật ký truy cập 90 ngày của một app.
pub async fn detail(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    Path(client_id): Path<String>,
    headers: HeaderMap,
) -> Response {
    let subject = match current_subject(&state, lang, &headers, &format!("/apps/{client_id}")).await
    {
        Ok(s) => s,
        Err(r) => return *r,
    };
    let name = match state.hydra.get_client(&client_id).await {
        Ok(Some(c)) => AppInfo::from_client(&c).name,
        Ok(None) => client_id.clone(),
        Err(e) => return unavailable(lang, e),
    };
    let entries = match store::access_log(&state.db, subject, Some(&client_id)).await {
        Ok(v) => v,
        Err(e) => return unavailable(lang, e),
    };
    let log = entries
        .into_iter()
        .map(|e| LogRow {
            at: format_time(e.created_at),
            scopes: data_scope_names(lang, &json!(e.scopes)).join(", "),
        })
        .collect();
    html(
        AppDetailPage {
            i18n: lang,
            name,
            log,
        },
        lang,
    )
}
