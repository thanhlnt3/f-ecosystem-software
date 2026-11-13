//! Thông báo (spec notifications, task 14.3): email và trung tâm thông báo trên web
//! (`/notifications`) và trong F-Launcher (`GET /v1/notifications`).
//!
//! - Loại bắt buộc (bảo mật, thanh toán, hóa đơn, đơn hàng, điều khoản) luôn được gửi.
//! - Loại tùy chọn (cập nhật app, gợi ý app, bản tin) tắt được từng loại; bản tin mặc định tắt.
//!   Email loại tùy chọn có liên kết hủy đăng ký dùng được mà không cần đăng nhập.
//! - Thông báo theo ngôn ngữ người dùng chọn (`traits.language`), có liên kết tới đúng màn hình,
//!   và không chứa mật khẩu, mã xác thực, số thẻ hay nội dung tin nhắn trong đơn.

use std::sync::Arc;

use askama::Template;
use axum::{
    body::Bytes,
    extract::{Path, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{Html, IntoResponse, Response},
    Json,
};
use serde::Serialize;
use serde_json::{json, Value};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{connected::current_subject, csrf, i18n::Lang, views::Msg, AppState};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Security,
    Payment,
    Invoice,
    Order,
    Terms,
    AppUpdates,
    Recommendations,
    Newsletter,
}

impl Category {
    pub const OPTIONAL: [Category; 3] = [
        Category::AppUpdates,
        Category::Recommendations,
        Category::Newsletter,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Category::Security => "security",
            Category::Payment => "payment",
            Category::Invoice => "invoice",
            Category::Order => "order",
            Category::Terms => "terms",
            Category::AppUpdates => "app_updates",
            Category::Recommendations => "recommendations",
            Category::Newsletter => "newsletter",
        }
    }

    pub fn parse(s: &str) -> Option<Category> {
        [
            Category::Security,
            Category::Payment,
            Category::Invoice,
            Category::Order,
            Category::Terms,
            Category::AppUpdates,
            Category::Recommendations,
            Category::Newsletter,
        ]
        .into_iter()
        .find(|c| c.as_str() == s)
    }

    /// Bắt buộc: luôn gửi, không tắt được.
    pub fn mandatory(self) -> bool {
        !Category::OPTIONAL.contains(&self)
    }

    /// Mặc định khi người dùng chưa chọn: bản tin tắt, các loại tùy chọn khác bật.
    pub fn default_enabled(self) -> bool {
        self != Category::Newsletter
    }

    fn label_key(self) -> &'static str {
        match self {
            Category::AppUpdates => "notify.cat.app_updates",
            Category::Recommendations => "notify.cat.recommendations",
            Category::Newsletter => "notify.cat.newsletter",
            _ => "notify.cat.mandatory",
        }
    }
}

/// Nội dung một thông báo, theo hai ngôn ngữ (thiếu tiếng Anh thì dùng tiếng Việt).
#[derive(Debug, Clone)]
pub struct Note {
    pub title_vi: String,
    pub body_vi: String,
    pub title_en: Option<String>,
    pub body_en: Option<String>,
    /// Đường dẫn tương đối tới màn hình liên quan, ví dụ `/orders/{id}#quote`.
    pub link: String,
}

impl Note {
    pub fn vi(title: impl Into<String>, body: impl Into<String>, link: impl Into<String>) -> Self {
        Note {
            title_vi: title.into(),
            body_vi: body.into(),
            title_en: None,
            body_en: None,
            link: link.into(),
        }
    }

    pub fn en(mut self, title: impl Into<String>, body: impl Into<String>) -> Self {
        self.title_en = Some(title.into());
        self.body_en = Some(body.into());
        self
    }

    fn pick(&self, lang: Lang) -> (&str, &str) {
        match (lang, &self.title_en, &self.body_en) {
            (Lang::En, Some(t), Some(b)) => (t, b),
            _ => (&self.title_vi, &self.body_vi),
        }
    }
}

pub async fn enabled(state: &AppState, subject: Uuid, category: Category) -> bool {
    if category.mandatory() {
        return true;
    }
    sqlx::query_scalar::<_, bool>(
        "SELECT enabled FROM notification_preferences WHERE subject = $1 AND category = $2",
    )
    .bind(subject)
    .bind(category.as_str())
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten()
    .unwrap_or(category.default_enabled())
}

