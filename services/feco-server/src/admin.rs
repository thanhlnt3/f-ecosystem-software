//! Bảng quản trị (spec admin-console, nhóm task 14): `/admin`.
//!
//! - Năm vai trò: Người duyệt app, Hỗ trợ, Quản lý dự án, Tài chính, Quản trị hệ thống. Mỗi vai trò
//!   chỉ thấy và gọi được phần của mình: chức năng khác bị ẩn và lời gọi trực tiếp trả 403.
//! - Vào bảng quản trị cần phiên đã xác thực bằng passkey hoặc khóa bảo mật phần cứng (WebAuthn).
//! - Mọi thao tác ghi nhật ký kiểm toán; xem hồ sơ người dùng phải nhập lý do.
//! - Không có chức năng đăng nhập thay người dùng hay xem dữ liệu trong app con.

use std::sync::Arc;

use askama::Template;
use axum::{
    body::Bytes,
    extract::{Path, Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{Html, IntoResponse, Response},
};
use serde::Deserialize;
use serde_json::{json, Value};
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

use crate::{
    audit::{self, Actor},
    csrf,
    i18n::Lang,
    kratos::{FlowKind, InitOptions, Whoami},
    orders::{self, OrderError, Status},
    payments::{self, Requester},
    review,
    views::Msg,
    AppState,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Reviewer,
    Support,
    ProjectManager,
    Finance,
    Sysadmin,
}

impl Role {
    pub const ALL: [Role; 5] = [
        Role::Reviewer,
        Role::Support,
        Role::ProjectManager,
        Role::Finance,
        Role::Sysadmin,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Role::Reviewer => "reviewer",
            Role::Support => "support",
            Role::ProjectManager => "project_manager",
            Role::Finance => "finance",
            Role::Sysadmin => "sysadmin",
        }
    }

    pub fn parse(s: &str) -> Option<Role> {
        Role::ALL.into_iter().find(|r| r.as_str() == s)
    }

    pub fn label(self, lang: Lang) -> &'static str {
        lang.t(match self {
            Role::Reviewer => "admin.role.reviewer",
            Role::Support => "admin.role.support",
            Role::ProjectManager => "admin.role.project_manager",
            Role::Finance => "admin.role.finance",
            Role::Sysadmin => "admin.role.sysadmin",
        })
    }

    /// Mục của bảng quản trị dành cho vai trò.
    fn section(self) -> (&'static str, &'static str) {
        match self {
            Role::Reviewer => ("/admin/reviews", "admin.nav.reviews"),
            Role::Support => ("/admin/users", "admin.nav.users"),
            Role::ProjectManager => ("/admin/orders", "admin.nav.orders"),
            Role::Finance => ("/admin/refunds", "admin.nav.refunds"),
            Role::Sysadmin => ("/admin/members", "admin.nav.members"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Admin {
    pub subject: Uuid,
    pub email: String,
    pub roles: Vec<Role>,
}

impl Admin {
    pub fn has(&self, r: Role) -> bool {
        self.roles.contains(&r)
    }

    fn actor(&self) -> Actor {
        Actor::Admin(self.email.clone())
    }
}

/// Phương thức xác thực trong phiên được coi là passkey hoặc khóa bảo mật phần cứng.
pub fn has_security_key(session: &Value) -> bool {
    session["authentication_methods"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|m| matches!(m["method"].as_str(), Some("webauthn" | "passkey")))
}

pub async fn admin_roles(
    state: &AppState,
    subject: Uuid,
) -> sqlx::Result<Option<(String, Vec<Role>)>> {
    let row: Option<(String, Vec<String>)> =
        sqlx::query_as("SELECT email, roles FROM admin_members WHERE subject = $1")
            .bind(subject)
            .fetch_optional(&state.db)
            .await?;
    Ok(row.map(|(e, r)| (e, r.iter().filter_map(|x| Role::parse(x)).collect())))
}

fn page_status(lang: Lang, status: StatusCode, key: &str) -> Response {
    (status, Html(format!("<!doctype html><meta charset=\"utf-8\"><title>F-Eco</title><p>{}</p><p><a href=\"/\">F-Eco</a></p>", lang.t(key)))).into_response()
}

fn see_other(to: &str) -> Response {
    let mut res = StatusCode::SEE_OTHER.into_response();
    if let Ok(v) = HeaderValue::from_str(to) {
        res.headers_mut().insert(header::LOCATION, v);
    }
    res
}

/// Cổng vào bảng quản trị: đăng nhập, là thành viên quản trị, phiên có passkey hoặc khóa bảo mật,
/// và (nếu có) đúng vai trò. Sai vai trò trả 403.
pub async fn gate(
    state: &AppState,
    lang: Lang,
    headers: &HeaderMap,
    path: &str,
    need: Option<Role>,
) -> Result<Admin, Box<Response>> {
    let cookie = headers.get(header::COOKIE).and_then(|v| v.to_str().ok());
    let session = match state.kratos.whoami(cookie).await {
        Ok(Whoami::Session(s)) => s,
        Ok(Whoami::NeedsSecondFactor(to)) => return Err(Box::new(see_other(&to))),
        Ok(Whoami::Anonymous) => {
            let url = state.kratos.browser_init_url(
                FlowKind::Login,
                &InitOptions {
                    return_to: Some(format!(
                        "{}{}",
                        state.public_url,
                        path.trim_start_matches('/')
                    )),
                    ..Default::default()
                },
            );
            return Err(Box::new(see_other(url.as_str())));
        }
        Err(e) => {
            tracing::error!(error = %e, "không hỏi được phiên Kratos");
            return Err(Box::new(page_status(
                lang,
                StatusCode::SERVICE_UNAVAILABLE,
                "error.unavailable",
            )));
        }
    };
    let subject: Uuid = session["identity"]["id"]
        .as_str()
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| Box::new(page_status(lang, StatusCode::FORBIDDEN, "admin.forbidden")))?;
    let (email, roles) = match admin_roles(state, subject).await {
        Ok(Some(x)) => x,
        Ok(None) => {
            return Err(Box::new(page_status(
                lang,
                StatusCode::FORBIDDEN,
                "admin.forbidden",
            )))
        }
        Err(e) => {
            tracing::error!(error = %e, "không đọc được vai trò quản trị");
            return Err(Box::new(page_status(
                lang,
                StatusCode::SERVICE_UNAVAILABLE,
                "error.unavailable",
            )));
        }
    };
    if !has_security_key(&session) {
        return Err(Box::new(page_status(
            lang,
            StatusCode::FORBIDDEN,
            "admin.need_security_key",
        )));
    }
    let admin = Admin {
        subject,
        email,
        roles,
    };
    if let Some(r) = need {
        if !admin.has(r) {
            audit::record(
                &state.db,
                admin.actor(),
                "admin.forbidden",
                Some(path),
                None,
                json!({"needed": r.as_str()}),
            )
            .await;
            return Err(Box::new(page_status(
                lang,
                StatusCode::FORBIDDEN,
                "admin.role_forbidden",
            )));
        }
    }
    Ok(admin)
}

fn form(body: &Bytes) -> Vec<(String, String)> {
    form_urlencoded::parse(body).into_owned().collect()
}

fn field<'a>(f: &'a [(String, String)], k: &str) -> &'a str {
    f.iter()
        .find(|(n, _)| n == k)
        .map(|(_, v)| v.as_str())
        .unwrap_or_default()
}

