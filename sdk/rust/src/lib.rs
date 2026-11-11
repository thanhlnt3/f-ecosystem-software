//! F-SDK cho Rust (spec app-sdk). License MIT OR Apache-2.0.
//!
//! ```no_run
//! # async fn chay() -> Result<(), fsdk::Error> {
//! // Được F-Launcher mở: đổi mã khởi chạy lấy phiên, không cần trình duyệt.
//! // Chạy trực tiếp: mở trình duyệt để đăng nhập bằng PKCE.
//! let config = fsdk::Config::from_env()?;
//! let session = fsdk::session(&config).await?;
//! let me = session.me().await?;
//! println!("Xin chào {}", me.name.unwrap_or_default());
//! # Ok(()) }
//! ```
//!
//! - [`session_from_launcher`]: phiên từ mã khởi chạy (`FECO_LAUNCH_CODE`) mà launcher truyền.
//! - [`login_with_browser`]: đăng nhập qua trình duyệt hệ thống (Authorization Code + PKCE,
//!   địa chỉ chuyển hướng loopback). App desktop không có client secret.
//! - [`Session`]: tự làm mới access token; gọi `GET /v1/me`, `GET /v1/entitlements`, báo lỗi
//!   khi chạy (`POST /v1/apps/crashes`).
//! - [`verify_webhook`]: kiểm chữ ký `F-Eco-Signature` của webhook (app có máy chủ riêng).
//!
//! F-SDK chỉ gói gọn các chuẩn OAuth 2.0 và OpenID Connect; app vẫn tích hợp được bằng thư viện
//! OIDC chuẩn bất kỳ.

use std::{sync::Arc, time::Duration};

use base64::Engine;
use serde::Deserialize;
use serde_json::Value;
use sha2::Digest;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    sync::Mutex,
};
use url::Url;

/// Phiên bản giao diện C (xem crate `fsdk-c`).
pub const ABI_VERSION: u32 = 1;
/// Địa chỉ chuyển hướng mặc định của app desktop; nhà phát triển đăng ký nó ở Cổng nhà phát triển.
pub const DEFAULT_REDIRECT_URI: &str = "http://127.0.0.1:47280/callback";
/// Scope mặc định: định danh, refresh token và hồ sơ.
pub const DEFAULT_SCOPES: &str = "openid offline_access profile";
const TIMEOUT: Duration = Duration::from_secs(20);
const LOGIN_TIMEOUT: Duration = Duration::from_secs(600);

/// Lỗi của API F-Eco theo định dạng thống nhất (spec app-sdk, "Mã lỗi thống nhất").
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct ApiError {
    #[serde(default)]
    pub status: u16,
    #[serde(default)]
    pub error: String,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub message_vi: String,
    #[serde(default)]
    pub request_id: String,
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} {}: {} (request_id {})",
            self.status, self.error, self.message, self.request_id
        )
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// App không được launcher khởi chạy (không có `FECO_LAUNCH_CODE`).
    #[error("app không được F-Launcher khởi chạy")]
    NotLaunched,
    /// Người dùng chưa từng đồng ý các scope này cho app: cần đăng nhập qua trình duyệt một lần.
    #[error("người dùng chưa đồng ý các scope này cho app")]
    ConsentRequired,
    #[error("cấu hình F-SDK: {0}")]
    Config(String),
    #[error("mạng: {0}")]
    Network(String),
    #[error("đăng nhập: {0}")]
    Auth(String),
    #[error("API: {0}")]
    Api(ApiError),
}

pub type Result<T> = std::result::Result<T, Error>;

fn net(e: reqwest::Error) -> Error {
    Error::Network(e.to_string())
}

/// Cấu hình của app.
#[derive(Debug, Clone)]
pub struct Config {
    pub client_id: String,
    /// Máy chủ OAuth 2.0 / OIDC (issuer), ví dụ `https://auth.f-eco.vn/`.
    pub issuer: Url,
    /// API F-Eco, ví dụ `https://id.f-eco.vn/`.
    pub server: Url,
    pub redirect_uri: String,
    pub scopes: String,
}

