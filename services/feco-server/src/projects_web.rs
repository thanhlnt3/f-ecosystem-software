//! Trang web của dự án phần mềm và vai trò (thay đổi OpenSpec fcoin-wallet-and-marketplace):
//! - `GET/POST /account/role`: xin vai trò;
//! - `GET /projects`: dự án của tôi, hồ sơ nhận việc; `GET /projects/providers`: người nhận việc;
//! - `GET/POST /projects/new`: gửi yêu cầu; `POST /projects/profile`: lưu hồ sơ nhận việc;
//! - `GET /projects/{id}`, `POST /projects/{id}/{action}`: chi tiết và các bước của dự án;
//! - `GET /admin/projects`, `GET /admin/projects/{id}`, `POST /admin/projects/{id}/resolve`.

use std::sync::Arc;

use askama::Template;
use axum::{
    body::Bytes,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    admin::{self, Role},
    chat,
    connected::{current_subject, format_time},
    csrf,
    i18n::Lang,
    projects::{self, Project, Side},
    roles::{self, PlatformRole},
    views::Msg,
    wallet::{self, coin},
    wallet_web::{explain, form, get, identity_email, render, see_other},
    AppState,
};

fn status_key(s: &str) -> &'static str {
    match s {
        "requested" => "proj.st.requested",
        "negotiating" => "proj.st.negotiating",
        "agreed" => "proj.st.agreed",
        "in_progress" => "proj.st.in_progress",
        "delivered" => "proj.st.delivered",
        "completed" => "proj.st.completed",
        "disputed" => "proj.st.disputed",
        "resolved" => "proj.st.resolved",
        "cancelled" => "proj.st.cancelled",
        _ => "proj.st.refunded",
    }
}

// ---------- Vai trò ----------

pub struct RoleChoice {
    pub code: &'static str,
    pub label: &'static str,
    pub organization: bool,
}

#[derive(Template)]
#[template(path = "account_role.html")]
struct RolePage {
    i18n: Lang,
    messages: Vec<Msg>,
    csrf: String,
    role: &'static str,
    role_code: &'static str,
    choices: Vec<RoleChoice>,
    pending: Option<String>,
    last_decision: Option<String>,
    is_admin: bool,
}

async fn render_role(
    state: &AppState,
    lang: Lang,
    headers: &HeaderMap,
    subject: Uuid,
    messages: Vec<Msg>,
    status: StatusCode,
) -> Response {
    let role = roles::role_of(&state.db, subject)
        .await
        .unwrap_or(PlatformRole::User);
    let latest = roles::latest_request(&state.db, subject)
        .await
        .ok()
        .flatten();
    let (token, set) = csrf::issue(headers, state.public_url.scheme() == "https");
    render(
        RolePage {
            i18n: lang,
            messages,
            csrf: token,
            role: role.label(lang),
            role_code: role.as_str(),
            choices: PlatformRole::REQUESTABLE
                .into_iter()
                .filter(|r| *r != role)
                .map(|r| RoleChoice {
                    code: r.as_str(),
                    label: r.label(lang),
                    organization: r.is_organization(),
                })
                .collect(),
            pending: latest
                .as_ref()
                .filter(|r| r.status == "pending")
                .map(|r| r.platform_role().label(lang).to_owned()),
            last_decision: latest
                .filter(|r| r.status == "rejected")
                .and_then(|r| r.decision_note),
            is_admin: role == PlatformRole::Admin,
        },
        lang,
        status,
        set,
    )
}

/// GET /account/role
pub async fn role_page(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    headers: HeaderMap,
) -> Response {
    let subject = match current_subject(&state, lang, &headers, "/account/role").await {
        Ok(s) => s,
        Err(r) => return *r,
    };
    render_role(&state, lang, &headers, subject, Vec::new(), StatusCode::OK).await
}