fn explain(lang: Lang, e: OrderError) -> Msg {
    match e {
        OrderError::Problem { vi, en, .. } => Msg::error(if lang == Lang::Vi { vi } else { en }),
        OrderError::Db(e) => {
            tracing::error!(error = %e, "lỗi cơ sở dữ liệu ở bảng quản trị");
            Msg::error(lang.t("error.unavailable"))
        }
    }
}

fn when(t: OffsetDateTime) -> String {
    t.format(&time::macros::format_description!(
        "[day]/[month]/[year] [hour]:[minute]"
    ))
    .unwrap_or_default()
}

/// Email che bớt: `an***@example.com`.
pub fn mask_email(email: &str) -> String {
    match email.split_once('@') {
        Some((local, domain)) => {
            let keep: String = local.chars().take(2).collect();
            format!("{keep}***@{domain}")
        }
        None => "***".into(),
    }
}

// ---------- Trang ----------

pub struct NavItem {
    pub href: &'static str,
    pub label: &'static str,
    pub current: bool,
}

pub struct ReviewView {
    pub version_id: String,
    pub app: String,
    pub version: String,
    pub waited: String,
    pub reason: String,
    pub checks: Vec<String>,
    pub diff: Vec<String>,
}

pub struct OrderRow {
    pub id: String,
    pub code: String,
    pub title: String,
    pub status: String,
    pub due: String,
    pub warning: bool,
}

pub struct MilestoneRow {
    pub position: i32,
    pub title: String,
    pub amount: String,
    pub status: String,
    pub due: String,
}

pub struct ThreadRow {
    pub author: String,
    pub body: String,
    pub at: String,
}

pub struct OrderDetail {
    pub id: String,
    pub code: String,
    pub title: String,
    pub status: String,
    pub description: String,
    pub milestones: Vec<MilestoneRow>,
    pub thread: Vec<ThreadRow>,
    pub can_quote: bool,
    pub disputed: bool,
    pub quote_example: String,
}

pub struct RefundRow {
    pub id: String,
    pub transaction: String,
    pub amount: String,
    pub status: String,
    pub reason: String,
    pub approvals: String,
    pub needed: usize,
}

/// Giao dịch thành công: id, mã đơn, cổng, số tiền, đã hoàn.
type TxQueryRow = (Uuid, String, String, i64, i64);

pub struct TxRow {
    pub id: String,
    pub order: String,
    pub provider: String,
    pub amount: String,
    pub refunded: String,
}

pub struct UserRow {
    pub id: String,
    pub email: String,
}

pub struct UserProfile {
    pub id: String,
    pub email: String,
    pub verified: bool,
    pub created: String,
    pub sessions: usize,
    pub orders: Vec<String>,
    pub reason: String,
}

pub struct MemberRow {
    pub subject: String,
    pub email: String,
    pub roles: String,
}

pub struct RoleOption {
    pub code: &'static str,
    pub label: &'static str,
}

#[derive(Template)]
#[template(path = "admin.html")]
struct AdminPage {
    i18n: Lang,
    messages: Vec<Msg>,
    csrf: String,
    me: String,
    roles: String,
    nav: Vec<NavItem>,
    section: &'static str,
    reviews: Vec<ReviewView>,
    clauses: Vec<(String, String)>,
    orders: Vec<OrderRow>,
    order: Option<OrderDetail>,
    refunds: Vec<RefundRow>,
    transactions: Vec<TxRow>,
    query: String,
    users: Vec<UserRow>,
    profile: Option<UserProfile>,
    members: Vec<MemberRow>,
    role_options: Vec<RoleOption>,
}

impl AdminPage {
    fn new(lang: Lang, admin: &Admin, section: &'static str) -> Self {
        let nav = Role::ALL
            .into_iter()
            .filter(|r| admin.has(*r))
            .map(|r| {
                let (href, key) = r.section();
                NavItem {
                    href,
                    label: lang.t(key),
                    current: href.ends_with(section),
                }
            })
            .collect();
        AdminPage {
            i18n: lang,
            messages: Vec::new(),
            csrf: String::new(),
            me: admin.email.clone(),
            roles: admin
                .roles
                .iter()
                .map(|r| r.label(lang))
                .collect::<Vec<_>>()
                .join(", "),
            nav,
            section,
            reviews: Vec::new(),
            clauses: Vec::new(),
            orders: Vec::new(),
            order: None,
            refunds: Vec::new(),
            transactions: Vec::new(),
            query: String::new(),
            users: Vec::new(),
            profile: None,
            members: Vec::new(),
            role_options: Role::ALL
                .into_iter()
                .map(|r| RoleOption {
                    code: r.as_str(),
                    label: r.label(lang),
                })
                .collect(),
        }
    }
}

