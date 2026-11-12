//! Gửi email của chính F-Eco (cảnh báo bảo mật). Email xác minh và khôi phục do
//! Kratos gửi. Nội dung email không bao giờ chứa mật khẩu hay mã bí mật
//! (spec notifications, "Không lộ thông tin nhạy cảm qua thông báo").

use std::sync::{Arc, Mutex};

use lettre::{
    message::{header::ContentType, Attachment as MailAttachment, Mailbox, MultiPart, SinglePart},
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
};
use url::Url;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SentMail {
    pub to: String,
    pub subject: String,
    pub body: String,
    /// Tên các tệp đính kèm.
    pub attachments: Vec<String>,
}

/// Tệp đính kèm email (ví dụ hóa đơn).
#[derive(Debug, Clone)]
pub struct Attachment {
    pub name: String,
    pub content_type: String,
    pub bytes: Vec<u8>,
}

#[derive(Clone)]
pub enum Mailer {
    Smtp {
        transport: Box<AsyncSmtpTransport<Tokio1Executor>>,
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
    /// Tạo mailer SMTP từ địa chỉ:
    /// - `smtps://user:pass@host:465`: TLS ngay từ đầu (khuyến nghị cho production);
    /// - `smtp://user:pass@host:587?tls=starttls`: STARTTLS bắt buộc;
    /// - `smtp://host:1025`: không mã hóa, chỉ cho dev (Mailpit).
    ///
    /// Máy chủ chuyển tiếp ký DKIM cho tên miền gửi (xem `maildns`).
    pub fn smtp(url: &Url, from: &str) -> Result<Self, MailError> {
        let host = url
            .host_str()
            .ok_or_else(|| MailError::Config("thiếu host".into()))?;
        let starttls = url
            .query_pairs()
            .any(|(k, v)| k == "tls" && v == "starttls");
        let builder = match url.scheme() {
            "smtps" => AsyncSmtpTransport::<Tokio1Executor>::relay(host)
                .map_err(|e| MailError::Config(e.to_string()))?
                .port(url.port().unwrap_or(465)),
            "smtp" if starttls => AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(host)
                .map_err(|e| MailError::Config(e.to_string()))?
                .port(url.port().unwrap_or(587)),
            "smtp" => AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(host)
                .port(url.port().unwrap_or(25)),
            other => {
                return Err(MailError::Config(format!(
                    "chỉ hỗ trợ smtp:// và smtps://, nhận {other}://"
                )))
            }
        };
        let builder = if url.username().is_empty() {
            builder
        } else {
            let user = percent_decode(url.username());
            let pass = percent_decode(url.password().unwrap_or_default());
            builder.credentials(lettre::transport::smtp::authentication::Credentials::new(
                user, pass,
            ))
        };
        let transport = Box::new(builder.build());
        let from = from
            .parse::<Mailbox>()
            .map_err(|e| MailError::Address(e.to_string()))?;
        Ok(Mailer::Smtp { transport, from })
    }

    /// SMTP có mã hóa không (production phải có).
    pub fn smtp_url_is_encrypted(url: &Url) -> bool {
        url.scheme() == "smtps"
            || url
                .query_pairs()
                .any(|(k, v)| k == "tls" && v == "starttls")
    }

    pub fn memory() -> (Self, Arc<Mutex<Vec<SentMail>>>) {
        let store = Arc::new(Mutex::new(Vec::new()));
        (Mailer::Memory(store.clone()), store)
    }

    pub async fn send(&self, to: &str, subject: &str, body: &str) -> Result<(), MailError> {
        self.send_with(to, subject, body, None).await
    }

    /// Gửi email kèm một tệp.
    pub async fn send_with_attachment(
        &self,
        to: &str,
        subject: &str,
        body: &str,
        attachment: &Attachment,
    ) -> Result<(), MailError> {
        self.send_with(to, subject, body, Some(attachment)).await
    }

    async fn send_with(
        &self,
        to: &str,
        subject: &str,
        body: &str,
        attachment: Option<&Attachment>,
    ) -> Result<(), MailError> {
        match self {
            Mailer::Memory(store) => {
                store.lock().expect("khóa hộp thư").push(SentMail {
                    to: to.to_owned(),
                    subject: subject.to_owned(),
                    body: body.to_owned(),
                    attachments: attachment.map(|a| a.name.clone()).into_iter().collect(),
                });
                Ok(())
            }
            Mailer::Smtp { transport, from } => {
                let to = to
                    .parse::<Mailbox>()
                    .map_err(|e| MailError::Address(e.to_string()))?;
                let builder = Message::builder()
                    .from(from.clone())
                    .to(to)
                    .subject(subject);
                let msg = match attachment {
                    None => builder
                        .header(ContentType::TEXT_PLAIN)
                        .body(body.to_owned()),
                    Some(a) => {
                        let kind = ContentType::parse(&a.content_type)
                            .map_err(|e| MailError::Send(e.to_string()))?;
                        builder.multipart(
                            MultiPart::mixed()
                                .singlepart(SinglePart::plain(body.to_owned()))
                                .singlepart(
                                    MailAttachment::new(a.name.clone()).body(a.bytes.clone(), kind),
                                ),
                        )
                    }
                }
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

fn percent_decode(s: &str) -> String {
    form_urlencoded::parse(format!("x={s}").as_bytes())
        .next()
        .map(|(_, v)| v.into_owned())
        .unwrap_or_default()
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

    pub fn new_device(device: &str, location: &str, when: &str, not_me: &str) -> (String, String) {
        let loc = if location.is_empty() { "?" } else { location };
        let subject = "Đăng nhập từ thiết bị mới / New sign-in to your F-Eco account".to_owned();
        let body = format!(
            "Xin chào,\n\n\
             Tài khoản F-Eco của bạn vừa được đăng nhập từ một thiết bị mới.\n\
             Thiết bị: {device}\nVị trí gần đúng: {loc}\nThời điểm: {when}\n\n\
             Nếu đó là bạn, bạn không cần làm gì. Nếu không phải bạn, hãy mở liên kết sau để \
             đăng xuất mọi thiết bị rồi đặt lại mật khẩu (liên kết dùng được một lần, trong 7 ngày):\n\
             {not_me}\n\n\
             ---\n\
             Hello,\n\n\
             Your F-Eco account was just signed in from a new device.\n\
             Device: {device}\nApproximate location: {loc}\nTime: {when}\n\n\
             If this was you, no action is needed. If not, open this link to sign out all \
             devices and then reset your password (single use, valid for 7 days):\n{not_me}\n"
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
    fn smtp_schemes_and_encryption() {
        let from = "F-Eco <no-reply@f-eco.example>";
        let url = Url::parse("http://mail.example.com").unwrap();
        assert!(matches!(
            Mailer::smtp(&url, from),
            Err(MailError::Config(_))
        ));
        for (raw, encrypted) in [
            ("smtps://user:p%40ss@mail.example.com", true),
            ("smtp://user:pass@mail.example.com:587?tls=starttls", true),
            ("smtp://127.0.0.1:1025", false),
        ] {
            let url = Url::parse(raw).unwrap();
            assert!(Mailer::smtp(&url, from).is_ok(), "{raw}");
            assert_eq!(Mailer::smtp_url_is_encrypted(&url), encrypted, "{raw}");
        }
        assert_eq!(percent_decode("p%40ss"), "p@ss");
    }

    #[test]
    fn alert_emails_contain_no_secrets_and_link_to_settings() {
        let (_, body) = templates::failed_logins("https://f-eco.example/");
        assert!(body.contains("https://f-eco.example/settings"));
        assert!(!body.to_lowercase().contains("mật khẩu của bạn là"));
    }
}