/// POST /account/role
pub async fn role_request(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let f = form(&body);
    if !csrf::verify(&headers, Some(get(&f, csrf::FIELD))) {
        return (StatusCode::FORBIDDEN, lang.t("error.csrf")).into_response();
    }
    let subject = match current_subject(&state, lang, &headers, "/account/role").await {
        Ok(s) => s,
        Err(r) => return *r,
    };
    let Some(role) = PlatformRole::parse(get(&f, "role")) else {
        return render_role(
            &state,
            lang,
            &headers,
            subject,
            vec![Msg::error(lang.t("role.err.not_requestable"))],
            StatusCode::BAD_REQUEST,
        )
        .await;
    };
    let email = identity_email(&state, subject).await;
    let org = roles::Organization {
        legal_name: get(&f, "legal_name").to_owned(),
        tax_code: get(&f, "tax_code").to_owned(),
    };
    let (messages, status) =
        match roles::request_role(&state, subject, &email, role, get(&f, "note"), Some(&org)).await
        {
            Ok(_) => (
                vec![Msg::success(lang.t("role.request_sent"))],
                StatusCode::OK,
            ),
            Err(e) => {
                let status = if matches!(e, roles::RoleError::Db(_)) {
                    StatusCode::SERVICE_UNAVAILABLE
                } else {
                    StatusCode::BAD_REQUEST
                };
                (vec![Msg::error(lang.t(e.key()))], status)
            }
        };
    render_role(&state, lang, &headers, subject, messages, status).await
}

// ---------- Danh sách, người nhận việc, hồ sơ, yêu cầu mới ----------

pub struct ProjectRow {
    pub id: String,
    pub code: String,
    pub title: String,
    pub side: &'static str,
    pub other: String,
    pub price: String,
    pub status: String,
    pub status_label: &'static str,
    pub updated: String,
}

pub struct ProviderView {
    pub id: String,
    pub name: String,
    pub kind: &'static str,
    pub headline: String,
    pub skills: String,
}

#[derive(Template)]
#[template(path = "projects.html")]
struct ProjectsPage {
    i18n: Lang,
    messages: Vec<Msg>,
    csrf: String,
    /// `mine`, `providers` hoặc `new`.
    section: &'static str,
    role: &'static str,
    can_commission: bool,
    can_take: bool,
    projects: Vec<ProjectRow>,
    providers: Vec<ProviderView>,
    query: String,
    // Hồ sơ nhận việc.
    profile_name: String,
    profile_headline: String,
    profile_skills: String,
    profile_accepting: bool,
    has_profile: bool,
    // Yêu cầu mới.
    provider: Option<ProviderView>,
    title: String,
    description: String,
    budget: String,
    days: String,
}

impl ProjectsPage {
    fn new(lang: Lang, section: &'static str, role: PlatformRole) -> Self {
        ProjectsPage {
            i18n: lang,
            messages: Vec::new(),
            csrf: String::new(),
            section,
            role: role.label(lang),
            can_commission: role.can_commission(),
            can_take: role.can_take_projects(),
            projects: Vec::new(),
            providers: Vec::new(),
            query: String::new(),
            profile_name: String::new(),
            profile_headline: String::new(),
            profile_skills: String::new(),
            profile_accepting: true,
            has_profile: false,
            provider: None,
            title: String::new(),
            description: String::new(),
            budget: String::new(),
            days: String::new(),
        }
    }
}

fn provider_view(p: projects::Profile) -> ProviderView {
    ProviderView {
        id: p.subject.to_string(),
        kind: if p.kind == "company" {
            "role.dev_company"
        } else {
            "role.developer"
        },
        name: p.display_name,
        headline: p.headline,
        skills: p.skills,
    }
}

fn finish(
    state: &AppState,
    headers: &HeaderMap,
    mut page: ProjectsPage,
    status: StatusCode,
) -> Response {
    let (token, set) = csrf::issue(headers, state.public_url.scheme() == "https");
    page.csrf = token;
    let lang = page.i18n;
    render(page, lang, status, set)
}