fn render(
    state: &AppState,
    headers: &HeaderMap,
    mut page: AdminPage,
    status: StatusCode,
) -> Response {
    let (token, set) = csrf::issue(headers, state.public_url.scheme() == "https");
    page.csrf = token;
    let lang = page.i18n;
    let mut res = match page.render() {
        Ok(h) => (status, Html(h)).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "lỗi dựng trang quản trị");
            (StatusCode::INTERNAL_SERVER_ERROR, lang.t("error.render")).into_response()
        }
    };
    if let Some(c) = set {
        res.headers_mut().append(header::SET_COOKIE, c);
    }
    res
}

/// GET /admin
pub async fn home(State(state): State<Arc<AppState>>, lang: Lang, headers: HeaderMap) -> Response {
    let admin = match gate(&state, lang, &headers, "/admin", None).await {
        Ok(a) => a,
        Err(r) => return *r,
    };
    render(
        &state,
        &headers,
        AdminPage::new(lang, &admin, "home"),
        StatusCode::OK,
    )
}

// ---------- Hàng chờ duyệt app ----------

/// Khác biệt scope và quyền so với phiên bản trước (spec "Xem khác biệt quyền").
pub fn manifest_diff(
    lang: Lang,
    prev: Option<&feco_manifest::Manifest>,
    cur: &feco_manifest::Manifest,
) -> Vec<String> {
    let vi = lang == Lang::Vi;
    let purpose = |d: &feco_manifest::ScopeDecl| {
        if vi {
            d.purpose.clone()
        } else {
            d.purpose_en.clone().unwrap_or_else(|| d.purpose.clone())
        }
    };
    let mut out = Vec::new();
    let Some(prev) = prev else {
        out.push(lang.t("admin.diff.first").to_owned());
        for d in &cur.scopes {
            out.push(format!(
                "{} {} ({} {})",
                lang.t("admin.diff.scope"),
                d.scope.as_str(),
                lang.t("admin.diff.purpose"),
                purpose(d)
            ));
        }
        return out;
    };
    for d in &cur.scopes {
        match prev.scopes.iter().find(|p| p.scope == d.scope) {
            None => out.push(format!(
                "{} {} ({} {})",
                lang.t("admin.diff.add_scope"),
                d.scope.as_str(),
                lang.t("admin.diff.purpose"),
                purpose(d)
            )),
            Some(p) if p.purpose != d.purpose => out.push(format!(
                "{} {}: {} → {}",
                lang.t("admin.diff.purpose_changed"),
                d.scope.as_str(),
                p.purpose,
                d.purpose
            )),
            _ => {}
        }
    }
    for p in &prev.scopes {
        if !cur.scopes.iter().any(|d| d.scope == p.scope) {
            out.push(format!(
                "{} {}",
                lang.t("admin.diff.remove_scope"),
                p.scope.as_str()
            ));
        }
    }
    let (a, b) = (&prev.permissions, &cur.permissions);
    for (name, before, after) in [
        ("network", a.network, b.network),
        ("camera", a.camera, b.camera),
        ("microphone", a.microphone, b.microphone),
    ] {
        if after && !before {
            out.push(format!("{} {name}", lang.t("admin.diff.add_permission")));
        }
        if before && !after {
            out.push(format!("{} {name}", lang.t("admin.diff.remove_permission")));
        }
    }
    for f in &b.user_folders {
        if !a.user_folders.contains(f) {
            out.push(format!("{} {f}", lang.t("admin.diff.add_folder")));
        }
    }
    if out.is_empty() {
        out.push(lang.t("admin.diff.none").to_owned());
    }
    out
}

async fn review_queue(
    state: &AppState,
    lang: Lang,
    now: OffsetDateTime,
) -> sqlx::Result<Vec<ReviewView>> {
    let mut out = Vec::new();
    for q in review::manual_queue(&state.db).await? {
        let row: Option<(Value, Option<Value>, OffsetDateTime)> =
            sqlx::query_as("SELECT manifest, checks, created_at FROM app_versions WHERE id = $1")
                .bind(q.version_id)
                .fetch_optional(&state.db)
                .await?;
        let Some((manifest, checks, created)) = row else {
            continue;
        };
        let prev: Option<Value> = sqlx::query_scalar(
            "SELECT manifest FROM app_versions WHERE app_id = $1 AND created_at < $2
               AND status IN ('approved', 'released', 'paused', 'revoked') ORDER BY created_at DESC LIMIT 1",
        )
        .bind(&q.app_id)
        .bind(created)
        .fetch_optional(&state.db)
        .await?;
        let cur: Option<feco_manifest::Manifest> = serde_json::from_value(manifest).ok();
        let prev: Option<feco_manifest::Manifest> =
            prev.and_then(|p| serde_json::from_value(p).ok());
        let diff = cur
            .as_ref()
            .map(|c| manifest_diff(lang, prev.as_ref(), c))
            .unwrap_or_default();
        let checks = match checks {
            Some(Value::Object(m)) => m
                .iter()
                .map(|(k, v)| {
                    format!(
                        "{k}: {}",
                        v.as_str()
                            .map(str::to_owned)
                            .unwrap_or_else(|| v.to_string())
                    )
                })
                .collect(),
            Some(Value::Array(a)) => a
                .iter()
                .map(|v| {
                    v.as_str()
                        .map(str::to_owned)
                        .unwrap_or_else(|| v.to_string())
                })
                .collect(),
            _ => Vec::new(),
        };
        let waited = q.submitted_at.map(|s| now - s).unwrap_or_default();
        out.push(ReviewView {
            version_id: q.version_id.to_string(),
            app: q.app_id,
            version: q.version,
            waited: format!(
                "{}h{:02}",
                waited.whole_hours(),
                waited.whole_minutes() % 60
            ),
            reason: q.reason,
            checks,
            diff,
        });
    }
    Ok(out)
}

