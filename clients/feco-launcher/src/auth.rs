//! Đăng nhập launcher (spec launcher, "Đăng nhập trong launcher"; design D4).
//!
//! - Mở trang đăng nhập F-Eco trong trình duyệt mặc định (OAuth 2.0 Authorization Code + PKCE,
//!   client công khai, địa chỉ chuyển hướng loopback `http://127.0.0.1:47201/callback`).
//!   Launcher không có ô nhập mật khẩu.
//! - Refresh token nằm trong kho bảo mật của hệ điều hành (Windows Credential Manager, macOS
//!   Keychain) qua `keyring-core` và kho gốc của từng hệ điều hành; access token chỉ nằm trong bộ nhớ. Khởi động lại máy rồi
//!   mở launcher thì dùng refresh token để vào lại, không phải đăng nhập lại.

use crate::broker::LaunchCode;
use crate::chat::{self, ChatError, Conversation, Message};
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
        // Đổi token qua điểm nhận token của F-Eco (chuyển tiếp tới Hydra), để F-Eco nhận ra
        // refresh token bị dùng lại (thay đổi oidc-login-and-token-reuse, design T1).
        let url = self
            .server
            .join("oauth2/token")
            .expect("URL máy chủ hợp lệ");
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
        } else if body["error"] == "invalid_grant" || body["error"] == "token_inactive" {
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

    /// Vai trò nền tảng của người dùng (`GET /v1/account/role`, chỉ token của launcher).
    pub async fn account_role(&self, access: &str) -> Result<AccountRole, AuthError> {
        let url = self
            .server
            .join("v1/account/role")
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
            s => return Err(AuthError::Failed(format!("/v1/account/role trả {s}"))),
        }
        let body: Value = res
            .json()
            .await
            .map_err(|e| AuthError::Failed(e.to_string()))?;
        Ok(AccountRole::from_json(&body))
    }

    /// Giá F-Coin của các app có giá, từ danh mục công khai (`GET /v1/catalog`), để hiện giá cả
    /// khi chưa đăng nhập.
    pub async fn catalog_prices(&self) -> Result<HashMap<String, i64>, AuthError> {
        let url = self.server.join("v1/catalog").expect("URL máy chủ hợp lệ");
        let res = self
            .http
            .get(url)
            .send()
            .await
            .map_err(|e| AuthError::Offline(e.to_string()))?;
        if !res.status().is_success() {
            return Err(AuthError::Failed(format!(
                "/v1/catalog trả {}",
                res.status()
            )));
        }
        let body: Value = res
            .json()
            .await
            .map_err(|e| AuthError::Failed(e.to_string()))?;
        Ok(parse_catalog_prices(&body))
    }

    /// Ví F-Coin của người dùng (`GET /v1/wallet`, chỉ token của launcher).
    pub async fn wallet(&self, access: &str) -> Result<WalletInfo, AuthError> {
        let url = self.server.join("v1/wallet").expect("URL máy chủ hợp lệ");
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
            s => return Err(AuthError::Failed(format!("/v1/wallet trả {s}"))),
        }
        let body: Value = res
            .json()
            .await
            .map_err(|e| AuthError::Failed(e.to_string()))?;
        Ok(WalletInfo::from_json(&body))
    }

    /// Mua app bằng F-Coin (`POST /v1/wallet/purchases`). Trả số dư sau khi mua; lỗi nghiệp vụ
    /// (không đủ F-Coin, giá đổi...) trả mã lỗi của máy chủ.
    pub async fn purchase(
        &self,
        access: &str,
        app_id: &str,
        expected_price: i64,
    ) -> Result<i64, PurchaseError> {
        let url = self
            .server
            .join("v1/wallet/purchases")
            .expect("URL máy chủ hợp lệ");
        let res = self
            .http
            .post(url)
            .bearer_auth(access)
            .json(&serde_json::json!({ "app_id": app_id, "expected_price": expected_price }))
            .send()
            .await
            .map_err(|e| PurchaseError::Auth(AuthError::Offline(e.to_string())))?;
        let status = res.status();
        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(PurchaseError::Auth(AuthError::Expired));
        }
        let body: Value = res.json().await.unwrap_or(Value::Null);
        if status.is_success() {
            return Ok(body["balance"].as_i64().unwrap_or(0));
        }
        Err(PurchaseError::Api {
            code: body["error"].as_str().unwrap_or("unknown").to_owned(),
            message: body["message_vi"]
                .as_str()
                .or(body["message"].as_str())
                .unwrap_or_default()
                .to_owned(),
        })
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

    /// Báo F-Store đã gỡ app (`DELETE /v1/catalog/installs/{app}`, thay đổi app-update-notices).
    pub async fn remove_install(&self, access: &str, app_id: &str) -> Result<(), AuthError> {
        let mut url = self
            .server
            .join("v1/catalog/installs/")
            .expect("URL máy chủ hợp lệ");
        url.path_segments_mut()
            .expect("URL máy chủ có đường dẫn")
            .pop_if_empty()
            .push(app_id);
        let res = self
            .http
            .delete(url)
            .bearer_auth(access)
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

    // ---- Tin nhắn (`/v1/chat`, thay đổi chat-and-source-scan, design C4).

    fn chat_url(&self, path: &str) -> Result<Url, ChatError> {
        self.server
            .join(&format!("v1/chat/{path}"))
            .map_err(|e| ChatError::Auth(AuthError::Failed(e.to_string())))
    }

    fn conversation_url(&self, id: &str, tail: &str) -> Result<Url, ChatError> {
        if !chat::safe_id(id) {
            return Err(ChatError::Api {
                status: 404,
                code: "chat_not_found".into(),
            });
        }
        self.chat_url(&format!("conversations/{id}/{tail}"))
    }

    /// Gửi yêu cầu chat; 401 thành [`AuthError::Expired`], lỗi khác thành mã `{"error": …}`.
    async fn chat_send_request(
        &self,
        req: reqwest::RequestBuilder,
        access: &str,
    ) -> Result<Value, ChatError> {
        let res = req
            .bearer_auth(access)
            .send()
            .await
            .map_err(|e| ChatError::Auth(AuthError::Offline(e.to_string())))?;
        let status = res.status();
        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(ChatError::Auth(AuthError::Expired));
        }
        if status == reqwest::StatusCode::NO_CONTENT {
            return Ok(Value::Null);
        }
        let body: Value = res.json().await.unwrap_or(Value::Null);
        if status.is_success() {
            Ok(body)
        } else {
            Err(ChatError::Api {
                status: status.as_u16(),
                code: body["error"].as_str().unwrap_or("unknown").to_owned(),
            })
        }
    }

    /// Tổng số tin chưa đọc (`GET /v1/chat/unread`).
    pub async fn chat_unread(&self, access: &str) -> Result<usize, ChatError> {
        let url = self.chat_url("unread")?;
        let body = self.chat_send_request(self.http.get(url), access).await?;
        Ok(chat::parse_unread(&body))
    }

    /// Các hội thoại, mới nhất trước (`GET /v1/chat/conversations`).
    pub async fn chat_conversations(&self, access: &str) -> Result<Vec<Conversation>, ChatError> {
        let url = self.chat_url("conversations")?;
        let body = self.chat_send_request(self.http.get(url), access).await?;
        Ok(chat::parse_conversations(&body))
    }

    /// Một trang tin sau tin `after`, cũ trước (`GET …/messages?after=&limit=`).
    pub async fn chat_messages(
        &self,
        access: &str,
        conversation: &str,
        after: i64,
        limit: usize,
    ) -> Result<Vec<Message>, ChatError> {
        let mut url = self.conversation_url(conversation, "messages")?;
        url.query_pairs_mut()
            .append_pair("after", &after.max(0).to_string())
            .append_pair("limit", &limit.clamp(1, chat::PAGE_LIMIT).to_string());
        let body = self.chat_send_request(self.http.get(url), access).await?;
        Ok(chat::parse_messages(&body))
    }

    /// Mọi tin sau tin `after`: tải từng trang 200 tin tới khi trang cuối không đầy (tối đa 50
    /// trang mỗi lần, phần còn lại lấy ở lần hỏi sau).
    pub async fn chat_messages_after(
        &self,
        access: &str,
        conversation: &str,
        after: i64,
    ) -> Result<Vec<Message>, ChatError> {
        let mut out: Vec<Message> = Vec::new();
        let mut cursor = after;
        for _ in 0..50 {
            let page = self
                .chat_messages(access, conversation, cursor, chat::PAGE_LIMIT)
                .await?;
            let full = page.len() >= chat::PAGE_LIMIT;
            let last = page.iter().map(|m| m.id).max().unwrap_or(cursor);
            chat::merge_messages(&mut out, page);
            if !full || last <= cursor {
                break;
            }
            cursor = last;
        }
        Ok(out)
    }

    /// Gửi tin văn bản (`POST …/messages`); trả tin máy chủ đã lưu.
    pub async fn chat_send(
        &self,
        access: &str,
        conversation: &str,
        body: &str,
    ) -> Result<Message, ChatError> {
        let url = self.conversation_url(conversation, "messages")?;
        let req = self
            .http
            .post(url)
            .json(&serde_json::json!({ "body": body }));
        let res = self.chat_send_request(req, access).await?;
        chat::parse_message(&res)
            .ok_or_else(|| ChatError::Auth(AuthError::Failed(format!("tin không hợp lệ: {res}"))))
    }

    /// Đánh dấu đã đọc tới tin `up_to` (`POST …/read`).
    pub async fn chat_read(
        &self,
        access: &str,
        conversation: &str,
        up_to: i64,
    ) -> Result<(), ChatError> {
        let url = self.conversation_url(conversation, "read")?;
        let req = self
            .http
            .post(url)
            .json(&serde_json::json!({ "up_to": up_to }));
        self.chat_send_request(req, access).await.map(|_| ())
    }
}

