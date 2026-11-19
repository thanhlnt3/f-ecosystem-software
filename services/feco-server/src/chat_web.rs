//! Trang web của chat (thay đổi chat-and-source-scan, design C3, C5):
//! - `GET /messages`: danh sách hội thoại (lọc trực tiếp, dự án);
//! - `POST /messages/start`: tạo hoặc mở hội thoại trực tiếp (`developer` hoặc `app_id`);
//! - `GET /messages/{id}`, `POST /messages/{id}`: luồng tin, gửi tin (kèm một tệp);
//! - `GET /messages/{id}/after/{last}`: HTML các tin mới (trang gọi khi nhận sự kiện);
//! - `POST /messages/{id}/{block|unblock|report}`, `GET /messages/{id}/files/{att}`;
//! - `GET /messages/events`: Server-Sent Events, `GET /messages/chat.js`: JavaScript nhỏ của trang;
//! - `GET /admin/chat-reports`, `GET /admin/chat-reports/{id}`, `POST /admin/chat-reports/{id}/handled`;
//! - số tin chưa đọc trên thanh trên cùng ([`nav_badge`]).
//!
//! Không có JavaScript thì form gửi tin vẫn chạy (gửi rồi chuyển hướng).

use std::{cell::Cell, collections::HashMap, convert::Infallible, sync::Arc, time::Duration};

use askama::Template;
use axum::{
    body::{Body, Bytes},
    extract::{Multipart, Path, Query, Request, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    middleware::Next,
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse, Response,
    },
};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::{
    admin::{self, Role as AdminRole},
    chat::{self, ChatError, Conversation, Role, Upload},
    connected::{current_subject, format_time},
    csrf,
    i18n::Lang,
    views::Msg,
    wallet_web::{form, get, render, see_other},
    AppState,
};

// ---------- Số chưa đọc trên thanh trên cùng ----------

tokio::task_local! {
    /// Người dùng của yêu cầu hiện tại, ghi khi phiên Kratos được đọc.
    static VIEWER: Cell<Option<Uuid>>;
}

/// Ghi người dùng của yêu cầu hiện tại (gọi từ `KratosClient::whoami`).
pub fn remember_viewer(subject: Uuid) {
    let _ = VIEWER.try_with(|v| v.set(Some(subject)));
}

/// Chỗ trong `base.html` được thay bằng số tin chưa đọc.
const MARKER: &[u8] = b"<!--feco-unread-->";

/// Thêm số tin chưa đọc vào mục "Tin nhắn" của trang HTML khi người xem đã đăng nhập.
pub async fn nav_badge(State(state): State<Arc<AppState>>, req: Request, next: Next) -> Response {
    let (res, viewer) = VIEWER
        .scope(Cell::new(None), async move {
            let res = next.run(req).await;
            (res, VIEWER.with(Cell::get))
        })
        .await;
    let Some(subject) = viewer else {
        return res;
    };
    let is_html = res
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.starts_with("text/html"));
    if !is_html {
        return res;
    }
    let n = chat::unread_total(&state.db, subject).await.unwrap_or(0);
    if n <= 0 {
        return res;
    }
    let (mut parts, body) = res.into_parts();
    let bytes = match axum::body::to_bytes(body, 16 * 1024 * 1024).await {
        Ok(b) => b,
        Err(e) => {
            tracing::warn!(error = %e, "không đọc được trang để gắn số tin chưa đọc");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };
    let out = match bytes.windows(MARKER.len()).position(|w| w == MARKER) {
        Some(i) => {
            let mut out = Vec::with_capacity(bytes.len() + 64);
            out.extend_from_slice(&bytes[..i]);
            out.extend_from_slice(
                format!(" <span class=\"badge brand\" id=\"nav-unread\">{n}</span>").as_bytes(),
            );
            out.extend_from_slice(&bytes[i + MARKER.len()..]);
            Bytes::from(out)
        }
        None => bytes,
    };
    parts.headers.remove(header::CONTENT_LENGTH);
    Response::from_parts(parts, Body::from(out))
}

// ---------- Hiển thị tin ----------

pub struct AttView {
    pub name: String,
    pub size: String,
    pub status: String,
    /// Khóa i18n của trạng thái.
    pub label: &'static str,
    pub url: String,
    pub ready: bool,
}