async fn render_reviews(
    state: &AppState,
    lang: Lang,
    headers: &HeaderMap,
    admin: &Admin,
    messages: Vec<Msg>,
    status: StatusCode,
) -> Response {
    let mut page = AdminPage::new(lang, admin, "reviews");
    page.messages = messages;
    match review_queue(state, lang, OffsetDateTime::now_utc()).await {
        Ok(r) => page.reviews = r,
        Err(e) => {
            tracing::error!(error = %e, "không đọc được hàng chờ duyệt");
            page.messages.push(Msg::error(lang.t("error.unavailable")));
        }
    }
    page.clauses = review::CLAUSES
        .iter()
        .map(|c| {
            (
                c.code.to_owned(),
                format!(
                    "{} {}",
                    c.number,
                    if lang == Lang::Vi {
                        c.title_vi
                    } else {
                        c.title_en
                    }
                ),
            )
        })
        .collect();
    render(state, headers, page, status)
}

/// GET /admin/reviews (Người duyệt app)
pub async fn reviews(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    headers: HeaderMap,
) -> Response {
    let admin = match gate(
        &state,
        lang,
        &headers,
        "/admin/reviews",
        Some(Role::Reviewer),
    )
    .await
    {
        Ok(a) => a,
        Err(r) => return *r,
    };
    render_reviews(&state, lang, &headers, &admin, Vec::new(), StatusCode::OK).await
}

/// POST /admin/reviews/{version}/{approve|reject}: quyết định kèm lý do theo điều khoản.
pub async fn review_action(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    Path((version, action)): Path<(Uuid, String)>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let f = form(&body);
    if !csrf::verify(&headers, Some(field(&f, csrf::FIELD))) {
        return page_status(lang, StatusCode::FORBIDDEN, "error.csrf");
    }
    let admin = match gate(
        &state,
        lang,
        &headers,
        "/admin/reviews",
        Some(Role::Reviewer),
    )
    .await
    {
        Ok(a) => a,
        Err(r) => return *r,
    };
    let note = field(&f, "note").trim();
    if note.is_empty() {
        return render_reviews(
            &state,
            lang,
            &headers,
            &admin,
            vec![Msg::error(lang.t("admin.need_reason"))],
            StatusCode::BAD_REQUEST,
        )
        .await;
    }
    let result = match action.as_str() {
        "approve" => review::approve(&state, version, &admin.email, note).await,
        "reject" => review::reject(&state, version, &admin.email, field(&f, "clause"), note).await,
        _ => return page_status(lang, StatusCode::NOT_FOUND, "admin.not_found"),
    };
    match result {
        Ok(()) => {
            audit::record(
                &state.db,
                admin.actor(),
                &format!("admin.review_{action}"),
                Some(&version.to_string()),
                None,
                json!({"clause": field(&f, "clause"), "note": note}),
            )
            .await;
            render_reviews(
                &state,
                lang,
                &headers,
                &admin,
                vec![Msg::success(lang.t("admin.saved"))],
                StatusCode::OK,
            )
            .await
        }
        Err(e) => {
            tracing::warn!(error = %e, "quyết định duyệt thất bại");
            render_reviews(
                &state,
                lang,
                &headers,
                &admin,
                vec![Msg::error(e.to_string())],
                StatusCode::BAD_REQUEST,
            )
            .await
        }
    }
}

// ---------- Quản lý đơn xây app ----------

/// Đơn còn dưới 2 ngày tới hạn mốc mà chưa bàn giao được cảnh báo và đứng đầu danh sách.
pub const DUE_WARNING_DAYS: i64 = 2;

pub async fn order_rows(
    state: &AppState,
    lang: Lang,
    now: OffsetDateTime,
) -> sqlx::Result<Vec<OrderRow>> {
    let vi = lang == Lang::Vi;
    let rows: Vec<(Uuid, String, String, String, Option<OffsetDateTime>)> = sqlx::query_as(
        "SELECT o.id, o.code, o.title, o.status,
                (SELECT min(m.due_at) FROM order_milestones m JOIN order_quotes q ON q.id = m.quote_id
                  WHERE m.order_id = o.id AND q.status = 'accepted' AND m.status IN ('pending', 'awaiting_payment', 'in_progress'))
         FROM orders o WHERE o.status NOT IN ('completed', 'cancelled', 'draft') ORDER BY o.created_at DESC LIMIT 500",
    )
    .fetch_all(&state.db)
    .await?;
    let mut out: Vec<(bool, Option<OffsetDateTime>, OrderRow)> = rows
        .into_iter()
        .map(|(id, code, title, status, due)| {
            let warning = due.is_some_and(|d| d - now < Duration::days(DUE_WARNING_DAYS));
            (
                warning,
                due,
                OrderRow {
                    id: id.to_string(),
                    code,
                    title,
                    status: Status::parse(&status)
                        .map(|s| s.label(vi).to_owned())
                        .unwrap_or(status),
                    due: due.map(orders::date).unwrap_or_default(),
                    warning,
                },
            )
        })
        .collect();
    // Cảnh báo trước (hạn gần nhất trước), rồi các đơn khác.
    out.sort_by(|a, b| {
        b.0.cmp(&a.0).then_with(|| {
            if a.0 {
                a.1.cmp(&b.1)
            } else {
                std::cmp::Ordering::Equal
            }
        })
    });
    Ok(out.into_iter().map(|x| x.2).collect())
}

