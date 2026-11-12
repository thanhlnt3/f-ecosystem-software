//! Đăng nhập launcher (spec launcher, "Đăng nhập trong launcher"; design D4).
//!
//! - Mở trang đăng nhập F-Eco trong trình duyệt mặc định (OAuth 2.0 Authorization Code + PKCE,
//!   client công khai, địa chỉ chuyển hướng loopback `http://127.0.0.1:47201/callback`).
//!   Launcher không có ô nhập mật khẩu.
//! - Refresh token nằm trong kho bảo mật của hệ điều hành (Windows Credential Manager, macOS
//!   Keychain) qua `keyring-core` và kho gốc của từng hệ điều hành; access token chỉ nằm trong bộ nhớ. Khởi động lại máy rồi
//!   mở launcher thì dùng refresh token để vào lại, không phải đăng nhập lại.

use crate::broker::LaunchCode;
use base64::Engine;
use jiff::{SignedDuration, Timestamp};
use serde_json::Value;
use sha2::Digest;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use url::Url;

pub const REDIRECT_PORT: u16 = 47201;
pub const SCOPE: &str = "openid offline_access profile";
const KEYRING_SERVICE: &str = "F-Eco Launcher";

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("Đang ngoại tuyến ({0})")]
    Offline(String),
    #[error("Phiên đăng nhập đã hết hạn hoặc bị thu hồi, hãy đăng nhập lại")]
    Expired,
    #[error("Đăng nhập không thành công: {0}")]
    Failed(String),
    #[error("Kho bảo mật của hệ điều hành: {0}")]
    Store(String),
}

/// Nơi giữ refresh token.
pub trait SecretStore: Send + Sync {
    fn load(&self) -> Result<Option<String>, String>;
    fn save(&self, value: &str) -> Result<(), String>;
    fn clear(&self) -> Result<(), String>;
}

/// Kho bảo mật của hệ điều hành; mỗi client là một mục riêng.
pub struct KeyringStore {
    user: String,
}

impl KeyringStore {
    pub fn for_client(client_id: &str) -> Self {
        Self {
            user: format!("refresh-token:{client_id}"),
        }
    }

    fn entry(&self) -> Result<keyring_core::Entry, String> {
        os_store()?;
        keyring_core::Entry::new(KEYRING_SERVICE, &self.user).map_err(|e| e.to_string())
    }
}