pub struct RowView {
    pub id: i64,
    pub side: String,
    pub who: String,
    pub mine: bool,
    pub body: String,
    pub at: String,
    pub att: Option<AttView>,
}

#[derive(Template)]
#[template(path = "chat_rows.html")]
struct RowsFragment {
    i18n: Lang,
    rows: Vec<RowView>,
}

fn size_label(n: i64) -> String {
    if n >= 1024 * 1024 {
        format!("{:.1} MB", n as f64 / (1024.0 * 1024.0))
    } else if n >= 1024 {
        format!("{} KB", n / 1024)
    } else {
        format!("{n} B")
    }
}

fn row_view(lang: Lang, c: &Conversation, m: &chat::Message, viewer: Option<Uuid>) -> RowView {
    let mine = viewer.is_some() && m.sender == viewer;
    let who = if mine {
        lang.t("chat.you").to_owned()
    } else {
        match m.side.as_str() {
            "customer" => c.customer_name.clone(),
            "developer" => c.developer_name.clone(),
            "admin" => lang.t("chat.side.admin").to_owned(),
            _ => lang.t("chat.side.system").to_owned(),
        }
    };
    RowView {
        id: m.id,
        side: m.side.clone(),
        who,
        mine,
        body: m.body.clone(),
        at: format_time(m.at),
        att: m.att_id.map(|a| {
            let status = m.att_status.clone().unwrap_or_else(|| "pending".into());
            AttView {
                name: m.att_name.clone().unwrap_or_default(),
                size: size_label(m.att_size.unwrap_or(0)),
                label: match status.as_str() {
                    "clean" => "chat.att.clean",
                    "infected" => "chat.att.infected",
                    "error" => "chat.att.error",
                    _ => "chat.att.pending",
                },
                ready: status == "clean",
                status,
                url: format!("/messages/{}/files/{a}", c.id),
            }
        }),
    }
}

fn rows_html(lang: Lang, c: &Conversation, list: &[chat::Message], viewer: Option<Uuid>) -> String {
    RowsFragment {
        i18n: lang,
        rows: list.iter().map(|m| row_view(lang, c, m, viewer)).collect(),
    }
    .render()
    .unwrap_or_else(|e| {
        tracing::error!(error = %e, "lỗi dựng tin nhắn");
        String::new()
    })
}

/// Luồng tin của hội thoại (200 tin mới nhất) dạng HTML và id tin cuối.
pub async fn thread(
    state: &AppState,
    lang: Lang,
    c: &Conversation,
    viewer: Option<Uuid>,
) -> (String, i64) {
    let list = chat::messages(&state.db, c.id, 0, 200)
        .await
        .unwrap_or_default();
    let last = list.last().map(|m| m.id).unwrap_or(0);
    (rows_html(lang, c, &list, viewer), last)
}

// ---------- Danh sách ----------

pub struct ItemView {
    pub id: String,
    pub kind: String,
    pub peer: String,
    pub title: String,
    pub preview: String,
    pub at: String,
    pub unread: i64,
    pub blocked: bool,
}

#[derive(Template)]
#[template(path = "messages.html")]
struct ListPage {
    i18n: Lang,
    messages: Vec<Msg>,
    kind: String,
    items: Vec<ItemView>,
}

#[derive(Deserialize, Default)]
pub struct ListQuery {
    pub kind: Option<String>,
}

async fn render_list(
    state: &AppState,
    lang: Lang,
    subject: Uuid,
    kind: Option<&str>,
    messages: Vec<Msg>,
    status: StatusCode,
) -> Response {
    let kind = kind.filter(|k| matches!(*k, "direct" | "project"));
    let items = chat::list(&state.db, subject, kind)
        .await
        .unwrap_or_default()
        .iter()
        .map(|s| {
            let role = s.conversation.role_of(subject).unwrap_or(Role::Customer);
            ItemView {
                id: s.conversation.id.to_string(),
                kind: s.conversation.kind.clone(),
                peer: s.conversation.peer_name(role).to_owned(),
                title: s.title(role),
                preview: s.preview().unwrap_or_default(),
                at: format_time(s.conversation.last_message_at),
                unread: s.unread,
                blocked: s.conversation.blocked_at.is_some(),
            }
        })
        .collect();
    render(
        ListPage {
            i18n: lang,
            messages,
            kind: kind.unwrap_or_default().to_owned(),
            items,
        },
        lang,
        status,
        None,
    )
}