fn with_slash(mut url: Url) -> Url {
    if !url.path().ends_with('/') {
        let path = format!("{}/", url.path());
        url.set_path(&path);
    }
    url
}

fn parse_url(name: &str, value: &str) -> Result<Url> {
    Url::parse(value)
        .map(with_slash)
        .map_err(|e| Error::Config(format!("{name} không hợp lệ: {e}")))
}

impl Config {
    pub fn new(client_id: &str, issuer: &str, server: &str) -> Result<Self> {
        Ok(Self {
            client_id: client_id.to_owned(),
            issuer: parse_url("issuer", issuer)?,
            server: parse_url("server", server)?,
            redirect_uri: DEFAULT_REDIRECT_URI.to_owned(),
            scopes: DEFAULT_SCOPES.to_owned(),
        })
    }

    pub fn redirect_uri(mut self, uri: &str) -> Self {
        uri.clone_into(&mut self.redirect_uri);
        self
    }

    pub fn scopes(mut self, scopes: &str) -> Self {
        scopes.clone_into(&mut self.scopes);
        self
    }

    /// Đọc từ biến môi trường: `FECO_CLIENT_ID`, `FECO_ISSUER_URL`, `FECO_SERVER_URL` (launcher
    /// đặt sẵn khi mở app), `FSDK_REDIRECT_URI`, `FSDK_SCOPES` (không bắt buộc).
    pub fn from_env() -> Result<Self> {
        let get = |k: &str| std::env::var(k).ok().filter(|v| !v.trim().is_empty());
        let need =
            |k: &str| get(k).ok_or_else(|| Error::Config(format!("thiếu biến môi trường {k}")));
        let mut config = Self::new(
            &need("FECO_CLIENT_ID")?,
            &need("FECO_ISSUER_URL")?,
            &need("FECO_SERVER_URL")?,
        )?;
        if let Some(uri) = get("FSDK_REDIRECT_URI") {
            config.redirect_uri = uri;
        }
        if let Some(scopes) = get("FSDK_SCOPES") {
            config.scopes = scopes;
        }
        Ok(config)
    }
}

/// PKCE (RFC 7636).
#[derive(Debug, Clone)]
pub struct Pkce {
    pub verifier: String,
    pub challenge: String,
}

fn random(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    getrandom::fill(&mut buf).expect("nguồn ngẫu nhiên của hệ điều hành");
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(buf)
}

impl Pkce {
    pub fn new() -> Self {
        let verifier = random(32);
        let challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(sha2::Sha256::digest(verifier.as_bytes()));
        Self {
            verifier,
            challenge,
        }
    }
}

impl Default for Pkce {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Deserialize)]
struct Discovery {
    authorization_endpoint: String,
    token_endpoint: String,
    #[serde(default)]
    revocation_endpoint: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct TokenResponse {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    id_token: Option<String>,
    #[serde(default)]
    expires_in: Option<u64>,
    #[serde(default)]
    scope: Option<String>,
}

#[derive(Debug, Clone)]
struct Tokens {
    access: String,
    refresh: Option<String>,
    id_token: Option<String>,
    scope: String,
    expires_at: std::time::Instant,
}

fn http() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(TIMEOUT)
        .build()
        .map_err(net)
}

async fn discover(http: &reqwest::Client, issuer: &Url) -> Result<Discovery> {
    let url = issuer
        .join(".well-known/openid-configuration")
        .map_err(|e| Error::Config(e.to_string()))?;
    let res = http.get(url).send().await.map_err(net)?;
    if !res.status().is_success() {
        return Err(Error::Network(format!(
            "khám phá OIDC trả {}",
            res.status()
        )));
    }
    res.json().await.map_err(net)
}

/// Đọc lỗi API theo định dạng thống nhất; phản hồi không đúng định dạng vẫn giữ mã HTTP.
async fn api_error(res: reqwest::Response) -> Error {
    let status = res.status().as_u16();
    let text = res.text().await.unwrap_or_default();
    let mut e: ApiError = serde_json::from_str(&text).unwrap_or_else(|_| ApiError {
        error: "unexpected_response".into(),
        message: text.chars().take(200).collect(),
        ..Default::default()
    });
    e.status = status;
    if e.error == "consent_required" {
        return Error::ConsentRequired;
    }
    Error::Api(e)
}

