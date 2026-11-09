//! Client mỏng cho Ory Kratos (design D2). feco-server không tự xử lý mật khẩu hay
//! phiên: nó chỉ chuyển yêu cầu của trình duyệt sang Kratos (kèm cookie) và trả lại
//! cookie Kratos đặt, để chèn được các bước của F-Eco (làm chậm, chống bot, không lộ
//! email đã tồn tại) vào giữa.

use axum::http::{header, HeaderValue, StatusCode};
use reqwest::redirect::Policy;
use serde_json::Value;
use url::Url;

/// Các luồng tự phục vụ của Kratos mà F-Eco hiển thị.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlowKind {
    Login,
    Registration,
    Verification,
    Recovery,
    Settings,
}

impl FlowKind {
    pub const fn path(self) -> &'static str {
        match self {
            FlowKind::Login => "login",
            FlowKind::Registration => "registration",
            FlowKind::Verification => "verification",
            FlowKind::Recovery => "recovery",
            FlowKind::Settings => "settings",
        }
    }
}

/// Phản hồi của Kratos cho một lần gửi biểu mẫu.
#[derive(Debug)]
pub struct KratosResponse {
    pub status: StatusCode,
    pub body: Value,
    /// Các header `Set-Cookie` cần chuyển nguyên cho trình duyệt.
    pub set_cookies: Vec<HeaderValue>,
}

impl KratosResponse {
    /// Mã các thông báo cấp luồng và cấp trường (ví dụ 4000006: sai thông tin đăng nhập).
    pub fn message_ids(&self) -> Vec<i64> {
        message_ids(&self.body)
    }
}

pub fn message_ids(flow: &Value) -> Vec<i64> {
    let ui = &flow["ui"];
    let flow_msgs = ui["messages"].as_array().into_iter().flatten();
    let node_msgs = ui["nodes"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|n| n["messages"].as_array().into_iter().flatten());
    flow_msgs
        .chain(node_msgs)
        .filter_map(|m| m["id"].as_i64())
        .collect()
}

/// Thông tin trình duyệt gửi kèm cho Kratos, để Kratos ghi đúng thiết bị và địa chỉ IP
/// của người dùng vào phiên (thay vì của chính feco-server).
///
/// IP lấy từ `X-Forwarded-For` (khi chạy sau proxy) hoặc từ địa chỉ kết nối. Ở
/// production, proxy phía trước phải ghi đè `X-Forwarded-For` để người dùng không tự
/// khai IP giả (chỉ ảnh hưởng tới phần hiển thị thiết bị).
#[derive(Debug, Clone, Default)]
pub struct ClientInfo {
    pub user_agent: Option<String>,
    pub ip: Option<String>,
}

impl<S: Send + Sync> axum::extract::FromRequestParts<S> for ClientInfo {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        _: &S,
    ) -> Result<Self, Self::Rejection> {
        let header_str = |name: &str| {
            parts
                .headers
                .get(name)
                .and_then(|v| v.to_str().ok())
                .map(str::to_owned)
        };
        let forwarded = header_str("x-forwarded-for")
            .and_then(|v| v.split(',').next().map(|s| s.trim().to_owned()))
            .filter(|s| !s.is_empty());
        let remote = parts
            .extensions
            .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
            .map(|c| c.0.ip().to_string());
        Ok(ClientInfo {
            user_agent: header_str("user-agent"),
            ip: forwarded.or(remote),
        })
    }
}

/// Tùy chọn khi khởi tạo luồng.
#[derive(Debug, Clone, Default)]
pub struct InitOptions {
    pub return_to: Option<String>,
    /// `aal2` để yêu cầu bước xác thực thứ hai.
    pub aal: Option<String>,
    /// Bắt đăng nhập lại dù đang có phiên (để có phiên đặc quyền).
    pub refresh: bool,
}

/// Kết quả hỏi phiên hiện tại.
#[derive(Debug)]
pub enum Whoami {
    Anonymous,
    Session(Value),
    /// Phiên mới qua bước một; cần bước thứ hai trước khi dùng. Chứa địa chỉ chuyển hướng.
    NeedsSecondFactor(String),
}

#[derive(Debug, thiserror::Error)]
pub enum KratosError {
    #[error("không gọi được Kratos: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Kratos trả về dữ liệu không hợp lệ: {0}")]
    Body(String),
}

#[derive(Clone)]
pub struct KratosClient {
    http: reqwest::Client,
    public: Url,
    admin: Url,
}

impl KratosClient {
    pub fn new(public: Url, admin: Url) -> Self {
        let http = reqwest::Client::builder()
            // Proxy tự xử lý mọi chuyển hướng (303, 422) để trả đúng cho trình duyệt.
            .redirect(Policy::none())
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .expect("tạo HTTP client");
        KratosClient {
            http,
            public,
            admin,
        }
    }

    fn public_url(&self, path: &str) -> Url {
        self.public.join(path).expect("đường dẫn Kratos hợp lệ")
    }