/// GET /messages
pub async fn list(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    headers: HeaderMap,
    Query(q): Query<ListQuery>,
) -> Response {
    let subject = match current_subject(&state, lang, &headers, "/messages").await {
        Ok(s) => s,
        Err(r) => return *r,
    };
    render_list(
        &state,
        lang,
        subject,
        q.kind.as_deref(),
        Vec::new(),
        StatusCode::OK,
    )
    .await
}

/// POST /messages/start: `developer` (id danh tính) hoặc `app_id`.
pub async fn start(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let f = form(&body);
    if !csrf::verify(&headers, Some(get(&f, csrf::FIELD))) {
        return (StatusCode::FORBIDDEN, lang.t("error.csrf")).into_response();
    }
    let back = match get(&f, "app_id") {
        "" => "/projects/providers".to_owned(),
        app => format!("/store/{app}"),
    };
    let subject = match current_subject(&state, lang, &headers, &back).await {
        Ok(s) => s,
        Err(r) => return *r,
    };
    let target = match (get(&f, "developer").trim(), get(&f, "app_id").trim()) {
        ("", "") => Err(ChatError::NotDeveloper),
        (d, "") => d
            .parse()
            .map(chat::Target::Developer)
            .map_err(|_| ChatError::NotDeveloper),
        (_, app) => Ok(chat::Target::App(app.to_owned())),
    };
    match async { chat::start_direct(&state, subject, target?).await }.await {
        Ok((c, _)) => see_other(&format!("/messages/{}", c.id)),
        Err(e) => {
            let status = e.status();
            render_list(
                &state,
                lang,
                subject,
                None,
                vec![Msg::error(e.text(lang))],
                status,
            )
            .await
        }
    }
}

// ---------- Hội thoại ----------

#[derive(Template)]
#[template(path = "conversation.html")]
struct ConversationPage {
    i18n: Lang,
    messages: Vec<Msg>,
    csrf: String,
    id: String,
    title: String,
    peer: String,
    kind: String,
    project_link: String,
    app_link: String,
    app_name: String,
    blocked: bool,
    /// Khóa i18n của thông báo chặn (khác nhau cho khách hàng và nhà phát triển).
    blocked_key: &'static str,
    can_send: bool,
    can_block: bool,
    thread_html: String,
    last_id: i64,
}

async fn render_conversation(
    state: &AppState,
    lang: Lang,
    headers: &HeaderMap,
    id: Uuid,
    subject: Uuid,
    messages: Vec<Msg>,
    status: StatusCode,
) -> Response {
    let (c, role) = match chat::for_participant(&state.db, id, subject).await {
        Ok(x) => x,
        Err(e) => return (e.status(), e.text(lang)).into_response(),
    };
    // Mở hội thoại thì đánh dấu đã đọc.
    if let Err(e) = chat::mark_read(&state.db, &c, role, None).await {
        tracing::error!(error = %e, "không đánh dấu được đã đọc");
    }
    let summary = chat::summary(&state.db, id, subject).await.ok().flatten();
    let title = summary
        .as_ref()
        .map(|s| s.title(role))
        .unwrap_or_else(|| c.peer_name(role).to_owned());
    let (thread_html, last_id) = thread(state, lang, &c, Some(subject)).await;
    let blocked = c.blocked_at.is_some();
    let app_name = match &c.app_id {
        Some(a) => sqlx::query_scalar("SELECT name FROM catalog_apps WHERE id = $1")
            .bind(a)
            .fetch_optional(&state.db)
            .await
            .ok()
            .flatten()
            .unwrap_or_else(|| a.clone()),
        None => String::new(),
    };
    let (token, set) = csrf::issue(headers, state.public_url.scheme() == "https");
    render(
        ConversationPage {
            i18n: lang,
            messages,
            csrf: token,
            id: c.id.to_string(),
            title,
            peer: c.peer_name(role).to_owned(),
            kind: c.kind.clone(),
            project_link: c
                .project_id
                .map(|p| format!("/projects/{p}"))
                .unwrap_or_default(),
            app_link: c
                .app_id
                .as_ref()
                .map(|a| format!("/store/{a}"))
                .unwrap_or_default(),
            app_name,
            blocked,
            blocked_key: if role == Role::Developer {
                "chat.blocked_by_me"
            } else {
                "chat.blocked_by_peer"
            },
            can_send: !(blocked && role == Role::Customer),
            can_block: c.is_direct() && role == Role::Developer,
            thread_html,
            last_id,
        },
        lang,
        status,
        set,
    )
}

