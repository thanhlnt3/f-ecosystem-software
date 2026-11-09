//! Thử thách chống bot (spec account-identity, "Chống dò mật khẩu").
//!
//! Hiện chỉ có nhà cung cấp `Test` cho dev và CI: một ô đánh dấu. Production bị chặn
//! dùng `Test` ngay từ bước đọc cấu hình; nhà cung cấp thật (ví dụ Cloudflare
//! Turnstile, hCaptcha) là câu hỏi mở cần chủ dự án chọn.

use feco_config::ChallengeProvider;

/// Tên trường biểu mẫu chứa kết quả thử thách. Trường này không được chuyển tới Kratos.
pub const FIELD: &str = "feco_challenge";
const TEST_PASS: &str = "pass";

#[derive(Debug, Clone)]
pub enum Challenge {
    Test,
}

impl Challenge {
    pub fn from_config(p: &ChallengeProvider) -> Self {
        match p {
            ChallengeProvider::Test => Challenge::Test,
        }
    }

    /// Kiểm tra kết quả thử thách mà trình duyệt gửi lên.
    pub async fn verify(&self, response: Option<&str>) -> bool {
        match self {
            Challenge::Test => response == Some(TEST_PASS),
        }
    }

    /// Nhãn của ô thử thách trên biểu mẫu.
    pub fn label(&self) -> &'static str {
        match self {
            Challenge::Test => {
                "Tôi không phải robot (thử thách thử nghiệm, chỉ có ở môi trường dev)"
            }
        }
    }

    pub fn test_value(&self) -> &'static str {
        TEST_PASS
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_provider_accepts_only_its_value() {
        let c = Challenge::Test;
        assert!(c.verify(Some("pass")).await);
        assert!(!c.verify(Some("PASS")).await);
        assert!(!c.verify(None).await);
    }
}
