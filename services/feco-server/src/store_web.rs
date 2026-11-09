//! F-Store trên web và API danh mục (spec app-catalog).
//!
//! - `GET /store`, `GET /store/{id}`: danh mục, tìm kiếm, bộ lọc, trang chi tiết (scope dữ liệu
//!   kèm mục đích trước nút cài, license, chính sách quyền riêng tư, lịch sử thay đổi).
//! - `POST /store/{id}/reviews`, `/reviews/{rid}/reply`, `/report`: đánh giá (chỉ người đã cài),
//!   nhà phát triển trả lời, báo cáo vi phạm.
//! - `GET /v1/catalog`, `GET /v1/catalog/{id}`: cùng dữ liệu dạng JSON cho launcher và SDK.
//! - `POST /v1/catalog/installs`: launcher báo đã cài (điều kiện để đánh giá).

use std::sync::Arc;

use askama::Template;
use axum::{
    body::Bytes,
    extract::{Path, Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{Html, IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{
    api::api_error,
    apps::AppInfo,
    catalog::{self, Detail, Filters, Listing, Price, ReplyOutcome, Review, ReviewOutcome},
    connected::{current_subject, format_time},
    csrf,
    i18n::Lang,
    kratos::Whoami,
    mailer::MailError,
    views::Msg,
    AppState,
};

#[derive(Debug, Default, Deserialize)]
pub struct CatalogQuery {
    pub q: Option<String>,
    pub category: Option<String>,
    pub os: Option<String>,
    pub price: Option<String>,
    pub verified: Option<String>,
    /// Thông báo sau khi gửi biểu mẫu (khóa cố định, không phản chiếu nội dung người dùng).
    pub ok: Option<String>,
}

impl CatalogQuery {
    pub fn filters(&self) -> Filters {
        let some = |v: &Option<String>| {
            v.as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
        };
        Filters {
            q: some(&self.q),
            category: some(&self.category),
            os: some(&self.os),
            price: match self.price.as_deref() {
                Some("free") => Some(Price::Free),
                Some("paid") => Some(Price::Paid),
                _ => None,
            },
            verified_only: matches!(self.verified.as_deref(), Some("1" | "true" | "on")),
        }
    }
}

pub struct Card {
    pub id: String,
    pub name: String,
    pub developer: String,
    pub verified: bool,
    pub first_party: bool,
    pub summary: String,
    pub price: String,
    pub rating: String,
    pub platforms: String,
}

pub struct ScopeLine {
    pub text: String,
    pub required: bool,
}

pub struct VersionLine {
    pub version: String,
    pub platform: String,
    pub date: String,
    pub size: String,
    pub changelog: String,
}

pub struct ReviewLine {
    pub id: String,
    pub stars: String,
    pub author: String,
    pub date: String,
    pub body: String,
    pub reply: Option<String>,
}

#[derive(Template)]
#[template(path = "store_list.html")]
struct ListPage {
    i18n: Lang,
    cards: Vec<Card>,
    q: String,
    category: String,
    categories: Vec<String>,
    os: String,
    price: String,
    verified: bool,
}

#[derive(Template)]
#[template(path = "store_app.html")]
struct AppPage {
    i18n: Lang,
    card: Card,
    description: String,
    license: String,
    policy_uri: String,
    screenshots: Vec<String>,
    scopes: Vec<ScopeLine>,
    latest: String,
    updated: String,
    size: String,
    versions: Vec<VersionLine>,
    reviews: Vec<ReviewLine>,
    messages: Vec<Msg>,
    signed_in: bool,
    is_owner: bool,
    csrf: String,
    reasons: Vec<(String, String)>,
}

fn html(t: impl Template, lang: Lang, status: StatusCode) -> Response {
    match t.render() {
        Ok(h) => (status, Html(h)).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "lỗi dựng trang");
            (StatusCode::INTERNAL_SERVER_ERROR, lang.t("error.render")).into_response()
        }
    }
}

fn unavailable(lang: Lang, e: impl std::fmt::Display) -> Response {
    tracing::error!(error = %e, "lỗi khi đọc danh mục");
    (StatusCode::SERVICE_UNAVAILABLE, lang.t("error.unavailable")).into_response()
}

fn api_unavailable(e: impl std::fmt::Display) -> Response {
    tracing::error!(error = %e, "lỗi khi đọc danh mục");
    api_error(
        StatusCode::SERVICE_UNAVAILABLE,
        "temporarily_unavailable",
        "The service is temporarily unavailable.",
        "Dịch vụ đang tạm gián đoạn.",
    )
}

fn see_other(to: &str) -> Response {
    let mut res = StatusCode::SEE_OTHER.into_response();
    if let Ok(v) = HeaderValue::from_str(to) {
        res.headers_mut().insert(header::LOCATION, v);
    }
    res
}

pub fn platform_label(p: &str) -> String {
    let (os, arch) = p.split_once('-').unwrap_or((p, ""));
    let os = match os {
        "windows" => "Windows",
        "macos" => "macOS",
        "linux" => "Linux",
        other => other,
    };
    if arch.is_empty() {
        os.to_owned()
    } else {
        format!("{os} ({arch})")
    }
}

fn size_label(bytes: i64) -> String {
    format!("{:.1} MB", bytes.max(0) as f64 / 1_048_576.0)
}

fn price_label(lang: Lang, vnd: u64) -> String {
    if vnd == 0 {
        lang.t("store.price_free").to_owned()
    } else {
        feco_core::Vnd::new(vnd).to_string()
    }
}

fn card(lang: Lang, l: &Listing) -> Card {
    Card {
        id: l.id.clone(),
        name: l.name.clone(),
        developer: l.developer.clone(),
        verified: l.developer_verified,
        first_party: l.first_party,
        summary: l.summary.clone(),
        price: price_label(lang, l.price_vnd),
        rating: match l.rating_avg {
            Some(avg) => format!("★ {avg:.1} ({})", l.rating_count),
            None => lang.t("store.no_ratings").to_owned(),
        },
        platforms: l
            .platforms
            .iter()
            .map(|p| platform_label(p))
            .collect::<Vec<_>>()
            .join(", "),
    }
}

fn scope_lines(lang: Lang, app: &AppInfo) -> Vec<ScopeLine> {
    app.scopes
        .iter()
        .map(|d| {
            let (desc, purpose) = match lang {
                Lang::Vi => (d.scope.description_vi(), d.purpose_vi.as_str()),
                Lang::En => (d.scope.description_en(), d.purpose_en.as_str()),
            };
            let flag = if d.required {
                lang.t("store.required")
            } else {
                lang.t("store.optional")
            };
            ScopeLine {
                text: format!("{desc} ({flag}): {purpose}"),
                required: d.required,
            }
        })
        .collect()
}

fn review_lines(reviews: &[Review]) -> Vec<ReviewLine> {
    reviews
        .iter()
        .map(|r| ReviewLine {
            id: r.id.to_string(),
            stars: format!(
                "{}{}",
                "★".repeat(r.rating.clamp(0, 5) as usize),
                "☆".repeat(5 - r.rating.clamp(0, 5) as usize)
            ),
            author: r.author_name.clone(),
            date: format_time(r.created_at),
            body: r.body.clone(),
            reply: r.reply_body.clone(),
        })
        .collect()
}

/// Người đang xem (không chuyển hướng nếu chưa đăng nhập).
async fn viewer(state: &AppState, headers: &HeaderMap) -> Option<(Uuid, Value)> {
    let cookie = headers.get(header::COOKIE).and_then(|v| v.to_str().ok());
    match state.kratos.whoami(cookie).await {
        Ok(Whoami::Session(s)) => {
            let id = s["identity"]["id"].as_str()?.parse().ok()?;
            Some((id, s["identity"].clone()))
        }
        _ => None,
    }
}

/// GET /store
pub async fn list(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    Query(q): Query<CatalogQuery>,
) -> Response {
    let filters = q.filters();
    let (apps, categories) = match tokio::try_join!(
        catalog::search_cached(&state.db, &state.catalog, &filters),
        catalog::categories(&state.db)
    ) {
        Ok(r) => r,
        Err(e) => return unavailable(lang, e),
    };
    html(
        ListPage {
            i18n: lang,
            cards: apps.iter().map(|a| card(lang, a)).collect(),
            q: q.q.clone().unwrap_or_default(),
            category: q.category.clone().unwrap_or_default(),
            categories,
            os: q.os.clone().unwrap_or_default(),
            price: q.price.clone().unwrap_or_default(),
            verified: filters.verified_only,
        },
        lang,
        StatusCode::OK,
    )
}

async fn app_page(
    state: &AppState,
    lang: Lang,
    headers: &HeaderMap,
    id: &str,
    messages: Vec<Msg>,
    status: StatusCode,
) -> Response {
    let detail = match catalog::detail(&state.db, id).await {
        Ok(Some(d)) => d,
        Ok(None) => {
            return (StatusCode::NOT_FOUND, lang.t("store.not_found")).into_response();
        }
        Err(e) => return unavailable(lang, e),
    };
    let client = match state.hydra.get_client(&detail.listing.client_id).await {
        Ok(c) => c.unwrap_or(Value::Null),
        Err(e) => return unavailable(lang, e),
    };
    let reviews = match catalog::reviews(&state.db, id).await {
        Ok(r) => r,
        Err(e) => return unavailable(lang, e),
    };
    let viewer = viewer(state, headers).await;
    let app = AppInfo::from_client(&client);
    let (token, set_cookie) = csrf::issue(headers, state.public_url.scheme() == "https");
    let page = page_from(
        lang,
        &detail,
        &app,
        &reviews,
        viewer.as_ref().map(|v| v.0),
        token,
        messages,
    );
    let mut res = html(page, lang, status);
    if let Some(c) = set_cookie {
        res.headers_mut().append(header::SET_COOKIE, c);
    }
    res
}

fn page_from(
    lang: Lang,
    d: &Detail,
    app: &AppInfo,
    reviews: &[Review],
    viewer: Option<Uuid>,
    csrf: String,
    messages: Vec<Msg>,
) -> AppPage {
    let latest = d.versions.first();
    AppPage {
        i18n: lang,
        card: card(lang, &d.listing),
        description: d.description.clone(),
        license: d.license.clone(),
        policy_uri: app.policy_uri.clone(),
        screenshots: d.screenshots.clone(),
        scopes: scope_lines(lang, app),
        latest: d.listing.latest_version.clone(),
        updated: format_time(d.listing.updated_at),
        size: latest.map(|v| size_label(v.size_bytes)).unwrap_or_default(),
        versions: d
            .versions
            .iter()
            .map(|v| VersionLine {
                version: v.version.clone(),
                platform: platform_label(&v.platform),
                date: format_time(v.released_at),
                size: size_label(v.size_bytes),
                changelog: v.changelog.clone(),
            })
            .collect(),
        reviews: review_lines(reviews),
        messages,
        signed_in: viewer.is_some(),
        is_owner: viewer.is_some() && viewer == d.owner_subject,
        csrf,
        reasons: catalog::REPORT_REASONS
            .iter()
            .map(|r| ((*r).to_owned(), lang.t(&format!("reason.{r}")).to_owned()))
            .collect(),
    }
}

/// GET /store/{id}
pub async fn show(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    Path(id): Path<String>,
    Query(q): Query<CatalogQuery>,
    headers: HeaderMap,
) -> Response {
    let messages = match q.ok.as_deref() {
        Some("review") => vec![Msg::success(lang.t("store.ok_review"))],
        Some("reply") => vec![Msg::success(lang.t("store.ok_reply"))],
        Some("report") => vec![Msg::success(lang.t("store.ok_report"))],
        _ => vec![],
    };
    app_page(&state, lang, &headers, &id, messages, StatusCode::OK).await
}

fn form(body: &Bytes) -> Vec<(String, String)> {
    form_urlencoded::parse(body).into_owned().collect()
}

fn field<'a>(form: &'a [(String, String)], k: &str) -> Option<&'a str> {
    form.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_str())
}