async fn render_mine(
    state: &AppState,
    lang: Lang,
    headers: &HeaderMap,
    subject: Uuid,
    messages: Vec<Msg>,
    status: StatusCode,
) -> Response {
    let role = roles::role_of(&state.db, subject)
        .await
        .unwrap_or(PlatformRole::User);
    let mut page = ProjectsPage::new(lang, "mine", role);
    page.messages = messages;
    page.projects = projects::list_for(&state.db, subject)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|p| {
            let side = p.side_of(subject).unwrap_or(Side::Customer);
            ProjectRow {
                id: p.id.to_string(),
                code: p.code,
                title: p.title,
                side: match side {
                    Side::Customer => "proj.as_customer",
                    Side::Provider => "proj.as_provider",
                },
                other: match side {
                    Side::Customer => p.provider_email,
                    Side::Provider => p.customer_email,
                },
                price: p
                    .price_coin
                    .or(p.budget_coin)
                    .map(|c| coin(c, lang))
                    .unwrap_or_else(|| "—".into()),
                status_label: status_key(&p.status),
                status: p.status,
                updated: format_time(p.updated_at),
            }
        })
        .collect();
    if let Ok(Some(pr)) = projects::profile(&state.db, subject).await {
        page.has_profile = true;
        page.profile_name = pr.display_name;
        page.profile_headline = pr.headline;
        page.profile_skills = pr.skills;
        page.profile_accepting = pr.accepting;
    }
    finish(state, headers, page, status)
}

/// GET /projects
pub async fn list(State(state): State<Arc<AppState>>, lang: Lang, headers: HeaderMap) -> Response {
    let subject = match current_subject(&state, lang, &headers, "/projects").await {
        Ok(s) => s,
        Err(r) => return *r,
    };
    render_mine(&state, lang, &headers, subject, Vec::new(), StatusCode::OK).await
}

#[derive(Deserialize, Default)]
pub struct ProvidersQuery {
    pub q: Option<String>,
    pub provider: Option<Uuid>,
}

/// GET /projects/providers
pub async fn providers(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    headers: HeaderMap,
    Query(q): Query<ProvidersQuery>,
) -> Response {
    let subject = match current_subject(&state, lang, &headers, "/projects/providers").await {
        Ok(s) => s,
        Err(r) => return *r,
    };
    let role = roles::role_of(&state.db, subject)
        .await
        .unwrap_or(PlatformRole::User);
    let mut page = ProjectsPage::new(lang, "providers", role);
    page.query = q.q.clone().unwrap_or_default();
    page.providers = projects::providers(&state.db, &page.query)
        .await
        .unwrap_or_default()
        .into_iter()
        .filter(|p| p.subject != subject)
        .map(provider_view)
        .collect();
    finish(&state, &headers, page, StatusCode::OK)
}

/// POST /projects/profile
pub async fn save_profile(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let f = form(&body);
    if !csrf::verify(&headers, Some(get(&f, csrf::FIELD))) {
        return (StatusCode::FORBIDDEN, lang.t("error.csrf")).into_response();
    }
    let subject = match current_subject(&state, lang, &headers, "/projects").await {
        Ok(s) => s,
        Err(r) => return *r,
    };
    let email = identity_email(&state, subject).await;
    let (messages, status) = match projects::save_profile(
        &state,
        subject,
        &email,
        get(&f, "display_name"),
        get(&f, "headline"),
        get(&f, "skills"),
        get(&f, "accepting") == "on",
    )
    .await
    {
        Ok(()) => (vec![Msg::success(lang.t("admin.saved"))], StatusCode::OK),
        Err(e) => explain(lang, e).swap(),
    };
    render_mine(&state, lang, &headers, subject, messages, status).await
}

trait Swap {
    fn swap(self) -> (Vec<Msg>, StatusCode);
}

impl Swap for (StatusCode, Msg) {
    fn swap(self) -> (Vec<Msg>, StatusCode) {
        (vec![self.1], self.0)
    }
}

#[allow(clippy::too_many_arguments)]
async fn render_new(
    state: &AppState,
    lang: Lang,
    headers: &HeaderMap,
    subject: Uuid,
    provider: Option<Uuid>,
    f: Option<&std::collections::BTreeMap<String, String>>,
    messages: Vec<Msg>,
    status: StatusCode,
) -> Response {
    let role = roles::role_of(&state.db, subject)
        .await
        .unwrap_or(PlatformRole::User);
    let mut page = ProjectsPage::new(lang, "new", role);
    page.messages = messages;
    if let Some(p) = provider {
        page.provider = projects::profile(&state.db, p)
            .await
            .ok()
            .flatten()
            .filter(|p| p.accepting)
            .map(provider_view);
    }
    if let Some(f) = f {
        page.title = get(f, "title").to_owned();
        page.description = get(f, "description").to_owned();
        page.budget = get(f, "budget").to_owned();
        page.days = get(f, "days").to_owned();
    }
    finish(state, headers, page, status)
}

