//! API cục bộ của launcher cho app con (spec launcher, "App con chạy với quyền người dùng
//! thường", scenario "App xin token của app khác").
//!
//! Khi mở một app, launcher đăng ký một bí mật riêng cho lần chạy đó và truyền cho app qua
//! `FECO_LAUNCHER_SECRET`. App gọi `POST /v1/launch-code` (127.0.0.1) kèm bí mật này để xin mã
//! khởi chạy mới (ví dụ khi mã đầu đã hết hạn). Launcher chỉ cấp mã cho đúng client ID của app
//! đã được khởi chạy bằng bí mật đó; xin cho app khác thì bị từ chối và ghi sự kiện bảo mật.
//! Launcher không bao giờ đưa token của mình hay của app khác ra API này.

use crate::events::EventLog;
use axum::{
    extract::State,
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::Digest;
use std::collections::HashMap;
use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct LaunchCode {
    pub code: String,
    pub expires_in: u64,
}

/// Hàm xin mã khởi chạy từ máy chủ cho một client ID (dùng token của launcher).
pub type Issuer = Arc<
    dyn Fn(String) -> Pin<Box<dyn Future<Output = Result<LaunchCode, String>> + Send>>
        + Send
        + Sync,
>;

struct Inner {
    /// SHA-256 của bí mật → client ID của app đã được khởi chạy.
    launched: Mutex<HashMap<String, String>>,
    issuer: Issuer,
    events: EventLog,
}

#[derive(Clone)]
pub struct Broker {
    inner: Arc<Inner>,
}

#[derive(Debug, Deserialize)]
struct CodeRequest {
    client_id: String,
}

fn hash(secret: &str) -> String {
    hex::encode(sha2::Sha256::digest(secret.as_bytes()))
}

fn error(status: StatusCode, code: &str, vi: &str) -> Response {
    (status, Json(json!({ "error": code, "message_vi": vi }))).into_response()
}

impl Broker {
    pub fn new(issuer: Issuer, events: EventLog) -> Self {
        Self {
            inner: Arc::new(Inner {
                launched: Mutex::new(HashMap::new()),
                issuer,
                events,
            }),
        }
    }

    /// Đăng ký một lần chạy của app `client_id`; trả bí mật để truyền cho app.
    pub fn register(&self, client_id: &str) -> String {
        let secret = crate::random_hex(32);
        self.inner
            .launched
            .lock()
            .expect("khóa broker")
            .insert(hash(&secret), client_id.to_owned());
        secret
    }

    /// Chạy API ở một cổng ngẫu nhiên trên 127.0.0.1; trả địa chỉ.
    pub async fn serve(&self) -> std::io::Result<SocketAddr> {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;
        let router = Router::new()
            .route("/v1/launch-code", post(launch_code))
            .with_state(self.clone());
        tokio::spawn(async move {
            if let Err(e) = axum::serve(listener, router).await {
                eprintln!("API cục bộ của launcher dừng: {e}");
            }
        });
        Ok(addr)
    }
}

async fn launch_code(
    State(broker): State<Broker>,
    headers: HeaderMap,
    Json(req): Json<CodeRequest>,
) -> Response {
    let secret = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(str::trim)
        .unwrap_or_default();
    let launched = broker
        .inner
        .launched
        .lock()
        .expect("khóa broker")
        .get(&hash(secret))
        .cloned();
    let Some(launched) = launched else {
        return error(
            StatusCode::UNAUTHORIZED,
            "unknown_caller",
            "Tiến trình này không do launcher khởi chạy.",
        );
    };
    if launched != req.client_id {
        broker.inner.events.record(
            "token_request_for_other_app",
            json!({ "launched": launched, "requested": req.client_id }),
        );
        return error(
            StatusCode::FORBIDDEN,
            "client_mismatch",
            "Client ID không khớp với app đã được launcher khởi chạy.",
        );
    }
    match (broker.inner.issuer)(launched).await {
        Ok(code) => Json(code).into_response(),
        Err(e) => error(StatusCode::SERVICE_UNAVAILABLE, "unavailable", &e),
    }
}