fn display_name(identity: &Value) -> String {
    let t = &identity["traits"];
    t["name"]
        .as_str()
        .filter(|n| !n.trim().is_empty())
        .map(str::to_owned)
        .or_else(|| {
            t["email"]
                .as_str()
                .and_then(|e| e.split('@').next())
                .map(str::to_owned)
        })
        .unwrap_or_else(|| "F-Eco".into())
}

/// POST /store/{id}/reviews
pub async fn submit_review(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    Path(id): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let f = form(&body);
    if !csrf::verify(&headers, field(&f, csrf::FIELD)) {
        return (StatusCode::FORBIDDEN, lang.t("error.csrf")).into_response();
    }
    let back = format!("/store/{id}");
    let subject = match current_subject(&state, lang, &headers, &back).await {
        Ok(s) => s,
        Err(r) => return *r,
    };
    let name = match state.kratos.get_identity(&subject.to_string()).await {
        Ok(Some(identity)) => display_name(&identity),
        Ok(None) => "F-Eco".into(),
        Err(e) => return unavailable(lang, e),
    };
    let rating = field(&f, "rating")
        .and_then(|r| r.parse().ok())
        .unwrap_or(0);
    let text = field(&f, "body").unwrap_or_default();
    match catalog::upsert_review(&state.db, &state.catalog, &id, subject, &name, rating, text).await
    {
        Ok(ReviewOutcome::Saved(_)) => see_other(&format!("/store/{id}?ok=review#reviews")),
        Ok(ReviewOutcome::NotInstalled) => {
            let msg = vec![Msg::error(lang.t("store.need_install"))];
            app_page(&state, lang, &headers, &id, msg, StatusCode::FORBIDDEN).await
        }
        Ok(ReviewOutcome::Invalid) => {
            let msg = vec![Msg::error(lang.t("store.invalid"))];
            app_page(&state, lang, &headers, &id, msg, StatusCode::BAD_REQUEST).await
        }
        Err(e) => unavailable(lang, e),
    }
}