/// Chờ trình duyệt quay về `http://127.0.0.1:<cổng>/callback?code=...&state=...`.
pub async fn wait_for_callback(
    listener: tokio::net::TcpListener,
    expected_state: &str,
    timeout: Duration,
) -> Result<String, AuthError> {
    use axum::{extract::Query, http::StatusCode, response::Html, routing::get, Router};

    let (tx, rx) = tokio::sync::oneshot::channel::<HashMap<String, String>>();
    let tx = Arc::new(Mutex::new(Some(tx)));
    let expected = expected_state.to_owned();
    let router = Router::new().route(
        "/callback",
        get(move |Query(q): Query<HashMap<String, String>>| {
            let tx = tx.clone();
            let expected = expected.clone();
            async move {
                // Yêu cầu có state lạ (trang khác gọi vào cổng loopback) bị bỏ qua; vẫn chờ
                // callback đúng tới hết thời gian.
                if q.get("state") != Some(&expected) {
                    return (
                        StatusCode::BAD_REQUEST,
                        Html(
                            "<!doctype html><meta charset=utf-8><title>F-Launcher</title>\
                             <p>Yêu cầu không hợp lệ.</p><p>Invalid request.</p>",
                        ),
                    );
                }
                if let Some(tx) = tx.lock().expect("khóa").take() {
                    let _ = tx.send(q);
                }
                (
                    StatusCode::OK,
                    Html(
                        "<!doctype html><meta charset=utf-8><title>F-Launcher</title>\
                         <p>Đã đăng nhập. Hãy quay lại F-Launcher.</p>\
                         <p>Signed in. Return to F-Launcher.</p>",
                    ),
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

/// Liên kết máy chủ gửi về mà launcher được mở: `https://` (máy chủ bất kỳ), hoặc `http://` chỉ
/// tới máy dev (`127.0.0.1`, `localhost`, `[::1]`); không có tên đăng nhập, mật khẩu. Phân tích
/// bằng `url::Url` và so đúng scheme, host (không so tiền tố chuỗi: `http://127.0.0.1.evil.example`
/// không qua được).
pub fn web_link(s: &str) -> bool {
    let Ok(u) = Url::parse(s) else {
        return false;
    };
    if !u.username().is_empty() || u.password().is_some() {
        return false;
    }
    match u.scheme() {
        "https" => u.host_str().is_some_and(|h| !h.is_empty()),
        "http" => matches!(u.host_str(), Some("127.0.0.1" | "localhost" | "[::1]")),
        _ => false,
    }
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

/// Vai trò nền tảng (thay đổi platform-roles): người sử dụng ứng dụng, lập trình viên, quản trị
/// viên. Chỉ để hiển thị và gợi lối tắt; mọi kiểm soát quyền nằm ở máy chủ.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AccountRole {
    /// `user`, `customer`, `business_customer`, `developer`, `dev_company` hoặc `admin`.
    pub role: String,
    #[serde(default)]
    pub developer_portal: Option<String>,
    #[serde(default)]
    pub admin_console: Option<String>,
    /// Các mục quản trị được vào: (khóa, địa chỉ), ví dụ `("reviews", "https://…/admin/reviews")`
    /// (thay đổi admin-list-tools, design L5). Chỉ nhận liên kết https hoặc máy dev.
    #[serde(default)]
    pub admin_sections: Vec<(String, String)>,
}

impl AccountRole {
    pub fn from_json(body: &Value) -> Self {
        let role = match body["role"].as_str() {
            Some(
                r @ ("customer" | "business_customer" | "developer" | "dev_company" | "admin"),
            ) => r.to_owned(),
            _ => "user".to_owned(),
        };
        let https_or_local = |v: &Value| v.as_str().filter(|u| web_link(u)).map(str::to_owned);
        let admin_sections = body["admin_sections"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|s| {
                let key = s["key"].as_str().filter(|k| {
                    !k.is_empty()
                        && k.len() <= 32
                        && k.chars().all(|c| c.is_ascii_lowercase() || c == '-')
                })?;
                Some((key.to_owned(), https_or_local(&s["url"])?))
            })
            .collect();
        AccountRole {
            role,
            developer_portal: https_or_local(&body["developer_portal"]),
            admin_console: https_or_local(&body["admin_console"]),
            admin_sections,
        }
    }

    pub fn can_develop(&self) -> bool {
        matches!(self.role.as_str(), "developer" | "dev_company" | "admin")
    }

    pub fn is_admin(&self) -> bool {
        self.role == "admin"
    }
}

/// Giá F-Coin (> 0) theo app từ `GET /v1/catalog`.
pub fn parse_catalog_prices(body: &Value) -> HashMap<String, i64> {
    body["apps"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|a| {
            let price = a["price_coin"].as_i64().filter(|p| *p > 0)?;
            Some((a["id"].as_str()?.to_owned(), price))
        })
        .collect()
}

/// Ví F-Coin như máy chủ trả về (thay đổi fcoin-wallet-and-marketplace).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WalletInfo {
    pub balance: i64,
    /// Số đồng cho 1 F-Coin.
    pub rate_vnd: i64,
    pub frozen: bool,
    /// App đã mua bằng F-Coin.
    pub owned: Vec<String>,
    /// Giá các app có giá (F-Coin).
    pub prices: HashMap<String, i64>,
    /// Trang ví trên web (nạp, rút).
    pub wallet_url: Option<String>,
}

impl WalletInfo {
    pub fn from_json(body: &Value) -> Self {
        WalletInfo {
            balance: body["balance"].as_i64().unwrap_or(0),
            rate_vnd: body["rate_vnd"].as_i64().unwrap_or(0),
            frozen: body["frozen"].as_bool().unwrap_or(false),
            owned: body["owned"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect(),
            prices: body["prices"]
                .as_object()
                .into_iter()
                .flatten()
                .filter_map(|(k, v)| Some((k.clone(), v.as_i64().filter(|p| *p > 0)?)))
                .collect(),
            wallet_url: body["wallet_url"]
                .as_str()
                .filter(|u| web_link(u))
                .map(str::to_owned),
        }
    }

    /// Giá của app (0: miễn phí hoặc không có trong danh sách).
    pub fn price(&self, app: &str) -> i64 {
        self.prices.get(app).copied().unwrap_or(0)
    }

    pub fn owns(&self, app: &str) -> bool {
        self.owned.iter().any(|a| a == app)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PurchaseError {
    #[error(transparent)]
    Auth(AuthError),
    /// Lỗi nghiệp vụ của máy chủ: `insufficient_funds`, `already_owned`, `price_changed`,
    /// `wallet_frozen`...
    #[error("{message}")]
    Api { code: String, message: String },
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
                    .filter(|l| web_link(l))
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

    /// platform-roles: vai trò lạ coi như người sử dụng; chỉ nhận liên kết https (hoặc máy dev).
    #[test]
    fn account_role_is_parsed_safely() {
        let dev = AccountRole::from_json(&serde_json::json!({
            "role": "developer",
            "developer_portal": "https://f-eco.example/developer",
            "admin_console": "javascript:alert(1)",
        }));
        assert!(dev.can_develop() && !dev.is_admin());
        assert_eq!(
            dev.developer_portal.as_deref(),
            Some("https://f-eco.example/developer")
        );
        assert_eq!(dev.admin_console, None);
        let odd = AccountRole::from_json(&serde_json::json!({"role": "root"}));
        assert_eq!(odd.role, "user");
        assert!(!odd.can_develop());
        assert!(AccountRole::from_json(&serde_json::json!({"role": "admin"})).is_admin());
    }

    /// admin-list-tools: mục quản trị chỉ nhận khóa hợp lệ và liên kết https (hoặc máy dev).
    #[test]
    fn admin_sections_are_parsed_safely() {
        let admin = AccountRole::from_json(&serde_json::json!({
            "role": "admin",
            "admin_console": "https://f-eco.example/admin",
            "admin_sections": [
                {"key": "reviews", "url": "https://f-eco.example/admin/reviews"},
                {"key": "chat-reports", "url": "http://127.0.0.1:8080/admin/chat-reports"},
                {"key": "orders", "url": "javascript:alert(1)"},
                {"key": "<b>", "url": "https://f-eco.example/admin/x"},
                {"url": "https://f-eco.example/admin/y"},
            ],
        }));
        assert_eq!(
            admin.admin_sections,
            [
                (
                    "reviews".to_owned(),
                    "https://f-eco.example/admin/reviews".to_owned()
                ),
                (
                    "chat-reports".to_owned(),
                    "http://127.0.0.1:8080/admin/chat-reports".to_owned()
                ),
            ]
        );
        // Bản cũ của máy chủ không có trường này.
        assert!(
            AccountRole::from_json(&serde_json::json!({"role": "admin"}))
                .admin_sections
                .is_empty()
        );
    }

    /// Liên kết được phân tích và so đúng host, không so tiền tố chuỗi.
    #[test]
    fn web_links_compare_scheme_and_exact_host() {
        for ok in [
            "https://f-eco.vn/wallet",
            "http://127.0.0.1:8080/wallet",
            "http://localhost:8080/developer",
            "http://[::1]:8080/",
        ] {
            assert!(web_link(ok), "{ok}");
        }
        for bad in [
            "http://127.0.0.1.evil.example/wallet",
            "http://127.0.0.1@evil.example/",
            "https://user:pass@f-eco.vn/",
            "http://f-eco.vn/",
            "javascript:alert(1)",
            "file:///etc/passwd",
            "https//f-eco.vn",
            "",
        ] {
            assert!(!web_link(bad), "{bad}");
        }
        let wallet = WalletInfo::from_json(&serde_json::json!({
            "wallet_url": "http://127.0.0.1.evil.example/wallet"
        }));
        assert_eq!(wallet.wallet_url, None);
        let role = AccountRole::from_json(&serde_json::json!({
            "role": "admin", "admin_console": "http://127.0.0.1:8080/admin"
        }));
        assert_eq!(
            role.admin_console.as_deref(),
            Some("http://127.0.0.1:8080/admin")
        );
    }

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