/// GET /admin/orders (Quản lý dự án)
pub async fn orders_list(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    headers: HeaderMap,
) -> Response {
    let admin = match gate(
        &state,
        lang,
        &headers,
        "/admin/orders",
        Some(Role::ProjectManager),
    )
    .await
    {
        Ok(a) => a,
        Err(r) => return *r,
    };
    let mut page = AdminPage::new(lang, &admin, "orders");
    match order_rows(&state, lang, OffsetDateTime::now_utc()).await {
        Ok(o) => page.orders = o,
        Err(e) => {
            tracing::error!(error = %e, "không đọc được danh sách đơn");
            page.messages.push(Msg::error(lang.t("error.unavailable")));
        }
    }
    render(&state, &headers, page, StatusCode::OK)
}

async fn render_order(
    state: &AppState,
    lang: Lang,
    headers: &HeaderMap,
    admin: &Admin,
    id: Uuid,
    messages: Vec<Msg>,
    status: StatusCode,
) -> Response {
    let vi = lang == Lang::Vi;
    let mut page = AdminPage::new(lang, admin, "orders");
    page.messages = messages;
    let detail: Result<OrderDetail, OrderError> = async {
        let o = orders::get(&state.db, id).await?;
        let quote = orders::current_quote(&state.db, id).await?;
        let milestones = quote
            .map(|q| {
                q.milestones
                    .iter()
                    .map(|m| MilestoneRow {
                        position: m.position,
                        title: m.title.clone(),
                        amount: orders::vnd(m.amount_vnd),
                        status: m.status.clone(),
                        due: m.due_at.map(orders::date).unwrap_or_default(),
                    })
                    .collect()
            })
            .unwrap_or_default();
        let thread = orders::messages(&state.db, id)
            .await?
            .into_iter()
            .map(|m| ThreadRow {
                author: if m.author_name.is_empty() {
                    m.author
                } else {
                    format!("{} ({})", m.author, m.author_name)
                },
                body: m.body,
                at: when(m.at),
            })
            .collect();
        Ok(OrderDetail {
            id: o.id.to_string(),
            code: o.code.clone(),
            title: o.title.clone(),
            status: o.status.label(vi).to_owned(),
            description: o.description.clone(),
            milestones,
            thread,
            can_quote: matches!(o.status, Status::Submitted | Status::Quoted),
            disputed: o.status == Status::Disputed,
            quote_example: include_str!("../../../docs/order-quote.example.json").to_owned(),
        })
    }
    .await;
    match detail {
        Ok(d) => page.order = Some(d),
        Err(e) => page.messages.push(explain(lang, e)),
    }
    render(state, headers, page, status)
}