/// POST /store/{id}/reviews/{rid}/reply
pub async fn submit_reply(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    Path((id, rid)): Path<(String, Uuid)>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let f = form(&body);
    if !csrf::verify(&headers, field(&f, csrf::FIELD)) {
        return (StatusCode::FORBIDDEN, lang.t("error.csrf")).into_response();
    }
    let subject = match current_subject(&state, lang, &headers, &format!("/store/{id}")).await {
        Ok(s) => s,
        Err(r) => return *r,
    };
    let text = field(&f, "body").unwrap_or_default();
    if text.trim().is_empty() || text.chars().count() > 4000 {
        let msg = vec![Msg::error(lang.t("store.invalid"))];
        return app_page(&state, lang, &headers, &id, msg, StatusCode::BAD_REQUEST).await;
    }
    match catalog::reply(&state.db, &id, rid, subject, text).await {
        Ok(ReplyOutcome::Saved) => see_other(&format!("/store/{id}?ok=reply#review-{rid}")),
        Ok(ReplyOutcome::NotOwner) => {
            let msg = vec![Msg::error(lang.t("store.not_owner"))];
            app_page(&state, lang, &headers, &id, msg, StatusCode::FORBIDDEN).await
        }
        Ok(ReplyOutcome::NotFound) => {
            (StatusCode::NOT_FOUND, lang.t("store.not_found")).into_response()
        }
        Err(e) => unavailable(lang, e),
    }
}