impl SecretStore for KeyringStore {
    fn load(&self) -> Result<Option<String>, String> {
        match self.entry()?.get_password() {
            Ok(v) => Ok(Some(v)),
            Err(keyring_core::Error::NoEntry) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    fn save(&self, value: &str) -> Result<(), String> {
        self.entry()?.set_password(value).map_err(|e| e.to_string())
    }

    fn clear(&self) -> Result<(), String> {
        match self.entry()?.delete_credential() {
            Ok(()) | Err(keyring_core::Error::NoEntry) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }
}

/// Chọn kho bảo mật của hệ điều hành làm kho mặc định (một lần).
fn os_store() -> Result<(), String> {
    static INIT: std::sync::OnceLock<Result<(), String>> = std::sync::OnceLock::new();
    INIT.get_or_init(|| {
        #[cfg(windows)]
        {
            let store = windows_native_keyring_store::Store::new().map_err(|e| e.to_string())?;
            keyring_core::set_default_store(store);
            Ok(())
        }
        #[cfg(target_os = "macos")]
        {
            let store =
                apple_native_keyring_store::keychain::Store::new().map_err(|e| e.to_string())?;
            keyring_core::set_default_store(store);
            Ok(())
        }
        #[cfg(not(any(windows, target_os = "macos")))]
        {
            Err(
                "F-Launcher chỉ hỗ trợ Windows và macOS: chưa có kho bảo mật cho hệ điều hành này"
                    .into(),
            )
        }
    })
    .clone()
}

/// Kho trong bộ nhớ (kiểm thử).
#[derive(Default)]
pub struct MemoryStore(Mutex<Option<String>>);

impl SecretStore for MemoryStore {
    fn load(&self) -> Result<Option<String>, String> {
        Ok(self.0.lock().expect("khóa").clone())
    }

    fn save(&self, value: &str) -> Result<(), String> {
        *self.0.lock().expect("khóa") = Some(value.to_owned());
        Ok(())
    }

    fn clear(&self) -> Result<(), String> {
        *self.0.lock().expect("khóa") = None;
        Ok(())
    }
}

/// PKCE (RFC 7636) và `state` chống CSRF.
pub struct Pkce {
    pub verifier: String,
    pub challenge: String,
    pub state: String,
}

impl Pkce {
    pub fn new() -> Self {
        let mut buf = [0u8; 32];
        getrandom::fill(&mut buf).expect("nguồn ngẫu nhiên");
        let verifier = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(buf);
        let challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(sha2::Sha256::digest(verifier.as_bytes()));
        Self {
            verifier,
            challenge,
            state: crate::random_hex(16),
        }
    }
}

impl Default for Pkce {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug)]
pub struct Session {
    pub access_token: String,
    pub name: Option<String>,
    pub expires_at: Timestamp,
}

impl Session {
    /// Còn dùng được ít nhất 1 phút nữa.
    pub fn is_fresh(&self, now: Timestamp) -> bool {
        self.expires_at > now + SignedDuration::from_mins(1)
    }
}

pub struct Auth {
    hydra: Url,
    server: Url,
    client_id: String,
    redirect_port: u16,
    http: reqwest::Client,
    store: Arc<dyn SecretStore>,
}

impl Auth {
    pub fn new(
        hydra: Url,
        server: Url,
        client_id: impl Into<String>,
        redirect_port: u16,
        store: Arc<dyn SecretStore>,
    ) -> Self {
        Self {
            hydra,
            server,
            client_id: client_id.into(),
            redirect_port,
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(15))
                .build()
                .expect("tạo HTTP client"),
            store,
        }
    }

    pub fn redirect_uri(&self) -> String {
        format!("http://127.0.0.1:{}/callback", self.redirect_port)
    }

    pub fn authorize_url(&self, pkce: &Pkce) -> Url {
        let mut url = self.hydra.join("oauth2/auth").expect("URL Hydra hợp lệ");
        url.query_pairs_mut()
            .append_pair("client_id", &self.client_id)
            .append_pair("response_type", "code")
            .append_pair("scope", SCOPE)
            .append_pair("redirect_uri", &self.redirect_uri())
            .append_pair("state", &pkce.state)
            .append_pair("code_challenge", &pkce.challenge)
            .append_pair("code_challenge_method", "S256");
        url
    }

    /// Đăng nhập qua trình duyệt: mở `authorize_url` bằng `open_browser`, chờ callback ở
    /// 127.0.0.1 (tối đa 10 phút), đổi code lấy token, lưu refresh token.
    pub async fn login(
        &self,
        open_browser: impl FnOnce(&Url) -> Result<(), String>,
    ) -> Result<Session, AuthError> {
        let pkce = Pkce::new();
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", self.redirect_port))
            .await
            .map_err(|e| AuthError::Failed(format!("cổng {} đang bận: {e}", self.redirect_port)))?;
        open_browser(&self.authorize_url(&pkce)).map_err(AuthError::Failed)?;
        let code = wait_for_callback(listener, &pkce.state, Duration::from_secs(600)).await?;
        let redirect_uri = self.redirect_uri();
        let tokens = self
            .token_request(&[
                ("grant_type", "authorization_code"),
                ("code", &code),
                ("redirect_uri", &redirect_uri),
                ("client_id", &self.client_id),
                ("code_verifier", &pkce.verifier),
            ])
            .await?;
        self.finish(tokens).await
    }

    /// Vào lại bằng refresh token đã lưu. `Ok(None)`: chưa đăng nhập. `Offline`: giữ nguyên
    /// refresh token để thử lại khi có mạng. `Expired`: token đã bị thu hồi, đã xóa khỏi kho.
    pub async fn restore(&self) -> Result<Option<Session>, AuthError> {
        let Some(refresh) = self.store.load().map_err(AuthError::Store)? else {
            return Ok(None);
        };
        let tokens = match self
            .token_request(&[
                ("grant_type", "refresh_token"),
                ("refresh_token", &refresh),
                ("client_id", &self.client_id),
            ])
            .await
        {
            Err(AuthError::Expired) => {
                self.store.clear().map_err(AuthError::Store)?;
                return Err(AuthError::Expired);
            }
            other => other?,
        };
        self.finish(tokens).await.map(Some)
    }

    /// Đăng xuất: thu hồi refresh token ở máy chủ (nếu có mạng) và xóa khỏi kho.
    pub async fn logout(&self) -> Result<(), AuthError> {
        if let Ok(Some(refresh)) = self.store.load() {
            let url = self.hydra.join("oauth2/revoke").expect("URL Hydra hợp lệ");
            let _ = self
                .http
                .post(url)
                .form(&[("token", refresh.as_str()), ("client_id", &self.client_id)])
                .send()
                .await;
        }
        self.store.clear().map_err(AuthError::Store)
    }

    async fn finish(&self, tokens: Value) -> Result<Session, AuthError> {
        let access = tokens["access_token"]
            .as_str()
            .ok_or_else(|| AuthError::Failed(format!("không có access token: {tokens}")))?
            .to_owned();
        // Hydra xoay refresh token mỗi lần dùng: luôn lưu bản mới.
        if let Some(refresh) = tokens["refresh_token"].as_str() {
            self.store.save(refresh).map_err(AuthError::Store)?;
        }
        let expires_in = tokens["expires_in"].as_i64().unwrap_or(3600);
        let name = self.me(&access).await.unwrap_or(None);
        Ok(Session {
            access_token: access,
            name,
            expires_at: Timestamp::now() + SignedDuration::from_secs(expires_in),
        })
    }

    async fn token_request(&self, form: &[(&str, &str)]) -> Result<Value, AuthError> {
        let url = self.hydra.join("oauth2/token").expect("URL Hydra hợp lệ");
        let res = self
            .http
            .post(url)
            .form(form)
            .send()
            .await
            .map_err(|e| AuthError::Offline(e.to_string()))?;
        let status = res.status();
        let body: Value = res
            .json()
            .await
            .map_err(|e| AuthError::Offline(e.to_string()))?;
        if status.is_success() {
            Ok(body)
        } else if body["error"] == "invalid_grant" {
            Err(AuthError::Expired)
        } else {
            Err(AuthError::Failed(format!("{status}: {body}")))
        }
    }

    /// Tên người dùng từ `GET /v1/me` (scope `profile`).
    pub async fn me(&self, access: &str) -> Result<Option<String>, AuthError> {
        let url = self.server.join("v1/me").expect("URL máy chủ hợp lệ");
        let res = self
            .http
            .get(url)
            .bearer_auth(access)
            .send()
            .await
            .map_err(|e| AuthError::Offline(e.to_string()))?;
        if !res.status().is_success() {
            return Err(AuthError::Failed(format!("/v1/me trả {}", res.status())));
        }
        let body: Value = res
            .json()
            .await
            .map_err(|e| AuthError::Failed(e.to_string()))?;
        Ok(body["name"].as_str().map(str::to_owned))
    }

    /// Phiên bản máy chủ cho phép thiết bị này nhận (phát hành dần; spec developer-portal).
    /// `Ok(None)`: app không có trong danh mục của máy chủ.
    pub async fn rollout_version(
        &self,
        app_id: &str,
        platform: &str,
        device: &str,
    ) -> Result<Option<String>, AuthError> {
        let mut url = self
            .server
            .join(&format!("v1/catalog/{app_id}/update"))
            .map_err(|e| AuthError::Failed(e.to_string()))?;
        url.query_pairs_mut()
            .append_pair("platform", platform)
            .append_pair("device", device);
        let res = self
            .http
            .get(url)
            .send()
            .await
            .map_err(|e| AuthError::Offline(e.to_string()))?;
        if res.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !res.status().is_success() {
            return Err(AuthError::Failed(format!("máy chủ trả {}", res.status())));
        }
        let body: Value = res
            .json()
            .await
            .map_err(|e| AuthError::Failed(e.to_string()))?;
        Ok(body["version"].as_str().map(str::to_owned))
    }

    /// Trung tâm thông báo của người dùng (spec notifications: báo giá, đơn hàng, cập nhật app...).
    pub async fn notifications(&self, access: &str) -> Result<Vec<Notification>, AuthError> {
        let url = self
            .server
            .join("v1/notifications")
            .expect("URL máy chủ hợp lệ");
        let res = self
            .http
            .get(url)
            .bearer_auth(access)
            .send()
            .await
            .map_err(|e| AuthError::Offline(e.to_string()))?;
        match res.status() {
            s if s.is_success() => {}
            reqwest::StatusCode::UNAUTHORIZED => return Err(AuthError::Expired),
            s => return Err(AuthError::Failed(format!("/v1/notifications trả {s}"))),
        }
        let body: Value = res
            .json()
            .await
            .map_err(|e| AuthError::Failed(e.to_string()))?;
        Ok(parse_notifications(&body))
    }

    /// App riêng tư của người dùng (ví dụ bản build bàn giao của đơn xây app).
    pub async fn private_apps(&self, access: &str) -> Result<Vec<String>, AuthError> {
        let url = self
            .server
            .join("v1/catalog/private")
            .expect("URL máy chủ hợp lệ");
        let res = self
            .http
            .get(url)
            .bearer_auth(access)
            .send()
            .await
            .map_err(|e| AuthError::Offline(e.to_string()))?;
        match res.status() {
            s if s.is_success() => {}
            reqwest::StatusCode::UNAUTHORIZED => return Err(AuthError::Expired),
            s => return Err(AuthError::Failed(format!("/v1/catalog/private trả {s}"))),
        }
        let body: Value = res
            .json()
            .await
            .map_err(|e| AuthError::Failed(e.to_string()))?;
        Ok(body["apps"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|a| a["id"].as_str().map(str::to_owned))
            .collect())
    }

    /// Báo F-Store rằng người dùng đã cài app (điều kiện để đánh giá; spec app-catalog).
    pub async fn record_install(
        &self,
        access: &str,
        app_id: &str,
        version: &str,
    ) -> Result<(), AuthError> {
        let url = self
            .server
            .join("v1/catalog/installs")
            .expect("URL máy chủ hợp lệ");
        let res = self
            .http
            .post(url)
            .bearer_auth(access)
            .json(&serde_json::json!({ "app_id": app_id, "version": version }))
            .send()
            .await
            .map_err(|e| AuthError::Offline(e.to_string()))?;
        match res.status() {
            s if s.is_success() => Ok(()),
            reqwest::StatusCode::UNAUTHORIZED => Err(AuthError::Expired),
            s => Err(AuthError::Failed(format!("/v1/catalog/installs trả {s}"))),
        }
    }

    /// Xin mã khởi chạy cho app `client_id` (spec launcher, "Mở app con và truyền phiên đăng nhập").
    pub async fn launch_code(
        &self,
        access: &str,
        client_id: &str,
    ) -> Result<LaunchCode, AuthError> {
        let url = self
            .server
            .join("v1/launch-codes")
            .expect("URL máy chủ hợp lệ");
        let res = self
            .http
            .post(url)
            .bearer_auth(access)
            .json(&serde_json::json!({ "client_id": client_id }))
            .send()
            .await
            .map_err(|e| AuthError::Offline(e.to_string()))?;
        let status = res.status();
        let body: Value = res
            .json()
            .await
            .map_err(|e| AuthError::Failed(e.to_string()))?;
        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(AuthError::Expired);
        }
        serde_json::from_value(body.clone())
            .map_err(|_| AuthError::Failed(format!("{status}: {body}")))
    }
}

