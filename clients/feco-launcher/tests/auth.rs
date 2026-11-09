//! Đăng nhập launcher với một máy chủ OAuth giả lập (luồng thật với Hydra được kiểm ở E2E
//! `launcher_login_and_single_use_launch_codes` của feco-server).

use axum::{extract::Form, routing::get, routing::post, Json, Router};
use feco_launcher::auth::{Auth, AuthError, MemoryStore, SecretStore};
use serde_json::json;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Máy chủ giả: `/oauth2/token` và `/v1/me`. `valid` là refresh token còn hiệu lực.
async fn fake_server(valid: Arc<Mutex<String>>) -> url::Url {
    let router = Router::new()
        .route(
            "/oauth2/token",
            post(move |Form(form): Form<HashMap<String, String>>| {
                let valid = valid.clone();
                async move {
                    let ok = match form.get("grant_type").map(String::as_str) {
                        Some("refresh_token") => {
                            form.get("refresh_token") == Some(&*valid.lock().unwrap())
                        }
                        Some("authorization_code") => {
                            form.get("code").map(String::as_str) == Some("code-123")
                                && form.get("code_verifier").is_some_and(|v| v.len() >= 43)
                        }
                        _ => false,
                    };
                    if !ok {
                        return (
                            axum::http::StatusCode::BAD_REQUEST,
                            Json(json!({ "error": "invalid_grant" })),
                        );
                    }
                    let rotated = format!("refresh-{}", uuid_like());
                    *valid.lock().unwrap() = rotated.clone();
                    (
                        axum::http::StatusCode::OK,
                        Json(json!({
                            "access_token": "access-abc",
                            "refresh_token": rotated,
                            "expires_in": 3600,
                        })),
                    )
                }
            }),
        )
        .route(
            "/v1/me",
            get(|| async { Json(json!({ "name": "Lan Nguyễn" })) }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    url::Url::parse(&format!("http://{addr}/")).unwrap()
}

fn uuid_like() -> String {
    format!(
        "{:x}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    )
}

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn auth(base: &url::Url, store: Arc<dyn SecretStore>, port: u16) -> Auth {
    Auth::new(base.clone(), base.clone(), "launcher", port, store)
}

/// Scenario "Đăng nhập lần đầu": mở trình duyệt, nhận code ở loopback, hiện tên người dùng;
/// refresh token nằm trong kho bảo mật, không có ô nhập mật khẩu.
#[tokio::test]
async fn first_login_through_browser_with_pkce() {
    let valid = Arc::new(Mutex::new(String::new()));
    let base = fake_server(valid.clone()).await;
    let store = Arc::new(MemoryStore::default());
    let port = free_port();
    let a = auth(&base, store.clone(), port);

    let session = a
        .login(|url| {
            let q: HashMap<String, String> = url.query_pairs().into_owned().collect();
            assert_eq!(q["code_challenge_method"], "S256");
            assert_eq!(q["scope"], "openid offline_access profile");
            assert_eq!(
                q["redirect_uri"],
                format!("http://127.0.0.1:{port}/callback")
            );
            assert!(url.as_str().starts_with(&format!("{base}oauth2/auth?")));
            // "Trình duyệt" đăng nhập xong rồi quay về địa chỉ loopback.
            let back = format!(
                "http://127.0.0.1:{port}/callback?code=code-123&state={}",
                q["state"]
            );
            std::thread::spawn(move || {
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async { reqwest::get(back).await.unwrap() });
            });
            Ok(())
        })
        .await
        .unwrap();
    assert_eq!(session.name.as_deref(), Some("Lan Nguyễn"));
    assert_eq!(store.load().unwrap(), Some(valid.lock().unwrap().clone()));
}

/// Callback có state lạ (CSRF) bị bỏ qua.
#[tokio::test]
async fn callback_with_wrong_state_is_rejected() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        let _ = reqwest::get(format!("http://{addr}/callback?code=x&state=gia-mao")).await;
    });
    let err = feco_launcher::auth::wait_for_callback(
        listener,
        "dung-state",
        std::time::Duration::from_secs(5),
    )
    .await
    .unwrap_err();
    assert!(err.to_string().contains("state"), "{err}");
}

/// Scenario "Khởi động lại máy": vẫn đăng nhập nhờ refresh token đã lưu; token được xoay.
#[tokio::test]
async fn restart_restores_session_from_stored_refresh_token() {
    let valid = Arc::new(Mutex::new("refresh-0".to_owned()));
    let base = fake_server(valid.clone()).await;
    let store = Arc::new(MemoryStore::default());
    let a = auth(&base, store.clone(), free_port());
    assert!(a.restore().await.unwrap().is_none(), "chưa đăng nhập");

    store.save("refresh-0").unwrap();
    let session = a.restore().await.unwrap().expect("vẫn đăng nhập");
    assert_eq!(session.name.as_deref(), Some("Lan Nguyễn"));
    assert!(session.is_fresh(jiff::Timestamp::now()));
    let rotated = store.load().unwrap().unwrap();
    assert_ne!(rotated, "refresh-0");
    assert_eq!(rotated, *valid.lock().unwrap());
    // Lần khởi động sau dùng token mới.
    assert!(a.restore().await.unwrap().is_some());
}

/// Ngoại tuyến: giữ refresh token để thử lại; token bị thu hồi: xóa và yêu cầu đăng nhập lại.
#[tokio::test]
async fn offline_keeps_token_and_revoked_token_is_cleared() {
    let store = Arc::new(MemoryStore::default());
    store.save("refresh-0").unwrap();
    let unreachable = url::Url::parse("http://127.0.0.1:9/").unwrap();
    let err = auth(&unreachable, store.clone(), free_port())
        .restore()
        .await
        .unwrap_err();
    assert!(matches!(err, AuthError::Offline(_)), "{err:?}");
    assert_eq!(store.load().unwrap().as_deref(), Some("refresh-0"));

    let base = fake_server(Arc::new(Mutex::new("khac".to_owned()))).await;
    let err = auth(&base, store.clone(), free_port())
        .restore()
        .await
        .unwrap_err();
    assert!(matches!(err, AuthError::Expired), "{err:?}");
    assert_eq!(store.load().unwrap(), None);
}

/// Refresh token nằm trong kho bảo mật thật của hệ điều hành (Windows Credential Manager,
/// macOS Keychain), mỗi client một mục riêng.
#[cfg(any(windows, target_os = "macos"))]
#[test]
fn os_keyring_round_trip() {
    use feco_launcher::auth::KeyringStore;
    let a = KeyringStore::for_client(&format!("test-{}", uuid_like()));
    let b = KeyringStore::for_client(&format!("test-other-{}", uuid_like()));
    assert_eq!(a.load().unwrap(), None);
    a.save("refresh-bi-mat").unwrap();
    assert_eq!(a.load().unwrap().as_deref(), Some("refresh-bi-mat"));
    assert_eq!(b.load().unwrap(), None, "mục của client khác tách riêng");
    a.clear().unwrap();
    assert_eq!(a.load().unwrap(), None);
    a.clear().unwrap();
}