/// Phiên đăng nhập của app. Sao chép rẻ (dùng chung bên trong).
#[derive(Clone)]
pub struct Session {
    http: reqwest::Client,
    client_id: String,
    server: Url,
    token_endpoint: String,
    revocation_endpoint: Option<String>,
    tokens: Arc<Mutex<Tokens>>,
}

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session")
            .field("client_id", &self.client_id)
            .finish_non_exhaustive()
    }
}

/// Dữ liệu từ `GET /v1/me`: chỉ có các trường thuộc scope đã được đồng ý.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Me {
    pub sub: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub picture: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub email_verified: Option<bool>,
    #[serde(default)]
    pub locale: Option<String>,
    #[serde(default)]
    pub zoneinfo: Option<String>,
    #[serde(default)]
    pub phone_number: Option<String>,
    /// Toàn bộ phản hồi (gồm cả trường F-SDK chưa biết).
    #[serde(skip)]
    pub raw: Value,
}

/// Một quyền sử dụng của người dùng với chính app này.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Entitlement {
    pub kind: String,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub granted_at: Option<String>,
    #[serde(default)]
    pub expires_at: Option<String>,
}

impl Session {
    async fn from_tokens(
        http: reqwest::Client,
        config: &Config,
        discovery: Discovery,
        t: TokenResponse,
    ) -> Self {
        Self {
            http,
            client_id: config.client_id.clone(),
            server: config.server.clone(),
            token_endpoint: discovery.token_endpoint,
            revocation_endpoint: discovery.revocation_endpoint,
            tokens: Arc::new(Mutex::new(Tokens {
                access: t.access_token,
                refresh: t.refresh_token,
                id_token: t.id_token,
                scope: t.scope.unwrap_or_default(),
                expires_at: std::time::Instant::now()
                    + Duration::from_secs(t.expires_in.unwrap_or(3600)),
            })),
        }
    }

    /// Tiếp tục phiên từ refresh token đã lưu (ví dụ trong kho bảo mật của hệ điều hành).
    pub async fn resume(config: &Config, refresh_token: &str) -> Result<Self> {
        let http = http()?;
        let discovery = discover(&http, &config.issuer).await?;
        let t = token_request(
            &http,
            &discovery.token_endpoint,
            &[
                ("grant_type", "refresh_token"),
                ("refresh_token", refresh_token),
                ("client_id", &config.client_id),
            ],
        )
        .await?;
        Ok(Self::from_tokens(http, config, discovery, t).await)
    }

    /// Access token còn hạn; tự làm mới bằng refresh token khi còn dưới 60 giây.
    pub async fn access_token(&self) -> Result<String> {
        let mut t = self.tokens.lock().await;
        if t.expires_at > std::time::Instant::now() + Duration::from_secs(60) {
            return Ok(t.access.clone());
        }
        self.refresh_locked(&mut t).await?;
        Ok(t.access.clone())
    }

    /// Làm mới ngay (Hydra xoay refresh token mỗi lần dùng).
    pub async fn refresh(&self) -> Result<()> {
        let mut t = self.tokens.lock().await;
        self.refresh_locked(&mut t).await
    }

    async fn refresh_locked(&self, t: &mut Tokens) -> Result<()> {
        let refresh = t.refresh.clone().ok_or_else(|| {
            Error::Auth("phiên không có refresh token (cần scope offline_access)".into())
        })?;
        let r = token_request(
            &self.http,
            &self.token_endpoint,
            &[
                ("grant_type", "refresh_token"),
                ("refresh_token", &refresh),
                ("client_id", &self.client_id),
            ],
        )
        .await?;
        t.access = r.access_token;
        if r.refresh_token.is_some() {
            t.refresh = r.refresh_token;
        }
        if r.id_token.is_some() {
            t.id_token = r.id_token;
        }
        if let Some(scope) = r.scope {
            t.scope = scope;
        }
        t.expires_at =
            std::time::Instant::now() + Duration::from_secs(r.expires_in.unwrap_or(3600));
        Ok(())
    }

