//! Gửi email của chính F-Eco (cảnh báo bảo mật). Email xác minh và khôi phục do
//! Kratos gửi. Nội dung email không bao giờ chứa mật khẩu hay mã bí mật
//! (spec notifications, "Không lộ thông tin nhạy cảm qua thông báo").

use std::sync::{Arc, Mutex};

use lettre::{
    message::{header::ContentType, Mailbox},
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
};
use url::Url;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SentMail {
    pub to: String,
    pub subject: String,
    pub body: String,
}

#[derive(Clone)]
pub enum Mailer {
    Smtp {
        transport: AsyncSmtpTransport<Tokio1Executor>,
        from: Mailbox,
    },
    /// Lưu email trong bộ nhớ, dùng cho test.
    Memory(Arc<Mutex<Vec<SentMail>>>),
}

#[derive(Debug, thiserror::Error)]
pub enum MailError {
    #[error("địa chỉ email không hợp lệ: {0}")]
    Address(String),
    #[error("cấu hình SMTP không hợp lệ: {0}")]
    Config(String),
    #[error("gửi email thất bại: {0}")]
    Send(String),
}

impl Mailer {
    /// Tạo mailer SMTP từ địa chỉ dạng `smtp://host:port`. Hiện chỉ hỗ trợ SMTP
    /// không mã hóa cho dev; SMTP có TLS được thêm cùng nhóm task thông báo (14.3).
    pub fn smtp(url: &Url, from: &str) -> Result<Self, MailError> {
        if url.scheme() != "smtp" {
            return Err(MailError::Config(format!(
                "chỉ hỗ trợ smtp:// ở bản hiện tại, nhận {}://",
                url.scheme()
            )));
        }
        let host = url
            .host_str()
            .ok_or_else(|| MailError::Config("thiếu host".into()))?;
        let transport = AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(host)
            .port(url.port().unwrap_or(25))
            .build();
        let from = from
            .parse::<Mailbox>()
            .map_err(|e| MailError::Address(e.to_string()))?;
        Ok(Mailer::Smtp { transport, from })
    }

    pub fn memory() -> (Self, Arc<Mutex<Vec<SentMail>>>) {
        let store = Arc::new(Mutex::new(Vec::new()));
        (Mailer::Memory(store.clone()), store)
    }

    pub async fn send(&self, to: &str, subject: &str, body: &str) -> Result<(), MailError> {
        match self {
            Mailer::Memory(store) => {
                store.lock().expect("khóa hộp thư").push(SentMail {
                    to: to.to_owned(),
                    subject: subject.to_owned(),
                    body: body.to_owned(),
                });
                Ok(())
            }
            Mailer::Smtp { transport, from } => {
                let to = to
                    .parse::<Mailbox>()
                    .map_err(|e| MailError::Address(e.to_string()))?;
                let msg = Message::builder()
                    .from(from.clone())
                    .to(to)
                    .subject(subject)
                    .header(ContentType::TEXT_PLAIN)
                    .body(body.to_owned())
                    .map_err(|e| MailError::Send(e.to_string()))?;
                transport
                    .send(msg)
                    .await
                    .map(|_| ())
                    .map_err(|e| MailError::Send(e.to_string()))
            }
        }
    }
}

/// Nội dung các email cảnh báo, song ngữ Việt/Anh.
pub mod templates {
    pub fn failed_logins(public_url: &str) -> (String, String) {
        let subject = "Cảnh báo đăng nhập sai nhiều lần / Repeated failed sign-ins".to_owned();
        let body = format!(
            "Xin chào,\n\n\
             Có 5 lần đăng nhập sai vào tài khoản F-Eco của bạn trong 15 phút qua. \
             Các lần đăng nhập tiếp theo sẽ cần vượt qua thử thách chống bot.\n\n\
             Nếu đó là bạn, bạn không cần làm gì. Nếu không phải bạn, hãy đổi mật khẩu \
             và bật xác thực hai lớp tại {public_url}settings\n\n\
             ---\n\
             Hello,\n\n\
             There were 5 failed sign-in attempts to your F-Eco account in the last 15 minutes. \
             Further attempts will require an anti-bot challenge.\n\n\
             If this was you, no action is needed. If not, change your password and turn on \
             two-factor authentication at {public_url}settings\n"
        );
        (subject, body)
    }

    pub fn export_ready(link: &str) -> (String, String) {
        let subject =
            "Dữ liệu cá nhân của bạn đã sẵn sàng / Your personal data is ready".to_owned();
        let body = format!(
            "Xin chào,\n\n\
             File chứa dữ liệu cá nhân bạn yêu cầu đã sẵn sàng. Hãy đăng nhập rồi tải tại:\n{link}\n\n\
             Liên kết hết hạn sau 7 ngày.\n\n\
             ---\n\
             Hello,\n\n\
             The personal data file you requested is ready. Sign in and download it at:\n{link}\n\n\
             The link expires in 7 days.\n"
        );
        (subject, body)
    }

    pub fn registration_attempt(public_url: &str) -> (String, String) {
        let subject =
            "Có người thử đăng ký bằng email của bạn / Sign-up attempt with your email".to_owned();
        let body = format!(
            "Xin chào,\n\n\
             Có người vừa thử tạo tài khoản F-Eco mới bằng địa chỉ email này, nhưng email này \
             đã có tài khoản. Không có tài khoản mới nào được tạo.\n\n\
             Nếu đó là bạn và bạn quên mật khẩu, hãy khôi phục tài khoản tại {public_url}recovery\n\
             Nếu không phải bạn, bạn có thể bỏ qua email này.\n\n\
             ---\n\
             Hello,\n\n\
             Someone tried to create a new F-Eco account with this email address, but it already \
             has an account. No new account was created.\n\n\
             If this was you and you forgot your password, recover your account at {public_url}recovery\n\
             If not, you can ignore this email.\n"
        );
        (subject, body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn memory_mailer_records() {
        let (m, store) = Mailer::memory();
        m.send("lan@example.com", "Chủ đề", "Nội dung")
            .await
            .unwrap();
        assert_eq!(store.lock().unwrap()[0].to, "lan@example.com");
    }

    #[test]
    fn smtp_requires_smtp_scheme() {
        let url = Url::parse("smtps://mail.example.com").unwrap();
        assert!(matches!(
            Mailer::smtp(&url, "F-Eco <no-reply@f-eco.example>"),
            Err(MailError::Config(_))
        ));
    }

    #[test]
    fn alert_emails_contain_no_secrets_and_link_to_settings() {
        let (_, body) = templates::failed_logins("https://f-eco.example/");
        assert!(body.contains("https://f-eco.example/settings"));
        assert!(!body.to_lowercase().contains("mật khẩu của bạn là"));
    }
}