/// Chờ trình duyệt quay về `http://127.0.0.1:<cổng>/callback?code=...&state=...`.
pub async fn wait_for_callback(
    listener: tokio::net::TcpListener,
    expected_state: &str,
    timeout: Duration,
) -> Result<String, AuthError> {
    use axum::{extract::Query, response::Html, routing::get, Router};

    let (tx, rx) = tokio::sync::oneshot::channel::<HashMap<String, String>>();
    let tx = Arc::new(Mutex::new(Some(tx)));
    let router = Router::new().route(
        "/callback",
        get(move |Query(q): Query<HashMap<String, String>>| {
            let tx = tx.clone();
            async move {
                if let Some(tx) = tx.lock().expect("khóa").take() {
                    let _ = tx.send(q);
                }
                Html(
                    "<!doctype html><meta charset=utf-8><title>F-Launcher</title>\
                     <p>Đã đăng nhập. Hãy quay lại F-Launcher.</p>\
                     <p>Signed in. Return to F-Launcher.</p>",
                )
            }
        }),
    );
    let server = tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });
    let result = tokio::time::timeout(timeout, rx).await;
    server.abort();
    let query = result
        .map_err(|_| AuthError::Failed("hết thời gian chờ đăng nhập".into()))?
        .map_err(|_| AuthError::Failed("không nhận được kết quả từ trình duyệt".into()))?;
    if let Some(error) = query.get("error") {
        return Err(AuthError::Failed(format!(
            "{error}: {}",
            query.get("error_description").cloned().unwrap_or_default()
        )));
    }
    if query.get("state").map(String::as_str) != Some(expected_state) {
        return Err(AuthError::Failed("state không khớp, bỏ qua kết quả".into()));
    }
    query
        .get("code")
        .cloned()
        .ok_or_else(|| AuthError::Failed("callback không có code".into()))
}

