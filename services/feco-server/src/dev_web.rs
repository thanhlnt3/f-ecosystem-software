//! Trang web và API của cổng nhà phát triển (spec developer-portal).
//!
//! Web (phiên đăng nhập F-Eco): `/developer`, `/developer/apps/{id}` và các biểu mẫu của chúng.
//! API cho CLI `f-dev` (token nhà phát triển `Authorization: Bearer fdev_...`):
//! - `POST /v1/dev/apps/{app}/versions`: thân là `f-app.toml` → phiên bản nháp.
//! - `PUT /v1/dev/versions/{id}/packages/{platform}`: thân là gói zip, header `X-Feco-Sha256`.
//! - `POST /v1/dev/versions/{id}/submit`: gửi duyệt → trạng thái `pending_review`.
//!
//! API cho launcher và app: `GET /v1/catalog/{id}/update` (phát hành dần),
//! `POST /v1/apps/crashes` (app báo lỗi khi chạy, bằng access token của app).

use std::{path::PathBuf, sync::Arc};

use askama::Template;
use axum::{
    body::{Body, Bytes},
    extract::{Multipart, Path, Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{Html, IntoResponse, Response},
    Json,
};
use futures_util::StreamExt;
use serde::Deserialize;
use sha2::Digest;
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

use crate::{
    api::{api_error, bearer},
    connected::current_subject,
    csrf,
    developer::{self as dev, DevApp, DevError, DevResult, Membership, Problem, Role},
    i18n::Lang,
    views::Msg,
    AppState,
};

pub struct MemberView {
    pub email: String,
    pub role: String,
    pub active: bool,
}

pub struct InviteView {
    pub account: String,
    pub name: String,
    pub role: String,
}

#[derive(Template)]
#[template(path = "developer.html")]
struct HomePage {
    i18n: Lang,
    messages: Vec<Msg>,
    csrf: String,
    /// Bí mật chỉ hiện một lần (token CLI, client secret).
    secret: Option<String>,
    /// Chưa đủ điều kiện (email chưa xác minh hoặc chưa bật xác thực hai lớp).
    blocked: bool,
    registered: bool,
    invitations: Vec<InviteView>,
    agreement_version: &'static str,
    agreement_current: bool,
    name: String,
    organization: bool,
    role: String,
    can_manage: bool,
    can_edit: bool,
    domain: String,
    domain_verified: bool,
    txt_record: String,
    members: Vec<MemberView>,
    apps: Vec<DevApp>,
}

pub struct VersionView {
    pub id: String,
    pub version: String,
    pub status: String,
    pub channel: String,
    pub percent: i32,
    pub platforms: String,
    pub missing: Vec<String>,
    pub draft: bool,
    pub releasable: bool,
    pub released: bool,
    pub note: String,
}

#[derive(Template)]
#[template(path = "developer_app.html")]
struct AppPage {
    i18n: Lang,
    messages: Vec<Msg>,
    csrf: String,
    secret: Option<String>,
    app: DevApp,
    server_side: bool,
    redirects: String,
    versions: Vec<VersionView>,
    can_edit: bool,
    can_release: bool,
    can_delete: bool,
    installs: i64,
    active_7d: i64,
    active_30d: i64,
    crashes_30d: i64,
    rating: String,
    distribution: Vec<(String, i64)>,
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

fn see_other(to: &str) -> Response {
    let mut res = StatusCode::SEE_OTHER.into_response();
    if let Ok(v) = HeaderValue::from_str(to) {
        res.headers_mut().insert(header::LOCATION, v);
    }
    res
}

fn text(lang: Lang, p: &Problem) -> String {
    match lang {
        Lang::Vi => p.vi.clone(),
        Lang::En => p.en.clone(),
    }
}

/// Thông báo và mã HTTP cho một lỗi; lỗi hệ thống được ghi nhật ký và thay bằng thông báo chung.
fn explain(lang: Lang, e: DevError) -> (StatusCode, Msg) {
    match e {
        DevError::Problem(p) => (p.status, Msg::error(text(lang, &p))),
        other => {
            tracing::error!(error = %other, "lỗi ở cổng nhà phát triển");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Msg::error(lang.t("error.unavailable")),
            )
        }
    }
}

fn role_label(lang: Lang, role: Role) -> String {
    match lang {
        Lang::Vi => role.label_vi().to_owned(),
        Lang::En => match role {
            Role::Owner => "Owner",
            Role::Admin => "Admin",
            Role::Developer => "Developer",
            Role::Viewer => "Viewer",
        }
        .to_owned(),
    }
}

fn status_label(lang: Lang, status: &str) -> String {
    lang.t(&format!("dev.status.{status}")).to_owned()
}

fn ok_message(lang: Lang, key: Option<&str>) -> Vec<Msg> {
    const KEYS: [&str; 10] = [
        "registered",
        "agreement",
        "verified",
        "invited",
        "joined",
        "redirects",
        "manifest",
        "package",
        "submitted",
        "released",
    ];
    key.filter(|k| KEYS.contains(k))
        .map(|k| vec![Msg::success(lang.t(&format!("dev.ok.{k}")))])
        .unwrap_or_default()
}

#[derive(Debug, Default, Deserialize)]
pub struct OkQuery {
    pub ok: Option<String>,
}

async fn render_home(
    state: &AppState,
    lang: Lang,
    headers: &HeaderMap,
    subject: Uuid,
    messages: Vec<Msg>,
    secret: Option<String>,
    status: StatusCode,
) -> Response {
    let result: DevResult<HomePage> = async {
        let identity = state
            .kratos
            .get_identity(&subject.to_string())
            .await
            .map_err(|e| DevError::Upstream(e.to_string()))?
            .unwrap_or_default();
        let membership = dev::membership(&state.db, subject).await?;
        let mut messages = messages;
        let mut page = HomePage {
            i18n: lang,
            messages: Vec::new(),
            csrf: String::new(),
            secret,
            blocked: false,
            registered: membership.is_some(),
            invitations: Vec::new(),
            agreement_version: dev::AGREEMENT_VERSION,
            agreement_current: false,
            name: String::new(),
            organization: false,
            role: String::new(),
            can_manage: false,
            can_edit: false,
            domain: String::new(),
            domain_verified: false,
            txt_record: String::new(),
            members: Vec::new(),
            apps: Vec::new(),
        };
        match membership {
            None => match dev::identity_requirements(&identity) {
                Ok(email) => {
                    page.invitations = dev::invitations(&state.db, &email)
                        .await?
                        .into_iter()
                        .map(|(id, name, role)| InviteView {
                            account: id.to_string(),
                            name,
                            role: role_label(lang, role),
                        })
                        .collect();
                }
                Err(DevError::Problem(p)) => {
                    page.blocked = true;
                    messages.push(Msg::error(text(lang, &p)));
                }
                Err(e) => return Err(e),
            },
            Some(m) => {
                page.agreement_current = m.account.agreement_current();
                page.name = m.account.display_name.clone();
                page.organization = m.account.kind == "organization";
                page.role = role_label(lang, m.role);
                page.can_manage = m.role.can_release();
                page.can_edit = m.role.can_edit();
                page.domain = m.account.domain.clone().unwrap_or_default();
                page.domain_verified = m.account.domain_verified;
                page.txt_record = m.account.txt_record();
                page.members = dev::members(&state.db, m.account.id)
                    .await?
                    .into_iter()
                    .map(|x| MemberView {
                        email: x.email,
                        role: role_label(lang, x.role),
                        active: x.active,
                    })
                    .collect();
                page.apps = dev::apps(&state.db, m.account.id).await?;
            }
        }
        page.messages = messages;
        Ok(page)
    }
    .await;
    match result {
        Ok(mut page) => {
            let (token, set) = csrf::issue(headers, state.public_url.scheme() == "https");
            page.csrf = token;
            html(page, lang, status, set)
        }
        Err(e) => {
            let (status, msg) = explain(lang, e);
            (status, msg.text).into_response()
        }
    }
}

#[allow(clippy::too_many_arguments)] // cùng dạng với render_home, thêm tài khoản và mã app
async fn render_app(
    state: &AppState,
    lang: Lang,
    headers: &HeaderMap,
    m: &Membership,
    app_id: &str,
    messages: Vec<Msg>,
    secret: Option<String>,
    status: StatusCode,
) -> Response {
    let result: DevResult<AppPage> = async {
        let app = dev::app(&state.db, m, app_id).await?;
        let redirects = dev::redirect_uris(state, &app).await?.join("\n");
        let metrics = dev::metrics(&state.db, app_id).await?;
        let versions = dev::versions(&state.db, app_id)
            .await?
            .into_iter()
            .map(|v| VersionView {
                id: v.id.to_string(),
                draft: v.status == "draft",
                releasable: ["approved", "released", "paused"].contains(&v.status.as_str()),
                released: v.status == "released",
                status: status_label(lang, &v.status),
                channel: v.channel.unwrap_or_default(),
                percent: v.rollout_percent,
                missing: v
                    .platforms
                    .iter()
                    .filter(|p| !v.uploaded.contains(p))
                    .cloned()
                    .collect(),
                platforms: v.platforms.join(", "),
                version: v.version,
                note: v.review_note.unwrap_or_default(),
            })
            .collect();
        Ok(AppPage {
            i18n: lang,
            messages,
            csrf: String::new(),
            secret,
            server_side: app.kind == "server",
            app,
            redirects,
            versions,
            can_edit: m.role.can_edit(),
            can_release: m.role.can_release(),
            can_delete: m.role.can_delete(),
            installs: metrics.installs,
            active_7d: metrics.active_7d,
            active_30d: metrics.active_30d,
            crashes_30d: metrics.crashes_30d,
            rating: match metrics.rating_avg {
                Some(avg) => format!("★ {avg:.1} ({})", metrics.rating_count),
                None => "—".into(),
            },
            distribution: metrics.versions,
        })
    }
    .await;
    match result {
        Ok(mut page) => {
            let (token, set) = csrf::issue(headers, state.public_url.scheme() == "https");
            page.csrf = token;
            html(page, lang, status, set)
        }
        Err(e) => {
            let (status, msg) = explain(lang, e);
            (status, msg.text).into_response()
        }
    }
}

/// GET /developer
pub async fn home(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    Query(q): Query<OkQuery>,
    headers: HeaderMap,
) -> Response {
    let subject = match current_subject(&state, lang, &headers, "/developer").await {
        Ok(s) => s,
        Err(r) => return *r,
    };
    let messages = ok_message(lang, q.ok.as_deref());
    render_home(
        &state,
        lang,
        &headers,
        subject,
        messages,
        None,
        StatusCode::OK,
    )
    .await
}

#[derive(Template)]
#[template(path = "developer_docs.html")]
struct DocsPage {
    i18n: Lang,
    server: String,
    hydra: String,
}

/// GET /developer/docs (công khai)
pub async fn docs(State(state): State<Arc<AppState>>, lang: Lang) -> Response {
    html(
        DocsPage {
            i18n: lang,
            server: state.public_url.to_string(),
            hydra: state.hydra_public.to_string(),
        },
        lang,
        StatusCode::OK,
        None,
    )
}

/// GET /developer/apps/{id}
pub async fn app_page(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    Path(id): Path<String>,
    Query(q): Query<OkQuery>,
    headers: HeaderMap,
) -> Response {
    let back = format!("/developer/apps/{id}");
    let (_, m) = match member(&state, lang, &headers, &back).await {
        Ok(x) => x,
        Err(r) => return *r,
    };
    let messages = ok_message(lang, q.ok.as_deref());
    render_app(
        &state,
        lang,
        &headers,
        &m,
        &id,
        messages,
        None,
        StatusCode::OK,
    )
    .await
}

/// Người đang đăng nhập và tài khoản nhà phát triển của họ; chưa đăng ký thì về `/developer`.
async fn member(
    state: &AppState,
    lang: Lang,
    headers: &HeaderMap,
    back: &str,
) -> Result<(Uuid, Membership), Box<Response>> {
    let subject = current_subject(state, lang, headers, back).await?;
    match dev::membership(&state.db, subject).await {
        Ok(Some(m)) => Ok((subject, m)),
        Ok(None) => Err(Box::new(see_other("/developer"))),
        Err(e) => Err(Box::new(explain(lang, e).1.text.into_response())),
    }
}

/// Kết quả của một thao tác trên tài khoản.
enum Done {
    /// Khóa thông báo thành công (`dev.ok.<khóa>`).
    Notice(&'static str),
    /// Bí mật chỉ hiện một lần.
    Secret(String),
    /// App vừa tạo, kèm client secret nếu là app có máy chủ riêng.
    App(String, Option<String>),
}

struct Form(Vec<(String, String)>);

impl Form {
    fn parse(body: &Bytes) -> Self {
        Form(form_urlencoded::parse(body).into_owned().collect())
    }

    fn get(&self, k: &str) -> &str {
        self.0
            .iter()
            .find(|(n, _)| n == k)
            .map(|(_, v)| v.as_str())
            .unwrap_or_default()
    }

    fn csrf_ok(&self, headers: &HeaderMap) -> bool {
        csrf::verify(headers, Some(self.get(csrf::FIELD)))
    }
}

fn csrf_failed(lang: Lang) -> Response {
    (StatusCode::FORBIDDEN, lang.t("error.csrf")).into_response()
}

/// POST /developer/register
pub async fn register(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let form = Form::parse(&body);
    if !form.csrf_ok(&headers) {
        return csrf_failed(lang);
    }
    let subject = match current_subject(&state, lang, &headers, "/developer").await {
        Ok(s) => s,
        Err(r) => return *r,
    };
    let result: DevResult<()> = async {
        let identity = state
            .kratos
            .get_identity(&subject.to_string())
            .await
            .map_err(|e| DevError::Upstream(e.to_string()))?
            .unwrap_or_default();
        let email = dev::identity_requirements(&identity)?;
        dev::register(
            &state.db,
            subject,
            &email,
            &dev::Registration {
                organization: form.get("kind") == "organization",
                display_name: form.get("display_name").to_owned(),
                legal_name: form.get("legal_name").to_owned(),
                business_id: form.get("business_id").to_owned(),
                domain: form.get("domain").to_owned(),
                accept_agreement: form.get("accept_agreement") == "on",
            },
        )
        .await?;
        Ok(())
    }
    .await;
    match result {
        Ok(()) => see_other("/developer?ok=registered"),
        Err(e) => {
            let (status, msg) = explain(lang, e);
            render_home(&state, lang, &headers, subject, vec![msg], None, status).await
        }
    }
}

/// POST /developer/{action}: các thao tác trên tài khoản.
pub async fn account_action(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    Path(action): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let form = Form::parse(&body);
    if !form.csrf_ok(&headers) {
        return csrf_failed(lang);
    }
    let subject = match current_subject(&state, lang, &headers, "/developer").await {
        Ok(s) => s,
        Err(r) => return *r,
    };
    // Chấp nhận lời mời là thao tác duy nhất của người chưa có tài khoản nhà phát triển.
    if action == "join" {
        let result: DevResult<()> = async {
            let identity = state
                .kratos
                .get_identity(&subject.to_string())
                .await
                .map_err(|e| DevError::Upstream(e.to_string()))?
                .unwrap_or_default();
            let email = dev::identity_requirements(&identity)?;
            let account = form.get("account").parse::<Uuid>().unwrap_or_default();
            dev::accept_invitation(&state.db, account, subject, &email).await
        }
        .await;
        return match result {
            Ok(()) => see_other("/developer?ok=joined"),
            Err(e) => {
                let (status, msg) = explain(lang, e);
                render_home(&state, lang, &headers, subject, vec![msg], None, status).await
            }
        };
    }
    let m = match dev::membership(&state.db, subject).await {
        Ok(Some(m)) => m,
        Ok(None) => return see_other("/developer"),
        Err(e) => return explain(lang, e).1.text.into_response(),
    };
    let result: DevResult<Done> = async {
        Ok(match action.as_str() {
            "agreement" => {
                if form.get("accept_agreement") != "on" {
                    return Err(DevError::Problem(Problem {
                        status: StatusCode::BAD_REQUEST,
                        code: "invalid_request",
                        vi: "Hãy tích chọn để chấp nhận thỏa thuận.".into(),
                        en: "Tick the box to accept the agreement.".into(),
                    }));
                }
                dev::accept_agreement(&state.db, &m).await?;
                Done::Notice("agreement")
            }
            "domain" => {
                if dev::verify_domain(&state, &m).await? {
                    Done::Notice("verified")
                } else {
                    return Err(DevError::Problem(Problem {
                        status: StatusCode::CONFLICT,
                        code: "txt_not_found",
                        vi: format!("Chưa thấy bản ghi TXT \"{}\" ở tên miền. DNS có thể cần vài phút để cập nhật.", m.account.txt_record()),
                        en: format!("The TXT record \"{}\" was not found on the domain. DNS can take a few minutes to update.", m.account.txt_record()),
                    }));
                }
            }
            "members" => {
                let role = Role::parse(form.get("role")).unwrap_or(Role::Viewer);
                dev::invite(&state.db, &m, form.get("email"), role).await?;
                let (subject_line, body) = (
                    "Lời mời vào nhóm nhà phát triển F-Eco / F-Eco developer team invitation",
                    format!(
                        "{name} mời bạn tham gia nhóm nhà phát triển trên F-Eco với vai trò {role}.\nMở {url}developer để chấp nhận.\n\n\
                         {name} invited you to their F-Eco developer team.\nOpen {url}developer to accept.\n",
                        name = m.account.display_name,
                        role = role.label_vi(),
                        url = state.public_url
                    ),
                );
                if let Err(e) = state.mailer.send(form.get("email").trim(), subject_line, &body).await {
                    tracing::warn!(error = %e, "không gửi được email mời");
                }
                Done::Notice("invited")
            }
            "tokens" => Done::Secret(dev::create_token(&state.db, &m, form.get("name")).await?),
            "apps" => {
                let created = dev::create_app(
                    &state,
                    &m,
                    form.get("app_id"),
                    form.get("name"),
                    form.get("kind") == "server",
                    form.get("redirects"),
                )
                .await?;
                Done::App(created.app.app_id, created.client_secret)
            }
            _ => {
                return Err(DevError::Problem(Problem {
                    status: StatusCode::NOT_FOUND,
                    code: "not_found",
                    vi: "Không tìm thấy.".into(),
                    en: "Not found.".into(),
                }))
            }
        })
    }
    .await;
    match result {
        Ok(Done::Notice(key)) => see_other(&format!("/developer?ok={key}")),
        Ok(Done::Secret(secret)) => {
            let msg = vec![Msg::success(lang.t("dev.token_created"))];
            render_home(
                &state,
                lang,
                &headers,
                subject,
                msg,
                Some(secret),
                StatusCode::OK,
            )
            .await
        }
        Ok(Done::App(id, None)) => see_other(&format!("/developer/apps/{id}")),
        Ok(Done::App(id, Some(secret))) => {
            let msg = vec![Msg::success(lang.t("dev.secret_created"))];
            render_app(
                &state,
                lang,
                &headers,
                &m,
                &id,
                msg,
                Some(secret),
                StatusCode::OK,
            )
            .await
        }
        Err(e) => {
            let (status, msg) = explain(lang, e);
            render_home(&state, lang, &headers, subject, vec![msg], None, status).await
        }
    }
}

/// POST /developer/apps/{id}/{action}
pub async fn app_action(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    Path((id, action)): Path<(String, String)>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let form = Form::parse(&body);
    if !form.csrf_ok(&headers) {
        return csrf_failed(lang);
    }
    let back = format!("/developer/apps/{id}");
    let (_, m) = match member(&state, lang, &headers, &back).await {
        Ok(x) => x,
        Err(r) => return *r,
    };
    let result: DevResult<Result<&'static str, String>> = async {
        let app = dev::app(&state.db, &m, &id).await?;
        Ok(match action.as_str() {
            "redirects" => {
                dev::set_redirects(&state, &m, &app, form.get("redirects")).await?;
                Ok("redirects")
            }
            "secret" => Err(dev::rotate_secret(&state, &m, &app).await?),
            "manifest" => {
                dev::save_manifest(&state.db, &m, &app, form.get("manifest")).await?;
                Ok("manifest")
            }
            "delete" => {
                dev::delete_app(&state, &m, &app).await?;
                Ok("deleted")
            }
            _ => {
                return Err(DevError::Problem(Problem {
                    status: StatusCode::NOT_FOUND,
                    code: "not_found",
                    vi: "Không tìm thấy.".into(),
                    en: "Not found.".into(),
                }))
            }
        })
    }
    .await;
    match result {
        Ok(Ok("deleted")) => see_other("/developer"),
        Ok(Ok(key)) => see_other(&format!("{back}?ok={key}")),
        Ok(Err(secret)) => {
            let msg = vec![Msg::success(lang.t("dev.secret_created"))];
            render_app(
                &state,
                lang,
                &headers,
                &m,
                &id,
                msg,
                Some(secret),
                StatusCode::OK,
            )
            .await
        }
        Err(e) => {
            let (status, msg) = explain(lang, e);
            render_app(&state, lang, &headers, &m, &id, vec![msg], None, status).await
        }
    }
}

/// POST /developer/versions/{id}/{action}: gửi duyệt, phát hành, dừng phát hành.
pub async fn version_action(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    Path((id, action)): Path<(Uuid, String)>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let form = Form::parse(&body);
    if !form.csrf_ok(&headers) {
        return csrf_failed(lang);
    }
    let (_, m) = match member(&state, lang, &headers, "/developer").await {
        Ok(x) => x,
        Err(r) => return *r,
    };
    let app_id = match dev::version(&state.db, &m, id).await {
        Ok((info, _)) => info.app_id,
        Err(e) => {
            let (status, msg) = explain(lang, e);
            return (status, msg.text).into_response();
        }
    };
    let result: DevResult<&'static str> = async {
        Ok(match action.as_str() {
            "submit" => {
                dev::submit(&state.db, &m, id).await?;
                "submitted"
            }
            "release" => {
                let percent = form.get("percent").parse().unwrap_or(0);
                dev::release(&state, &m, id, form.get("channel"), percent).await?;
                "released"
            }
            "pause" => {
                dev::pause(&state, &m, id).await?;
                "released"
            }
            _ => {
                return Err(DevError::Problem(Problem {
                    status: StatusCode::NOT_FOUND,
                    code: "not_found",
                    vi: "Không tìm thấy.".into(),
                    en: "Not found.".into(),
                }))
            }
        })
    }
    .await;
    match result {
        Ok(key) => see_other(&format!("/developer/apps/{app_id}?ok={key}")),
        Err(e) => {
            let (status, msg) = explain(lang, e);
            render_app(&state, lang, &headers, &m, &app_id, vec![msg], None, status).await
        }
    }
}