/// GET /projects/new?provider=
pub async fn new_form(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    headers: HeaderMap,
    Query(q): Query<ProvidersQuery>,
) -> Response {
    let subject = match current_subject(&state, lang, &headers, "/projects/providers").await {
        Ok(s) => s,
        Err(r) => return *r,
    };
    render_new(
        &state,
        lang,
        &headers,
        subject,
        q.provider,
        None,
        Vec::new(),
        StatusCode::OK,
    )
    .await
}

fn opt_num<T: std::str::FromStr>(s: &str) -> Option<T> {
    let s = s.trim();
    if s.is_empty() {
        None
    } else {
        s.parse().ok()
    }
}

/// POST /projects/new
pub async fn create(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let f = form(&body);
    if !csrf::verify(&headers, Some(get(&f, csrf::FIELD))) {
        return (StatusCode::FORBIDDEN, lang.t("error.csrf")).into_response();
    }
    let subject = match current_subject(&state, lang, &headers, "/projects").await {
        Ok(s) => s,
        Err(r) => return *r,
    };
    let provider = get(&f, "provider").parse::<Uuid>().ok();
    let email = identity_email(&state, subject).await;
    let result = match provider {
        Some(p) => {
            projects::create(
                &state,
                subject,
                &email,
                p,
                get(&f, "title"),
                get(&f, "description"),
                opt_num(get(&f, "budget")),
                opt_num(get(&f, "days")),
            )
            .await
        }
        None => Err(crate::orders::problem(
            StatusCode::BAD_REQUEST,
            "provider_unavailable",
            "Hãy chọn người nhận việc.",
            "Choose a provider.",
        )),
    };
    match result {
        Ok(id) => see_other(&format!("/projects/{id}")),
        Err(e) => {
            let (status, m) = explain(lang, e);
            render_new(
                &state,
                lang,
                &headers,
                subject,
                provider,
                Some(&f),
                vec![m],
                status,
            )
            .await
        }
    }
}

// ---------- Chi tiết dự án ----------

pub struct OfferView {
    pub id: String,
    pub side: &'static str,
    pub price: String,
    pub days: i32,
    pub note: String,
    pub status: String,
    pub acceptable: bool,
    pub at: String,
}

pub struct EventView {
    pub status: &'static str,
    pub actor: String,
    pub note: String,
    pub at: String,
}

pub struct ContractView {
    pub number: String,
    pub body: String,
    pub sha256: String,
    pub status: String,
    pub customer_name: String,
    pub provider_name: String,
    pub customer_signed: String,
    pub provider_signed: String,
    /// Bên đang xem còn phải ký.
    pub my_turn: bool,
    pub my_name: String,
}

#[derive(Template)]
#[template(path = "project.html")]
struct ProjectPage {
    i18n: Lang,
    messages: Vec<Msg>,
    csrf: String,
    p: Project,
    side: &'static str,
    customer: bool,
    status_label: &'static str,
    budget: String,
    price: String,
    escrow: String,
    balance: String,
    offers: Vec<OfferView>,
    /// Hội thoại dự án (thay đổi chat-and-source-scan): luồng tin dạng HTML, id hội thoại, id tin
    /// cuối, người xem gửi được không.
    thread_html: String,
    chat_id: Option<String>,
    chat_last: i64,
    chat_can_send: bool,
    events: Vec<EventView>,
    contract: Option<ContractView>,
    can_contract: bool,
    contract_blocks_funding: bool,
    default_customer_name: String,
    default_provider_name: String,
}

fn side_key(s: &str) -> &'static str {
    match s {
        "customer" => "proj.side.customer",
        "provider" => "proj.side.provider",
        "admin" => "proj.side.admin",
        _ => "proj.side.system",
    }
}