    /// Refresh token hiện tại, để app tự lưu vào kho bảo mật.
    pub async fn refresh_token(&self) -> Option<String> {
        self.tokens.lock().await.refresh.clone()
    }

    pub async fn id_token(&self) -> Option<String> {
        self.tokens.lock().await.id_token.clone()
    }

    /// Scope được cấp thật (có thể ít hơn scope đã xin).
    pub async fn granted_scopes(&self) -> String {
        self.tokens.lock().await.scope.clone()
    }

    /// Đánh dấu access token đã hết hạn (dùng trong test làm mới token).
    #[doc(hidden)]
    pub async fn expire_now(&self) {
        self.tokens.lock().await.expires_at = std::time::Instant::now();
    }

    async fn get(&self, path: &str) -> Result<Value> {
        let token = self.access_token().await?;
        let url = self
            .server
            .join(path)
            .map_err(|e| Error::Config(e.to_string()))?;
        let res = self
            .http
            .get(url)
            .bearer_auth(token)
            .send()
            .await
            .map_err(net)?;
        if !res.status().is_success() {
            return Err(api_error(res).await);
        }
        res.json().await.map_err(net)
    }

    /// `GET /v1/me`.
    pub async fn me(&self) -> Result<Me> {
        let raw = self.get("v1/me").await?;
        let mut me: Me =
            serde_json::from_value(raw.clone()).map_err(|e| Error::Network(e.to_string()))?;
        me.raw = raw;
        Ok(me)
    }

    /// `GET /v1/entitlements`: quyền của người dùng với chính app này (cần scope `purchases`).
    pub async fn entitlements(&self) -> Result<Vec<Entitlement>> {
        let body = self.get("v1/entitlements").await?;
        serde_json::from_value(body["entitlements"].clone())
            .map_err(|e| Error::Network(e.to_string()))
    }

    /// Báo một lần app gặp sự cố (tùy chọn); nhà phát triển chỉ thấy số tổng hợp.
    pub async fn report_crash(&self, version: &str) -> Result<()> {
        let token = self.access_token().await?;
        let url = self
            .server
            .join("v1/apps/crashes")
            .map_err(|e| Error::Config(e.to_string()))?;
        let res = self
            .http
            .post(url)
            .bearer_auth(token)
            .json(&serde_json::json!({ "version": version }))
            .send()
            .await
            .map_err(net)?;
        if res.status().is_success() {
            Ok(())
        } else {
            Err(api_error(res).await)
        }
    }

    /// Đăng xuất: thu hồi refresh token ở máy chủ.
    pub async fn logout(&self) -> Result<()> {
        let t = self.tokens.lock().await;
        if let (Some(endpoint), Some(refresh)) = (&self.revocation_endpoint, &t.refresh) {
            self.http
                .post(endpoint)
                .form(&[("token", refresh.as_str()), ("client_id", &self.client_id)])
                .send()
                .await
                .map_err(net)?;
        }
        Ok(())
    }
}

async fn token_request(
    http: &reqwest::Client,
    endpoint: &str,
    form: &[(&str, &str)],
) -> Result<TokenResponse> {
    let res = http.post(endpoint).form(form).send().await.map_err(net)?;
    let status = res.status();
    let body: Value = res.json().await.map_err(net)?;
    if !status.is_success() {
        return Err(Error::Auth(format!(
            "{}: {}",
            body["error"].as_str().unwrap_or("lỗi"),
            body["error_description"].as_str().unwrap_or_default()
        )));
    }
    serde_json::from_value(body).map_err(|e| Error::Auth(e.to_string()))
}

/// Mã khởi chạy mà F-Launcher truyền cho app.
#[derive(Debug, Clone)]
pub struct Launch {
    pub code: String,
}

impl Launch {
    /// `None` nếu app không được launcher khởi chạy.
    pub fn from_env() -> Option<Self> {
        std::env::var("FECO_LAUNCH_CODE")
            .ok()
            .filter(|c| !c.trim().is_empty())
            .map(|code| Self { code })
    }
}

