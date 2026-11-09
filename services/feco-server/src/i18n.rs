//! Ngôn ngữ giao diện (spec platform-support, "Ngôn ngữ"): tiếng Việt và tiếng Anh,
//! mặc định theo trình duyệt, đổi được bằng liên kết `/lang/{vi|en}` (lưu cookie).

use axum::{
    extract::FromRequestParts,
    http::{header, request::Parts},
};

pub const COOKIE: &str = "feco_lang";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Lang {
    #[default]
    Vi,
    En,
}

impl Lang {
    pub fn parse(s: &str) -> Option<Lang> {
        match s {
            "vi" => Some(Lang::Vi),
            "en" => Some(Lang::En),
            _ => None,
        }
    }

    pub const fn code(self) -> &'static str {
        match self {
            Lang::Vi => "vi",
            Lang::En => "en",
        }
    }

    /// Chọn ngôn ngữ: cookie `feco_lang` trước, rồi `Accept-Language`, mặc định tiếng Việt.
    pub fn detect(cookie: Option<&str>, accept_language: Option<&str>) -> Lang {
        if let Some(lang) = cookie.and_then(|c| {
            c.split(';')
                .filter_map(|p| p.trim().split_once('='))
                .find(|(k, _)| *k == COOKIE)
                .and_then(|(_, v)| Lang::parse(v))
        }) {
            return lang;
        }
        // Chỉ xét ngôn ngữ được ưu tiên đầu tiên.
        match accept_language
            .and_then(|a| a.split(',').next())
            .map(|first| first.trim().to_ascii_lowercase())
        {
            Some(l) if l.starts_with("en") => Lang::En,
            _ => Lang::Vi,
        }
    }

    pub fn t(self, key: &str) -> &'static str {
        t(self, key)
    }
}

impl<S: Send + Sync> FromRequestParts<S> for Lang {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        let get = |h| parts.headers.get(h).and_then(|v| v.to_str().ok());
        Ok(Lang::detect(
            get(header::COOKIE),
            get(header::ACCEPT_LANGUAGE),
        ))
    }
}

