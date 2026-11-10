//! Client cho API quản trị của Ory Hydra (design D2). feco-server là "login & consent
//! provider" của Hydra: Hydra lo OAuth 2.0/OIDC, feco-server quyết định ai đăng nhập
//! và người dùng đồng ý cho app những scope nào.

use reqwest::StatusCode;
use serde_json::{json, Value};
use url::Url;

#[derive(Debug, thiserror::Error)]
pub enum HydraError {
    #[error("không gọi được Hydra: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Hydra trả mã {status}: {body}")]
    Status { status: u16, body: String },
}

#[derive(Clone)]
pub struct HydraClient {
    http: reqwest::Client,
    admin: Url,
}

fn enc(s: &str) -> String {
    form_urlencoded::byte_serialize(s.as_bytes()).collect()
}

impl HydraClient {
    pub fn new(admin: Url) -> Self {
        HydraClient {
            http: reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .timeout(std::time::Duration::from_secs(15))
                .build()
                .expect("tạo HTTP client"),
            admin,
        }
    }

    fn url(&self, path: &str) -> Url {
        self.admin.join(path).expect("đường dẫn hợp lệ")
    }

    async fn json(res: reqwest::Response) -> Result<Value, HydraError> {
        let status = res.status();
        let text = res.text().await?;
        if !status.is_success() {
            return Err(HydraError::Status {
                status: status.as_u16(),
                body: text.chars().take(500).collect(),
            });
        }
        Ok(if text.is_empty() {
            Value::Null
        } else {
            serde_json::from_str(&text).unwrap_or(Value::Null)
        })
    }

    pub async fn login_request(&self, challenge: &str) -> Result<Value, HydraError> {
        let url = self.url(&format!(
            "admin/oauth2/auth/requests/login?login_challenge={}",
            enc(challenge)
        ));
        Self::json(self.http.get(url).send().await?).await
    }

    /// Chấp nhận đăng nhập; trả về địa chỉ chuyển hướng tiếp theo.
    pub async fn accept_login(
        &self,
        challenge: &str,
        subject: &str,
        pairwise_subject: &str,
    ) -> Result<String, HydraError> {
        let url = self.url(&format!(
            "admin/oauth2/auth/requests/login/accept?login_challenge={}",
            enc(challenge)
        ));
        // remember=false: Hydra luôn hỏi lại feco-server, nên phiên Kratos (đăng xuất,
        // thu hồi thiết bị) luôn là nguồn sự thật.
        let body = json!({
            "subject": subject,
            "remember": false,
            "force_subject_identifier": pairwise_subject,
        });
        let v = Self::json(self.http.put(url).json(&body).send().await?).await?;
        Ok(v["redirect_to"].as_str().unwrap_or_default().to_owned())
    }

    pub async fn reject_login(&self, challenge: &str, reason: &str) -> Result<String, HydraError> {
        let url = self.url(&format!(
            "admin/oauth2/auth/requests/login/reject?login_challenge={}",
            enc(challenge)
        ));
        let body = json!({ "error": "access_denied", "error_description": reason });
        let v = Self::json(self.http.put(url).json(&body).send().await?).await?;
        Ok(v["redirect_to"].as_str().unwrap_or_default().to_owned())
    }

    pub async fn consent_request(&self, challenge: &str) -> Result<Value, HydraError> {
        let url = self.url(&format!(
            "admin/oauth2/auth/requests/consent?consent_challenge={}",
            enc(challenge)
        ));
        Self::json(self.http.get(url).send().await?).await
    }

    /// Chấp nhận đồng ý với đúng các scope được cấp; nhớ lựa chọn cho lần sau.
    pub async fn accept_consent(
        &self,
        challenge: &str,
        grant_scope: &[String],
        audience: &Value,
    ) -> Result<String, HydraError> {
        let url = self.url(&format!(
            "admin/oauth2/auth/requests/consent/accept?consent_challenge={}",
            enc(challenge)
        ));
        let body = json!({
            "grant_scope": grant_scope,
            "grant_access_token_audience": audience,
            "remember": true,
            "remember_for": 0,
        });
        let v = Self::json(self.http.put(url).json(&body).send().await?).await?;
        Ok(v["redirect_to"].as_str().unwrap_or_default().to_owned())
    }