async fn project_page(
    state: &AppState,
    lang: Lang,
    p: Project,
    viewer: Option<(Uuid, Side)>,
    messages: Vec<Msg>,
) -> ProjectPage {
    let db = &state.db;
    let (offers, conversation, events, contract) = tokio::join!(
        projects::offers(db, p.id),
        chat::of_project(db, p.id),
        projects::events(db, p.id),
        projects::contract(db, p.id)
    );
    let side = viewer.map(|v| v.1);
    let (mut thread_html, mut chat_last, mut chat_id) = (String::new(), 0, None);
    if let Ok(Some(c)) = conversation {
        // Người tham gia mở dự án thì coi như đã đọc hội thoại dự án.
        if let Some(role) = viewer.and_then(|v| c.role_of(v.0)) {
            if let Err(e) = chat::mark_read(db, &c, role, None).await {
                tracing::error!(error = %e, "không đánh dấu được đã đọc");
            }
        }
        (thread_html, chat_last) =
            crate::chat_web::thread(state, lang, &c, viewer.map(|v| v.0)).await;
        chat_id = Some(c.id.to_string());
    }
    // Dự án đã hủy hoặc hoàn tiền quá 30 ngày thì hội thoại chỉ còn đọc.
    let chat_can_send = viewer.is_some()
        && !(matches!(p.status.as_str(), "cancelled" | "refunded")
            && p.updated_at < time::OffsetDateTime::now_utc() - time::Duration::days(30));
    let (role, balance) = match viewer {
        Some((s, Side::Customer)) => (
            roles::role_of(db, s).await.unwrap_or(PlatformRole::User),
            wallet::summary(db, s).await.map(|w| w.balance).unwrap_or(0),
        ),
        Some((s, Side::Provider)) => (roles::role_of(db, s).await.unwrap_or(PlatformRole::User), 0),
        None => (PlatformRole::User, 0),
    };
    let negotiating = p.status == "negotiating";
    let provider_name = projects::profile(db, p.provider)
        .await
        .ok()
        .flatten()
        .map(|x| x.display_name)
        .unwrap_or_default();
    let contract = contract.ok().flatten().map(|c| {
        let my_turn = c.status == "draft"
            && match side {
                Some(Side::Customer) => c.customer_signed_at.is_none(),
                Some(Side::Provider) => c.provider_signed_at.is_none(),
                None => false,
            };
        ContractView {
            my_name: match side {
                Some(Side::Customer) => c.customer_name.clone(),
                _ => c.provider_name.clone(),
            },
            number: c.number,
            body: c.body,
            sha256: c.sha256,
            status: c.status,
            customer_name: c.customer_name,
            provider_name: c.provider_name,
            customer_signed: c.customer_signed_at.map(format_time).unwrap_or_default(),
            provider_signed: c.provider_signed_at.map(format_time).unwrap_or_default(),
            my_turn,
        }
    });
    ProjectPage {
        i18n: lang,
        messages,
        csrf: String::new(),
        side: match side {
            Some(Side::Customer) => "proj.as_customer",
            Some(Side::Provider) => "proj.as_provider",
            None => "proj.side.admin",
        },
        customer: side == Some(Side::Customer),
        status_label: status_key(&p.status),
        budget: p
            .budget_coin
            .map(|c| coin(c, lang))
            .unwrap_or_else(|| "—".into()),
        price: p
            .price_coin
            .map(|c| {
                format!(
                    "{} · {} {}",
                    coin(c, lang),
                    p.days.unwrap_or(0),
                    lang.t("proj.days")
                )
            })
            .unwrap_or_else(|| "—".into()),
        escrow: coin(p.escrow_coin, lang),
        balance: coin(balance, lang),
        offers: offers
            .unwrap_or_default()
            .into_iter()
            .map(|o| OfferView {
                acceptable: negotiating
                    && o.status == "open"
                    && side.is_some_and(|s| s.as_str() != o.side),
                id: o.id.to_string(),
                side: side_key(&o.side),
                price: coin(o.price_coin, lang),
                days: o.days,
                note: o.note,
                status: o.status,
                at: format_time(o.created_at),
            })
            .collect(),
        thread_html,
        chat_id,
        chat_last,
        chat_can_send,
        events: events
            .unwrap_or_default()
            .into_iter()
            .map(|e| EventView {
                status: status_key(&e.to_status),
                actor: e.actor,
                note: e.note,
                at: format_time(e.at),
            })
            .collect(),
        can_contract: p.status == "agreed"
            && role.can_sign_contracts()
            && contract_signed_or_none(&contract),
        contract_blocks_funding: contract.as_ref().is_some_and(|c| c.status == "draft"),
        contract,
        default_customer_name: p.customer_email.clone(),
        default_provider_name: if provider_name.is_empty() {
            p.provider_email.clone()
        } else {
            provider_name
        },
        p,
    }
}