/// POST /store/{id}/report
pub async fn submit_report(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    Path(id): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let f = form(&body);
    if !csrf::verify(&headers, field(&f, csrf::FIELD)) {
        return (StatusCode::FORBIDDEN, lang.t("error.csrf")).into_response();
    }
    let subject = match current_subject(&state, lang, &headers, &format!("/store/{id}")).await {
        Ok(s) => s,
        Err(r) => return *r,
    };
    let review = field(&f, "review_id").and_then(|r| r.parse::<Uuid>().ok());
    let reason = field(&f, "reason").unwrap_or_default();
    let details = field(&f, "details").unwrap_or_default();
    match catalog::report(&state.db, subject, &id, review, reason, details).await {
        Ok(Some(_)) => see_other(&format!("/store/{id}?ok=report")),
        Ok(None) => {
            let msg = vec![Msg::error(lang.t("store.invalid"))];
            app_page(&state, lang, &headers, &id, msg, StatusCode::BAD_REQUEST).await
        }
        Err(e) => unavailable(lang, e),
    }
}

/// Quản trị viên xử lý một báo cáo và gửi email kết quả cho người báo cáo. Giao diện quản trị
/// ở nhóm 14; tạm gọi qua công cụ `feco-catalog`.
pub async fn resolve_and_notify(
    state: &AppState,
    report: Uuid,
    upheld: bool,
    resolution: &str,
) -> Result<bool, String> {
    let Some(r) = catalog::resolve_report(&state.db, &state.catalog, report, upheld, resolution)
        .await
        .map_err(|e| e.to_string())?
    else {
        return Ok(false);
    };
    let email = state
        .kratos
        .get_identity(&r.reporter.to_string())
        .await
        .map_err(|e| e.to_string())?
        .and_then(|i| i["traits"]["email"].as_str().map(str::to_owned));
    if let Some(email) = email {
        let (vi, en) = if upheld {
            ("đã được xác nhận và xử lý", "was upheld and acted on")
        } else {
            (
                "đã được xem xét nhưng không vi phạm chính sách",
                "was reviewed and found not to break our policies",
            )
        };
        let target = if r.review_id.is_some() {
            "nhận xét / review"
        } else {
            "app"
        };
        let body = format!(
            "Báo cáo của bạn về {target} \"{app}\" {vi}.\nGhi chú: {resolution}\n\n\
             Your report about the {target} \"{app}\" {en}.\nNote: {resolution}\n",
            app = r.app_id
        );
        state
            .mailer
            .send(
                &email,
                "Kết quả xử lý báo cáo / Your report was reviewed",
                &body,
            )
            .await
            .map_err(|e: MailError| e.to_string())?;
    }
    Ok(true)
}