/// Ghi gói đang tải lên vào file tạm, vừa ghi vừa tính SHA-256 và chặn ở 2 GB.
struct Incoming {
    path: PathBuf,
    file: tokio::fs::File,
    hasher: sha2::Sha256,
    size: u64,
}

impl Incoming {
    async fn create(state: &AppState) -> DevResult<Self> {
        let dir = state.package_dir.join(".incoming");
        tokio::fs::create_dir_all(&dir)
            .await
            .map_err(|e| DevError::Upstream(e.to_string()))?;
        let path = dir.join(format!("{}.zip", Uuid::new_v4()));
        let file = tokio::fs::File::create(&path)
            .await
            .map_err(|e| DevError::Upstream(e.to_string()))?;
        Ok(Self {
            path,
            file,
            hasher: sha2::Sha256::new(),
            size: 0,
        })
    }

    async fn push(&mut self, chunk: &[u8]) -> DevResult<()> {
        self.size += chunk.len() as u64;
        if self.size > dev::MAX_PACKAGE_BYTES {
            return Err(DevError::Problem(Problem {
                status: StatusCode::PAYLOAD_TOO_LARGE,
                code: "package_too_large",
                vi: "Gói vượt quá 2 GB.".into(),
                en: "The package is larger than 2 GB.".into(),
            }));
        }
        self.hasher.update(chunk);
        self.file
            .write_all(chunk)
            .await
            .map_err(|e| DevError::Upstream(e.to_string()))
    }

