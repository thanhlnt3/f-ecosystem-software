//! Xuất dữ liệu cá nhân (spec user-data-consent, "Xuất dữ liệu cá nhân"): một file ZIP
//! chứa JSON của mọi nhóm dữ liệu nền tảng đang lưu. Liên kết tải gửi qua email, hết hạn
//! sau 7 ngày, và chỉ chủ tài khoản đang đăng nhập mới tải được.

use std::{io::Write as _, sync::Arc};

use askama::Template;
use axum::{
    body::Bytes,
    extract::{Path, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{Html, IntoResponse, Response},
};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{connected, csrf, i18n::Lang, store, views::Msg, AppState};

#[derive(Template)]
#[template(path = "export.html")]
struct ExportPage {
    i18n: Lang,
    messages: Vec<Msg>,
    last: Option<(String, String)>,
    csrf: String,
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

fn unavailable(lang: Lang, e: impl std::fmt::Display) -> Response {
    tracing::error!(error = %e, "lỗi khi xuất dữ liệu");
    (StatusCode::SERVICE_UNAVAILABLE, lang.t("error.unavailable")).into_response()
}

async fn page(state: &AppState, lang: Lang, headers: &HeaderMap, messages: Vec<Msg>) -> Response {
    let subject = match connected::current_subject(state, lang, headers, "/account/export").await {
        Ok(s) => s,
        Err(r) => return *r,
    };
    let last = match store::latest_export(&state.db, subject).await {
        Ok(l) => l.map(|e| {
            let status = lang.t(match e.status.as_str() {
                "ready" => "export.status_ready",
                "failed" => "export.status_failed",
                _ => "export.status_pending",
            });
            (connected::format_time(e.created_at), status.to_owned())
        }),
        Err(e) => return unavailable(lang, e),
    };
    let (token, set) = csrf::issue(headers, state.public_url.scheme() == "https");
    let mut res = html(
        ExportPage {
            i18n: lang,
            messages,
            last,
            csrf: token,
        },
        lang,
    );
    if let Some(c) = set {
        res.headers_mut().append(header::SET_COOKIE, c);
    }
    res
}

/// GET /account/export
pub async fn show(State(state): State<Arc<AppState>>, lang: Lang, headers: HeaderMap) -> Response {
    page(&state, lang, &headers, vec![]).await
}

/// POST /account/export: tạo yêu cầu và dựng file ở nền.
pub async fn request(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let token = form_urlencoded::parse(&body)
        .find(|(k, _)| k == csrf::FIELD)
        .map(|(_, v)| v.into_owned());
    if !csrf::verify(&headers, token.as_deref()) {
        return page(
            &state,
            lang,
            &headers,
            vec![Msg::error(lang.t("error.csrf"))],
        )
        .await;
    }
    let subject = match connected::current_subject(&state, lang, &headers, "/account/export").await
    {
        Ok(s) => s,
        Err(r) => return *r,
    };
    let id = match store::create_export(&state.db, subject).await {
        Ok(id) => id,
        Err(e) => return unavailable(lang, e),
    };
    crate::audit::record(
        &state.db,
        crate::audit::Actor::User(subject),
        "data.export_requested",
        Some(&id.to_string()),
        None,
        json!({}),
    )
    .await;
    let st = state.clone();
    tokio::spawn(async move {
        if let Err(e) = build_and_send(&st, id, subject).await {
            tracing::error!(error = %e, "xuất dữ liệu thất bại");
            let _ = store::fail_export(&st.db, id).await;
        }
    });
    page(
        &state,
        lang,
        &headers,
        vec![Msg::info(lang.t("export.requested"))],
    )
    .await
}

fn random_token() -> String {
    let mut buf = [0u8; 32];
    getrandom::fill(&mut buf).expect("nguồn ngẫu nhiên");
    buf.iter().map(|b| format!("{b:02x}")).collect()
}

/// Gom dữ liệu, nén ZIP, lưu, rồi gửi email kèm liên kết tải.
pub async fn build_and_send(state: &AppState, id: Uuid, subject: Uuid) -> Result<(), String> {
    let sid = subject.to_string();
    let identity = state
        .kratos
        .get_identity(&sid)
        .await
        .map_err(|e| e.to_string())?
        .ok_or("không tìm thấy danh tính")?;
    let sessions = state
        .kratos
        .identity_sessions(&sid)
        .await
        .map_err(|e| e.to_string())?;
    let consents = store::consent_history(&state.db, subject)
        .await
        .map_err(|e| e.to_string())?;
    let access = store::access_log(&state.db, subject, None)
        .await
        .map_err(|e| e.to_string())?;
    let connected_apps: Vec<Value> = state
        .hydra
        .consent_sessions(&sid)
        .await
        .map_err(|e| e.to_string())?
        .iter()
        .map(|s| {
            json!({
                "client_id": s["consent_request"]["client"]["client_id"],
                "app_name": s["consent_request"]["client"]["client_name"],
                "granted_scopes": s["grant_scope"],
                "granted_at": s["handled_at"],
            })
        })
        .collect();
    let profile = json!({
        "id": identity["id"],
        "traits": identity["traits"],
        "verifiable_addresses": identity["verifiable_addresses"],
        "created_at": identity["created_at"],
        "updated_at": identity["updated_at"],
    });
    let sessions: Vec<Value> = sessions
        .iter()
        .map(|s| json!({"authenticated_at": s["authenticated_at"], "devices": s["devices"], "active": s["active"]}))
        .collect();

    let files: Vec<(&str, Value)> = vec![
        ("profile.json", profile),
        ("sessions.json", json!(sessions)),
        ("connected_apps.json", json!(connected_apps)),
        ("consents.json", json!(consents)),
        ("access_log_90_days.json", json!(access)),
        // Có dữ liệu từ nhóm task 11 (F-Builder), 12 (đơn hàng), 13 (hóa đơn).
        ("builder_projects.json", json!([])),
        ("orders.json", json!([])),
        ("invoices.json", json!([])),
    ];
    let zip = tokio::task::spawn_blocking(move || -> Result<Vec<u8>, String> {
        let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        w.start_file("README.txt", opts)
            .map_err(|e| e.to_string())?;
        w.write_all(README.as_bytes()).map_err(|e| e.to_string())?;
        for (name, value) in files {
            w.start_file(name, opts).map_err(|e| e.to_string())?;
            let bytes = serde_json::to_vec_pretty(&value).map_err(|e| e.to_string())?;
            w.write_all(&bytes).map_err(|e| e.to_string())?;
        }
        Ok(w.finish().map_err(|e| e.to_string())?.into_inner())
    })
    .await
    .map_err(|e| e.to_string())??;

    let token = random_token();
    store::finish_export(&state.db, &state.data_key, id, &zip, &token)
        .await
        .map_err(|e| e.to_string())?;
    let email = identity["traits"]["email"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    let link = format!("{}account/export/{token}", state.public_url);
    let (subject_line, body) = crate::mailer::templates::export_ready(&link);
    state
        .mailer
        .send(&email, &subject_line, &body)
        .await
        .map_err(|e| e.to_string())
}

const README: &str = "Dữ liệu cá nhân của bạn trên F-Eco / Your personal data on F-Eco\n\n\
profile.json            Hồ sơ / Profile\n\
sessions.json           Phiên đăng nhập / Sign-in sessions\n\
connected_apps.json     Ứng dụng đã kết nối / Connected apps\n\
consents.json           Lịch sử đồng ý / Consent history\n\
access_log_90_days.json Nhật ký app đọc dữ liệu (90 ngày) / App data access log (90 days)\n\
builder_projects.json   Dự án F-Builder / F-Builder projects\n\
orders.json             Đơn xây app / Custom app orders\n\
invoices.json           Hóa đơn / Invoices\n";

/// GET /account/export/{token}
pub async fn download(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    Path(token): Path<String>,
    headers: HeaderMap,
) -> Response {
    let subject = match connected::current_subject(
        &state,
        lang,
        &headers,
        &format!("/account/export/{token}"),
    )
    .await
    {
        Ok(s) => s,
        Err(r) => return *r,
    };
    match store::export_file(&state.db, &state.data_key, subject, &token).await {
        Ok(Some(file)) => {
            crate::audit::record(
                &state.db,
                crate::audit::Actor::User(subject),
                "data.export_downloaded",
                Some(&subject.to_string()),
                None,
                json!({}),
            )
            .await;
            let mut res = file.into_response();
            let h = res.headers_mut();
            h.insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/zip"),
            );
            h.insert(
                header::CONTENT_DISPOSITION,
                HeaderValue::from_static("attachment; filename=\"f-eco-du-lieu-ca-nhan.zip\""),
            );
            res
        }
        Ok(None) => (StatusCode::NOT_FOUND, lang.t("export.expired")).into_response(),
        Err(e) => unavailable(lang, e),
    }
}