/// GET /admin/orders/{id}
pub async fn order_detail(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Response {
    let admin = match gate(
        &state,
        lang,
        &headers,
        "/admin/orders",
        Some(Role::ProjectManager),
    )
    .await
    {
        Ok(a) => a,
        Err(r) => return *r,
    };
    render_order(
        &state,
        lang,
        &headers,
        &admin,
        id,
        Vec::new(),
        StatusCode::OK,
    )
    .await
}

/// POST /admin/orders/{id}/{quote|deliver|message|resolve}
pub async fn order_action(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    Path((id, action)): Path<(Uuid, String)>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let f = form(&body);
    if !csrf::verify(&headers, Some(field(&f, csrf::FIELD))) {
        return page_status(lang, StatusCode::FORBIDDEN, "error.csrf");
    }
    let admin = match gate(
        &state,
        lang,
        &headers,
        "/admin/orders",
        Some(Role::ProjectManager),
    )
    .await
    {
        Ok(a) => a,
        Err(r) => return *r,
    };
    let now = OffsetDateTime::now_utc();
    let result: Result<(), OrderError> = match action.as_str() {
        "quote" => match serde_json::from_str::<orders::QuoteInput>(field(&f, "quote")) {
            Ok(q) => orders::create_quote(&state, id, &admin.email, &q, now)
                .await
                .map(|_| ()),
            Err(e) => Err(orders::problem(
                StatusCode::BAD_REQUEST,
                "invalid_quote",
                format!("Báo giá không hợp lệ: {e}"),
                format!("Invalid quote: {e}"),
            )),
        },
        "deliver" => {
            let d = orders::Delivery {
                app_id: field(&f, "app").to_owned(),
                app_version: field(&f, "version").to_owned(),
                notes: field(&f, "notes").to_owned(),
                check_report: field(&f, "checks").to_owned(),
            };
            orders::deliver(
                &state,
                id,
                field(&f, "milestone").parse().unwrap_or(0),
                &admin.email,
                &d,
                now,
            )
            .await
        }
        "message" => orders::post_message(&state, id, Some(&admin.email), field(&f, "body"))
            .await
            .map(|_| ()),
        "resolve" => {
            let r = match field(&f, "resolution") {
                "continue" => orders::Resolution::Continue,
                "full" => orders::Resolution::FullRefund,
                _ => orders::Resolution::PartialRefund(
                    field(&f, "amount")
                        .replace(['.', ','], "")
                        .parse()
                        .unwrap_or(0),
                ),
            };
            orders::resolve_dispute(&state, id, &admin.email, r)
                .await
                .map(|_| ())
        }
        _ => return page_status(lang, StatusCode::NOT_FOUND, "admin.not_found"),
    };
    match result {
        Ok(()) => {
            audit::record(
                &state.db,
                admin.actor(),
                &format!("admin.order_{action}"),
                Some(&id.to_string()),
                None,
                json!({}),
            )
            .await;
            render_order(
                &state,
                lang,
                &headers,
                &admin,
                id,
                vec![Msg::success(lang.t("admin.saved"))],
                StatusCode::OK,
            )
            .await
        }
        Err(e) => {
            render_order(
                &state,
                lang,
                &headers,
                &admin,
                id,
                vec![explain(lang, e)],
                StatusCode::BAD_REQUEST,
            )
            .await
        }
    }
}

// ---------- Hoàn tiền (Tài chính) ----------

async fn render_refunds(
    state: &AppState,
    lang: Lang,
    headers: &HeaderMap,
    admin: &Admin,
    messages: Vec<Msg>,
    status: StatusCode,
) -> Response {
    let vi = lang == Lang::Vi;
    let mut page = AdminPage::new(lang, admin, "refunds");
    page.messages = messages;
    match payments::open_refunds(state).await {
        Ok(r) => {
            page.refunds = r
                .into_iter()
                .map(|r| RefundRow {
                    id: r.id.to_string(),
                    transaction: r.transaction_id.to_string(),
                    amount: orders::vnd(r.amount_vnd),
                    status: r.label(vi).to_owned(),
                    needed: r.approvals_needed(),
                    approvals: r.approvals.join(", "),
                    reason: r.reason,
                })
                .collect()
        }
        Err(e) => page.messages.push(explain(lang, e)),
    }
    let txs: Result<Vec<TxQueryRow>, sqlx::Error> = sqlx::query_as(
        "SELECT t.id, o.code, t.provider, t.amount_vnd, t.refunded_vnd FROM payment_transactions t
         JOIN payment_requests r ON r.id = t.request_id JOIN orders o ON o.id = r.order_id
         WHERE t.status = 'succeeded' ORDER BY t.settled_at DESC NULLS LAST LIMIT 50",
    )
    .fetch_all(&state.db)
    .await;
    if let Ok(txs) = txs {
        page.transactions = txs
            .into_iter()
            .map(|(id, order, provider, amount, refunded)| TxRow {
                id: id.to_string(),
                order,
                provider,
                amount: orders::vnd(amount),
                refunded: orders::vnd(refunded),
            })
            .collect();
    }
    render(state, headers, page, status)
}

/// GET /admin/refunds (Tài chính)
pub async fn refunds(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    headers: HeaderMap,
) -> Response {
    let admin = match gate(
        &state,
        lang,
        &headers,
        "/admin/refunds",
        Some(Role::Finance),
    )
    .await
    {
        Ok(a) => a,
        Err(r) => return *r,
    };
    render_refunds(&state, lang, &headers, &admin, Vec::new(), StatusCode::OK).await
}

/// POST /admin/refunds: tạo lệnh hoàn (transaction, amount, reason). Chỉ vai trò Tài chính.
pub async fn create_refund(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let f = form(&body);
    if !csrf::verify(&headers, Some(field(&f, csrf::FIELD))) {
        return page_status(lang, StatusCode::FORBIDDEN, "error.csrf");
    }
    let admin = match gate(
        &state,
        lang,
        &headers,
        "/admin/refunds",
        Some(Role::Finance),
    )
    .await
    {
        Ok(a) => a,
        Err(r) => return *r,
    };
    let tx: Option<Uuid> = field(&f, "transaction").trim().parse().ok();
    let amount: i64 = field(&f, "amount")
        .replace(['.', ',', ' '], "")
        .parse()
        .unwrap_or(0);
    let result = match tx {
        Some(tx) => {
            payments::request_refund(
                &state,
                tx,
                amount,
                field(&f, "reason"),
                Requester::Admin(admin.email.clone()),
                None,
            )
            .await
        }
        None => Err(orders::problem(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "Thiếu mã giao dịch.",
            "Missing transaction id.",
        )),
    };
    match result {
        Ok(id) => {
            audit::record(
                &state.db,
                admin.actor(),
                "admin.refund_create",
                Some(&id.to_string()),
                None,
                json!({"amount": amount}),
            )
            .await;
            render_refunds(
                &state,
                lang,
                &headers,
                &admin,
                vec![Msg::success(lang.t("admin.saved"))],
                StatusCode::OK,
            )
            .await
        }
        Err(e) => {
            let msg = explain(lang, e);
            render_refunds(
                &state,
                lang,
                &headers,
                &admin,
                vec![msg],
                StatusCode::BAD_REQUEST,
            )
            .await
        }
    }
}

/// POST /admin/refunds/{id}/approve (Tài chính; người thứ hai với lệnh từ 10.000.000 ₫).
pub async fn approve_refund(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let f = form(&body);
    if !csrf::verify(&headers, Some(field(&f, csrf::FIELD))) {
        return page_status(lang, StatusCode::FORBIDDEN, "error.csrf");
    }
    let admin = match gate(
        &state,
        lang,
        &headers,
        "/admin/refunds",
        Some(Role::Finance),
    )
    .await
    {
        Ok(a) => a,
        Err(r) => return *r,
    };
    match payments::approve_refund(&state, id, &admin.email).await {
        Ok(_) => {
            render_refunds(
                &state,
                lang,
                &headers,
                &admin,
                vec![Msg::success(lang.t("admin.saved"))],
                StatusCode::OK,
            )
            .await
        }
        Err(e) => {
            let msg = explain(lang, e);
            render_refunds(
                &state,
                lang,
                &headers,
                &admin,
                vec![msg],
                StatusCode::BAD_REQUEST,
            )
            .await
        }
    }
}

// ---------- Hỗ trợ người dùng ----------

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub q: Option<String>,
}