    async fn finish(mut self) -> DevResult<(PathBuf, String, u64)> {
        self.file
            .flush()
            .await
            .map_err(|e| DevError::Upstream(e.to_string()))?;
        let sha = self
            .hasher
            .finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        Ok((self.path, sha, self.size))
    }
}

/// POST /developer/versions/{id}/packages (multipart: feco_csrf, platform, file)
pub async fn upload_package(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Response {
    let (_, m) = match member(&state, lang, &headers, "/developer").await {
        Ok(x) => x,
        Err(r) => return *r,
    };
    let app_id = match dev::version(&state.db, &m, id).await {
        Ok((info, _)) => info.app_id,
        Err(e) => {
            let (status, msg) = explain(lang, e);
            return (status, msg.text).into_response();
        }
    };
    let result: DevResult<()> = async {
        let bad = |e: axum::extract::multipart::MultipartError| DevError::Upstream(e.to_string());
        let (mut csrf_ok, mut platform, mut stored) = (false, String::new(), None);
        while let Some(mut field) = multipart.next_field().await.map_err(bad)? {
            match field.name().unwrap_or_default() {
                csrf::FIELD => {
                    csrf_ok = csrf::verify(&headers, Some(&field.text().await.map_err(bad)?))
                }
                "platform" => platform = field.text().await.map_err(bad)?,
                "file" => {
                    // Biểu mẫu đặt token CSRF trước file: chưa hợp lệ thì không nhận dữ liệu.
                    if !csrf_ok {
                        break;
                    }
                    let mut incoming = Incoming::create(&state).await?;
                    while let Some(chunk) = field.chunk().await.map_err(bad)? {
                        if let Err(e) = incoming.push(&chunk).await {
                            let _ = tokio::fs::remove_file(&incoming.path).await;
                            return Err(e);
                        }
                    }
                    stored = Some(incoming.finish().await?);
                }
                _ => {}
            }
        }
        let Some((path, sha, size)) = stored else {
            return Err(DevError::Problem(Problem {
                status: StatusCode::FORBIDDEN,
                code: "csrf",
                vi: lang.t("error.csrf").into(),
                en: lang.t("error.csrf").into(),
            }));
        };
        let result = dev::store_package(
            &state,
            &m,
            id,
            &platform,
            dev::Upload {
                path: &path,
                declared_sha256: None,
                sha256: &sha,
                size,
            },
        )
        .await;
        if result.is_err() {
            let _ = tokio::fs::remove_file(&path).await;
        }
        result
    }
    .await;
    match result {
        Ok(()) => see_other(&format!("/developer/apps/{app_id}?ok=package")),
        Err(e) => {
            let (status, msg) = explain(lang, e);
            render_app(&state, lang, &headers, &m, &app_id, vec![msg], None, status).await
        }
    }
}

// ---------- API cho CLI f-dev ----------

fn api_problem(e: DevError) -> Response {
    match e {
        DevError::Problem(p) => api_error(p.status, p.code, &p.en, &p.vi),
        other => {
            tracing::error!(error = %other, "lỗi ở API nhà phát triển");
            api_error(
                StatusCode::SERVICE_UNAVAILABLE,
                "temporarily_unavailable",
                "The service is temporarily unavailable.",
                "Dịch vụ đang tạm gián đoạn.",
            )
        }
    }
}

async fn token_member(state: &AppState, headers: &HeaderMap) -> Result<Membership, Box<Response>> {
    let invalid = || {
        Box::new(api_error(
            StatusCode::UNAUTHORIZED,
            "invalid_token",
            "The developer token is missing or revoked.",
            "Token nhà phát triển bị thiếu hoặc đã bị thu hồi.",
        ))
    };
    let token = bearer(headers).ok_or_else(invalid)?;
    match dev::token_membership(&state.db, token).await {
        Ok(Some(m)) => Ok(m),
        Ok(None) => Err(invalid()),
        Err(e) => Err(Box::new(api_problem(e))),
    }
}

/// POST /v1/dev/apps/{app}/versions
pub async fn api_manifest(
    State(state): State<Arc<AppState>>,
    Path(app_id): Path<String>,
    headers: HeaderMap,
    body: String,
) -> Response {
    let m = match token_member(&state, &headers).await {
        Ok(m) => m,
        Err(r) => return *r,
    };
    let result = async {
        let app = dev::app(&state.db, &m, &app_id).await?;
        dev::save_manifest(&state.db, &m, &app, &body).await
    }
    .await;
    match result {
        Ok(v) => (StatusCode::CREATED, Json(v)).into_response(),
        Err(e) => api_problem(e),
    }
}

/// PUT /v1/dev/versions/{id}/packages/{platform}
pub async fn api_package(
    State(state): State<Arc<AppState>>,
    Path((id, platform)): Path<(Uuid, String)>,
    headers: HeaderMap,
    body: Body,
) -> Response {
    let m = match token_member(&state, &headers).await {
        Ok(m) => m,
        Err(r) => return *r,
    };
    let declared = headers
        .get("x-feco-sha256")
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    let result: DevResult<()> = async {
        // Kiểm quyền trước khi nhận dữ liệu.
        dev::version(&state.db, &m, id).await?;
        let mut incoming = Incoming::create(&state).await?;
        let mut stream = body.into_data_stream();
        while let Some(chunk) = stream.next().await {
            let pushed = match chunk {
                Ok(c) => incoming.push(&c).await,
                Err(e) => Err(DevError::Upstream(e.to_string())),
            };
            if let Err(e) = pushed {
                let _ = tokio::fs::remove_file(&incoming.path).await;
                return Err(e);
            }
        }
        let (path, sha, size) = incoming.finish().await?;
        let result = dev::store_package(
            &state,
            &m,
            id,
            &platform,
            dev::Upload {
                path: &path,
                declared_sha256: declared.as_deref(),
                sha256: &sha,
                size,
            },
        )
        .await;
        if result.is_err() {
            let _ = tokio::fs::remove_file(&path).await;
        }
        result
    }
    .await;
    match result {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => api_problem(e),
    }
}

/// POST /v1/dev/versions/{id}/submit
pub async fn api_submit(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Response {
    let m = match token_member(&state, &headers).await {
        Ok(m) => m,
        Err(r) => return *r,
    };
    match dev::submit(&state.db, &m, id).await {
        Ok(v) => Json(v).into_response(),
        Err(e) => api_problem(e),
    }
}

#[derive(Debug, Deserialize)]
pub struct UpdateQuery {
    pub platform: String,
    pub device: String,
    #[serde(default)]
    pub channel: Option<String>,
}

/// GET /v1/catalog/{id}/update?platform=&device=&channel=
///
/// Phiên bản launcher này nên dùng, theo kênh và phần trăm phát hành dần.
pub async fn api_update(
    State(state): State<Arc<AppState>>,
    Path(app_id): Path<String>,
    Query(q): Query<UpdateQuery>,
) -> Response {
    let channel = q.channel.as_deref().unwrap_or("stable");
    if q.device.len() < 8 || q.device.len() > 100 {
        return api_error(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "The device identifier is not valid.",
            "Mã thiết bị không hợp lệ.",
        );
    }
    match dev::update_for(&state.db, &app_id, &q.platform, channel, &q.device).await {
        Ok(Some(u)) => Json(u).into_response(),
        Ok(None) => api_error(
            StatusCode::NOT_FOUND,
            "not_found",
            "No released version for this app and platform.",
            "Không có phiên bản nào đang phát hành cho app và nền tảng này.",
        ),
        Err(e) => api_problem(e),
    }
}

#[derive(Debug, Deserialize)]
pub struct CrashReport {
    pub version: String,
}

/// POST /v1/apps/crashes (access token của app)
pub async fn api_crash(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(report): Json<CrashReport>,
) -> Response {
    let invalid = || {
        api_error(
            StatusCode::UNAUTHORIZED,
            "invalid_token",
            "The access token is missing, expired or revoked.",
            "Access token bị thiếu, đã hết hạn hoặc đã bị thu hồi.",
        )
    };
    let Some(token) = bearer(&headers) else {
        return invalid();
    };
    let client_id = match state.hydra.introspect(token).await {
        Ok(Some(info)) => info["client_id"].as_str().map(str::to_owned),
        Ok(None) => None,
        Err(e) => return api_problem(DevError::Upstream(e.to_string())),
    };
    let Some(client_id) = client_id else {
        return invalid();
    };
    if report.version.len() > 64 {
        return api_error(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "The version is not valid.",
            "Phiên bản không hợp lệ.",
        );
    }
    match dev::record_crash(&state.db, &client_id, &report.version).await {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => api_problem(e),
    }
}