    /// Địa chỉ để trình duyệt khởi tạo một luồng mới (Kratos sẽ chuyển hướng lại UI).
    pub fn browser_init_url(&self, kind: FlowKind, opts: &InitOptions) -> Url {
        let mut url = self.public_url(&format!("self-service/{}/browser", kind.path()));
        {
            let mut q = url.query_pairs_mut();
            if let Some(r) = &opts.return_to {
                q.append_pair("return_to", r);
            }
            if kind == FlowKind::Login {
                if let Some(aal) = &opts.aal {
                    q.append_pair("aal", aal);
                }
                if opts.refresh {
                    q.append_pair("refresh", "true");
                }
            }
        }
        if url.query() == Some("") {
            url.set_query(None);
        }
        url
    }

    async fn into_response(res: reqwest::Response) -> Result<KratosResponse, KratosError> {
        let status = StatusCode::from_u16(res.status().as_u16())
            .map_err(|e| KratosError::Body(e.to_string()))?;
        let set_cookies = res
            .headers()
            .get_all(reqwest::header::SET_COOKIE)
            .iter()
            .filter_map(|v| HeaderValue::from_bytes(v.as_bytes()).ok())
            .collect();
        let bytes = res.bytes().await?;
        let body = if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes).map_err(|e| KratosError::Body(e.to_string()))?
        };
        Ok(KratosResponse {
            status,
            body,
            set_cookies,
        })
    }

    fn with_cookie(req: reqwest::RequestBuilder, cookie: Option<&str>) -> reqwest::RequestBuilder {
        let req = req.header(header::ACCEPT.as_str(), "application/json");
        match cookie {
            Some(c) => req.header(header::COOKIE.as_str(), c),
            None => req,
        }
    }

    /// Lấy một luồng theo id (luồng kiểu browser cần cookie CSRF của trình duyệt).
    pub async fn get_flow(
        &self,
        kind: FlowKind,
        id: &str,
        cookie: Option<&str>,
    ) -> Result<KratosResponse, KratosError> {
        let mut url = self.public_url(&format!("self-service/{}/flows", kind.path()));
        url.query_pairs_mut().append_pair("id", id);
        let res = Self::with_cookie(self.http.get(url), cookie).send().await?;
        Self::into_response(res).await
    }

    /// Gửi biểu mẫu của một luồng, dạng `application/x-www-form-urlencoded`.
    pub async fn submit(
        &self,
        kind: FlowKind,
        id: &str,
        cookie: Option<&str>,
        form: &[(String, String)],
        client: &ClientInfo,
    ) -> Result<KratosResponse, KratosError> {
        let mut url = self.public_url(&format!("self-service/{}", kind.path()));
        url.query_pairs_mut().append_pair("flow", id);
        let body = form_urlencoded::Serializer::new(String::new())
            .extend_pairs(form.iter().map(|(k, v)| (k.as_str(), v.as_str())))
            .finish();
        let res = Self::with_cookie(self.http.post(url), cookie)
            .header(
                header::CONTENT_TYPE.as_str(),
                "application/x-www-form-urlencoded",
            )
            .body(body);
        let res = Self::with_client(res, client).send().await?;
        Self::into_response(res).await
    }

    fn with_client(req: reqwest::RequestBuilder, client: &ClientInfo) -> reqwest::RequestBuilder {
        let req = match &client.user_agent {
            Some(ua) => req.header(header::USER_AGENT.as_str(), ua),
            None => req,
        };
        match &client.ip {
            Some(ip) => req.header("x-forwarded-for", ip),
            None => req,
        }
    }

    /// Phiên hiện tại của trình duyệt.
    pub async fn whoami(&self, cookie: Option<&str>) -> Result<Whoami, KratosError> {
        let Some(cookie) = cookie else {
            return Ok(Whoami::Anonymous);
        };
        let res = Self::with_cookie(
            self.http.get(self.public_url("sessions/whoami")),
            Some(cookie),
        )
        .send()
        .await?;
        let r = Self::into_response(res).await?;
        Ok(match r.status {
            StatusCode::OK => Whoami::Session(r.body),
            StatusCode::FORBIDDEN if r.body["error"]["id"] == "session_aal2_required" => {
                Whoami::NeedsSecondFactor(
                    r.body["redirect_browser_to"]
                        .as_str()
                        .map(str::to_owned)
                        .unwrap_or_else(|| {
                            self.browser_init_url(
                                FlowKind::Login,
                                &InitOptions {
                                    aal: Some("aal2".into()),
                                    ..Default::default()
                                },
                            )
                            .to_string()
                        }),
                )
            }
            _ => Whoami::Anonymous,
        })
    }

    /// Các phiên đang hoạt động khác của người dùng hiện tại (không gồm phiên này).
    pub async fn other_sessions(&self, cookie: Option<&str>) -> Result<Vec<Value>, KratosError> {
        let mut url = self.public_url("sessions");
        url.query_pairs_mut()
            .append_pair("active", "true")
            .append_pair("page_size", "250");
        let res = Self::with_cookie(self.http.get(url), cookie).send().await?;
        let r = Self::into_response(res).await?;
        Ok(r.body.as_array().cloned().unwrap_or_default())
    }

    /// Thu hồi một phiên khác của chính người dùng hiện tại.
    pub async fn revoke_session(
        &self,
        cookie: Option<&str>,
        id: &str,
    ) -> Result<bool, KratosError> {
        let url = self.public_url(&format!(
            "sessions/{}",
            form_urlencoded::byte_serialize(id.as_bytes()).collect::<String>()
        ));
        let res = Self::with_cookie(self.http.delete(url), cookie)
            .send()
            .await?;
        Ok(res.status().is_success())
    }

    /// Thu hồi mọi phiên khác của người dùng hiện tại.
    pub async fn revoke_other_sessions(&self, cookie: Option<&str>) -> Result<bool, KratosError> {
        let res = Self::with_cookie(self.http.delete(self.public_url("sessions")), cookie)
            .send()
            .await?;
        Ok(res.status().is_success())
    }

    fn admin_url(&self, path: &str) -> Url {
        self.admin.join(path).expect("đường dẫn hợp lệ")
    }

    fn identity_url(&self, id: &str, suffix: &str) -> Url {
        let id: String = form_urlencoded::byte_serialize(id.as_bytes()).collect();
        self.admin_url(&format!("admin/identities/{id}{suffix}"))
    }

    /// Đọc danh tính (API quản trị), gồm cả `metadata_admin`.
    pub async fn get_identity(&self, id: &str) -> Result<Option<Value>, KratosError> {
        let res = self.http.get(self.identity_url(id, "")).send().await?;
        let r = Self::into_response(res).await?;
        Ok((r.status == StatusCode::OK).then_some(r.body))
    }

    /// Ghi đè `metadata_admin` của danh tính (JSON Patch).
    pub async fn set_metadata_admin(&self, id: &str, metadata: &Value) -> Result<(), KratosError> {
        let patch =
            serde_json::json!([{ "op": "replace", "path": "/metadata_admin", "value": metadata }]);
        let res = self
            .http
            .patch(self.identity_url(id, ""))
            .json(&patch)
            .send()
            .await?;
        if res.status().is_success() {
            Ok(())
        } else {
            Err(KratosError::Body(format!(
                "cập nhật metadata thất bại: {}",
                res.status()
            )))
        }
    }

    /// Thu hồi mọi phiên của một danh tính (API quản trị).
    pub async fn revoke_identity_sessions(&self, id: &str) -> Result<(), KratosError> {
        self.http
            .delete(self.identity_url(id, "/sessions"))
            .send()
            .await?;
        Ok(())
    }

    /// Xóa hẳn một danh tính (API quản trị).
    pub async fn delete_identity(&self, id: &str) -> Result<bool, KratosError> {
        let res = self.http.delete(self.identity_url(id, "")).send().await?;
        Ok(res.status().is_success() || res.status() == reqwest::StatusCode::NOT_FOUND)
    }

    /// Duyệt toàn bộ danh tính, từng trang (API quản trị).
    pub async fn list_identities(&self) -> Result<Vec<Value>, KratosError> {
        let mut all = Vec::new();
        let mut url = self.admin_url("admin/identities");
        url.query_pairs_mut().append_pair("page_size", "500");
        loop {
            let res = self.http.get(url.clone()).send().await?;
            let next = res
                .headers()
                .get_all(reqwest::header::LINK)
                .iter()
                .filter_map(|v| v.to_str().ok())
                .flat_map(|v| v.split(','))
                .find(|p| p.contains("rel=\"next\""))
                .and_then(|p| p.split(['<', '>']).nth(1).map(str::to_owned));
            let r = Self::into_response(res).await?;
            all.extend(r.body.as_array().cloned().unwrap_or_default());
            match next.and_then(|n| self.admin.join(&n).ok()) {
                Some(n) => url = n,
                None => break,
            }
        }
        Ok(all)
    }

    /// Địa chỉ đăng xuất (kèm token chống CSRF) cho phiên hiện tại.
    pub async fn logout_url(&self, cookie: Option<&str>) -> Result<Option<String>, KratosError> {
        let res = Self::with_cookie(
            self.http
                .get(self.public_url("self-service/logout/browser")),
            cookie,
        )
        .send()
        .await?;
        let r = Self::into_response(res).await?;
        Ok(r.body["logout_url"].as_str().map(str::to_owned))
    }

    /// Chi tiết lỗi tự phục vụ theo id.
    pub async fn error(&self, id: &str) -> Result<Value, KratosError> {
        let mut url = self.public_url("self-service/errors");
        url.query_pairs_mut().append_pair("id", id);
        let res = self.http.get(url).send().await?;
        Ok(Self::into_response(res).await?.body)
    }

    /// Có tài khoản nào dùng email này làm định danh đăng nhập không (API quản trị).
    pub async fn identity_exists(&self, email: &str) -> Result<bool, KratosError> {
        let mut url = self
            .admin
            .join("admin/identities")
            .expect("đường dẫn hợp lệ");
        url.query_pairs_mut()
            .append_pair("credentials_identifier", email);
        let res = self.http.get(url).send().await?;
        let r = Self::into_response(res).await?;
        Ok(r.body.as_array().is_some_and(|a| !a.is_empty()))
    }
}