pub async fn set_enabled(
    state: &AppState,
    subject: Uuid,
    category: Category,
    on: bool,
) -> sqlx::Result<()> {
    if category.mandatory() {
        return Ok(());
    }
    sqlx::query(
        "INSERT INTO notification_preferences (subject, category, enabled) VALUES ($1, $2, $3)
         ON CONFLICT (subject, category) DO UPDATE SET enabled = $3, updated_at = now()",
    )
    .bind(subject)
    .bind(category.as_str())
    .bind(on)
    .execute(&state.db)
    .await?;
    Ok(())
}

/// Mã hủy đăng ký một loại thông báo (không cần đăng nhập): `{subject}.{loại}.{chữ ký}`.
pub fn unsubscribe_token(state: &AppState, subject: Uuid, category: Category) -> String {
    let sig = state.pairwise.subject(
        &format!("unsubscribe:{}", category.as_str()),
        &subject.to_string(),
    );
    format!("{subject}.{}.{sig}", category.as_str())
}

fn check_token(state: &AppState, token: &str) -> Option<(Uuid, Category)> {
    let mut parts = token.splitn(3, '.');
    let subject: Uuid = parts.next()?.parse().ok()?;
    let category = Category::parse(parts.next()?)?;
    let sig = parts.next()?;
    let expected = unsubscribe_token(state, subject, category);
    let ok = expected.len() == token.len()
        && expected
            .bytes()
            .zip(token.bytes())
            .fold(0u8, |a, (x, y)| a | (x ^ y))
            == 0;
    (ok && !category.mandatory() && !sig.is_empty()).then_some((subject, category))
}

/// Ngôn ngữ người dùng chọn khi đăng ký hoặc trong Cài đặt: trait `locale` của lược đồ danh tính
/// (`infra/kratos/identity.schema.json`). Mặc định tiếng Việt.
pub fn identity_language(identity: &Value) -> Lang {
    identity["traits"]["locale"]
        .as_str()
        .and_then(Lang::parse)
        .unwrap_or_default()
}

/// Ngôn ngữ và email của người dùng (từ Kratos).
async fn recipient(state: &AppState, subject: Uuid) -> (Lang, Option<String>) {
    match state.kratos.get_identity(&subject.to_string()).await {
        Ok(Some(i)) => (
            identity_language(&i),
            i["traits"]["email"].as_str().map(str::to_owned),
        ),
        _ => (Lang::Vi, None),
    }
}

/// Ngôn ngữ người dùng đã chọn (trait `locale`), mặc định tiếng Việt.
pub async fn language_of(state: &AppState, subject: Uuid) -> Lang {
    recipient(state, subject).await.0
}

/// Gửi một thông báo: luôn vào trung tâm thông báo; email nếu loại bắt buộc hoặc người dùng bật.
/// `email` dùng khi đã biết địa chỉ (đỡ một lần hỏi Kratos).
pub async fn notify(
    state: &AppState,
    subject: Uuid,
    email: Option<&str>,
    category: Category,
    note: &Note,
) {
    let (lang, kratos_email) = recipient(state, subject).await;
    let (title, body) = note.pick(lang);
    if let Err(e) = sqlx::query("INSERT INTO notifications (subject, category, title, body, link) VALUES ($1, $2, $3, $4, $5)")
        .bind(subject)
        .bind(category.as_str())
        .bind(title)
        .bind(body)
        .bind(&note.link)
        .execute(&state.db)
        .await
    {
        tracing::error!(error = %e, "không lưu được thông báo");
    }
    if !enabled(state, subject, category).await {
        return;
    }
    let Some(to) = email.map(str::to_owned).or(kratos_email) else {
        return;
    };
    let link = format!("{}{}", state.public_url, note.link.trim_start_matches('/'));
    let mut text = format!("{body}\n\n{} {link}\n", lang.t("notify.open"));
    if !category.mandatory() {
        text.push_str(&format!(
            "\n{} {}notifications/unsubscribe/{}\n",
            lang.t("notify.unsubscribe"),
            state.public_url,
            unsubscribe_token(state, subject, category)
        ));
    }
    if let Err(e) = state.mailer.send(&to, title, &text).await {
        tracing::warn!(error = %e, "không gửi được email thông báo");
    }
}