/// Một thông báo trong trung tâm thông báo của launcher.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notification {
    pub title: String,
    pub body: String,
    /// Địa chỉ màn hình liên quan (mở trong trình duyệt).
    pub link: String,
    pub created_at: String,
    pub read: bool,
}

pub fn parse_notifications(body: &Value) -> Vec<Notification> {
    body["notifications"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|n| {
            Some(Notification {
                title: n["title"].as_str()?.to_owned(),
                body: n["body"].as_str().unwrap_or_default().to_owned(),
                link: n["link"]
                    .as_str()
                    .filter(|l| l.starts_with("https://") || l.starts_with("http://"))
                    .unwrap_or_default()
                    .to_owned(),
                created_at: n["created_at"].as_str().unwrap_or_default().to_owned(),
                read: n["read"].as_bool().unwrap_or(false),
            })
        })
        .collect()
}

#[cfg(test)]
mod notification_tests {
    use super::*;

    #[test]
    fn notifications_keep_only_web_links() {
        let body = serde_json::json!({"notifications": [
            {"title": "Báo giá cho đơn DH-000001", "body": "Đội F-Eco đã gửi báo giá.", "link": "https://f-eco.vn/orders/1#quote", "created_at": "2026-11-12T03:00:00Z", "read": false},
            {"title": "Lạ", "body": "", "link": "file:///etc/passwd", "created_at": "", "read": true},
            {"body": "thiếu tiêu đề"}
        ]});
        let n = parse_notifications(&body);
        assert_eq!(n.len(), 2);
        assert_eq!(n[0].link, "https://f-eco.vn/orders/1#quote");
        assert!(n[1].link.is_empty(), "chỉ mở liên kết web");
    }
}