/// GET /messages/{id}
pub async fn show(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Response {
    let subject = match current_subject(&state, lang, &headers, &format!("/messages/{id}")).await {
        Ok(s) => s,
        Err(r) => return *r,
    };
    render_conversation(
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

/// Đọc biểu mẫu gửi tin (multipart): các trường chữ và tối đa một tệp `file`.
async fn read_multipart(
    multipart: &mut Multipart,
) -> Result<(std::collections::BTreeMap<String, String>, Option<Upload>), ChatError> {
    let mut fields = std::collections::BTreeMap::new();
    let mut upload = None;
    loop {
        let field = match multipart.next_field().await {
            Ok(Some(f)) => f,
            Ok(None) => break,
            // Quá giới hạn thân yêu cầu (tệp lớn hơn 20 MB) hoặc biểu mẫu hỏng.
            Err(_) => return Err(ChatError::AttachmentTooLarge),
        };
        let name = field.name().unwrap_or_default().to_owned();
        let file_name = field.file_name().map(str::to_owned);
        match file_name {
            Some(file_name) if name == "file" => {
                let bytes = field
                    .bytes()
                    .await
                    .map_err(|_| ChatError::AttachmentTooLarge)?;
                if !file_name.is_empty() || !bytes.is_empty() {
                    upload = Some(Upload {
                        name: file_name,
                        bytes: bytes.to_vec(),
                    });
                }
            }
            _ => {
                let text = field.text().await.map_err(|_| ChatError::Empty)?;
                fields.insert(name, text);
            }
        }
    }
    Ok((fields, upload))
}

/// POST /messages/{id} (multipart): `body`, `file` (tùy chọn), `back=project` để về trang dự án.
pub async fn send(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Response {
    let subject = match current_subject(&state, lang, &headers, &format!("/messages/{id}")).await {
        Ok(s) => s,
        Err(r) => return *r,
    };
    let (fields, upload) = match read_multipart(&mut multipart).await {
        Ok(x) => x,
        Err(e) => {
            let status = e.status();
            return render_conversation(
                &state,
                lang,
                &headers,
                id,
                subject,
                vec![Msg::error(e.text(lang))],
                status,
            )
            .await;
        }
    };
    if !csrf::verify(&headers, fields.get(csrf::FIELD).map(String::as_str)) {
        return (StatusCode::FORBIDDEN, lang.t("error.csrf")).into_response();
    }
    let body = fields.get("body").map(String::as_str).unwrap_or_default();
    match chat::send(&state, id, subject, body, upload).await {
        Ok((_, att)) => {
            if let Some((att, bytes)) = att {
                let s = state.clone();
                tokio::spawn(async move { chat::scan_upload(&s, att, bytes).await });
            }
            let project = if fields.get("back").map(String::as_str) == Some("project") {
                chat::get(&state.db, id)
                    .await
                    .ok()
                    .and_then(|c| c.project_id)
            } else {
                None
            };
            match project {
                Some(p) => see_other(&format!("/projects/{p}#chat")),
                None => see_other(&format!("/messages/{id}#chat-form")),
            }
        }
        Err(e) => {
            let status = e.status();
            if matches!(e, ChatError::Forbidden | ChatError::NotFound) {
                return (status, e.text(lang)).into_response();
            }
            render_conversation(
                &state,
                lang,
                &headers,
                id,
                subject,
                vec![Msg::error(e.text(lang))],
                status,
            )
            .await
        }
    }
}

/// GET /messages/{id}/after/{last}: HTML các tin sau `last` (đánh dấu đã đọc, vì người xem đang
/// mở hội thoại).
pub async fn after(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    Path((id, last)): Path<(Uuid, i64)>,
    headers: HeaderMap,
) -> Response {
    let subject = match current_subject(&state, lang, &headers, &format!("/messages/{id}")).await {
        Ok(s) => s,
        Err(_) => return StatusCode::UNAUTHORIZED.into_response(),
    };
    let (c, role) = match chat::for_participant(&state.db, id, subject).await {
        Ok(x) => x,
        Err(e) => return (e.status(), e.text(lang)).into_response(),
    };
    let list = match chat::messages(&state.db, id, last.max(0), 200).await {
        Ok(l) => l,
        Err(e) => return (ChatError::Db(e).status(), lang.t("error.unavailable")).into_response(),
    };
    if let Some(m) = list.last() {
        if let Err(e) = chat::mark_read(&state.db, &c, role, Some(m.id)).await {
            tracing::error!(error = %e, "không đánh dấu được đã đọc");
        }
    }
    // `last = 0` nghĩa là "từ đầu", không phải "200 tin mới nhất".
    let list = if last <= 0 {
        list
    } else {
        list.into_iter().filter(|m| m.id > last).collect()
    };
    let mut res = axum::response::Html(rows_html(lang, &c, &list, Some(subject))).into_response();
    res.headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    res
}

/// POST /messages/{id}/{action}: `block`, `unblock` (nhà phát triển), `report` (`reason`).
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
    let subject = match current_subject(&state, lang, &headers, &format!("/messages/{id}")).await {
        Ok(s) => s,
        Err(r) => return *r,
    };
    let result = match action.as_str() {
        "block" => chat::set_blocked(&state, id, subject, true)
            .await
            .map(|_| "chat.ok.blocked"),
        "unblock" => chat::set_blocked(&state, id, subject, false)
            .await
            .map(|_| "chat.ok.unblocked"),
        "report" => chat::report(&state, id, subject, get(&f, "reason"))
            .await
            .map(|_| "chat.ok.reported"),
        _ => return (StatusCode::NOT_FOUND, lang.t("error.not_found")).into_response(),
    };
    let (messages, status) = match result {
        Ok(key) => (vec![Msg::success(lang.t(key))], StatusCode::OK),
        Err(e) => {
            let status = e.status();
            if matches!(e, ChatError::NotFound) {
                return (status, e.text(lang)).into_response();
            }
            (vec![Msg::error(e.text(lang))], status)
        }
    };
    render_conversation(&state, lang, &headers, id, subject, messages, status).await
}

/// GET /messages/{id}/files/{att}: tệp đính kèm, chỉ người tham gia, chỉ khi đã quét sạch.
pub async fn download(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    Path((id, att)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
) -> Response {
    let subject = match current_subject(&state, lang, &headers, &format!("/messages/{id}")).await {
        Ok(s) => s,
        Err(r) => return *r,
    };
    let result = async {
        chat::for_participant(&state.db, id, subject).await?;
        let a = chat::attachment(&state.db, id, att).await?;
        chat::downloadable(&a)?;
        let bytes = tokio::fs::read(&a.path).await?;
        Ok::<_, ChatError>((a, bytes))
    }
    .await;
    let (a, bytes) = match result {
        Ok(x) => x,
        Err(e) => return e.api(),
    };
    let mut res = Body::from(bytes).into_response();
    let h = res.headers_mut();
    h.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/octet-stream"),
    );
    let ascii: String = a
        .name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let encoded: String = form_urlencoded::byte_serialize(a.name.as_bytes()).collect();
    if let Ok(v) = HeaderValue::from_str(&format!(
        "attachment; filename=\"{ascii}\"; filename*=UTF-8''{}",
        encoded.replace('+', "%20")
    )) {
        h.insert(header::CONTENT_DISPOSITION, v);
    }
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    res
}

// ---------- Thời gian thực ----------

#[derive(Deserialize, Default)]
pub struct EventsQuery {
    /// Hội thoại đang mở (để không gửi thông báo trong lúc xem).
    pub viewing: Option<Uuid>,
}

struct Listen {
    rx: tokio::sync::broadcast::Receiver<Uuid>,
    state: Arc<AppState>,
    subject: Uuid,
    member: HashMap<Uuid, bool>,
    _view: Option<chat::ViewGuard>,
}

/// GET /messages/events?viewing=<id>: sự kiện `message` mang `{"conversation": "<id>"}` cho các
/// hội thoại người xem tham gia; nhịp giữ kết nối mỗi 20 giây.
pub async fn events(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    headers: HeaderMap,
    Query(q): Query<EventsQuery>,
) -> Response {
    let subject = match current_subject(&state, lang, &headers, "/messages").await {
        Ok(s) => s,
        Err(_) => return StatusCode::UNAUTHORIZED.into_response(),
    };
    chat::start_listener(&state);
    let view = match q.viewing {
        Some(c) if chat::for_participant(&state.db, c, subject).await.is_ok() => {
            Some(chat::ViewGuard::new(state.clone(), c, subject))
        }
        _ => None,
    };
    let listen = Listen {
        rx: state.chat.events.subscribe(),
        state: state.clone(),
        subject,
        member: HashMap::new(),
        _view: view,
    };
    let ready = futures_util::stream::once(async {
        Ok::<_, Infallible>(Event::default().event("ready").data("{}"))
    });
    let updates = futures_util::stream::unfold(listen, |mut l| async move {
        loop {
            match l.rx.recv().await {
                Ok(id) => {
                    let member = match l.member.get(&id) {
                        Some(m) => *m,
                        None => {
                            let m = chat::for_participant(&l.state.db, id, l.subject)
                                .await
                                .is_ok();
                            l.member.insert(id, m);
                            m
                        }
                    };
                    if member {
                        let ev = Event::default()
                            .event("message")
                            .data(json!({ "conversation": id }).to_string());
                        return Some((Ok::<_, Infallible>(ev), l));
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => return None,
            }
        }
    });
    let mut res = Sse::new(futures_util::StreamExt::chain(ready, updates))
        .keep_alive(KeepAlive::new().interval(Duration::from_secs(20)))
        .into_response();
    res.headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    res
}

/// GET /messages/chat.js
pub async fn script() -> Response {
    (
        [
            (
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/javascript; charset=utf-8"),
            ),
            (
                header::CACHE_CONTROL,
                HeaderValue::from_static("public, max-age=300"),
            ),
        ],
        include_str!("chat.js"),
    )
        .into_response()
}

// ---------- Bảng quản trị: báo cáo hội thoại ----------

pub struct ReportView {
    pub id: i64,
    pub conversation: String,
    pub reason: String,
    pub at: String,
    pub handled: bool,
    pub handled_note: String,
}

#[derive(Template)]
#[template(path = "admin_chat.html")]
struct AdminChatPage {
    i18n: Lang,
    messages: Vec<Msg>,
    csrf: String,
    me: String,
    roles: String,
    nav: Vec<admin::NavItem>,
    dev_no_passkey: bool,
    reports: Vec<ReportView>,
    detail: Option<ReportView>,
    detail_title: String,
    thread_html: String,
}

fn report_view(r: chat::Report) -> ReportView {
    ReportView {
        id: r.id,
        conversation: r.conversation_id.to_string(),
        reason: r.reason,
        at: format_time(r.at),
        handled: r.handled_at.is_some(),
        handled_note: match (r.handled_at, r.handled_by) {
            (Some(t), Some(by)) => format!("{by} · {}", format_time(t)),
            (Some(t), None) => format_time(t),
            _ => String::new(),
        },
    }
}

async fn support_gate(
    state: &AppState,
    lang: Lang,
    headers: &HeaderMap,
) -> Result<admin::Admin, Box<Response>> {
    admin::gate(
        state,
        lang,
        headers,
        "/admin/chat-reports",
        Some(AdminRole::Support),
    )
    .await
}

async fn render_admin(
    state: &AppState,
    lang: Lang,
    headers: &HeaderMap,
    a: &admin::Admin,
    detail: Option<i64>,
    messages: Vec<Msg>,
    status: StatusCode,
) -> Response {
    let reports = chat::reports(&state.db).await.unwrap_or_default();
    let mut detail_title = String::new();
    let mut thread_html = String::new();
    let detail = match detail {
        Some(id) => match chat::report_by_id(&state.db, id).await.ok().flatten() {
            Some(r) => {
                if let Ok(c) = chat::get(&state.db, r.conversation_id).await {
                    detail_title = format!("{} ↔ {}", c.customer_name, c.developer_name);
                    // Chế độ chỉ đọc: không đánh dấu đã đọc, không có "Bạn".
                    thread_html = thread(state, lang, &c, None).await.0;
                }
                Some(report_view(r))
            }
            None => return (StatusCode::NOT_FOUND, lang.t("error.not_found")).into_response(),
        },
        None => None,
    };
    let (token, set) = csrf::issue(headers, state.public_url.scheme() == "https");
    render(
        AdminChatPage {
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
            nav: admin::nav(lang, a, "chat-reports"),
            dev_no_passkey: state.admin_passkey_optional,
            reports: reports.into_iter().map(report_view).collect(),
            detail,
            detail_title,
            thread_html,
        },
        lang,
        status,
        set,
    )
}

/// GET /admin/chat-reports
pub async fn admin_reports(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    headers: HeaderMap,
) -> Response {
    let a = match support_gate(&state, lang, &headers).await {
        Ok(a) => a,
        Err(r) => return *r,
    };
    render_admin(&state, lang, &headers, &a, None, Vec::new(), StatusCode::OK).await
}

/// GET /admin/chat-reports/{id}: báo cáo và hội thoại bị báo cáo (chỉ đọc).
pub async fn admin_report(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    Path(id): Path<i64>,
    headers: HeaderMap,
) -> Response {
    let a = match support_gate(&state, lang, &headers).await {
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

/// POST /admin/chat-reports/{id}/handled
pub async fn admin_handled(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    Path(id): Path<i64>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let f = form(&body);
    if !csrf::verify(&headers, Some(get(&f, csrf::FIELD))) {
        return (StatusCode::FORBIDDEN, lang.t("error.csrf")).into_response();
    }
    let a = match support_gate(&state, lang, &headers).await {
        Ok(a) => a,
        Err(r) => return *r,
    };
    let (messages, status) = match chat::mark_handled(&state.db, id, &a.email).await {
        Ok(_) => {
            crate::audit::record(
                &state.db,
                crate::audit::Actor::Admin(a.email.clone()),
                "chat.report_handled",
                Some(&id.to_string()),
                None,
                json!({}),
            )
            .await;
            (vec![Msg::success(lang.t("admin.saved"))], StatusCode::OK)
        }
        Err(e) => (
            vec![Msg::error(ChatError::Db(e).text(lang))],
            StatusCode::SERVICE_UNAVAILABLE,
        ),
    };
    render_admin(&state, lang, &headers, &a, Some(id), messages, status).await
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mọi khóa i18n dùng trong mã và mẫu trang của chat có đủ bốn thứ tiếng.
    #[test]
    fn every_chat_key_is_translated() {
        let sources = [
            include_str!("chat.rs"),
            include_str!("chat_web.rs"),
            include_str!("../templates/messages.html"),
            include_str!("../templates/conversation.html"),
            include_str!("../templates/chat_rows.html"),
            include_str!("../templates/admin_chat.html"),
            include_str!("../templates/project.html"),
            include_str!("../templates/admin_projects.html"),
            include_str!("../templates/projects.html"),
            include_str!("../templates/store_app.html"),
            include_str!("../templates/base.html"),
        ];
        let re = regex::Regex::new(
            r#"(?:\.t\(|=> |key: |label: )"((?:chat|nav|notify|admin\.nav)\.[a-z_.0-9]+)""#,
        )
        .unwrap();
        let mut n = 0;
        for src in sources {
            for cap in re.captures_iter(src) {
                for lang in Lang::ALL {
                    assert!(!lang.t(&cap[1]).is_empty(), "thiếu {} ({lang:?})", &cap[1]);
                }
                n += 1;
            }
        }
        assert!(n > 30, "chỉ thấy {n} khóa");
    }

    #[test]
    fn sizes() {
        assert_eq!(size_label(10), "10 B");
        assert_eq!(size_label(2048), "2 KB");
        assert_eq!(size_label(3 * 1024 * 1024), "3.0 MB");
    }

    #[tokio::test]
    async fn viewer_is_recorded_only_inside_a_request() {
        remember_viewer(Uuid::from_u128(1));
        let seen = VIEWER
            .scope(Cell::new(None), async {
                remember_viewer(Uuid::from_u128(7));
                VIEWER.with(Cell::get)
            })
            .await;
        assert_eq!(seen, Some(Uuid::from_u128(7)));
    }
}
