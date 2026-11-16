//! Thử thách chống bot (spec account-identity, "Chống dò mật khẩu").
//!
//! Nhà cung cấp `Test` cho dev và CI: một ô đánh dấu, bị chặn ở production ngay từ bước đọc
//! cấu hình. Production dùng Cloudflare Turnstile: trình duyệt nhận token từ widget, máy chủ
//! kiểm token qua API `siteverify`.

use feco_config::{ChallengeProvider, Secret};
use serde::Deserialize;

use crate::i18n::Lang;

/// Tên trường biểu mẫu chứa kết quả thử thách. Trường này không được chuyển tới Kratos
/// (widget Turnstile được đặt `data-response-field-name` bằng tên này).
pub const FIELD: &str = "feco_challenge";
const TEST_PASS: &str = "pass";

/// API kiểm token của Turnstile.
pub const TURNSTILE_VERIFY_URL: &str = "https://challenges.cloudflare.com/turnstile/v0/siteverify";
/// Script widget của Turnstile.
pub const TURNSTILE_SCRIPT: &str = "https://challenges.cloudflare.com/turnstile/v0/api.js";

#[derive(Debug, Clone)]
pub enum Challenge {
    Test,
    Turnstile {
        site_key: String,
        secret_key: Secret,
        http: reqwest::Client,
    },
}

/// Phản hồi của `siteverify` (chỉ phần cần dùng).
#[derive(Debug, Deserialize)]
struct SiteVerify {
    #[serde(default)]
    success: bool,
}

/// `true` khi phản hồi `siteverify` báo token hợp lệ.
fn siteverify_ok(body: &str) -> bool {
    serde_json::from_str::<SiteVerify>(body).is_ok_and(|r| r.success)
}

impl Challenge {
    pub fn from_config(p: &ChallengeProvider) -> Self {
        match p {
            ChallengeProvider::Test => Challenge::Test,
            ChallengeProvider::Turnstile {
                site_key,
                secret_key,
            } => Challenge::Turnstile {
                site_key: site_key.clone(),
                secret_key: secret_key.clone(),
                http: reqwest::Client::builder()
                    .redirect(reqwest::redirect::Policy::none())
                    .timeout(std::time::Duration::from_secs(10))
                    .build()
                    .expect("tạo HTTP client"),
            },
        }
    }

    /// Kiểm tra kết quả thử thách mà trình duyệt gửi lên. Lỗi mạng coi như không qua.
    pub async fn verify(&self, response: Option<&str>, remote_ip: Option<&str>) -> bool {
        match self {
            Challenge::Test => response == Some(TEST_PASS),
            Challenge::Turnstile {
                secret_key, http, ..
            } => {
                // Token Turnstile dài tối đa 2048 ký tự.
                let Some(token) = response.filter(|t| !t.is_empty() && t.len() <= 2048) else {
                    return false;
                };
                // Serializer không Send: dựng xong chuỗi trước khi await.
                let body = {
                    let mut f = form_urlencoded::Serializer::new(String::new());
                    f.append_pair("secret", secret_key.expose())
                        .append_pair("response", token);
                    if let Some(ip) = remote_ip {
                        f.append_pair("remoteip", ip);
                    }
                    f.finish()
                };
                let res = http
                    .post(TURNSTILE_VERIFY_URL)
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(body)
                    .send()
                    .await;
                match res {
                    Ok(r) if r.status().is_success() => {
                        r.text().await.is_ok_and(|b| siteverify_ok(&b))
                    }
                    Ok(r) => {
                        tracing::warn!(status = %r.status(), "Turnstile siteverify trả lỗi");
                        false
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "không gọi được Turnstile siteverify");
                        false
                    }
                }
            }
        }
    }

    /// Nhãn của ô thử thách (Turnstile tự hiện widget, không cần nhãn).
    pub fn label(&self, lang: Lang) -> &'static str {
        match self {
            Challenge::Test => lang.t("challenge.test_label"),
            Challenge::Turnstile { .. } => "",
        }
    }

    pub fn test_value(&self) -> &'static str {
        TEST_PASS
    }

    /// Site key của Turnstile để hiện widget; `None` với thử thách giả.
    pub fn site_key(&self) -> Option<&str> {
        match self {
            Challenge::Test => None,
            Challenge::Turnstile { site_key, .. } => Some(site_key),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_provider_accepts_only_its_value() {
        let c = Challenge::Test;
        assert!(c.verify(Some("pass"), None).await);
        assert!(!c.verify(Some("PASS"), None).await);
        assert!(!c.verify(None, None).await);
    }

    #[test]
    fn turnstile_siteverify_response_parsing() {
        assert!(siteverify_ok(
            r#"{"success":true,"challenge_ts":"2026-11-16T00:00:00Z","hostname":"f-eco.example","error-codes":[]}"#
        ));
        assert!(!siteverify_ok(
            r#"{"success":false,"error-codes":["invalid-input-response"]}"#
        ));
        assert!(!siteverify_ok(r#"{"success":"true"}"#));
        assert!(!siteverify_ok(r#"{"hostname":"f-eco.example"}"#));
        assert!(!siteverify_ok("not json"));
    }

    #[tokio::test]
    async fn turnstile_rejects_missing_token_without_network() {
        let c = Challenge::from_config(&ChallengeProvider::Turnstile {
            site_key: "site".into(),
            secret_key: Secret::new("secret"),
        });
        assert_eq!(c.site_key(), Some("site"));
        assert!(!c.verify(None, None).await);
        assert!(!c.verify(Some(""), Some("203.0.113.7")).await);
    }
}