/// Bảng chuỗi giao diện. Khóa không có bản dịch trả về chuỗi rỗng và ghi cảnh báo;
/// test `every_key_has_both_languages` bắt lỗi này trước khi phát hành.
pub fn t(lang: Lang, key: &str) -> &'static str {
    let (vi, en) = match key {
        "lang.switch.label" => ("English", "Tiếng Việt"),
        "lang.switch.code" => ("en", "vi"),

        "home.title" => ("Trang chủ", "Home"),
        "home.hello" => ("Xin chào", "Hello"),
        "home.verified" => ("Email đã xác minh.", "Email verified."),
        "home.unverified" => (
            "Email chưa xác minh. Bạn cần xác minh email trước khi mua hàng, đặt đơn xây app hay đăng ký nhà phát triển.",
            "Your email is not verified yet. You need to verify it before buying, ordering a custom app or registering as a developer.",
        ),
        "home.resend" => ("Gửi lại email xác minh", "Resend verification email"),
        "home.settings" => ("Cài đặt tài khoản", "Account settings"),
        "home.sessions" => ("Thiết bị đang đăng nhập", "Signed-in devices"),
        "home.logout" => ("Đăng xuất", "Sign out"),
        "home.welcome" => ("Chào mừng tới F-Eco", "Welcome to F-Eco"),
        "home.tagline" => (
            "Một tài khoản cho mọi app con trong hệ sinh thái F-Eco.",
            "One account for every app in the F-Eco ecosystem.",
        ),
        "home.login" => ("Đăng nhập", "Sign in"),
        "home.register" => ("Đăng ký", "Sign up"),
        "home.deletion_cancelled" => (
            "Bạn đã đăng nhập lại nên yêu cầu xóa tài khoản đã được hủy.",
            "You signed in again, so your account deletion request was cancelled.",
        ),

        "flow.login" => ("Đăng nhập", "Sign in"),
        "flow.login_aal2" => ("Xác thực hai lớp", "Two-factor authentication"),
        "flow.registration" => ("Đăng ký tài khoản F-Eco", "Create your F-Eco account"),
        "flow.verification" => ("Xác minh email", "Verify your email"),
        "flow.recovery" => ("Khôi phục tài khoản", "Recover your account"),
        "flow.settings" => ("Cài đặt tài khoản", "Account settings"),

        "group.profile" => ("Hồ sơ", "Profile"),
        "group.password" => ("Mật khẩu", "Password"),
        "group.totp" => ("Ứng dụng xác thực (TOTP)", "Authenticator app (TOTP)"),
        "group.lookup_secret" => ("Mã khôi phục", "Recovery codes"),
        "group.webauthn" => ("Passkey", "Passkey"),
        "group.code" => ("Mã qua email", "Code by email"),
        "group.oidc" => ("Tài khoản bên ngoài", "External accounts"),

        "link.no_account" => ("Chưa có tài khoản? Đăng ký", "No account yet? Sign up"),
        "link.forgot" => ("Quên mật khẩu?", "Forgot your password?"),
        "link.have_account" => ("Đã có tài khoản? Đăng nhập", "Already have an account? Sign in"),
        "link.back_login" => ("Về trang đăng nhập", "Back to sign in"),
        "link.back_home" => ("Về trang chủ", "Back to home"),
        "link.sessions" => ("Thiết bị đang đăng nhập", "Signed-in devices"),
        "link.delete_account" => ("Xóa tài khoản", "Delete account"),

        "reg.sent.title" => ("Hãy kiểm tra email", "Check your email"),
        "reg.sent.body" => (
            "Chúng tôi đã gửi email tới {email}. Hãy mở liên kết trong email để xác minh tài khoản (liên kết có hạn 24 giờ), rồi đăng nhập.",
            "We sent an email to {email}. Open the link in it to verify your account (the link expires in 24 hours), then sign in.",
        ),

        "error.title" => ("Có lỗi xảy ra", "Something went wrong"),
        "error.generic" => ("Đã có lỗi xảy ra.", "Something went wrong."),
        "error.unavailable" => (
            "Dịch vụ tài khoản đang tạm gián đoạn. Vui lòng thử lại sau ít phút.",
            "The account service is temporarily unavailable. Please try again in a few minutes.",
        ),
        "error.form" => ("Không mở được biểu mẫu. Vui lòng thử lại.", "Could not open the form. Please try again."),
        "error.submit" => (
            "Không hoàn tất được yêu cầu. Vui lòng thử lại.",
            "Could not complete the request. Please try again.",
        ),
        "error.csrf" => (
            "Biểu mẫu đã hết hạn hoặc không hợp lệ. Hãy mở lại trang và thử lại.",
            "The form expired or is invalid. Reload the page and try again.",
        ),
        "error.render" => ("Lỗi dựng trang", "Page rendering error"),

        "challenge.test_label" => (
            "Tôi không phải robot (thử thách thử nghiệm, chỉ có ở môi trường dev)",
            "I am not a robot (test challenge, dev environment only)",
        ),
        "challenge.needed" => (
            "Có nhiều lần đăng nhập sai vào tài khoản này. Hãy xác nhận bạn không phải robot rồi thử lại.",
            "There were many failed sign-ins to this account. Confirm you are not a robot and try again.",
        ),

        "sessions.title" => ("Thiết bị đang đăng nhập", "Signed-in devices"),
        "sessions.current" => ("Thiết bị này", "This device"),
        "sessions.signed_in_at" => ("Đăng nhập lúc", "Signed in at"),
        "sessions.ip" => ("Địa chỉ IP", "IP address"),
        "sessions.location" => ("Vị trí gần đúng", "Approximate location"),
        "sessions.unknown" => ("Không rõ", "Unknown"),
        "sessions.revoke" => ("Đăng xuất thiết bị này", "Sign out this device"),
        "sessions.revoke_others" => ("Đăng xuất mọi thiết bị khác", "Sign out all other devices"),
        "sessions.revoked" => ("Đã đăng xuất thiết bị.", "The device was signed out."),
        "sessions.revoked_others" => ("Đã đăng xuất mọi thiết bị khác.", "All other devices were signed out."),
        "sessions.none" => ("Không có thiết bị nào khác đang đăng nhập.", "No other devices are signed in."),

        "home.apps" => ("Ứng dụng đã kết nối", "Connected apps"),
        "home.export" => ("Xuất dữ liệu của tôi", "Export my data"),
        "link.apps" => ("Ứng dụng đã kết nối", "Connected apps"),
        "link.export" => ("Xuất dữ liệu của tôi", "Export my data"),

        "oauth.bad_request" => ("Yêu cầu đăng nhập không hợp lệ.", "Invalid sign-in request."),
        "consent.title" => ("Cho phép ứng dụng truy cập", "Allow app access"),
        "consent.by" => ("của", "by"),
        "consent.verified" => ("Đã xác minh", "Verified"),
        "consent.intro" => (
            "Ứng dụng này muốn dùng các dữ liệu sau trong tài khoản F-Eco của bạn:",
            "This app wants to use the following data from your F-Eco account:",
        ),
        "consent.required" => ("bắt buộc", "required"),
        "consent.optional" => ("không bắt buộc", "optional"),
        "consent.purpose" => ("Mục đích", "Purpose"),
        "consent.policy" => ("Chính sách quyền riêng tư của ứng dụng", "App privacy policy"),
        "consent.revoke_hint" => (
            "Bạn có thể thu hồi quyền bất kỳ lúc nào ở trang Ứng dụng đã kết nối.",
            "You can revoke access at any time on the Connected apps page.",
        ),
        "consent.accept" => ("Cho phép", "Allow"),
        "consent.deny" => ("Từ chối", "Deny"),

        "apps.title" => ("Ứng dụng đã kết nối", "Connected apps"),
        "apps.none" => ("Chưa có ứng dụng nào được bạn cấp quyền.", "You have not given any app access."),
        "apps.scopes" => ("Dữ liệu được cấp", "Data shared"),
        "apps.granted_at" => ("Cấp lúc", "Granted at"),
        "apps.last_access" => ("Truy cập gần nhất", "Last access"),
        "apps.never" => ("Chưa truy cập", "Never"),
        "apps.revoke" => ("Thu hồi quyền", "Revoke access"),
        "apps.revoked" => (
            "Đã thu hồi quyền. Ứng dụng không còn đọc được dữ liệu của bạn.",
            "Access revoked. The app can no longer read your data.",
        ),
        "apps.log_title" => ("Nhật ký truy cập 90 ngày", "Access log (90 days)"),
        "apps.log_empty" => ("Không có lần truy cập nào trong 90 ngày qua.", "No access in the last 90 days."),

        "export.title" => ("Xuất dữ liệu của tôi", "Export my data"),
        "export.body" => (
            "Bạn sẽ nhận email kèm liên kết tải một file ZIP chứa toàn bộ dữ liệu cá nhân F-Eco đang lưu: hồ sơ, phiên đăng nhập, ứng dụng đã kết nối, lịch sử đồng ý, nhật ký truy cập, dự án F-Builder, đơn hàng và hóa đơn. Liên kết có hạn 7 ngày.",
            "You will receive an email with a link to a ZIP file containing all personal data F-Eco keeps: profile, sign-in sessions, connected apps, consent history, access log, F-Builder projects, orders and invoices. The link expires in 7 days.",
        ),
        "export.button" => ("Yêu cầu xuất dữ liệu", "Request export"),
        "export.requested" => (
            "Đã nhận yêu cầu. Chúng tôi sẽ gửi email khi file sẵn sàng (chậm nhất 72 giờ).",
            "Request received. We will email you when the file is ready (within 72 hours).",
        ),
        "export.last" => ("Lần xuất gần nhất", "Last export"),
        "export.status_ready" => ("đã sẵn sàng", "ready"),
        "export.status_pending" => ("đang chuẩn bị", "being prepared"),
        "export.status_failed" => ("thất bại, hãy thử lại", "failed, please try again"),
        "export.expired" => ("Liên kết không hợp lệ hoặc đã hết hạn.", "The link is invalid or has expired."),

        "notme.title" => ("Không phải tôi đăng nhập", "This wasn't me"),
        "notme.body" => (
            "Bấm nút dưới đây để đăng xuất tài khoản khỏi mọi thiết bị. Sau đó hãy đặt lại mật khẩu và bật xác thực hai lớp.",
            "Press the button below to sign your account out of every device. Then reset your password and turn on two-factor authentication.",
        ),
        "notme.button" => ("Đăng xuất mọi thiết bị", "Sign out all devices"),
        "notme.done" => (
            "Đã đăng xuất mọi thiết bị. Hãy đặt lại mật khẩu ngay.",
            "All devices were signed out. Reset your password now.",
        ),
        "notme.invalid" => (
            "Liên kết không hợp lệ, đã được dùng hoặc đã hết hạn.",
            "The link is invalid, already used or expired.",
        ),
        "notme.reset" => ("Đặt lại mật khẩu", "Reset password"),

        "delete.title" => ("Xóa tài khoản", "Delete account"),
        "delete.body" => (
            "Tài khoản sẽ chuyển sang trạng thái chờ xóa trong 30 ngày, và mọi thiết bị bị đăng xuất. Nếu bạn đăng nhập lại trong 30 ngày, yêu cầu xóa sẽ bị hủy. Sau 30 ngày, hồ sơ, dữ liệu đã cấp cho app con và lịch sử F-Builder bị xóa; hóa đơn được giữ theo quy định pháp luật.",
            "Your account will be pending deletion for 30 days and all devices will be signed out. If you sign in again within 30 days, the deletion is cancelled. After 30 days your profile, data shared with apps and F-Builder history are deleted; invoices are kept as required by law.",
        ),
        "delete.confirm" => (
            "Tôi hiểu tài khoản sẽ bị xóa sau 30 ngày",
            "I understand my account will be deleted after 30 days",
        ),
        "delete.button" => ("Xóa tài khoản", "Delete account"),
        "delete.need_confirm" => ("Hãy đánh dấu ô xác nhận.", "Please tick the confirmation box."),
        "delete.requested" => (
            "Tài khoản đang chờ xóa và sẽ bị xóa sau 30 ngày. Đăng nhập lại trong thời gian này để hủy.",
            "Your account is pending deletion and will be deleted in 30 days. Sign in again during this time to cancel.",
        ),
        "delete.blocked_orders" => (
            "Bạn còn đơn xây app chưa hoàn tất. Hãy hủy hoặc hoàn tất đơn hàng trước khi xóa tài khoản.",
            "You still have custom app orders in progress. Cancel or complete them before deleting your account.",
        ),
        _ => {
            tracing::warn!(key, "thiếu bản dịch");
            ("", "")
        }
    };
    match lang {
        Lang::Vi => vi,
        Lang::En => en,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_language() {
        assert_eq!(Lang::detect(None, None), Lang::Vi);
        assert_eq!(Lang::detect(None, Some("en-US,en;q=0.9")), Lang::En);
        assert_eq!(Lang::detect(None, Some("vi-VN,en;q=0.5")), Lang::Vi);
        assert_eq!(
            Lang::detect(Some("a=1; feco_lang=en"), Some("vi-VN")),
            Lang::En,
            "cookie thắng Accept-Language"
        );
        assert_eq!(Lang::detect(Some("feco_lang=fr"), None), Lang::Vi);
    }

    /// Mọi khóa dùng trong mã và mẫu trang đều phải có bản dịch.
    #[test]
    fn every_key_has_both_languages() {
        let sources = [
            include_str!("auth.rs"),
            include_str!("account.rs"),
            include_str!("views.rs"),
            include_str!("../templates/base.html"),
            include_str!("../templates/home.html"),
            include_str!("../templates/flow.html"),
            include_str!("../templates/message.html"),
            include_str!("../templates/sessions.html"),
            include_str!("../templates/delete.html"),
            include_str!("oauth.rs"),
            include_str!("connected.rs"),
            include_str!("export.rs"),
            include_str!("../templates/consent.html"),
            include_str!("../templates/apps.html"),
            include_str!("../templates/app_detail.html"),
            include_str!("../templates/export.html"),
            include_str!("alerts.rs"),
            include_str!("../templates/not_me.html"),
        ];
        let re = regex::Regex::new(r#"\.t\("([a-z_.0-9]+)"\)"#).unwrap();
        let mut checked = 0;
        for src in sources {
            for cap in re.captures_iter(src) {
                let key = &cap[1];
                assert!(
                    !t(Lang::Vi, key).is_empty(),
                    "thiếu bản dịch tiếng Việt: {key}"
                );
                assert!(
                    !t(Lang::En, key).is_empty(),
                    "thiếu bản dịch tiếng Anh: {key}"
                );
                checked += 1;
            }
        }
        assert!(checked > 20, "chỉ tìm thấy {checked} khóa");
    }
}
