//! Tin nhắn trong F-Launcher với máy chủ `/v1/chat` giả lập theo design C4 (thay đổi
//! chat-and-source-scan). Luồng thật với máy chủ F-Eco được kiểm ở E2E "Launcher gửi tin".

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use feco_launcher::auth::{Auth, AuthError, MemoryStore};
use feco_launcher::chat::{self, ChatError, Due, Schedule, Thread};
use feco_launcher::library::badge_text;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const TOKEN: &str = "access-abc";
const CONV: &str = "6f1c2a9e-0000-4000-8000-000000000001";
const BLOCKED: &str = "6f1c2a9e-0000-4000-8000-000000000002";

/// Máy chủ giả: một hội thoại dự án giữa khách hàng (người dùng launcher) và nhà phát triển,
/// một hội thoại trực tiếp bị chặn.
#[derive(Default)]
struct Fake {
    /// (id, phía gửi, nội dung)
    messages: Vec<(i64, &'static str, String)>,
    /// Tin cuối khách hàng đã đọc.
    customer_read: i64,
    reads: Vec<i64>,
    /// `after` của các lần `GET …/messages`.
    afters: Vec<i64>,
}

impl Fake {
    fn next_id(&self) -> i64 {
        self.messages.last().map_or(0, |m| m.0) + 1
    }

    fn unread(&self) -> usize {
        self.messages
            .iter()
            .filter(|m| m.1 != "customer" && m.0 > self.customer_read)
            .count()
    }