/// Phiên từ mã khởi chạy: đổi mã lấy authorization code tại F-Eco, rồi đổi code lấy token bằng
/// PKCE verifier của chính app.
pub async fn session_from_launch(config: &Config, launch: &Launch) -> Result<Session> {
    let http = http()?;
    let discovery = discover(&http, &config.issuer).await?;
    let pkce = Pkce::new();
    let url = config
        .server
        .join("v1/launch-codes/exchange")
        .map_err(|e| Error::Config(e.to_string()))?;
    let res = http
        .post(url)
        .json(&serde_json::json!({
            "code": launch.code,
            "client_id": config.client_id,
            "redirect_uri": config.redirect_uri,
            "code_challenge": pkce.challenge,
            "scope": config.scopes,
        }))
        .send()
        .await
        .map_err(net)?;
    if !res.status().is_success() {
        return Err(api_error(res).await);
    }
    let body: Value = res.json().await.map_err(net)?;
    let code = body["authorization_code"]
        .as_str()
        .ok_or_else(|| Error::Auth("F-Eco không trả authorization code".into()))?;
    let t = token_request(
        &http,
        &discovery.token_endpoint,
        &[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", &config.redirect_uri),
            ("client_id", &config.client_id),
            ("code_verifier", &pkce.verifier),
        ],
    )
    .await?;
    Ok(Session::from_tokens(http, config, discovery, t).await)
}

/// Phiên khi được F-Launcher khởi chạy (đọc `FECO_LAUNCH_CODE` và cấu hình từ biến môi trường).
pub async fn session_from_launcher() -> Result<Session> {
    let launch = Launch::from_env().ok_or(Error::NotLaunched)?;
    session_from_launch(&Config::from_env()?, &launch).await
}

/// Đăng nhập qua trình duyệt hệ thống bằng PKCE.
pub async fn login_with_browser(config: &Config) -> Result<Session> {
    login_with(config, |url| {
        open::that(url.as_str()).map_err(|e| e.to_string())
    })
    .await
}

/// Như [`login_with_browser`] nhưng tự quyết cách mở địa chỉ đăng nhập.
pub async fn login_with(
    config: &Config,
    open: impl FnOnce(&Url) -> std::result::Result<(), String>,
) -> Result<Session> {
    let redirect = Url::parse(&config.redirect_uri)
        .map_err(|e| Error::Config(format!("redirect_uri không hợp lệ: {e}")))?;
    let host = redirect.host_str().unwrap_or_default();
    if redirect.scheme() != "http" || !matches!(host, "127.0.0.1" | "localhost") {
        return Err(Error::Config(
            "đăng nhập qua trình duyệt cần redirect_uri loopback http://127.0.0.1:<cổng>/..."
                .into(),
        ));
    }
    let port = redirect.port().unwrap_or(80);
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port))
        .await
        .map_err(|e| Error::Auth(format!("cổng {port} đang bận: {e}")))?;
    let http = http()?;
    let discovery = discover(&http, &config.issuer).await?;
    let pkce = Pkce::new();
    let state = random(16);
    let mut auth =
        Url::parse(&discovery.authorization_endpoint).map_err(|e| Error::Config(e.to_string()))?;
    auth.query_pairs_mut()
        .append_pair("client_id", &config.client_id)
        .append_pair("response_type", "code")
        .append_pair("scope", &config.scopes)
        .append_pair("redirect_uri", &config.redirect_uri)
        .append_pair("state", &state)
        .append_pair("code_challenge", &pkce.challenge)
        .append_pair("code_challenge_method", "S256");
    open(&auth).map_err(Error::Auth)?;
    let query = tokio::time::timeout(LOGIN_TIMEOUT, wait_for_redirect(&listener, redirect.path()))
        .await
        .map_err(|_| Error::Auth("hết thời gian chờ đăng nhập".into()))??;
    let get = |k: &str| query.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone());
    if let Some(error) = get("error") {
        return Err(Error::Auth(format!(
            "{error}: {}",
            get("error_description").unwrap_or_default()
        )));
    }
    if get("state").as_deref() != Some(state.as_str()) {
        return Err(Error::Auth("state không khớp".into()));
    }
    let code = get("code").ok_or_else(|| Error::Auth("không có code".into()))?;
    let t = token_request(
        &http,
        &discovery.token_endpoint,
        &[
            ("grant_type", "authorization_code"),
            ("code", &code),
            ("redirect_uri", &config.redirect_uri),
            ("client_id", &config.client_id),
            ("code_verifier", &pkce.verifier),
        ],
    )
    .await?;
    Ok(Session::from_tokens(http, config, discovery, t).await)
}