fn contract_signed_or_none(c: &Option<ContractView>) -> bool {
    c.as_ref().is_none_or(|c| c.status != "signed")
}

async fn render_project(
    state: &AppState,
    lang: Lang,
    headers: &HeaderMap,
    id: Uuid,
    subject: Uuid,
    messages: Vec<Msg>,
    status: StatusCode,
) -> Response {
    let (p, side) = match projects::for_party(&state.db, id, subject).await {
        Ok(x) => x,
        Err(e) => {
            let (s, m) = explain(lang, e);
            return (s, m.text).into_response();
        }
    };
    let mut page = project_page(state, lang, p, Some((subject, side)), messages).await;
    let (token, set) = csrf::issue(headers, state.public_url.scheme() == "https");
    page.csrf = token;
    render(page, lang, status, set)
}

/// GET /projects/{id}
pub async fn show(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Response {
    let subject = match current_subject(&state, lang, &headers, &format!("/projects/{id}")).await {
        Ok(s) => s,
        Err(r) => return *r,
    };
    render_project(
        &state,
        lang,
        &headers,
        id,
        subject,
        Vec::new(),
        StatusCode::OK,
    )
    .await
}

/// POST /projects/{id}/{action}
pub async fn action(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    Path((id, action)): Path<(Uuid, String)>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let f = form(&body);
    if !csrf::verify(&headers, Some(get(&f, csrf::FIELD))) {
        return (StatusCode::FORBIDDEN, lang.t("error.csrf")).into_response();
    }
    let subject = match current_subject(&state, lang, &headers, &format!("/projects/{id}")).await {
        Ok(s) => s,
        Err(r) => return *r,
    };
    let s = &state;
    let result = match action.as_str() {
        "offer" => {
            projects::offer(
                s,
                id,
                subject,
                get(&f, "price").trim().parse().unwrap_or(0),
                get(&f, "days").trim().parse().unwrap_or(0),
                get(&f, "note"),
            )
            .await
        }
        "accept-offer" => match get(&f, "offer").parse() {
            Ok(o) => projects::accept_offer(s, id, subject, o).await,
            Err(_) => Err(crate::orders::problem(
                StatusCode::BAD_REQUEST,
                "invalid_request",
                "Thiếu đề nghị.",
                "Missing offer.",
            )),
        },
        "cancel" => projects::cancel(s, id, subject, get(&f, "reason")).await,
        "fund" => projects::fund(s, id, subject).await,
        "deliver" => projects::deliver(s, id, subject, get(&f, "note"), get(&f, "link")).await,
        "accept" => projects::accept_delivery(s, id, subject).await,
        "revise" => projects::request_revision(s, id, subject, get(&f, "reason")).await,
        "withdraw" => projects::withdraw(s, id, subject, get(&f, "reason")).await,
        "dispute" => projects::dispute(s, id, subject, get(&f, "reason")).await,
        "message" => projects::message(s, id, subject, get(&f, "body")).await,
        "contract" => {
            projects::create_contract(
                s,
                id,
                subject,
                get(&f, "customer_name"),
                get(&f, "provider_name"),
                get(&f, "extra"),
            )
            .await
        }
        "sign" => {
            projects::sign_contract(s, id, subject, get(&f, "name"), get(&f, "agree") == "on").await
        }
        _ => return (StatusCode::NOT_FOUND, lang.t("error.not_found")).into_response(),
    };
    let (messages, status) = match result {
        Ok(()) => (vec![Msg::success(lang.t("admin.saved"))], StatusCode::OK),
        Err(e) => explain(lang, e).swap(),
    };
    render_project(&state, lang, &headers, id, subject, messages, status).await
}

// ---------- Bảng quản trị: Dự án ----------

#[derive(Template)]
#[template(path = "admin_projects.html")]
struct AdminProjectsPage {
    i18n: Lang,
    messages: Vec<Msg>,
    csrf: String,
    me: String,
    roles: String,
    nav: Vec<admin::NavItem>,
    dev_no_passkey: bool,
    projects: Vec<ProjectRow>,
    detail: Option<ProjectPage>,
}

async fn pm_gate(
    state: &AppState,
    lang: Lang,
    headers: &HeaderMap,
    path: &str,
) -> Result<admin::Admin, Box<Response>> {
    admin::gate(state, lang, headers, path, Some(Role::ProjectManager)).await
}

async fn render_admin(
    state: &AppState,
    lang: Lang,
    headers: &HeaderMap,
    a: &admin::Admin,
    detail: Option<Uuid>,
    messages: Vec<Msg>,
    status: StatusCode,
) -> Response {
    let projects = projects::list_all(&state.db, None)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|p| ProjectRow {
            id: p.id.to_string(),
            code: p.code,
            title: p.title,
            side: "proj.side.admin",
            other: format!("{} → {}", p.customer_email, p.provider_email),
            price: coin(p.escrow_coin, lang),
            status_label: status_key(&p.status),
            status: p.status,
            updated: format_time(p.updated_at),
        })
        .collect();
    let detail = match detail {
        Some(id) => match projects::get(&state.db, id).await {
            Ok(p) => Some(project_page(state, lang, p, None, Vec::new()).await),
            Err(_) => None,
        },
        None => None,
    };
    let (token, set) = csrf::issue(headers, state.public_url.scheme() == "https");
    render(
        AdminProjectsPage {
            i18n: lang,
            messages,
            csrf: token,
            me: a.email.clone(),
            roles: a
                .roles
                .iter()
                .map(|r| r.label(lang))
                .collect::<Vec<_>>()
                .join(", "),
            nav: admin::nav(lang, a, "projects"),
            dev_no_passkey: state.admin_passkey_optional,
            projects,
            detail,
        },
        lang,
        status,
        set,
    )
}