/// Báo bản cập nhật cho những người đã cài app (loại tùy chọn "cập nhật app").
pub async fn app_updated(state: &AppState, app_id: &str, name: &str, version: &str) {
    let subjects: Vec<Uuid> =
        sqlx::query_scalar("SELECT subject FROM catalog_installs WHERE app_id = $1")
            .bind(app_id)
            .fetch_all(&state.db)
            .await
            .unwrap_or_default();
    for s in subjects {
        let note = Note::vi(
            format!("{name} có bản cập nhật {version}"),
            format!("{name} {version} đã sẵn sàng trong F-Launcher."),
            format!("/store/{app_id}"),
        )
        .en(
            format!("{name} {version} is available"),
            format!("{name} {version} is ready in F-Launcher."),
        );
        notify(state, s, None, Category::AppUpdates, &note).await;
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Item {
    pub id: i64,
    pub category: String,
    pub title: String,
    pub body: String,
    pub link: String,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    pub read: bool,
}

type ItemRow = (
    i64,
    String,
    String,
    String,
    String,
    OffsetDateTime,
    Option<OffsetDateTime>,
);

pub async fn list(state: &AppState, subject: Uuid) -> sqlx::Result<Vec<Item>> {
    let rows: Vec<ItemRow> = sqlx::query_as(
        "SELECT id, category, title, body, link, created_at, read_at FROM notifications WHERE subject = $1 ORDER BY id DESC LIMIT 100",
    )
    .bind(subject)
    .fetch_all(&state.db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| Item {
            id: r.0,
            category: r.1,
            title: r.2,
            body: r.3,
            link: format!("{}{}", state.public_url, r.4.trim_start_matches('/')),
            created_at: r.5,
            read: r.6.is_some(),
        })
        .collect())
}

// ---------- Web ----------

pub struct PrefView {
    pub code: &'static str,
    pub label: &'static str,
    pub on: bool,
}

pub struct ItemView {
    pub title: String,
    pub body: String,
    pub link: String,
    pub at: String,
    pub read: bool,
}

#[derive(Template)]
#[template(path = "notifications.html")]
struct CenterPage {
    i18n: Lang,
    messages: Vec<Msg>,
    csrf: String,
    items: Vec<ItemView>,
    prefs: Vec<PrefView>,
}

fn html(
    t: impl Template,
    lang: Lang,
    status: StatusCode,
    set_cookie: Option<HeaderValue>,
) -> Response {
    let mut res = match t.render() {
        Ok(h) => (status, Html(h)).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "lỗi dựng trang");
            (StatusCode::INTERNAL_SERVER_ERROR, lang.t("error.render")).into_response()
        }
    };
    if let Some(c) = set_cookie {
        res.headers_mut().append(header::SET_COOKIE, c);
    }
    res
}

async fn render_center(
    state: &AppState,
    lang: Lang,
    headers: &HeaderMap,
    subject: Uuid,
    messages: Vec<Msg>,
) -> Response {
    let items = list(state, subject).await.unwrap_or_default();
    // Mở trung tâm thông báo thì coi như đã đọc.
    let _ = sqlx::query(
        "UPDATE notifications SET read_at = now() WHERE subject = $1 AND read_at IS NULL",
    )
    .bind(subject)
    .execute(&state.db)
    .await;
    let mut prefs = Vec::new();
    for c in Category::OPTIONAL {
        prefs.push(PrefView {
            code: c.as_str(),
            label: lang.t(c.label_key()),
            on: enabled(state, subject, c).await,
        });
    }
    let (token, set) = csrf::issue(headers, state.public_url.scheme() == "https");
    html(
        CenterPage {
            i18n: lang,
            messages,
            csrf: token,
            items: items
                .into_iter()
                .map(|i| ItemView {
                    title: i.title,
                    body: i.body,
                    link: i.link,
                    at: i
                        .created_at
                        .format(&time::macros::format_description!(
                            "[day]/[month]/[year] [hour]:[minute]"
                        ))
                        .unwrap_or_default(),
                    read: i.read,
                })
                .collect(),
            prefs,
        },
        lang,
        StatusCode::OK,
        set,
    )
}

/// GET /notifications
pub async fn center(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    headers: HeaderMap,
) -> Response {
    let subject = match current_subject(&state, lang, &headers, "/notifications").await {
        Ok(s) => s,
        Err(r) => return *r,
    };
    render_center(&state, lang, &headers, subject, Vec::new()).await
}