/// GET /admin/users?q= (Hỗ trợ): tìm theo email hoặc mã đơn; chỉ hiện email đã che.
pub async fn users(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    Query(q): Query<SearchQuery>,
    headers: HeaderMap,
) -> Response {
    let admin = match gate(&state, lang, &headers, "/admin/users", Some(Role::Support)).await {
        Ok(a) => a,
        Err(r) => return *r,
    };
    let mut page = AdminPage::new(lang, &admin, "users");
    let query = q.q.unwrap_or_default().trim().to_owned();
    if !query.is_empty() {
        let mut found: Vec<(String, String)> = Vec::new();
        if query.contains('@') {
            if let Ok(all) = state.kratos.list_identities().await {
                found.extend(
                    all.into_iter()
                        .filter(|i| {
                            i["traits"]["email"]
                                .as_str()
                                .is_some_and(|e| e.eq_ignore_ascii_case(&query))
                        })
                        .filter_map(|i| {
                            Some((
                                i["id"].as_str()?.to_owned(),
                                i["traits"]["email"].as_str()?.to_owned(),
                            ))
                        }),
                );
            }
        } else if let Ok(o) = orders::by_code(&state.db, &query).await {
            found.push((o.customer.to_string(), o.customer_email));
        }
        page.users = found
            .into_iter()
            .map(|(id, email)| UserRow {
                id,
                email: mask_email(&email),
            })
            .collect();
        audit::record(
            &state.db,
            admin.actor(),
            "admin.user_search",
            None,
            None,
            json!({"by": if query.contains('@') { "email" } else { "order" }}),
        )
        .await;
        if page.users.is_empty() {
            page.messages.push(Msg::info(lang.t("admin.no_user")));
        }
    }
    page.query = query;
    render(&state, &headers, page, StatusCode::OK)
}

async fn render_profile(
    state: &AppState,
    lang: Lang,
    headers: &HeaderMap,
    admin: &Admin,
    id: Uuid,
    reason: &str,
    messages: Vec<Msg>,
) -> Response {
    let mut page = AdminPage::new(lang, admin, "users");
    page.messages = messages;
    let identity = state
        .kratos
        .get_identity(&id.to_string())
        .await
        .ok()
        .flatten();
    let Some(identity) = identity else {
        return page_status(lang, StatusCode::NOT_FOUND, "admin.no_user");
    };
    let email = identity["traits"]["email"].as_str().unwrap_or_default();
    let sessions = state
        .kratos
        .identity_sessions(&id.to_string())
        .await
        .map(|s| s.len())
        .unwrap_or(0);
    let orders: Vec<String> =
        sqlx::query_scalar("SELECT code FROM orders WHERE customer = $1 ORDER BY created_at DESC")
            .bind(id)
            .fetch_all(&state.db)
            .await
            .unwrap_or_default();
    page.profile = Some(UserProfile {
        id: id.to_string(),
        email: mask_email(email),
        verified: identity["verifiable_addresses"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|a| a["verified"] == true),
        created: identity["created_at"]
            .as_str()
            .unwrap_or_default()
            .chars()
            .take(10)
            .collect(),
        sessions,
        orders,
        reason: reason.to_owned(),
    });
    render(state, headers, page, StatusCode::OK)
}

/// POST /admin/users/{id}/{view|resend|logout}: luôn kèm lý do (ví dụ mã phiếu hỗ trợ), ghi
/// nhật ký kiểm toán.
pub async fn user_action(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    Path((id, action)): Path<(Uuid, String)>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let f = form(&body);
    if !csrf::verify(&headers, Some(field(&f, csrf::FIELD))) {
        return page_status(lang, StatusCode::FORBIDDEN, "error.csrf");
    }
    let admin = match gate(&state, lang, &headers, "/admin/users", Some(Role::Support)).await {
        Ok(a) => a,
        Err(r) => return *r,
    };
    let reason = field(&f, "reason").trim();
    if reason.is_empty() {
        let mut page = AdminPage::new(lang, &admin, "users");
        page.messages.push(Msg::error(lang.t("admin.need_reason")));
        return render(&state, &headers, page, StatusCode::BAD_REQUEST);
    }
    let target = id.to_string();
    let message = match action.as_str() {
        "view" => None,
        "resend" => {
            let email = state
                .kratos
                .get_identity(&target)
                .await
                .ok()
                .flatten()
                .and_then(|i| i["traits"]["email"].as_str().map(str::to_owned));
            let sent = match email {
                Some(e) => state.kratos.send_verification(&e).await.unwrap_or(false),
                None => false,
            };
            Some(if sent {
                Msg::success(lang.t("admin.verification_sent"))
            } else {
                Msg::error(lang.t("error.unavailable"))
            })
        }
        "logout" => Some(match state.kratos.revoke_identity_sessions(&target).await {
            Ok(()) => Msg::success(lang.t("admin.sessions_revoked")),
            Err(_) => Msg::error(lang.t("error.unavailable")),
        }),
        _ => return page_status(lang, StatusCode::NOT_FOUND, "admin.not_found"),
    };
    audit::record(
        &state.db,
        admin.actor(),
        &format!("admin.user_{action}"),
        Some(&target),
        None,
        json!({"reason": reason}),
    )
    .await;
    render_profile(
        &state,
        lang,
        &headers,
        &admin,
        id,
        reason,
        message.into_iter().collect(),
    )
    .await
}

// ---------- Thành viên quản trị (Quản trị hệ thống) ----------

async fn render_members(
    state: &AppState,
    lang: Lang,
    headers: &HeaderMap,
    admin: &Admin,
    messages: Vec<Msg>,
    status: StatusCode,
) -> Response {
    let mut page = AdminPage::new(lang, admin, "members");
    page.messages = messages;
    let rows: Vec<(Uuid, String, Vec<String>)> =
        sqlx::query_as("SELECT subject, email, roles FROM admin_members ORDER BY email")
            .fetch_all(&state.db)
            .await
            .unwrap_or_default();
    page.members = rows
        .into_iter()
        .map(|(s, e, r)| MemberRow {
            subject: s.to_string(),
            email: e,
            roles: r
                .iter()
                .filter_map(|x| Role::parse(x))
                .map(|x| x.label(lang))
                .collect::<Vec<_>>()
                .join(", "),
        })
        .collect();
    render(state, headers, page, status)
}

