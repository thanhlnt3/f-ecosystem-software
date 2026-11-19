//! API chat cho F-Launcher máy tính và di động (thay đổi chat-and-source-scan, design C4):
//! - `GET /v1/chat/unread`;
//! - `GET /v1/chat/conversations`, `POST /v1/chat/conversations`;
//! - `GET /v1/chat/conversations/{id}/messages?after=&limit=`,
//!   `POST /v1/chat/conversations/{id}/messages`;
//! - `POST /v1/chat/conversations/{id}/read`.
//!
//! Token như các API dành cho launcher khác (`launch::launcher_subject`). Lỗi dạng
//! `{"error": "<mã>"}`.

use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use time::format_description::well_known::Rfc3339;
use uuid::Uuid;

use crate::{
    api::api_error,
    chat::{self, ChatError, Role, Summary, Target},
    launch::launcher_subject,
    AppState,
};

#[derive(Debug, Serialize)]
pub struct ConversationJson {
    pub id: Uuid,
    pub kind: String,
    pub role: &'static str,
    pub peer_name: String,
    pub title: String,
    pub project_id: Option<Uuid>,
    pub app_id: Option<String>,
    pub blocked: bool,
    pub last_message: Option<String>,
    pub last_message_at: String,
    pub unread: i64,
    pub web_url: String,
}

#[derive(Debug, Serialize)]
pub struct AttachmentJson {
    pub name: String,
    pub size: i64,
    pub status: String,
    pub url: String,
}

#[derive(Debug, Serialize)]
pub struct MessageJson {
    pub id: i64,
    pub side: String,
    pub mine: bool,
    pub body: String,
    pub at: String,
    pub attachment: Option<AttachmentJson>,
}

fn rfc3339(t: time::OffsetDateTime) -> String {
    t.format(&Rfc3339).unwrap_or_default()
}

pub fn conversation_json(state: &AppState, s: &Summary, subject: Uuid) -> ConversationJson {
    let c = &s.conversation;
    let role = c.role_of(subject).unwrap_or(Role::Customer);
    ConversationJson {
        id: c.id,
        kind: c.kind.clone(),
        role: role.as_str(),
        peer_name: c.peer_name(role).to_owned(),
        title: s.title(role),
        project_id: c.project_id,
        app_id: c.app_id.clone(),
        blocked: c.blocked_at.is_some(),
        last_message: s.preview(),
        last_message_at: rfc3339(c.last_message_at),
        unread: s.unread,
        web_url: format!("{}messages/{}", state.public_url, c.id),
    }
}

pub fn message_json(
    state: &AppState,
    conversation: Uuid,
    m: &chat::Message,
    subject: Uuid,
) -> MessageJson {
    MessageJson {
        id: m.id,
        side: m.side.clone(),
        mine: m.sender == Some(subject),
        body: m.body.clone(),
        at: rfc3339(m.at),
        attachment: m.att_id.map(|a| AttachmentJson {
            name: m.att_name.clone().unwrap_or_default(),
            size: m.att_size.unwrap_or(0),
            status: m.att_status.clone().unwrap_or_else(|| "pending".into()),
            url: format!("{}messages/{conversation}/files/{a}", state.public_url),
        }),
    }
}

fn db_error(e: sqlx::Error) -> Response {
    ChatError::Db(e).api()
}

/// GET /v1/chat/unread
pub async fn unread(State(state): State<Arc<AppState>>, headers: HeaderMap) -> Response {
    let subject = match launcher_subject(&state, &headers).await {
        Ok(s) => s,
        Err(r) => return *r,
    };
    match chat::unread_total(&state.db, subject).await {
        Ok(n) => Json(json!({ "unread": n })).into_response(),
        Err(e) => db_error(e),
    }
}

/// GET /v1/chat/conversations
pub async fn conversations(State(state): State<Arc<AppState>>, headers: HeaderMap) -> Response {
    let subject = match launcher_subject(&state, &headers).await {
        Ok(s) => s,
        Err(r) => return *r,
    };
    match chat::list(&state.db, subject, None).await {
        Ok(list) => {
            let items: Vec<ConversationJson> = list
                .iter()
                .map(|s| conversation_json(&state, s, subject))
                .collect();
            Json(json!({ "conversations": items })).into_response()
        }
        Err(e) => db_error(e),
    }
}

#[derive(Debug, Deserialize)]
pub struct StartBody {
    pub developer: Option<String>,
    pub app_id: Option<String>,
}

fn invalid(en: &str, vi: &str) -> Response {
    api_error(StatusCode::BAD_REQUEST, "invalid_request", en, vi)
}