/// POST /notifications/settings: bật hoặc tắt từng loại tùy chọn (ô được đánh dấu là bật).
pub async fn save_settings(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let form: Vec<(String, String)> = form_urlencoded::parse(&body).into_owned().collect();
    let token = form
        .iter()
        .find(|(k, _)| k == csrf::FIELD)
        .map(|(_, v)| v.as_str());
    if !csrf::verify(&headers, token) {
        return (StatusCode::FORBIDDEN, lang.t("error.csrf")).into_response();
    }
    let subject = match current_subject(&state, lang, &headers, "/notifications").await {
        Ok(s) => s,
        Err(r) => return *r,
    };
    for c in Category::OPTIONAL {
        let on = form.iter().any(|(k, v)| k == "on" && v == c.as_str());
        if let Err(e) = set_enabled(&state, subject, c, on).await {
            tracing::error!(error = %e, "không lưu được tùy chọn thông báo");
        }
    }
    render_center(
        &state,
        lang,
        &headers,
        subject,
        vec![Msg::success(lang.t("notify.saved"))],
    )
    .await
}

#[derive(Template)]
#[template(path = "unsubscribe.html")]
struct UnsubscribePage {
    i18n: Lang,
    csrf: String,
    token: String,
    category: &'static str,
    done: bool,
}

/// GET /notifications/unsubscribe/{token}: trang xác nhận (không cần đăng nhập).
pub async fn unsubscribe_page(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    Path(token): Path<String>,
    headers: HeaderMap,
) -> Response {
    let Some((_, category)) = check_token(&state, &token) else {
        return (StatusCode::NOT_FOUND, lang.t("notify.bad_link")).into_response();
    };
    let (csrf_token, set) = csrf::issue(&headers, state.public_url.scheme() == "https");
    html(
        UnsubscribePage {
            i18n: lang,
            csrf: csrf_token,
            token,
            category: lang.t(category.label_key()),
            done: false,
        },
        lang,
        StatusCode::OK,
        set,
    )
}

/// POST /notifications/unsubscribe/{token}: tắt loại thông báo đó.
pub async fn unsubscribe(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    Path(token): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let form: Vec<(String, String)> = form_urlencoded::parse(&body).into_owned().collect();
    if !csrf::verify(
        &headers,
        form.iter()
            .find(|(k, _)| k == csrf::FIELD)
            .map(|(_, v)| v.as_str()),
    ) {
        return (StatusCode::FORBIDDEN, lang.t("error.csrf")).into_response();
    }
    let Some((subject, category)) = check_token(&state, &token) else {
        return (StatusCode::NOT_FOUND, lang.t("notify.bad_link")).into_response();
    };
    if let Err(e) = set_enabled(&state, subject, category, false).await {
        tracing::error!(error = %e, "không hủy được đăng ký");
        return (StatusCode::SERVICE_UNAVAILABLE, lang.t("error.unavailable")).into_response();
    }
    crate::audit::record(
        &state.db,
        crate::audit::Actor::User(subject),
        "notify.unsubscribe",
        Some(category.as_str()),
        None,
        json!({}),
    )
    .await;
    html(
        UnsubscribePage {
            i18n: lang,
            csrf: String::new(),
            token,
            category: lang.t(category.label_key()),
            done: true,
        },
        lang,
        StatusCode::OK,
        None,
    )
}

/// GET /v1/notifications (access token của F-Launcher): trung tâm thông báo trong launcher.
pub async fn api_list(State(state): State<Arc<AppState>>, headers: HeaderMap) -> Response {
    let subject = match crate::launch::launcher_subject(&state, &headers).await {
        Ok(s) => s,
        Err(r) => return *r,
    };
    match list(&state, subject).await {
        Ok(items) => Json(json!({ "notifications": items })).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "không đọc được thông báo");
            StatusCode::SERVICE_UNAVAILABLE.into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ngôn ngữ thông báo lấy từ trait `locale` (trước đây đọc nhầm `language`, nên mọi người dùng
    /// đều nhận tiếng Việt).
    #[test]
    fn language_comes_from_the_locale_trait() {
        let en = serde_json::json!({"traits": {"email": "a@b.c", "locale": "en"}});
        assert_eq!(identity_language(&en), Lang::En);
        let vi = serde_json::json!({"traits": {"locale": "vi"}});
        assert_eq!(identity_language(&vi), Lang::Vi);
        let none = serde_json::json!({"traits": {"language": "en"}});
        assert_eq!(identity_language(&none), Lang::Vi);
    }

    #[test]
    fn categories() {
        for c in [
            Category::Security,
            Category::Payment,
            Category::Invoice,
            Category::Order,
            Category::Terms,
        ] {
            assert!(c.mandatory(), "{c:?}");
        }
        assert!(!Category::AppUpdates.mandatory() && Category::AppUpdates.default_enabled());
        assert!(
            !Category::Newsletter.default_enabled(),
            "bản tin mặc định tắt"
        );
        assert_eq!(Category::parse("newsletter"), Some(Category::Newsletter));
    }
}
