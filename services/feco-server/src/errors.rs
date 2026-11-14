//! Danh mục mã lỗi của API (spec app-sdk, "Mã lỗi thống nhất"): mọi lỗi API trả JSON gồm `error`,
//! `message`, `message_vi`, `request_id`. Trang `/developer/docs` hiện bảng này; test
//! `every_error_code_is_documented` bảo đảm mọi mã dùng trong mã nguồn đều có ở đây.

/// (mã, HTTP, cách xử lý tiếng Việt, cách xử lý tiếng Anh).
pub const CODES: &[(&str, u16, &str, &str)] = &[
    ("invalid_token", 401, "Access token thiếu, hết hạn hoặc bị thu hồi. Làm mới token (F-SDK tự làm); nếu vẫn lỗi, cho người dùng đăng nhập lại.", "The access token is missing, expired or revoked. Refresh it (F-SDK does this); if it still fails, sign the user in again."),
    ("insufficient_scope", 403, "Token thiếu scope cần cho trường hoặc endpoint này. Khai báo scope trong f-app.toml và xin người dùng đồng ý.", "The token lacks the scope this field or endpoint needs. Declare the scope in f-app.toml and ask the user to consent."),
    ("invalid_request", 400, "Tham số sai hoặc thiếu; đọc message để biết trường nào.", "A parameter is missing or invalid; message names it."),
    ("not_found", 404, "Không có tài nguyên này (hoặc bạn không có quyền xem).", "The resource does not exist (or you cannot see it)."),
    ("rate_limited", 429, "Vượt giới hạn tần suất. Chờ theo header Retry-After rồi thử lại.", "Rate limit exceeded. Wait for Retry-After, then retry."),
    ("temporarily_unavailable", 503, "Dịch vụ tạm gián đoạn. Thử lại sau, giãn dần thời gian chờ.", "The service is temporarily unavailable. Retry later with backoff."),
    ("invalid_grant", 400, "Mã khởi chạy sai, đã dùng hoặc quá 60 giây. Xin mã mới từ launcher (API cục bộ) hoặc đăng nhập qua trình duyệt.", "The launch code is wrong, used or older than 60 seconds. Ask the launcher for a new one or sign in through the browser."),
    ("consent_required", 403, "Người dùng chưa đồng ý các scope này cho app. Mở luồng đăng nhập qua trình duyệt (F-SDK tự làm).", "The user has not granted these scopes. Start the browser sign-in (F-SDK does this)."),
    ("launcher_only", 403, "Chỉ F-Launcher được gọi endpoint này.", "Only F-Launcher may call this endpoint."),
    ("not_enabled", 404, "Tính năng chưa bật trên máy chủ này.", "The feature is not enabled on this server."),
    ("unknown_client", 404, "Không có app nào có client ID này.", "No app has this client ID."),
    ("invalid_manifest", 422, "f-app.toml sai; message liệt kê từng trường cần sửa.", "f-app.toml is invalid; message lists each field to fix."),
    ("version_not_increasing", 422, "Phiên bản phải lớn hơn bản đã phát hành.", "The version must be greater than the released one."),
    ("version_exists", 409, "Phiên bản này đã được nộp; tăng số phiên bản.", "This version was already submitted; bump the version."),
    ("invalid_package", 422, "Gói sai: thiếu tệp chạy, chưa ký mã, sai SHA-256... message nói rõ.", "The package is invalid (missing entry file, unsigned, wrong SHA-256...); message explains."),
    ("package_too_large", 413, "Gói lớn hơn 2 GB.", "The package is larger than 2 GB."),
    ("not_draft", 409, "Chỉ sửa được phiên bản đang ở trạng thái nháp.", "Only draft versions can be changed."),
    ("not_ready", 409, "Chưa gửi duyệt được: thiếu gói cho một số nền tảng.", "Cannot submit: some platforms have no package."),
    ("not_approved", 409, "Chỉ phát hành được phiên bản đã qua kiểm duyệt.", "Only reviewed versions can be released."),
    ("not_released", 409, "Phiên bản chưa được phát hành.", "The version has not been released."),
    ("developer_role_required", 403, "Tài khoản chủ token không còn vai trò Lập trình viên. Gửi yêu cầu ở Cổng nhà phát triển hoặc liên hệ quản trị viên.", "The token owner no longer has the Developer role. Request it in the developer portal or contact an administrator."),
    ("insufficient_role", 403, "Vai trò của bạn trong nhóm không đủ cho thao tác này.", "Your team role does not allow this."),
    ("agreement_required", 403, "Tài khoản chưa chấp nhận Thỏa thuận nhà phát triển hiện hành.", "The account has not accepted the current Developer Agreement."),
    ("account_suspended", 403, "Tài khoản nhà phát triển đang bị tạm khóa chờ điều tra.", "The developer account is suspended pending investigation."),
    ("email_unverified", 403, "Cần xác minh email trước.", "Verify your email first."),
    ("second_factor_required", 403, "Cần bật xác thực hai lớp trước.", "Turn on two-factor authentication first."),
    ("txt_not_found", 409, "Chưa thấy bản ghi DNS TXT; DNS có thể cần vài phút.", "The DNS TXT record was not found yet; DNS can take a few minutes."),
    ("not_rejected", 409, "Chỉ kháng nghị được phiên bản đang bị từ chối.", "Only rejected versions can be appealed."),
    ("already_appealed", 409, "Mỗi lần từ chối chỉ được kháng nghị một lần.", "Each rejection can be appealed once."),
    ("no_open_appeal", 409, "Không có kháng nghị đang mở.", "There is no open appeal."),
    ("not_in_manual_review", 409, "Phiên bản không ở hàng chờ duyệt thủ công.", "The version is not in the manual review queue."),
    ("csrf", 403, "Biểu mẫu hết hạn; tải lại trang.", "The form expired; reload the page."),
];