/// Phiên theo cách app được mở: qua launcher thì dùng mã khởi chạy, không thì đăng nhập qua
/// trình duyệt. Mã khởi chạy mà người dùng chưa đồng ý scope cũng chuyển sang trình duyệt.
pub async fn session(config: &Config) -> Result<Session> {
    match Launch::from_env() {
        Some(launch) => match session_from_launch(config, &launch).await {
            Err(Error::ConsentRequired) => login_with_browser(config).await,
            other => other,
        },
        None => login_with_browser(config).await,
    }
}

/// Đọc một yêu cầu HTTP trên địa chỉ loopback; trả query của yêu cầu tới `path`.
async fn wait_for_redirect(
    listener: &tokio::net::TcpListener,
    path: &str,
) -> Result<Vec<(String, String)>> {
    loop {
        let (mut socket, _) = listener
            .accept()
            .await
            .map_err(|e| Error::Auth(e.to_string()))?;
        let mut buf = vec![0u8; 8192];
        let mut n = 0;
        while n < buf.len() {
            let read = socket.read(&mut buf[n..]).await.unwrap_or(0);
            if read == 0 {
                break;
            }
            n += read;
            if buf[..n].windows(4).any(|w| w == b"\r\n\r\n") {
                break;
            }
        }
        let request = String::from_utf8_lossy(&buf[..n]);
        let target = request
            .lines()
            .next()
            .and_then(|l| l.strip_prefix("GET "))
            .and_then(|l| l.split(' ').next())
            .unwrap_or_default()
            .to_owned();
        let url = Url::parse(&format!("http://127.0.0.1{target}")).ok();
        let matches = url.as_ref().is_some_and(|u| u.path() == path);
        let body = if matches {
            "<!doctype html><meta charset=utf-8><p>Đã đăng nhập. Hãy quay lại app.</p><p>Signed in. Return to the app.</p>"
        } else {
            "Not found"
        };
        let status = if matches { "200 OK" } else { "404 Not Found" };
        let response = format!(
            "HTTP/1.1 {status}\r\ncontent-type: text/html; charset=utf-8\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
            body.len()
        );
        let _ = socket.write_all(response.as_bytes()).await;
        let _ = socket.shutdown().await;
        if let (true, Some(url)) = (matches, url) {
            return Ok(url.query_pairs().into_owned().collect());
        }
    }
}