/// GET /v1/catalog
pub async fn api_list(
    State(state): State<Arc<AppState>>,
    Query(q): Query<CatalogQuery>,
) -> Response {
    match catalog::search_cached(&state.db, &state.catalog, &q.filters()).await {
        Ok(apps) => Json(json!({ "apps": apps })).into_response(),
        Err(e) => api_unavailable(e),
    }
}

/// GET /v1/catalog/{id}
pub async fn api_detail(State(state): State<Arc<AppState>>, Path(id): Path<String>) -> Response {
    let detail = match catalog::detail(&state.db, &id).await {
        Ok(Some(d)) => d,
        Ok(None) => {
            return api_error(
                StatusCode::NOT_FOUND,
                "not_found",
                "No app with this id is listed.",
                "Không có app này trong danh mục.",
            )
        }
        Err(e) => return api_unavailable(e),
    };
    let client = match state.hydra.get_client(&detail.listing.client_id).await {
        Ok(c) => c.unwrap_or(Value::Null),
        Err(e) => return api_unavailable(e),
    };
    let reviews = match catalog::reviews(&state.db, &id).await {
        Ok(r) => r,
        Err(e) => return api_unavailable(e),
    };
    let app = AppInfo::from_client(&client);
    let scopes: Vec<Value> = app
        .scopes
        .iter()
        .map(|d| {
            json!({
                "scope": d.scope.as_str(),
                "description_vi": d.scope.description_vi(),
                "description_en": d.scope.description_en(),
                "required": d.required,
                "purpose_vi": d.purpose_vi,
                "purpose_en": d.purpose_en,
            })
        })
        .collect();
    let mut body = serde_json::to_value(&detail).unwrap_or(Value::Null);
    body["scopes"] = Value::Array(scopes);
    body["policy_uri"] = json!(app.policy_uri);
    body["reviews"] = serde_json::to_value(&reviews).unwrap_or(Value::Null);
    Json(body).into_response()
}

#[derive(Debug, Deserialize)]
pub struct InstallRequest {
    pub app_id: String,
}

/// POST /v1/catalog/installs (chỉ token của F-Launcher)
pub async fn api_install(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<InstallRequest>,
) -> Response {
    let subject = match crate::launch::launcher_subject(&state, &headers).await {
        Ok(s) => s,
        Err(r) => return *r,
    };
    match catalog::record_install(&state.db, subject, &req.app_id).await {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => api_error(
            StatusCode::NOT_FOUND,
            "not_found",
            "No app with this id is listed.",
            "Không có app này trong danh mục.",
        ),
        Err(e) => api_unavailable(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filters_from_query() {
        let q = CatalogQuery {
            q: Some("  so tay ".into()),
            os: Some("windows".into()),
            price: Some("free".into()),
            verified: Some("on".into()),
            category: Some("".into()),
            ok: None,
        };
        let f = q.filters();
        assert_eq!(f.q.as_deref(), Some("so tay"));
        assert_eq!(f.category, None);
        assert_eq!(f.price, Some(Price::Free));
        assert!(f.verified_only);
        assert_eq!(platform_label("windows-x64"), "Windows (x64)");
        assert_eq!(platform_label("macos-arm64"), "macOS (arm64)");
        assert_eq!(price_label(Lang::Vi, 0), "Miễn phí");
        assert_eq!(price_label(Lang::Vi, 49_000), "49.000 ₫");
    }
}