/// Mã lỗi có trong danh mục không.
pub fn documented(code: &str) -> bool {
    CODES.iter().any(|(c, ..)| *c == code)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mọi mã lỗi API dùng trong mã nguồn đều được ghi trong tài liệu.
    #[test]
    fn every_error_code_is_documented() {
        let sources = [
            include_str!("api.rs"),
            include_str!("launch.rs"),
            include_str!("store_web.rs"),
            include_str!("developer.rs"),
            include_str!("dev_web.rs"),
            include_str!("review.rs"),
            include_str!("entitlements.rs"),
            include_str!("security.rs"),
        ];
        let patterns = [
            regex::Regex::new(r#"api_error\(\s*StatusCode::\w+,\s*"([a-z_]+)""#).unwrap(),
            regex::Regex::new(r#"problem\(\s*StatusCode::\w+,\s*"([a-z_]+)""#).unwrap(),
            regex::Regex::new(r#"code: "([a-z_]+)""#).unwrap(),
        ];
        let mut found = 0;
        for src in sources {
            // review.rs dùng `code: "..."` cho điều khoản chính sách, không phải mã lỗi API.
            let skip_clauses = src.contains("pub const CLAUSES");
            for (i, re) in patterns.iter().enumerate() {
                if i == 2 && skip_clauses {
                    continue;
                }
                for cap in re.captures_iter(src) {
                    found += 1;
                    assert!(
                        documented(&cap[1]),
                        "mã lỗi {} chưa có trong errors::CODES",
                        &cap[1]
                    );
                }
            }
        }
        assert!(found > 40, "chỉ tìm thấy {found} chỗ dùng mã lỗi");
        let mut seen = std::collections::BTreeSet::new();
        for (code, status, vi, en) in CODES {
            assert!(seen.insert(code), "trùng {code}");
            assert!((400..600).contains(status) && !vi.is_empty() && !en.is_empty());
        }
    }
}
