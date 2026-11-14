//! Ngôn ngữ giao diện (spec platform-support, "Ngôn ngữ"): tiếng Việt, tiếng Anh và tiếng Nhật
//! (thay đổi web-ui-and-japanese), mặc định theo trình duyệt, đổi được bằng liên kết
//! `/lang/{vi|en|ja}` (lưu cookie). Bản tiếng Nhật nằm ở `i18n_ja.rs`; nội dung do hệ thống tạo
//! chưa có bản tiếng Nhật thì hiện tiếng Anh.

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
    Ja,
}

impl Lang {
    pub fn parse(s: &str) -> Option<Lang> {
        match s {
            "vi" => Some(Lang::Vi),
            "en" => Some(Lang::En),
            "ja" => Some(Lang::Ja),
            _ => None,
        }
    }

    pub const fn code(self) -> &'static str {
        match self {
            Lang::Vi => "vi",
            Lang::En => "en",
            Lang::Ja => "ja",
        }
    }

    pub const ALL: [Lang; 3] = [Lang::Vi, Lang::En, Lang::Ja];

    /// Tên ngôn ngữ viết bằng chính ngôn ngữ đó (nút chọn ngôn ngữ).
    pub const fn native_name(self) -> &'static str {
        match self {
            Lang::Vi => "Tiếng Việt",
            Lang::En => "English",
            Lang::Ja => "日本語",
        }
    }

    /// Nội dung chỉ có tiếng Việt và tiếng Anh: chọn tiếng Việt hay không (tiếng Nhật dùng tiếng Anh).
    pub const fn is_vi(self) -> bool {
        matches!(self, Lang::Vi)
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
            Some(l) if l.starts_with("ja") => Lang::Ja,
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
    if lang == Lang::Ja {
        if let Some(ja) = crate::i18n_ja::ja(key) {
            return ja;
        }
    }
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
        "store.title" => ("F-Store", "F-Store"),
        "store.search" => ("Tìm", "Search"),
        "store.search_placeholder" => ("Tìm theo tên, mô tả, nhà phát triển", "Search by name, description, developer"),
        "store.category" => ("Danh mục", "Category"),
        "store.category_all" => ("Mọi danh mục", "All categories"),
        "store.os_all" => ("Mọi hệ điều hành", "All platforms"),
        "store.price" => ("Giá", "Price"),
        "store.price_all" => ("Mọi mức giá", "Any price"),
        "store.price_free" => ("Miễn phí", "Free"),
        "store.price_paid" => ("Trả phí", "Paid"),
        "store.verified_only" => ("Chỉ nhà phát triển đã xác minh", "Verified developers only"),
        "store.empty" => ("Không có app nào khớp.", "No apps match."),
        "store.verified" => ("Đã xác minh", "Verified"),
        "store.first_party" => ("Của F-Eco", "By F-Eco"),
        "store.no_ratings" => ("Chưa có đánh giá", "No ratings yet"),
        "store.back" => ("Về F-Store", "Back to F-Store"),
        "store.screenshots" => ("Ảnh chụp màn hình", "Screenshots"),
        "store.description" => ("Giới thiệu", "About"),
        "store.version" => ("Phiên bản", "Version"),
        "store.updated" => ("Cập nhật", "Updated"),
        "store.platforms" => ("Hệ điều hành", "Platforms"),
        "store.size" => ("Dung lượng", "Size"),
        "store.license" => ("License", "License"),
        "store.privacy" => ("Chính sách quyền riêng tư", "Privacy policy"),
        "store.data_title" => ("Dữ liệu app yêu cầu", "Data this app asks for"),
        "store.required" => ("bắt buộc", "required"),
        "store.optional" => ("không bắt buộc", "optional"),
        "store.no_data" => ("App không yêu cầu dữ liệu cá nhân của bạn.", "This app does not ask for your personal data."),
        "store.isolation" => ("App này chạy với quyền như mọi ứng dụng khác trên máy bạn.", "This app runs with the same permissions as any other app on your computer."),
        "store.install" => ("Cài bằng F-Launcher", "Install with F-Launcher"),
        "store.install_hint" => ("mở F-Launcher, chọn tab Danh mục rồi bấm Cài.", "open F-Launcher, go to the Catalog tab and click Install."),
        "store.changelog" => ("Lịch sử thay đổi", "Release history"),
        "store.reviews" => ("Đánh giá và nhận xét", "Ratings and reviews"),
        "store.write_review" => ("Viết nhận xét", "Write a review"),
        "store.rating" => ("Số sao", "Rating"),
        "store.review_body" => ("Nhận xét", "Review"),
        "store.submit_review" => ("Gửi nhận xét", "Post review"),
        "store.need_install" => ("Bạn cần cài app trước khi đánh giá", "You need to install the app before reviewing it"),
        "store.login_to_review" => ("Đăng nhập để viết nhận xét hoặc báo cáo vi phạm.", "Sign in to write a review or report a problem."),
        "store.developer_label" => ("Nhà phát triển", "Developer"),
        "store.reply" => ("Trả lời", "Reply"),
        "store.submit_reply" => ("Gửi trả lời", "Post reply"),
        "store.report_app" => ("Báo cáo app này", "Report this app"),
        "store.report_review" => ("Báo cáo nhận xét", "Report review"),
        "store.reason" => ("Lý do", "Reason"),
        "store.details" => ("Mô tả", "Details"),
        "store.submit_report" => ("Gửi báo cáo", "Send report"),
        "store.ok_review" => ("Đã lưu nhận xét của bạn.", "Your review was saved."),
        "store.ok_reply" => ("Đã gửi trả lời.", "Your reply was posted."),
        "store.ok_report" => ("Đã gửi báo cáo. Bạn sẽ nhận email khi báo cáo được xử lý.", "Report sent. You will get an email when it has been handled."),
        "store.not_owner" => ("Chỉ nhà phát triển của app được trả lời nhận xét.", "Only the app's developer can reply to reviews."),
        "store.invalid" => ("Dữ liệu gửi lên không hợp lệ.", "The submitted data is not valid."),
        "store.not_found" => ("Không có app này trong danh mục.", "This app is not in the catalog."),
        "reason.malware" => ("Mã độc", "Malware"),
        "reason.fraud" => ("Lừa đảo", "Fraud"),
        "reason.copyright" => ("Vi phạm bản quyền", "Copyright infringement"),
        "reason.inappropriate" => ("Nội dung không phù hợp", "Inappropriate content"),
        "reason.other" => ("Khác", "Other"),
        "dev.title" => ("Cổng nhà phát triển", "Developer portal"),
        "dev.secret_once" => ("Hãy chép giá trị này ngay; nó chỉ hiện một lần:", "Copy this value now; it is shown only once:"),
        "dev.token_created" => ("Đã tạo token cho f-dev.", "Created a token for f-dev."),
        "dev.secret_created" => ("Đã tạo client secret mới.", "Created a new client secret."),
        "dev.invited_to" => ("Bạn được mời vào nhóm", "You are invited to join"),
        "dev.accept_invite" => ("Tham gia", "Join"),
        "dev.register" => ("Đăng ký nhà phát triển", "Register as a developer"),
        "dev.kind" => ("Loại tài khoản", "Account type"),
        "dev.kind_individual" => ("Cá nhân", "Individual"),
        "dev.kind_organization" => ("Tổ chức", "Organization"),
        "dev.display_name" => ("Tên hiển thị trên F-Store", "Display name on F-Store"),
        "dev.org_only" => ("Ba mục dưới đây chỉ dành cho tổ chức.", "The three fields below are for organizations only."),
        "dev.legal_name" => ("Tên pháp lý", "Legal name"),
        "dev.business_id" => ("Mã số doanh nghiệp", "Business registration number"),
        "dev.domain" => ("Tên miền", "Domain"),
        "dev.accept_agreement" => ("Tôi đã đọc và chấp nhận Thỏa thuận nhà phát triển", "I have read and accept the Developer Agreement"),
        "dev.agreement_note" => ("Bạn phải chấp nhận thỏa thuận trước khi tạo app. Thời điểm và phiên bản thỏa thuận được ghi lại.", "You must accept the agreement before creating apps. The time and agreement version are recorded."),
        "dev.account" => ("Tài khoản", "Account"),
        "dev.your_role" => ("Vai trò của bạn", "Your role"),
        "dev.agreement" => ("Thỏa thuận nhà phát triển", "Developer Agreement"),
        "dev.agreement_required" => ("Tài khoản chưa chấp nhận phiên bản thỏa thuận hiện hành. Cần chấp nhận trước khi tạo app hay nộp phiên bản.", "This account has not accepted the current agreement. Accept it before creating apps or submitting versions."),
        "dev.agreement_submit" => ("Chấp nhận", "Accept"),
        "dev.domain_howto" => ("Thêm bản ghi DNS TXT sau vào tên miền", "Add this DNS TXT record to the domain"),
        "dev.domain_check" => ("Kiểm tra bản ghi TXT", "Check the TXT record"),
        "dev.members" => ("Thành viên", "Members"),
        "dev.pending" => ("đang chờ chấp nhận", "invitation pending"),
        "dev.invite_email" => ("Email người được mời", "Invitee's email"),
        "dev.role" => ("Vai trò", "Role"),
        "dev.role_owner" => ("Chủ sở hữu", "Owner"),
        "dev.role_admin" => ("Quản trị", "Admin"),
        "dev.role_developer" => ("Phát triển", "Developer"),
        "dev.role_viewer" => ("Chỉ xem", "Viewer"),
        "dev.invite" => ("Mời", "Invite"),
        "dev.apps" => ("App", "Apps"),
        "dev.no_apps" => ("Chưa có app nào.", "No apps yet."),
        "dev.new_app" => ("Tạo app mới", "Create a new app"),
        "dev.app_id" => ("Mã app (duy nhất, không đổi được)", "App ID (unique, cannot be changed)"),
        "dev.app_name" => ("Tên app", "App name"),
        "dev.app_kind" => ("Loại app", "App type"),
        "dev.app_kind_desktop" => ("App desktop (client công khai, PKCE, không có secret)", "Desktop app (public client, PKCE, no secret)"),
        "dev.app_kind_server" => ("App có máy chủ riêng (client secret xoay vòng được)", "App with its own server (rotatable client secret)"),
        "dev.redirects" => ("Địa chỉ chuyển hướng (mỗi dòng một địa chỉ)", "Redirect URIs (one per line)"),
        "dev.create_app" => ("Tạo app", "Create app"),
        "dev.tokens" => ("Token cho f-dev", "Tokens for f-dev"),
        "dev.tokens_note" => ("Token cho phép công cụ dòng lệnh f-dev nộp phiên bản thay bạn, với đúng vai trò của bạn.", "A token lets the f-dev command-line tool submit versions on your behalf, with your role."),
        "dev.token_name" => ("Tên token", "Token name"),
        "dev.create_token" => ("Tạo token", "Create token"),
        "dev.docs" => ("Tài liệu cho nhà phát triển", "Developer documentation"),
        "dev.credentials" => ("Thông tin đăng nhập của app", "App credentials"),
        "dev.save" => ("Lưu", "Save"),
        "dev.rotate_secret" => ("Tạo client secret mới", "Rotate client secret"),
        "dev.versions" => ("Phiên bản", "Versions"),
        "dev.no_versions" => ("Chưa có phiên bản nào.", "No versions yet."),
        "dev.package_for" => ("Gói cho", "Package for"),
        "dev.upload" => ("Tải gói lên", "Upload package"),
        "dev.submit_review" => ("Gửi duyệt", "Submit for review"),
        "dev.channel" => ("Kênh", "Channel"),
        "dev.percent" => ("Phần trăm người dùng", "Percentage of users"),
        "dev.release" => ("Phát hành", "Release"),
        "dev.pause" => ("Dừng phát hành dần", "Stop the rollout"),
        "dev.new_version" => ("Nộp phiên bản mới", "Submit a new version"),
        "dev.save_manifest" => ("Kiểm tra và lưu manifest", "Validate and save manifest"),
        "dev.metrics" => ("Số liệu", "Metrics"),
        "dev.metrics_note" => ("Chỉ là số tổng hợp; không có danh tính người dùng.", "Aggregate numbers only; no user identities."),
        "dev.metric_installs" => ("Lượt cài", "Installs"),
        "dev.metric_active7" => ("Người dùng hoạt động 7 ngày", "Active users, 7 days"),
        "dev.metric_active30" => ("Người dùng hoạt động 30 ngày", "Active users, 30 days"),
        "dev.metric_crashes" => ("Lỗi khi chạy trong 30 ngày", "Crashes, 30 days"),
        "dev.metric_rating" => ("Điểm đánh giá", "Rating"),
        "dev.metric_versions" => ("Phân bố phiên bản", "Version distribution"),
        "dev.delete_app" => ("Xóa app", "Delete app"),
        "dev.delete_warning" => ("App sẽ biến khỏi F-Store và mọi người dùng không đăng nhập được vào app nữa. Không hoàn tác được.", "The app disappears from F-Store and users can no longer sign in to it. This cannot be undone."),
        "dev.docs_manifest" => ("Mỗi phiên bản kèm một file f-app.toml. Trường thiếu hoặc sai bị từ chối kèm thông báo chỉ rõ trường.", "Every version ships an f-app.toml file. Missing or invalid fields are rejected with a message naming the field."),
        "dev.docs_cli" => ("f-dev kiểm tra manifest và nộp phiên bản từ dòng lệnh hoặc CI. Tạo token ở trang Cổng nhà phát triển.", "f-dev validates the manifest and submits versions from the command line or CI. Create a token on the Developer portal page."),
        "dev.docs_api" => ("App đăng nhập bằng OAuth 2.0 Authorization Code + PKCE, rồi gọi các API dưới đây.", "Apps sign in with OAuth 2.0 Authorization Code + PKCE, then call the APIs below."),
        "dev.docs_sample_title" => ("App mẫu", "Sample app"),
        "dev.docs_sample" => ("App mẫu đăng nhập, đổi mã khởi chạy và gọi /v1/me:", "A sample app that signs in, exchanges a launch code and calls /v1/me:"),
        "dev.docs_sandbox" => ("Môi trường sandbox là một bản triển khai riêng, có tài khoản thử và dữ liệu tách hẳn khỏi dữ liệu thật; token của sandbox không dùng được với API thật. Địa chỉ sandbox sẽ được công bố ở đây khi có hạ tầng.", "The sandbox is a separate deployment with test accounts and data fully separated from real data; sandbox tokens do not work with the real API. Its address will be published here once the infrastructure exists."),
        "dev.status.draft" => ("Nháp", "Draft"),
        "dev.status.pending_review" => ("Chờ duyệt", "Pending review"),
        "dev.status.approved" => ("Đã duyệt", "Approved"),
        "dev.status.rejected" => ("Bị từ chối", "Rejected"),
        "dev.status.released" => ("Đang phát hành", "Released"),
        "dev.status.paused" => ("Đã dừng phát hành", "Rollout stopped"),
        "dev.ok.registered" => ("Đã đăng ký nhà phát triển.", "Developer account created."),
        "dev.ok.agreement" => ("Đã ghi nhận việc chấp nhận thỏa thuận.", "Agreement acceptance recorded."),
        "dev.ok.verified" => ("Tên miền đã được xác minh.", "The domain is verified."),
        "dev.ok.invited" => ("Đã gửi lời mời.", "Invitation sent."),
        "dev.ok.joined" => ("Bạn đã tham gia nhóm.", "You joined the team."),
        "dev.ok.redirects" => ("Đã lưu địa chỉ chuyển hướng.", "Redirect URIs saved."),
        "dev.ok.manifest" => ("Manifest hợp lệ. Hãy tải gói lên cho từng nền tảng.", "The manifest is valid. Upload a package for each platform."),
        "dev.ok.package" => ("Đã nhận gói.", "Package received."),
        "dev.ok.submitted" => ("Đã gửi duyệt.", "Submitted for review."),
        "dev.ok.released" => ("Đã cập nhật việc phát hành.", "Release updated."),
        "dev.status.revoked" => ("Đã thu hồi", "Revoked"),
        "dev.status.checks" => ("Chờ duyệt: đang kiểm tra tự động", "Pending review: automated checks"),
        "dev.status.manual_review" => ("Chờ duyệt thủ công", "Pending manual review"),
        "dev.appeal" => ("Kháng nghị", "Appeal"),
        "dev.appeal_reason" => ("Lý do kháng nghị (mỗi lần từ chối được kháng nghị một lần)", "Why you disagree (each rejection can be appealed once)"),
        "dev.appeal_submit" => ("Gửi kháng nghị", "Send appeal"),
        "dev.ok.appealed" => ("Đã gửi kháng nghị. Bạn sẽ nhận email khi có kết quả.", "Appeal sent. You will get an email with the result."),
        "dev.suspended" => ("Tài khoản nhà phát triển đang bị tạm khóa chờ điều tra. Bạn không tạo app hay nộp phiên bản được cho tới khi F-Eco mở khóa.", "This developer account is suspended pending investigation. You cannot create apps or submit versions until F-Eco lifts the suspension."),
        "policy.title" => ("Chính sách nội dung", "Content policy"),
        "policy.link" => ("Đọc chính sách nội dung của F-Eco", "Read the F-Eco content policy"),
        "policy.intro" => ("Mọi app con trên F-Eco phải tuân thủ các điều khoản dưới đây. Phiên bản vi phạm bị từ chối kèm điều khoản bị vi phạm và cách khắc phục; nhà phát triển kháng nghị được một lần cho mỗi lần từ chối.", "Every app on F-Eco must follow the clauses below. Versions that break them are rejected with the clause and how to fix it; developers can appeal each rejection once."),
        "policy.clause" => ("Điều", "Clause"),
        "policy.review" => ("Phiên bản đầu tiên của mỗi app, và phiên bản xin thêm scope dữ liệu hay quyền hệ thống, được người duyệt kiểm tra trong 3 ngày làm việc. Các phiên bản khác được duyệt tự động nếu qua kiểm tra tự động (quét mã độc bằng hai engine, chữ ký mã, kiến trúc, packer).", "The first version of each app, and versions that ask for more data scopes or system permissions, are checked by a reviewer within 3 working days. Other versions are approved automatically if they pass the automated checks (two malware engines, code signing, architecture, packers)."),
        "dev.webhook" => ("Địa chỉ nhận webhook", "Webhook URL"),
        "dev.webhook_note" => ("Lưu sẽ tạo khóa ký mới (hiện một lần); khóa cũ hết hiệu lực.", "Saving creates a new signing key (shown once); the old key stops working."),
        "dev.webhook_save" => ("Lưu và tạo khóa ký", "Save and create signing key"),
        "dev.docs_events" => ("Sự kiện (webhook)", "Events (webhooks)"),
        "dev.docs_events_note" => ("Nền tảng gửi POST JSON có chữ ký HMAC tới địa chỉ webhook của app. App không trả 2xx thì nền tảng gửi lại sau 1, 5, 30 phút rồi giãn dần, trong tối đa 72 giờ. F-SDK có hàm kiểm chữ ký.", "The platform POSTs signed JSON to the app's webhook URL. If the app does not answer 2xx, it retries after 1, 5 and 30 minutes, then less often, for up to 72 hours. F-SDK has a signature check helper."),
        "dev.docs_error_code" => ("Mã", "Code"),
        "dev.docs_error_howto" => ("Cách xử lý", "What to do"),
        "dev.docs_errors" => ("Mã lỗi", "Error codes"),
        "dev.docs_errors_note" => ("Mọi lỗi API trả JSON như dưới đây. Ghi lại request_id khi liên hệ hỗ trợ.", "Every API error returns JSON like this. Keep the request_id when contacting support."),
        "dev.docs_versioning" => ("Phiên bản API", "API versions"),
        "dev.docs_versioning_note" => ("Phiên bản nằm trong đường dẫn (/v1/...). Một phiên bản được giữ ít nhất 12 tháng sau khi công bố ngừng hỗ trợ; trong thời gian đó phản hồi có header Deprecation và Sunset.", "The version is part of the path (/v1/...). A version is kept for at least 12 months after its deprecation is announced; during that time responses carry Deprecation and Sunset headers."),
        "order.title" => ("Đơn xây app", "Custom builds"),
        "order.intro" => ("Đội F-Eco xây hộ app theo yêu cầu của bạn: gửi yêu cầu, nhận báo giá theo mốc, thanh toán từng mốc, chạy thử từng bản qua F-Launcher và nghiệm thu.", "The F-Eco team builds an app to your request: send a request, get a quote in milestones, pay milestone by milestone, try each build in F-Launcher and accept it."),
        "order.mine" => ("Đơn của bạn", "Your orders"),
        "order.none" => ("Bạn chưa có đơn nào.", "You have no orders yet."),
        "order.new" => ("Gửi yêu cầu mới", "New request"),
        "order.verify_first" => ("Hãy xác minh email trước khi gửi yêu cầu.", "Verify your email before sending a request."),
        "order.project" => ("Tên dự án", "Project name"),
        "order.description" => ("Mô tả app cần xây", "What the app should do"),
        "order.platforms" => ("Nền tảng", "Platforms"),
        "order.wanted_by" => ("Thời hạn mong muốn (năm-tháng-ngày)", "Wanted by (year-month-day)"),
        "order.budget" => ("Ngân sách dự kiến (VND, tùy chọn)", "Expected budget (VND, optional)"),
        "order.attachments" => ("Tệp đính kèm (spec, bản vẽ, tài liệu)", "Attachments (spec, drawings, documents)"),
        "order.attachments_note" => ("Tổng tối đa 200 MB.", "200 MB in total at most."),
        "order.accept_tos" => ("Tôi đã đọc và chấp nhận", "I have read and accept the"),
        "order.tos" => ("Điều khoản dịch vụ xây app", "Custom-build Terms of Service"),
        "order.tos_version" => ("Phiên bản", "Version"),
        "order.tos_draft" => ("Bản nháp: điều khoản này đang chờ luật sư rà soát và có thể thay đổi trước khi dịch vụ mở chính thức.", "Draft: these terms are awaiting legal review and may change before the service officially opens."),
        "order.submit" => ("Gửi yêu cầu", "Send request"),
        "order.status" => ("Trạng thái", "Status"),
        "order.quote_due" => ("Báo giá trước", "Quote due by"),
        "order.quote" => ("Báo giá số", "Quote no."),
        "order.total" => ("Tổng giá (đã gồm thuế)", "Total (tax included)"),
        "order.deposit" => ("Đặt cọc (mốc 1)", "Deposit (milestone 1)"),
        "order.valid_until" => ("Hiệu lực tới", "Valid until"),
        "order.scope" => ("Phạm vi công việc", "Scope of work"),
        "order.exclusions" => ("Không bao gồm", "Not included"),
        "order.ownership" => ("Quyền sở hữu mã nguồn", "Source code ownership"),
        "order.milestone" => ("Mốc", "Milestone"),
        "order.paid" => ("đã thanh toán", "paid"),
        "order.days" => ("ngày", "days"),
        "order.due" => ("hạn", "due"),
        "order.revisions" => ("lượt sửa miễn phí còn lại", "free revisions left"),
        "order.build" => ("Bản build", "Build"),
        "order.build_note" => ("Mở F-Launcher để chạy thử: app riêng tư này chỉ bạn thấy.", "Open F-Launcher to try it: this private app is visible only to you."),
        "order.checks" => ("Kết quả kiểm tra theo spec", "Checks against the spec"),
        "order.accept_milestone" => ("Nghiệm thu mốc này", "Accept this milestone"),
        "order.revise" => ("Yêu cầu sửa", "Ask for changes"),
        "order.revise_note" => ("Mô tả lỗi hoặc điều cần sửa", "Describe the problem or what to change"),
        "order.dispute" => ("Mở tranh chấp", "Open a dispute"),
        "order.dispute_note" => ("Vì sao mốc này chưa đúng như đã thỏa thuận", "Why this milestone isn't what was agreed"),
        "order.accept_quote" => ("Chấp nhận báo giá", "Accept the quote"),
        "order.quote_expired" => ("Báo giá đã hết hạn.", "The quote has expired."),
        "order.requote" => ("Yêu cầu báo giá lại", "Ask for a new quote"),
        "order.adjust" => ("Đề nghị điều chỉnh", "Ask for an adjustment"),
        "order.payments" => ("Thanh toán", "Payments"),
        "order.payments_note" => ("Yêu cầu thanh toán được xử lý qua cổng thanh toán của F-Eco.", "Payment requests are handled by the F-Eco payment gateway."),
        "order.handover" => ("Bàn giao cuối", "Final handover"),
        "order.download_handover" => ("Tải bàn giao (mã nguồn, spec, tài liệu, bản build)", "Download the handover (source, spec, documents, builds)"),
        "order.handover_locked" => ("Tải mã nguồn mở khi đơn Hoàn tất và đã thanh toán đủ. Trước đó bạn chạy thử các bản build qua F-Launcher.", "The source can be downloaded once the order is completed and fully paid. Until then, try the builds in F-Launcher."),
        "order.handover_missing" => ("Đội F-Eco chưa tải gói bàn giao lên. Hãy nhắn trong đơn.", "The F-Eco team hasn't uploaded the handover package yet. Send a message in the order."),
        "order.messages" => ("Trao đổi", "Messages"),
        "order.send" => ("Gửi tin nhắn", "Send message"),
        "order.history" => ("Lịch sử trạng thái", "Status history"),
        "order.cancel" => ("Hủy đơn", "Cancel the order"),
        "order.cancel_note" => ("Trước khi đặt cọc: không mất phí. Sau khi đặt cọc: được hoàn các khoản đã trả cho mốc chưa bàn giao; tiền đặt cọc không hoàn khi công việc đã bắt đầu.", "Before the deposit: free. After the deposit: you get back what you paid for milestones not yet delivered; the deposit isn't refunded once work has started."),
        "order.cancel_reason" => ("Lý do (tùy chọn)", "Reason (optional)"),
        "order.ok.submitted" => ("Đã gửi yêu cầu. Email xác nhận kèm mã đơn đã được gửi cho bạn.", "Request sent. A confirmation email with the order code is on its way."),
        "order.ok.accepted" => ("Đã chấp nhận báo giá. Yêu cầu thanh toán tiền đặt cọc đã được tạo.", "Quote accepted. A payment request for the deposit has been created."),
        "order.ok.adjust" => ("Đã gửi đề nghị cho đội F-Eco.", "Your request was sent to the F-Eco team."),
        "order.ok.milestone" => ("Đã nghiệm thu mốc.", "Milestone accepted."),
        "order.ok.revision" => ("Đã gửi yêu cầu sửa.", "Change request sent."),
        "order.ok.dispute" => ("Đã mở tranh chấp. Quản trị viên sẽ phản hồi trong 2 ngày làm việc.", "Dispute opened. An administrator will answer within 2 business days."),
        "order.ok.cancel" => ("Đã hủy đơn.", "Order cancelled."),
        "order.ok.message" => ("Đã gửi tin nhắn.", "Message sent."),
        "home.orders" => ("Đơn xây app", "Custom builds"),
        "pay.title" => ("Thanh toán", "Payment"),
        "pay.order" => ("Đơn", "Order"),
        "pay.for" => ("Nội dung", "For"),
        "pay.amount" => ("Số tiền", "Amount"),
        "pay.method" => ("Phương thức thanh toán", "Payment method"),
        "pay.gateway_note" => ("Bạn sẽ được chuyển sang trang của cổng thanh toán ZaloPay để trả tiền; F-Eco không nhận số thẻ của bạn. Liên kết thanh toán hết hạn sau", "You'll be taken to the ZaloPay payment page to pay; F-Eco never receives your card number. The payment link expires after"),
        "pay.minutes" => ("phút", "minutes"),
        "pay.continue" => ("Thanh toán", "Pay"),
        "pay.already_paid" => ("Khoản này đã được thanh toán.", "This has already been paid."),
        "pay.not_payable" => ("Khoản này không còn cần thanh toán.", "This no longer needs paying."),
        "pay.unavailable" => ("Chưa cấu hình cổng thanh toán. Hãy liên hệ đội F-Eco.", "No payment gateway is configured. Contact the F-Eco team."),
        "pay.history" => ("Lịch sử thanh toán", "Payment history"),
        "pay.unknown" => ("Không có giao dịch này.", "No such transaction."),
        "pay.status" => ("Trạng thái giao dịch", "Transaction status"),
        "pay.pending_note" => ("F-Eco đang chờ cổng thanh toán xác nhận. Trạng thái sẽ tự cập nhật khi cổng báo kết quả; bạn không cần trả lại.", "F-Eco is waiting for the gateway to confirm. The status updates by itself when the gateway reports back; you don't need to pay again."),
        "pay.back_to_order" => ("Về trang đơn", "Back to the order"),
        "pay.none" => ("Bạn chưa có giao dịch nào.", "You have no transactions yet."),
        "pay.reference" => ("mã giao dịch", "reference"),
        "pay.refund" => ("Hoàn tiền", "Refund"),
        "pay.invoice" => ("Hóa đơn", "Invoice"),
        "pay.invoice_adjustment" => ("Hóa đơn điều chỉnh", "Adjustment invoice"),
        "pay.billing" => ("Thông tin xuất hóa đơn", "Invoice details"),
        "pay.billing_note" => ("Hóa đơn của các lần thanh toán sau dùng thông tin này. Doanh nghiệp cần mã số thuế.", "Invoices for future payments use these details. A company needs a tax code."),
        "pay.individual" => ("Cá nhân", "Individual"),
        "pay.company" => ("Doanh nghiệp", "Company"),
        "pay.billing_name" => ("Tên trên hóa đơn (họ tên hoặc tên công ty)", "Name on the invoice (person or company)"),
        "pay.tax_code" => ("Mã số thuế", "Tax code"),
        "pay.address" => ("Địa chỉ", "Address"),
        "pay.invoice_email" => ("Email nhận hóa đơn", "Invoice email"),
        "pay.save" => ("Lưu thông tin xuất hóa đơn", "Save invoice details"),
        "pay.billing_saved" => ("Đã lưu thông tin xuất hóa đơn.", "Invoice details saved."),
        "order.pay" => ("Thanh toán", "Pay"),
        "home.payments" => ("Lịch sử thanh toán", "Payment history"),
        "home.notifications" => ("Thông báo", "Notifications"),
        "admin.title" => ("Bảng quản trị", "Admin console"),
        "admin.intro" => ("Chọn một mục theo vai trò của bạn. Mọi thao tác được ghi vào nhật ký kiểm toán.", "Pick a section for your role. Every action is written to the audit log."),
        "admin.forbidden" => ("Bạn không có quyền vào bảng quản trị.", "You don't have access to the admin console."),
        "admin.role_forbidden" => ("Vai trò của bạn không được dùng chức năng này.", "Your role can't use this function."),
        "admin.need_security_key" => ("Bảng quản trị cần đăng nhập bằng passkey hoặc khóa bảo mật phần cứng. Thêm passkey trong Cài đặt rồi đăng xuất và đăng nhập lại bằng passkey.", "The admin console needs a sign-in with a passkey or hardware security key. Add a passkey in Settings, then sign out and sign in again with it."),
        "admin.not_found" => ("Không có chức năng này.", "No such function."),
        "admin.role.reviewer" => ("Người duyệt app", "App reviewer"),
        "admin.role.support" => ("Hỗ trợ", "Support"),
        "admin.role.project_manager" => ("Quản lý dự án", "Project manager"),
        "admin.role.finance" => ("Tài chính", "Finance"),
        "admin.role.sysadmin" => ("Quản trị hệ thống", "System administrator"),
        "admin.nav.home" => ("Tổng quan", "Overview"),
        "admin.nav.reviews" => ("Hàng chờ duyệt app", "Review queue"),
        "admin.nav.users" => ("Hỗ trợ người dùng", "User support"),
        "admin.nav.orders" => ("Đơn xây app", "Custom-build orders"),
        "admin.nav.refunds" => ("Hoàn tiền", "Refunds"),
        "admin.nav.members" => ("Phân quyền người dùng", "User roles"),
        "role.user" => ("Người sử dụng ứng dụng", "App user"),
        "role.developer" => ("Lập trình viên", "Developer"),
        "role.admin" => ("Quản trị viên", "Administrator"),
        "role.platform" => ("Vai trò", "Role"),
        "role.admin_subroles" => ("Vai trò con (khi là Quản trị viên)", "Admin duties (for Administrators)"),
        "role.reason" => ("Lý do (bắt buộc, ghi vào nhật ký kiểm toán)", "Reason (required, written to the audit log)"),
        "role.saved" => ("Đã đổi vai trò.", "Role changed."),
        "role.holders" => ("Lập trình viên và Quản trị viên", "Developers and administrators"),
        "role.holders_note" => ("Mọi tài khoản khác là Người sử dụng ứng dụng.", "Every other account is an App user."),
        "role.change" => ("Đổi vai trò", "Change a role"),
        "role.requests" => ("Yêu cầu quyền Lập trình viên", "Developer access requests"),
        "role.requests_empty" => ("Không có yêu cầu nào đang chờ.", "No pending requests."),
        "role.decision_note" => ("Ghi chú (bắt buộc khi từ chối)", "Note (required to decline)"),
        "role.approve" => ("Duyệt", "Approve"),
        "role.reject" => ("Từ chối", "Decline"),
        "role.decided" => ("Đã xử lý yêu cầu.", "Request handled."),
        "role.err.reason" => ("Cần nhập lý do.", "A reason is required."),
        "role.err.self" => ("Không được tự đổi vai trò của chính mình.", "You can't change your own role."),
        "role.err.subrole" => ("Quản trị viên cần ít nhất một vai trò con.", "An administrator needs at least one duty."),
        "role.err.last_sysadmin" => ("Phải còn ít nhất một Quản trị hệ thống.", "At least one system administrator must remain."),
        "role.err.pending" => ("Bạn đã có một yêu cầu đang chờ duyệt.", "You already have a pending request."),
        "role.err.already" => ("Bạn đã có quyền Lập trình viên.", "You already have Developer access."),
        "role.err.request" => ("Không tìm thấy yêu cầu đang chờ.", "Pending request not found."),
        "role.err.no_user" => ("Không có người dùng với email này.", "No user has this email."),
        "dev.need_role" => ("Cần quyền Lập trình viên", "Developer access required"),
        "dev.need_role_text" => ("Cổng nhà phát triển dành cho tài khoản có vai trò Lập trình viên. Gửi yêu cầu để quản trị viên xem xét; bạn sẽ nhận thông báo khi có kết quả.", "The developer portal is for accounts with the Developer role. Send a request for an administrator to review; you'll be notified of the decision."),
        "dev.request_note" => ("Bạn định phát hành app gì? (không bắt buộc)", "What do you plan to publish? (optional)"),
        "dev.request_send" => ("Gửi yêu cầu", "Send request"),
        "dev.request_pending" => ("Yêu cầu của bạn đang chờ quản trị viên duyệt.", "Your request is waiting for an administrator."),
        "dev.request_rejected" => ("Yêu cầu trước chưa được duyệt. Lý do:", "Your previous request was declined. Reason:"),
        "dev.ok.role_requested" => ("Đã gửi yêu cầu quyền Lập trình viên.", "Developer access request sent."),
        "admin.queue_empty" => ("Không có phiên bản nào chờ duyệt thủ công.", "No version is waiting for manual review."),
        "admin.waited" => ("chờ", "waiting"),
        "admin.diff" => ("Khác biệt so với phiên bản trước", "Changes from the previous version"),
        "admin.checks" => ("Kết quả kiểm tra tự động", "Automated check results"),
        "admin.vm_note" => ("Chạy thử trong máy ảo cô lập: chưa có hạ tầng (task 9.1).", "Trying the app in an isolated VM: infrastructure not ready (task 9.1)."),
        "admin.note" => ("Lý do (bắt buộc)", "Reason (required)"),
        "admin.approve" => ("Duyệt", "Approve"),
        "admin.reject" => ("Từ chối", "Reject"),
        "admin.need_reason" => ("Hãy nhập lý do.", "Enter a reason."),
        "admin.saved" => ("Đã lưu.", "Saved."),
        "admin.diff.first" => ("Phiên bản đầu tiên.", "First version."),
        "admin.diff.scope" => ("Scope:", "Scope:"),
        "admin.diff.purpose" => ("mục đích:", "purpose:"),
        "admin.diff.add_scope" => ("Thêm scope:", "New scope:"),
        "admin.diff.remove_scope" => ("Bỏ scope:", "Removed scope:"),
        "admin.diff.purpose_changed" => ("Đổi mục đích scope", "Purpose changed for scope"),
        "admin.diff.add_permission" => ("Thêm quyền:", "New permission:"),
        "admin.diff.remove_permission" => ("Bỏ quyền:", "Removed permission:"),
        "admin.diff.add_folder" => ("Thêm thư mục:", "New folder:"),
        "admin.diff.none" => ("Không đổi scope hay quyền.", "No scope or permission changes."),
        "admin.due_soon" => ("Sắp trễ hạn", "Due soon"),
        "admin.quote_json" => ("Báo giá (JSON, xem docs/order-quote.example.json)", "Quote (JSON, see docs/order-quote.example.json)"),
        "admin.send_quote" => ("Gửi báo giá", "Send quote"),
        "admin.deliver" => ("Bàn giao mốc", "Deliver a milestone"),
        "admin.notes" => ("Ghi chú thay đổi", "Change notes"),
        "admin.resolve" => ("Giải quyết tranh chấp", "Resolve the dispute"),
        "admin.new_refund" => ("Tạo lệnh hoàn", "New refund"),
        "admin.refunded" => ("đã hoàn", "refunded"),
        "admin.reason" => ("Lý do", "Reason"),
        "admin.user_search" => ("Email hoặc mã đơn", "Email or order code"),
        "admin.search" => ("Tìm", "Search"),
        "admin.no_user" => ("Không tìm thấy người dùng.", "No user found."),
        "admin.reason_ticket" => ("Lý do xem (ví dụ mã phiếu hỗ trợ)", "Reason (e.g. support ticket number)"),
        "admin.view_profile" => ("Xem hồ sơ", "View profile"),
        "admin.email" => ("Email", "Email"),
        "admin.verified" => ("Đã xác minh email", "Email verified"),
        "admin.created" => ("Ngày tạo", "Created"),
        "admin.sessions" => ("Phiên đang mở", "Active sessions"),
        "admin.resend" => ("Gửi lại email xác minh", "Resend verification email"),
        "admin.logout_all" => ("Đăng xuất mọi phiên", "Sign out all sessions"),
        "admin.verification_sent" => ("Đã gửi lại email xác minh.", "Verification email sent."),
        "admin.sessions_revoked" => ("Đã đăng xuất mọi phiên của người dùng.", "All of the user's sessions were signed out."),
        "admin.member_note" => ("Bỏ chọn mọi vai trò để gỡ quyền quản trị.", "Untick every role to remove admin access."),
        "admin.save" => ("Lưu", "Save"),
        "admin.keep_own_sysadmin" => ("Không tự bỏ vai trò Quản trị hệ thống của chính mình.", "You can't remove your own System administrator role."),
        "notify.title" => ("Thông báo", "Notifications"),
        "notify.none" => ("Chưa có thông báo nào.", "No notifications yet."),
        "notify.settings" => ("Nhận thông báo", "Notification settings"),
        "notify.mandatory_note" => ("Thông báo bảo mật, thanh toán, hóa đơn, đơn hàng và thay đổi điều khoản luôn được gửi.", "Security, payment, invoice, order and terms notifications are always sent."),
        "notify.save" => ("Lưu", "Save"),
        "notify.saved" => ("Đã lưu tùy chọn thông báo.", "Notification settings saved."),
        "notify.cat.app_updates" => ("Thông báo khi app có bản cập nhật", "When an app has an update"),
        "notify.cat.recommendations" => ("Gợi ý app mới", "New app suggestions"),
        "notify.cat.newsletter" => ("Bản tin F-Eco", "F-Eco newsletter"),
        "notify.cat.mandatory" => ("Thông báo bắt buộc", "Required notifications"),
        "notify.open" => ("Mở:", "Open:"),
        "notify.unsubscribe" => ("Hủy đăng ký loại thông báo này:", "Unsubscribe from these emails:"),
        "notify.unsubscribe_title" => ("Hủy đăng ký", "Unsubscribe"),
        "notify.unsubscribe_confirm" => ("Ngừng nhận:", "Stop receiving:"),
        "notify.unsubscribe_button" => ("Hủy đăng ký", "Unsubscribe"),
        "notify.unsubscribed" => ("Đã hủy đăng ký:", "Unsubscribed from:"),
        "notify.bad_link" => ("Liên kết hủy đăng ký không hợp lệ.", "This unsubscribe link is not valid."),
        "nav.account" => ("Tài khoản", "Account"),
        "nav.store" => ("Cửa hàng", "Store"),
        "nav.orders" => ("Đơn hàng", "Orders"),
        "nav.notifications" => ("Thông báo", "Notifications"),
        "nav.developer" => ("Nhà phát triển", "Developers"),
        "nav.preferences" => ("Giao diện", "Appearance"),
        "prefs.title" => ("Giao diện", "Appearance"),
        "prefs.intro" => ("Tùy chỉnh lưu trên trình duyệt này (cookie) và áp dụng cho mọi trang F-Eco, giống phần Giao diện trong F-Launcher.", "These settings are saved in this browser (a cookie) and apply to every F-Eco page, like Appearance in F-Launcher."),
        "prefs.language" => ("Ngôn ngữ", "Language"),
        "prefs.style" => ("Phong cách", "Style"),
        "prefs.style_note" => ("Các phong cách có bảng màu riêng không dùng chủ đề và màu nhấn.", "Styles with their own palette ignore the theme and accent color."),
        "prefs.follows_theme" => ("theo chủ đề", "follows theme"),
        "prefs.theme" => ("Chủ đề", "Theme"),
        "prefs.theme.system" => ("Theo hệ thống", "System"),
        "prefs.theme.light" => ("Sáng", "Light"),
        "prefs.theme.dark" => ("Tối", "Dark"),
        "prefs.accent" => ("Màu nhấn", "Accent color"),
        "prefs.accent.default" => ("Mặc định F-Eco", "F-Eco default"),
        "prefs.accent.blue" => ("Xanh dương", "Blue"),
        "prefs.accent.teal" => ("Xanh ngọc", "Teal"),
        "prefs.accent.green" => ("Xanh lá", "Green"),
        "prefs.accent.purple" => ("Tím", "Purple"),
        "prefs.accent.pink" => ("Hồng", "Pink"),
        "prefs.accent.orange" => ("Cam", "Orange"),
        "prefs.density" => ("Mật độ", "Density"),
        "prefs.density.normal" => ("Thường", "Normal"),
        "prefs.density.compact" => ("Gọn", "Compact"),
        "prefs.size" => ("Cỡ chữ", "Text size"),
        "prefs.save" => ("Lưu", "Save"),
        "prefs.reset" => ("Khôi phục mặc định", "Restore defaults"),
        "prefs.saved" => ("Đã lưu giao diện.", "Appearance saved."),
        "prefs.back" => ("Quay lại", "Back"),
        "prefs.style.plain" => ("Chân phương", "Plain"),
        "prefs.style.elegant" => ("Trang nhã", "Elegant"),
        "prefs.style.parchment" => ("Giấy da", "Parchment"),
        "prefs.style.fresco" => ("Bích họa", "Fresco"),
        "prefs.style.starry_night" => ("Trời sao", "Starry night"),
        "prefs.style.lotus_pond" => ("Hồ sen", "Lotus pond"),
        "prefs.style.cubist" => ("Lập thể", "Cubist"),
        "prefs.style.lily" => ("Hoa huệ", "Lily"),
        "prefs.style.old_quarter" => ("Phố cổ", "Old quarter"),
        "prefs.style.lacquer" => ("Sơn mài", "Lacquer"),
        "admin.dash.title" => ("Tổng quan", "Overview"),
        "admin.dash.role_requests" => ("Yêu cầu quyền Lập trình viên", "Developer role requests"),
        "admin.dash.users" => ("Người dùng", "Users"),
        "admin.dash.users_note" => ("{developers} lập trình viên · {admins} quản trị viên", "{developers} developers · {admins} administrators"),
        "admin.dash.reviews" => ("Phiên bản chờ duyệt thủ công", "Versions awaiting manual review"),
        "admin.dash.orders_due" => ("Đơn sắp trễ hạn", "Orders due soon"),
        "admin.dash.orders_note" => ("{open} đơn đang thực hiện", "{open} open orders"),
        "admin.dash.refunds" => ("Lệnh hoàn chờ phê duyệt", "Refunds awaiting approval"),
        "admin.dash.revenue" => ("Doanh thu 30 ngày", "Revenue, last 30 days"),
        "admin.dash.revenue_note" => ("{count} giao dịch thành công", "{count} successful payments"),
        "admin.dash.chart" => ("Doanh thu 14 ngày gần nhất", "Revenue, last 14 days"),
        "admin.dash.no_cards" => ("Vai trò của bạn chưa có thẻ số liệu; hãy dùng các mục ở thanh bên.", "Your role has no summary cards; use the sections in the sidebar."),
        "admin.dev_no_passkey" => ("Chế độ dev: không bắt buộc passkey (FECO_ADMIN_PASSKEY_OPTIONAL). Không bao giờ bật ở staging hay production.", "Dev mode: passkey not required (FECO_ADMIN_PASSKEY_OPTIONAL). Never enable this in staging or production."),
        _ => {
            tracing::warn!(key, "thiếu bản dịch");
            ("", "")
        }
    };
    match lang {
        Lang::Vi => vi,
        Lang::En | Lang::Ja => en,
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
        // Scenario "Trình duyệt tiếng Nhật".
        assert_eq!(
            Lang::detect(None, Some("ja-JP,ja;q=0.9,en;q=0.8")),
            Lang::Ja
        );
        assert_eq!(Lang::detect(Some("feco_lang=ja"), Some("vi-VN")), Lang::Ja);
        for l in Lang::ALL {
            assert_eq!(Lang::parse(l.code()), Some(l));
            assert!(!l.native_name().is_empty());
        }
    }

    /// Scenario "Thiếu bản dịch tiếng Nhật": mọi khóa của bảng có bản tiếng Nhật, cùng các chỗ
    /// `{...}` như bản tiếng Anh; bảng tiếng Nhật không có khóa lạ.
    #[test]
    fn every_key_has_japanese() {
        let src = include_str!("i18n.rs");
        let body = &src
            [src.find("let (vi, en) = match key {").unwrap()..src.find("#[cfg(test)]").unwrap()];
        let arm = regex::Regex::new(r#"(?m)^\s*"([a-z_.0-9]+)"(?:\s*\|\s*"([a-z_.0-9]+)")*\s*=>"#)
            .unwrap();
        let key_re = regex::Regex::new(r#""([a-z_.0-9]+)""#).unwrap();
        let holes = regex::Regex::new(r"\{[a-z_0-9]*\}").unwrap();
        let sorted = |s: &str| {
            let mut v: Vec<String> = holes.find_iter(s).map(|m| m.as_str().to_owned()).collect();
            v.sort();
            v
        };
        let mut keys = Vec::new();
        for m in arm.find_iter(body) {
            let text = m.as_str();
            for k in key_re.captures_iter(text) {
                keys.push(k[1].to_owned());
            }
        }
        assert!(keys.len() > 400, "chỉ tìm thấy {} khóa", keys.len());
        let mut missing = Vec::new();
        for key in &keys {
            match crate::i18n_ja::ja(key) {
                None => missing.push(key.clone()),
                Some(ja) => {
                    assert!(!ja.is_empty(), "bản tiếng Nhật rỗng: {key}");
                    assert_eq!(sorted(ja), sorted(t(Lang::En, key)), "chỗ {{…}} của {key}");
                }
            }
        }
        assert!(
            missing.is_empty(),
            "thiếu bản tiếng Nhật:\n{}",
            missing.join("\n")
        );
        assert_eq!(
            crate::i18n_ja::JA.len(),
            keys.len(),
            "bảng tiếng Nhật có khóa thừa"
        );
        assert_eq!(t(Lang::Ja, "khong.co.khoa.nay"), "");
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
            include_str!("store_web.rs"),
            include_str!("../templates/store_list.html"),
            include_str!("../templates/store_app.html"),
            include_str!("dev_web.rs"),
            include_str!("../templates/developer.html"),
            include_str!("../templates/developer_app.html"),
            include_str!("../templates/developer_docs.html"),
            include_str!("../templates/policy.html"),
            include_str!("orders_web.rs"),
            include_str!("../templates/orders.html"),
            include_str!("../templates/order.html"),
            include_str!("../templates/custom_build_terms.html"),
            include_str!("payments_web.rs"),
            include_str!("admin.rs"),
            include_str!("../templates/admin.html"),
            include_str!("notifications.rs"),
            include_str!("../templates/notifications.html"),
            include_str!("../templates/unsubscribe.html"),
            include_str!("../templates/pay.html"),
            include_str!("../templates/payment_return.html"),
            include_str!("../templates/payments.html"),
            include_str!("ui_prefs.rs"),
            include_str!("../templates/preferences.html"),
            include_str!("../templates/home.html"),
        ];
        for key in [
            "dev.status.draft",
            "dev.status.pending_review",
            "dev.status.approved",
            "dev.status.rejected",
            "dev.status.released",
            "dev.status.paused",
            "dev.ok.registered",
            "dev.ok.agreement",
            "dev.ok.verified",
            "dev.ok.invited",
            "dev.ok.joined",
            "dev.ok.redirects",
            "dev.ok.manifest",
            "dev.ok.package",
            "dev.ok.submitted",
            "dev.ok.released",
            "dev.ok.appealed",
            "dev.status.revoked",
            "dev.status.checks",
            "dev.status.manual_review",
        ] {
            assert!(
                !t(Lang::Vi, key).is_empty() && !t(Lang::En, key).is_empty(),
                "{key}"
            );
        }
        for r in crate::catalog::REPORT_REASONS {
            let key = format!("reason.{r}");
            assert!(
                !t(Lang::Vi, &key).is_empty() && !t(Lang::En, &key).is_empty(),
                "{key}"
            );
        }
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