/// GET /admin/members
pub async fn members(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    headers: HeaderMap,
) -> Response {
    let admin = match gate(
        &state,
        lang,
        &headers,
        "/admin/members",
        Some(Role::Sysadmin),
    )
    .await
    {
        Ok(a) => a,
        Err(r) => return *r,
    };
    render_members(&state, lang, &headers, &admin, Vec::new(), StatusCode::OK).await
}

/// Thêm hoặc đổi vai trò của một thành viên quản trị (dùng chung cho web và `feco-catalog`).
pub async fn set_member(
    state: &AppState,
    by: &str,
    email: &str,
    roles: &[Role],
) -> Result<Uuid, String> {
    let subject: Uuid = state
        .kratos
        .list_identities()
        .await
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|i| {
            i["traits"]["email"]
                .as_str()
                .is_some_and(|e| e.eq_ignore_ascii_case(email))
        })
        .and_then(|i| i["id"].as_str().and_then(|s| s.parse().ok()))
        .ok_or("không có người dùng với email này")?;
    if roles.is_empty() {
        sqlx::query("DELETE FROM admin_members WHERE subject = $1")
            .bind(subject)
            .execute(&state.db)
            .await
            .map_err(|e| e.to_string())?;
    } else {
        let roles: Vec<&str> = roles.iter().map(|r| r.as_str()).collect();
        sqlx::query(
            "INSERT INTO admin_members (subject, email, roles, added_by) VALUES ($1, $2, $3, $4)
             ON CONFLICT (subject) DO UPDATE SET roles = $3, email = $2",
        )
        .bind(subject)
        .bind(email)
        .bind(&roles)
        .bind(by)
        .execute(&state.db)
        .await
        .map_err(|e| e.to_string())?;
    }
    audit::record(
        &state.db,
        Actor::Admin(by.to_owned()),
        "admin.member_set",
        Some(&subject.to_string()),
        None,
        json!({"roles": roles.iter().map(|r| r.as_str()).collect::<Vec<_>>()}),
    )
    .await;
    Ok(subject)
}

/// POST /admin/members (email, roles[]): thêm, đổi vai trò, hoặc bỏ hết vai trò để gỡ.
pub async fn save_member(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let f = form(&body);
    if !csrf::verify(&headers, Some(field(&f, csrf::FIELD))) {
        return page_status(lang, StatusCode::FORBIDDEN, "error.csrf");
    }
    let admin = match gate(
        &state,
        lang,
        &headers,
        "/admin/members",
        Some(Role::Sysadmin),
    )
    .await
    {
        Ok(a) => a,
        Err(r) => return *r,
    };
    let roles: Vec<Role> = f
        .iter()
        .filter(|(k, _)| k == "role")
        .filter_map(|(_, v)| Role::parse(v))
        .collect();
    let email = field(&f, "email").trim();
    if email.eq_ignore_ascii_case(&admin.email) && !roles.contains(&Role::Sysadmin) {
        return render_members(
            &state,
            lang,
            &headers,
            &admin,
            vec![Msg::error(lang.t("admin.keep_own_sysadmin"))],
            StatusCode::BAD_REQUEST,
        )
        .await;
    }
    match set_member(&state, &admin.email, email, &roles).await {
        Ok(_) => {
            render_members(
                &state,
                lang,
                &headers,
                &admin,
                vec![Msg::success(lang.t("admin.saved"))],
                StatusCode::OK,
            )
            .await
        }
        Err(e) => {
            render_members(
                &state,
                lang,
                &headers,
                &admin,
                vec![Msg::error(e)],
                StatusCode::BAD_REQUEST,
            )
            .await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emails_are_masked() {
        assert_eq!(mask_email("nguyenvana@example.com"), "ng***@example.com");
        assert_eq!(mask_email("a@b.vn"), "a***@b.vn");
    }

    #[test]
    fn security_key_methods() {
        let s = json!({"authentication_methods": [{"method": "password"}, {"method": "totp"}]});
        assert!(!has_security_key(&s), "TOTP không phải khóa bảo mật");
        let s = json!({"authentication_methods": [{"method": "password"}, {"method": "webauthn"}]});
        assert!(has_security_key(&s));
    }

    /// Spec "Không giả danh người dùng": không API nào của bảng quản trị cấp token, mã khởi chạy
    /// hay phiên đứng tên người dùng.
    #[test]
    fn no_impersonation_api() {
        let src = include_str!("admin.rs");
        let code = &src[..src.find("#[cfg(test)]").unwrap()];
        for forbidden in [
            ["launch", "::issue"].concat(),
            ["hydra", "."].concat(),
            ["accept_", "login"].concat(),
            ["create_", "session"].concat(),
            ["access_", "token"].concat(),
        ] {
            assert!(
                !code.contains(&forbidden),
                "bảng quản trị không được dùng {forbidden}"
            );
        }
    }

    #[test]
    fn diff_shows_new_scope_with_purpose() {
        let m = |extra: &str| {
            feco_manifest::parse(&format!(
                r#"id = "soundlab"
name = "SoundLab"
version = "1.3.0"
license = "MIT"
privacy_policy = "https://soundlab.example/privacy"
summary = "Thu âm"
category = "Âm nhạc"

[[platforms]]
os = "windows"
arch = "x64"
entry = "SoundLab.exe"

[scopes.profile]
purpose = "Hiển thị trên bản nhạc bạn chia sẻ"
{extra}"#
            ))
            .unwrap()
        };
        let prev = m("");
        let cur = m(
            "\n[scopes.phone]\npurpose = \"gửi mã xác nhận\"\n\n[permissions]\nmicrophone = true\n",
        );
        let diff = manifest_diff(Lang::Vi, Some(&prev), &cur);
        assert!(
            diff.contains(&"Thêm scope: phone (mục đích: gửi mã xác nhận)".to_owned()),
            "{diff:?}"
        );
        assert!(
            diff.contains(&"Thêm quyền: microphone".to_owned()),
            "{diff:?}"
        );
    }
}