    pub async fn reject_consent(&self, challenge: &str) -> Result<String, HydraError> {
        let url = self.url(&format!(
            "admin/oauth2/auth/requests/consent/reject?consent_challenge={}",
            enc(challenge)
        ));
        let body = json!({
            "error": "access_denied",
            "error_description": "Người dùng từ chối chia sẻ dữ liệu / The user denied the request",
        });
        let v = Self::json(self.http.put(url).json(&body).send().await?).await?;
        Ok(v["redirect_to"].as_str().unwrap_or_default().to_owned())
    }

    pub async fn accept_logout(&self, challenge: &str) -> Result<String, HydraError> {
        let url = self.url(&format!(
            "admin/oauth2/auth/requests/logout/accept?logout_challenge={}",
            enc(challenge)
        ));
        let v = Self::json(self.http.put(url).send().await?).await?;
        Ok(v["redirect_to"].as_str().unwrap_or_default().to_owned())
    }

    /// Các lần đồng ý còn hiệu lực của một người dùng (mỗi app một mục).
    pub async fn consent_sessions(&self, subject: &str) -> Result<Vec<Value>, HydraError> {
        let url = self.url(&format!(
            "admin/oauth2/auth/sessions/consent?subject={}&page_size=500",
            enc(subject)
        ));
        Ok(Self::json(self.http.get(url).send().await?)
            .await?
            .as_array()
            .cloned()
            .unwrap_or_default())
    }

    /// Thu hồi đồng ý của một người dùng với một app (hoặc mọi app khi `client` là `None`).
    /// Hydra thu hồi luôn mọi token đã cấp cho app đó.
    pub async fn revoke_consent(
        &self,
        subject: &str,
        client: Option<&str>,
    ) -> Result<(), HydraError> {
        let mut path = format!(
            "admin/oauth2/auth/sessions/consent?subject={}",
            enc(subject)
        );
        match client {
            Some(c) => path.push_str(&format!("&client={}", enc(c))),
            None => path.push_str("&all=true"),
        }
        let res = self.http.delete(self.url(&path)).send().await?;
        if res.status().is_success() || res.status() == StatusCode::NOT_FOUND {
            Ok(())
        } else {
            Err(HydraError::Status {
                status: res.status().as_u16(),
                body: res.text().await.unwrap_or_default(),
            })
        }
    }

    /// Kiểm tra access token. Trả về `None` khi token không còn hiệu lực.
    pub async fn introspect(&self, token: &str) -> Result<Option<Value>, HydraError> {
        let body = form_urlencoded::Serializer::new(String::new())
            .append_pair("token", token)
            .finish();
        let res = self
            .http
            .post(self.url("admin/oauth2/introspect"))
            .header("content-type", "application/x-www-form-urlencoded")
            .body(body)
            .send()
            .await?;
        let v = Self::json(res).await?;
        Ok((v["active"] == true).then_some(v))
    }

    pub async fn get_client(&self, client_id: &str) -> Result<Option<Value>, HydraError> {
        let res = self
            .http
            .get(self.url(&format!("admin/clients/{}", enc(client_id))))
            .send()
            .await?;
        if res.status() == StatusCode::NOT_FOUND {
            return Ok(None);
        }
        Self::json(res).await.map(Some)
    }

    /// Ghi đè OAuth client (địa chỉ chuyển hướng, secret, metadata).
    pub async fn update_client(
        &self,
        client_id: &str,
        client: &Value,
    ) -> Result<Value, HydraError> {
        Self::json(
            self.http
                .put(self.url(&format!("admin/clients/{}", enc(client_id))))
                .json(client)
                .send()
                .await?,
        )
        .await
    }

    pub async fn delete_client(&self, client_id: &str) -> Result<(), HydraError> {
        let res = self
            .http
            .delete(self.url(&format!("admin/clients/{}", enc(client_id))))
            .send()
            .await?;
        if res.status() == StatusCode::NOT_FOUND {
            return Ok(());
        }
        Self::json(res).await.map(|_| ())
    }

    /// Tạo OAuth client cho một app (cổng nhà phát triển ở nhóm 8 sẽ gọi hàm này).
    pub async fn create_client(&self, client: &Value) -> Result<Value, HydraError> {
        Self::json(
            self.http
                .post(self.url("admin/clients"))
                .json(client)
                .send()
                .await?,
        )
        .await
    }
}