    fn message(&self, m: &(i64, &'static str, String)) -> Value {
        json!({"id": m.0, "side": m.1, "mine": m.1 == "customer", "body": m.2,
               "at": "2026-11-19T03:00:00Z", "attachment": null})
    }
}

type Shared = Arc<Mutex<Fake>>;

fn authorized(headers: &HeaderMap) -> bool {
    headers.get("authorization").and_then(|v| v.to_str().ok())
        == Some(format!("Bearer {TOKEN}").as_str())
}

fn error(status: StatusCode, code: &str) -> (StatusCode, Json<Value>) {
    (status, Json(json!({ "error": code })))
}

async fn fake_server(state: Shared) -> url::Url {
    let router = Router::new()
        .route(
            "/v1/chat/unread",
            get(|State(s): State<Shared>, h: HeaderMap| async move {
                if !authorized(&h) {
                    return error(StatusCode::UNAUTHORIZED, "unauthorized");
                }
                (
                    StatusCode::OK,
                    Json(json!({ "unread": s.lock().unwrap().unread() })),
                )
            }),
        )
        .route(
            "/v1/chat/conversations",
            get(|State(s): State<Shared>, h: HeaderMap| async move {
                if !authorized(&h) {
                    return error(StatusCode::UNAUTHORIZED, "unauthorized");
                }
                let s = s.lock().unwrap();
                let last = s.messages.last().map(|m| m.2.clone());
                (
                    StatusCode::OK,
                    Json(json!({"conversations": [
                        {"id": CONV, "kind": "project", "role": "customer", "peer_name": "Công ty Phần mềm A",
                         "title": "Ứng dụng quản lý kho", "project_id": "6f1c2a9e-0000-4000-8000-0000000000aa",
                         "app_id": null, "blocked": false, "last_message": last,
                         "last_message_at": "2026-11-19T03:00:00Z", "unread": s.unread(),
                         "web_url": format!("https://f-eco.example/messages/{CONV}")},
                        {"id": BLOCKED, "kind": "direct", "role": "customer", "peer_name": "Lan",
                         "title": "Mentor", "project_id": null, "app_id": "mentor", "blocked": true,
                         "last_message": null, "last_message_at": "2026-11-18T03:00:00Z", "unread": 0,
                         "web_url": format!("https://f-eco.example/messages/{BLOCKED}")}
                    ]})),
                )
            }),
        )
        .route(
            "/v1/chat/conversations/{id}/messages",
            get(
                |State(s): State<Shared>,
                 h: HeaderMap,
                 Path(id): Path<String>,
                 Query(q): Query<HashMap<String, String>>| async move {
                    if !authorized(&h) {
                        return error(StatusCode::UNAUTHORIZED, "unauthorized");
                    }
                    if id != CONV {
                        return error(StatusCode::FORBIDDEN, "chat_forbidden");
                    }
                    let after: i64 = q.get("after").and_then(|a| a.parse().ok()).unwrap_or(0);
                    let limit: usize = q
                        .get("limit")
                        .and_then(|a| a.parse().ok())
                        .unwrap_or(200)
                        .min(200);
                    let mut s = s.lock().unwrap();
                    s.afters.push(after);
                    let page: Vec<Value> = s
                        .messages
                        .iter()
                        .filter(|m| m.0 > after)
                        .take(limit)
                        .map(|m| s.message(m))
                        .collect();
                    (StatusCode::OK, Json(json!({ "messages": page })))
                },
            )
            .post(
                |State(s): State<Shared>,
                 h: HeaderMap,
                 Path(id): Path<String>,
                 Json(body): Json<Value>| async move {
                    if !authorized(&h) {
                        return error(StatusCode::UNAUTHORIZED, "unauthorized");
                    }
                    if id == BLOCKED {
                        return error(StatusCode::CONFLICT, "chat_blocked");
                    }
                    if id != CONV {
                        return error(StatusCode::FORBIDDEN, "chat_forbidden");
                    }
                    let text = body["body"].as_str().unwrap_or_default().trim().to_owned();
                    if text.is_empty() {
                        return error(StatusCode::UNPROCESSABLE_ENTITY, "chat_empty");
                    }
                    if text.chars().count() > 5000 {
                        return error(StatusCode::UNPROCESSABLE_ENTITY, "chat_too_long");
                    }
                    let mut s = s.lock().unwrap();
                    let m = (s.next_id(), "customer", text);
                    s.messages.push(m.clone());
                    (StatusCode::CREATED, Json(s.message(&m)))
                },
            ),
        )
        .route(
            "/v1/chat/conversations/{id}/read",
            post(
                |State(s): State<Shared>,
                 h: HeaderMap,
                 Path(id): Path<String>,
                 Json(body): Json<Value>| async move {
                    if !authorized(&h) || id != CONV {
                        return StatusCode::FORBIDDEN;
                    }
                    let up_to = body["up_to"].as_i64().unwrap_or(0);
                    let mut s = s.lock().unwrap();
                    s.reads.push(up_to);
                    s.customer_read = s.customer_read.max(up_to);
                    StatusCode::NO_CONTENT
                },
            ),
        )
        .with_state(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    url::Url::parse(&format!("http://{addr}/")).unwrap()
}

fn auth(base: &url::Url) -> Auth {
    Auth::new(
        base.clone(),
        base.clone(),
        "launcher",
        0,
        Arc::new(MemoryStore::default()),
    )
}

fn seeded(n: usize, side: &'static str) -> Shared {
    let mut fake = Fake::default();
    for i in 1..=n {
        fake.messages.push((i as i64, side, format!("Tin {i}")));
    }
    Arc::new(Mutex::new(fake))
}

/// Scenario "Huy hiệu chưa đọc": người dùng có 3 tin chưa đọc; launcher đồng bộ (hỏi số chưa
/// đọc ngay khi đăng nhập, rồi mỗi 60 giây) thì mục "Tin nhắn" có huy hiệu "3".
#[tokio::test]
async fn unread_badge_after_sync() {
    let state = seeded(3, "developer");
    let base = fake_server(state.clone()).await;
    let a = auth(&base);

    let now = Instant::now();
    let mut schedule = Schedule::default();
    assert_eq!(schedule.due(now, true), [Due::Unread], "đồng bộ ngay");
    let unread = a.chat_unread(TOKEN).await.unwrap();
    schedule.unread_done(now);
    assert_eq!(unread, 3);
    assert_eq!(badge_text(unread).as_deref(), Some("3"));

    // Có thêm tin: lần đồng bộ sau (60 giây) cập nhật huy hiệu.
    state
        .lock()
        .unwrap()
        .messages
        .push((4, "developer", "Tin 4".into()));
    assert!(schedule.due(now + Duration::from_secs(30), true).is_empty());
    assert_eq!(
        schedule.due(now + chat::UNREAD_INTERVAL, true),
        [Due::Unread]
    );
    assert_eq!(
        badge_text(a.chat_unread(TOKEN).await.unwrap()).as_deref(),
        Some("4")
    );

    // Token sai: báo hết phiên, không đoán số.
    let err = a.chat_unread("sai").await.unwrap_err();
    assert!(
        matches!(err, ChatError::Auth(AuthError::Expired)),
        "{err:?}"
    );
}

/// Scenario "Trả lời trong launcher": mở hội thoại (tải mọi tin qua các trang `after`, đánh dấu
/// đã đọc), gõ "Em gửi bản sửa nhé" và bấm Gửi; tin hiện ở cuối luồng và bên kia thấy tin (máy
/// chủ đã lưu). Lần hỏi 5 giây sau lấy tin trả lời mà không trùng tin vừa gửi.
#[tokio::test]
async fn reply_in_launcher() {
    let state = seeded(250, "developer");
    let base = fake_server(state.clone()).await;
    let a = auth(&base);

    let list = a.chat_conversations(TOKEN).await.unwrap();
    assert_eq!(list.len(), 2);
    let conv = &list[0];
    assert_eq!(conv.id, CONV);
    assert_eq!(conv.unread, 250);

    // Mở hội thoại: hai trang (200 + 50 tin), cũ trước.
    let mut thread = Thread::new(&conv.id);
    let page = a
        .chat_messages_after(TOKEN, &conv.id, thread.poll_after())
        .await
        .unwrap();
    assert_eq!(state.lock().unwrap().afters, [0, 200]);
    assert_eq!(thread.apply(page), 250);
    assert!(thread.messages.windows(2).all(|w| w[0].id < w[1].id));
    let up_to = thread.read_due().unwrap();
    a.chat_read(TOKEN, &conv.id, up_to).await.unwrap();
    thread.read_up_to = up_to;
    assert_eq!(state.lock().unwrap().reads, [250]);
    assert_eq!(a.chat_unread(TOKEN).await.unwrap(), 0);

    // Gửi.
    let body = chat::validate_body("  Em gửi bản sửa nhé \n").unwrap();
    let sent = a.chat_send(TOKEN, &conv.id, &body).await.unwrap();
    assert!(sent.mine);
    assert_eq!(sent.side, chat::Side::Customer);
    thread.push_sent(sent);
    let last = thread.messages.last().unwrap();
    assert_eq!(last.body, "Em gửi bản sửa nhé");
    assert!(last.mine);
    // Bên kia thấy tin: máy chủ đã lưu đúng nội dung.
    assert_eq!(
        state.lock().unwrap().messages.last().unwrap().2,
        "Em gửi bản sửa nhé"
    );

    // Nhà phát triển trả lời trên web; lần hỏi kế tiếp lấy được, không trùng tin đã gửi.
    state
        .lock()
        .unwrap()
        .messages
        .push((252, "developer", "Cảm ơn, anh xem nhé".into()));
    let page = a
        .chat_messages_after(TOKEN, &conv.id, thread.poll_after())
        .await
        .unwrap();
    assert_eq!(*state.lock().unwrap().afters.last().unwrap(), 250);
    assert_eq!(thread.apply(page), 1);
    let tail: Vec<&str> = thread.messages[250..]
        .iter()
        .map(|m| m.body.as_str())
        .collect();
    assert_eq!(tail, ["Em gửi bản sửa nhé", "Cảm ơn, anh xem nhé"]);
    assert_eq!(thread.read_due(), Some(252));
}

/// Lỗi của máy chủ đi kèm mã (design C2) để hiện câu dễ hiểu; mã hội thoại lạ không thành URL.
#[tokio::test]
async fn chat_errors_carry_codes() {
    let state = seeded(1, "developer");
    let base = fake_server(state.clone()).await;
    let a = auth(&base);
    let code = |e: ChatError| match e {
        ChatError::Api { code, .. } => code,
        other => panic!("{other:?}"),
    };
    let blocked = a.chat_send(TOKEN, BLOCKED, "Chào anh").await.unwrap_err();
    assert_eq!(blocked.message(), chat::error_message("chat_blocked"));
    assert_eq!(code(blocked), "chat_blocked");
    let long = "a".repeat(5001);
    assert_eq!(
        code(a.chat_send(TOKEN, CONV, &long).await.unwrap_err()),
        "chat_too_long"
    );
    assert_eq!(
        code(a.chat_send(TOKEN, CONV, "  ").await.unwrap_err()),
        "chat_empty"
    );
    assert_eq!(
        code(a.chat_messages(TOKEN, "khac", 0, 50).await.unwrap_err()),
        "chat_forbidden"
    );
    assert_eq!(
        code(a.chat_read(TOKEN, "../v1/wallet", 1).await.unwrap_err()),
        "chat_not_found"
    );
    assert_eq!(state.lock().unwrap().messages.len(), 1, "không lưu tin lỗi");
}