/// Kiểm chữ ký webhook `F-Eco-Signature: t=<giây>,v1=<hex>` với khóa ký của app; chấp nhận lệch
/// đồng hồ tối đa 5 phút.
pub fn verify_webhook(secret: &str, header: &str, body: &[u8], now_unix: i64) -> bool {
    use hmac::{Hmac, KeyInit, Mac};
    let mut t = None;
    let mut v1 = None;
    for part in header.split(',') {
        match part.trim().split_once('=') {
            Some(("t", x)) => t = x.parse::<i64>().ok(),
            Some(("v1", x)) => v1 = Some(x.to_owned()),
            _ => {}
        }
    }
    let (Some(t), Some(v1)) = (t, v1) else {
        return false;
    };
    if (now_unix - t).abs() > 300 {
        return false;
    }
    let Ok(expected) = (0..v1.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(v1.get(i..i + 2).unwrap_or("zz"), 16))
        .collect::<std::result::Result<Vec<u8>, _>>()
    else {
        return false;
    };
    let Ok(mut mac) = Hmac::<sha2::Sha256>::new_from_slice(secret.as_bytes()) else {
        return false;
    };
    mac.update(t.to_string().as_bytes());
    mac.update(b".");
    mac.update(body);
    // So sánh thời gian hằng định do thư viện hmac thực hiện.
    mac.verify_slice(&expected).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkce_shape() {
        let p = Pkce::new();
        assert_eq!(p.verifier.len(), 43);
        assert_ne!(p.verifier, Pkce::new().verifier);
        let expected = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(sha2::Sha256::digest(p.verifier.as_bytes()));
        assert_eq!(p.challenge, expected);
    }

    #[test]
    fn config_validates_urls() {
        let c = Config::new("app", "http://127.0.0.1:4444", "http://127.0.0.1:8080/api").unwrap();
        assert_eq!(c.issuer.as_str(), "http://127.0.0.1:4444/");
        assert_eq!(c.server.as_str(), "http://127.0.0.1:8080/api/");
        assert_eq!(c.redirect_uri, DEFAULT_REDIRECT_URI);
        assert!(Config::new("app", "khong-phai-url", "http://x").is_err());
    }

    #[test]
    fn webhook_signatures() {
        use hmac::{Hmac, KeyInit, Mac};
        let body = br#"{"event":"user.revoked"}"#;
        let mut mac = Hmac::<sha2::Sha256>::new_from_slice(b"bi-mat").unwrap();
        mac.update(b"1000.");
        mac.update(body);
        let hex: String = mac
            .finalize()
            .into_bytes()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        let header = format!("t=1000,v1={hex}");
        assert!(verify_webhook("bi-mat", &header, body, 1100));
        assert!(!verify_webhook("khac", &header, body, 1100));
        assert!(!verify_webhook("bi-mat", &header, b"{}", 1100));
        assert!(!verify_webhook("bi-mat", &header, body, 2000), "quá 5 phút");
        assert!(!verify_webhook("bi-mat", "t=1000", body, 1000));
    }

    #[tokio::test]
    async fn loopback_redirect_is_parsed() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            let http = reqwest::Client::new();
            let _ = http
                .get(format!("http://127.0.0.1:{port}/favicon.ico"))
                .send()
                .await;
            let res = http
                .get(format!(
                    "http://127.0.0.1:{port}/callback?code=abc&state=xyz"
                ))
                .send()
                .await
                .unwrap();
            assert!(res.text().await.unwrap().contains("Đã đăng nhập"));
        });
        let q = wait_for_redirect(&listener, "/callback").await.unwrap();
        assert!(
            q.contains(&("code".into(), "abc".into()))
                && q.contains(&("state".into(), "xyz".into()))
        );
    }

    #[tokio::test]
    async fn api_errors_keep_status_and_fields() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            for body in [
                r#"{"error":"invalid_token","message":"expired","message_vi":"hết hạn","request_id":"r-1"}"#,
                r#"{"error":"consent_required","message":"x","message_vi":"y","request_id":"r-2"}"#,
            ] {
                let (mut s, _) = listener.accept().await.unwrap();
                let mut buf = [0u8; 2048];
                let _ = s.read(&mut buf).await;
                let status = if body.contains("invalid_token") {
                    "401 Unauthorized"
                } else {
                    "403 Forbidden"
                };
                let res = format!(
                    "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                );
                s.write_all(res.as_bytes()).await.unwrap();
            }
        });
        let http = reqwest::Client::new();
        let res = http
            .get(format!("http://{addr}/v1/me"))
            .send()
            .await
            .unwrap();
        match api_error(res).await {
            Error::Api(e) => {
                assert_eq!(
                    (e.status, e.error.as_str(), e.request_id.as_str()),
                    (401, "invalid_token", "r-1")
                );
                assert_eq!(e.message_vi, "hết hạn");
            }
            other => panic!("{other:?}"),
        }
        let res = http
            .get(format!("http://{addr}/v1/me"))
            .send()
            .await
            .unwrap();
        assert!(matches!(api_error(res).await, Error::ConsentRequired));
    }
}
