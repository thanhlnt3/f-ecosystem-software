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
    pub fn browser_init_url(&self, kind: FlowKind, return_to: Option<&str>) -> Url {
        let mut url = self.public_url(&format!("self-service/{}/browser", kind.path()));
        if let Some(r) = return_to {
            url.query_pairs_mut().append_pair("return_to", r);
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
            .body(body)
            .send()
            .await?;
        Self::into_response(res).await
    }

    /// Phiên hiện tại của trình duyệt, nếu có.
    pub async fn whoami(&self, cookie: Option<&str>) -> Result<Option<Value>, KratosError> {
        let Some(cookie) = cookie else {
            return Ok(None);
        };
        let res = Self::with_cookie(
            self.http.get(self.public_url("sessions/whoami")),
            Some(cookie),
        )
        .send()
        .await?;
        let r = Self::into_response(res).await?;
        Ok((r.status == StatusCode::OK).then_some(r.body))
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