/// POST /v1/chat/conversations: `{"developer": "<subject>"}` hoặc `{"app_id": "..."}`; 201 khi
/// tạo mới, 200 khi đã có.
pub async fn start(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    let subject = match launcher_subject(&state, &headers).await {
        Ok(s) => s,
        Err(r) => return *r,
    };
    let Ok(b) = serde_json::from_slice::<StartBody>(&body) else {
        return invalid(
            "The body must be JSON with \"developer\" or \"app_id\".",
            "Thân yêu cầu phải là JSON có \"developer\" hoặc \"app_id\".",
        );
    };
    let target = match (b.developer.as_deref(), b.app_id.as_deref()) {
        (Some(d), None) => match d.trim().parse::<Uuid>() {
            Ok(d) => Target::Developer(d),
            // Không phải id danh tính hợp lệ thì không thể là nhà phát triển.
            Err(_) => return ChatError::NotDeveloper.api(),
        },
        (None, Some(a)) if !a.trim().is_empty() => Target::App(a.to_owned()),
        _ => {
            return invalid(
                "Give exactly one of \"developer\" or \"app_id\".",
                "Cần đúng một trong hai trường \"developer\" hoặc \"app_id\".",
            )
        }
    };
    let (c, created) = match chat::start_direct(&state, subject, target).await {
        Ok(x) => x,
        Err(e) => return e.api(),
    };
    match chat::summary(&state.db, c.id, subject).await {
        Ok(Some(s)) => (
            if created {
                StatusCode::CREATED
            } else {
                StatusCode::OK
            },
            Json(conversation_json(&state, &s, subject)),
        )
            .into_response(),
        Ok(None) => ChatError::NotFound.api(),
        Err(e) => db_error(e),
    }
}

#[derive(Debug, Deserialize)]
pub struct MessagesQuery {
    pub after: Option<i64>,
    pub limit: Option<i64>,
}

/// GET /v1/chat/conversations/{id}/messages?after=<id>&limit=<n>
pub async fn messages(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(q): Query<MessagesQuery>,
    headers: HeaderMap,
) -> Response {
    let subject = match launcher_subject(&state, &headers).await {
        Ok(s) => s,
        Err(r) => return *r,
    };
    let Ok(id) = id.parse::<Uuid>() else {
        return ChatError::NotFound.api();
    };
    if let Err(e) = chat::for_participant(&state.db, id, subject).await {
        return e.api();
    }
    match chat::messages(&state.db, id, q.after.unwrap_or(0), q.limit.unwrap_or(200)).await {
        Ok(list) => {
            let items: Vec<MessageJson> = list
                .iter()
                .map(|m| message_json(&state, id, m, subject))
                .collect();
            Json(json!({ "messages": items })).into_response()
        }
        Err(e) => db_error(e),
    }
}

/// POST /v1/chat/conversations/{id}/messages: `{"body": "..."}` (đính kèm chỉ qua web).
pub async fn send(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    let subject = match launcher_subject(&state, &headers).await {
        Ok(s) => s,
        Err(r) => return *r,
    };
    let Ok(id) = id.parse::<Uuid>() else {
        return ChatError::NotFound.api();
    };
    let Ok(v) = serde_json::from_slice::<Value>(&body) else {
        return invalid(
            "The body must be JSON: {\"body\": \"...\"}.",
            "Thân yêu cầu phải là JSON: {\"body\": \"...\"}.",
        );
    };
    let text = v["body"].as_str().unwrap_or_default();
    match chat::send(&state, id, subject, text, None).await {
        Ok((mid, _)) => match chat::message(&state.db, mid).await {
            Ok(m) => (
                StatusCode::CREATED,
                Json(message_json(&state, id, &m, subject)),
            )
                .into_response(),
            Err(e) => db_error(e),
        },
        Err(e) => e.api(),
    }
}

/// POST /v1/chat/conversations/{id}/read: `{"up_to": <id>}` → 204.
pub async fn read(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    let subject = match launcher_subject(&state, &headers).await {
        Ok(s) => s,
        Err(r) => return *r,
    };
    let Ok(id) = id.parse::<Uuid>() else {
        return ChatError::NotFound.api();
    };
    let up_to = serde_json::from_slice::<Value>(&body)
        .ok()
        .and_then(|v| v["up_to"].as_i64());
    let (c, role) = match chat::for_participant(&state.db, id, subject).await {
        Ok(x) => x,
        Err(e) => return e.api(),
    };
    match chat::mark_read(&state.db, &c, role, up_to).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => db_error(e),
    }
}