/// GET /admin/projects
pub async fn admin_list(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    headers: HeaderMap,
) -> Response {
    let a = match pm_gate(&state, lang, &headers, "/admin/projects").await {
        Ok(a) => a,
        Err(r) => return *r,
    };
    render_admin(&state, lang, &headers, &a, None, Vec::new(), StatusCode::OK).await
}

/// GET /admin/projects/{id}
pub async fn admin_detail(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Response {
    let a = match pm_gate(&state, lang, &headers, "/admin/projects").await {
        Ok(a) => a,
        Err(r) => return *r,
    };
    render_admin(
        &state,
        lang,
        &headers,
        &a,
        Some(id),
        Vec::new(),
        StatusCode::OK,
    )
    .await
}

/// POST /admin/projects/{id}/resolve: `mode` = release | refund | split (kèm `to_provider`).
pub async fn admin_resolve(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let f = form(&body);
    if !csrf::verify(&headers, Some(get(&f, csrf::FIELD))) {
        return (StatusCode::FORBIDDEN, lang.t("error.csrf")).into_response();
    }
    let a = match pm_gate(&state, lang, &headers, "/admin/projects").await {
        Ok(a) => a,
        Err(r) => return *r,
    };
    let escrow = projects::get(&state.db, id)
        .await
        .map(|p| p.escrow_coin)
        .unwrap_or(0);
    let to_provider = match get(&f, "mode") {
        "release" => escrow,
        "refund" => 0,
        _ => get(&f, "to_provider").trim().parse().unwrap_or(-1),
    };
    let (messages, status) =
        match projects::resolve(&state, id, &a.email, to_provider, get(&f, "note")).await {
            Ok(()) => (vec![Msg::success(lang.t("admin.saved"))], StatusCode::OK),
            Err(e) => explain(lang, e).swap(),
        };
    render_admin(&state, lang, &headers, &a, Some(id), messages, status).await
}

/// POST /admin/projects/{id}/message: Quản lý dự án nhắn trong hội thoại dự án (`side = 'admin'`).
pub async fn admin_message(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let f = form(&body);
    if !csrf::verify(&headers, Some(get(&f, csrf::FIELD))) {
        return (StatusCode::FORBIDDEN, lang.t("error.csrf")).into_response();
    }
    let a = match pm_gate(&state, lang, &headers, "/admin/projects").await {
        Ok(a) => a,
        Err(r) => return *r,
    };
    let (messages, status) = match chat::send_admin(&state, id, a.subject, get(&f, "body")).await {
        Ok(_) => (vec![Msg::success(lang.t("admin.saved"))], StatusCode::OK),
        Err(e) => (vec![Msg::error(e.text(lang))], e.status()),
    };
    render_admin(&state, lang, &headers, &a, Some(id), messages, status).await
}
