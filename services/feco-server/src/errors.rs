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
    ("invalid_source", 422, "Mã nguồn sai: không phải zip hợp lệ, sai SHA-256, hoặc repo GitHub hay commit không hợp lệ; message nói rõ.", "The source code is invalid (not a valid zip, wrong SHA-256, or a bad GitHub repository or commit); message explains."),
    ("source_too_large", 413, "Mã nguồn lớn hơn 200 MB.", "The source code is larger than 200 MB."),
    ("not_draft", 409, "Chỉ sửa được phiên bản đang ở trạng thái nháp.", "Only draft versions can be changed."),
    ("not_ready", 409, "Chưa gửi duyệt được: thiếu gói cho một số nền tảng.", "Cannot submit: some platforms have no package."),
    ("not_approved", 409, "Chỉ phát hành được phiên bản đã qua kiểm duyệt.", "Only reviewed versions can be released."),
    ("not_released", 409, "Phiên bản chưa được phát hành.", "The version has not been released."),
    ("target_revoked", 409, "Gói của phiên bản này đã bị thu hồi; phát hành phiên bản mới.", "This version's package was revoked; release a new version."),
    ("target_conflict", 409, "Tên gói đã được dùng cho app hoặc phiên bản khác; đổi mã app hoặc số phiên bản.", "The package name is already used by another app or version; change the app id or version."),
    ("not_entitled", 403, "Người dùng chưa có quyền dùng app (app có giá chưa mua, hoặc app riêng tư của người khác). Mua app trong F-Store.", "The user may not use this app (a paid app not bought yet, or someone else's private app). Buy it in F-Store."),
    ("developer_role_required", 403, "Tài khoản chủ token không còn vai trò Lập trình viên. Gửi yêu cầu ở Cổng nhà phát triển hoặc liên hệ quản trị viên.", "The token owner no longer has the Developer role. Request it in the developer portal or contact an administrator."),
    ("insufficient_role", 403, "Vai trò của bạn trong nhóm không đủ cho thao tác này.", "Your team role does not allow this."),
    ("agreement_required", 403, "Tài khoản chưa chấp nhận Thỏa thuận nhà phát triển hiện hành.", "The account has not accepted the current Developer Agreement."),
    ("forbidden", 403, "Vai trò của bạn không cho phép thao tác này.", "Your role does not allow this."),
    ("unavailable", 503, "Dịch vụ tạm gián đoạn. Thử lại sau.", "The service is temporarily unavailable. Retry later."),
    ("insufficient_funds", 402, "Ví F-Coin không đủ. Mở trang ví để nạp thêm rồi thử lại.", "Not enough F-Coin. Open the wallet page to top up, then retry."),
    ("wallet_frozen", 423, "Ví F-Coin của người dùng đang bị khóa; không nạp, rút, mua được.", "The user's F-Coin wallet is frozen; top-ups, withdrawals and purchases are blocked."),
    ("price_changed", 409, "Giá app khác giá đã hiện cho người dùng. Đọc lại giá (GET /v1/wallet) rồi hỏi lại người dùng.", "The app price differs from the one shown. Re-read prices (GET /v1/wallet) and ask the user again."),
    ("already_owned", 409, "Người dùng đã mua app này.", "The user already owns this app."),
    ("free_app", 400, "App miễn phí, không cần mua.", "The app is free; there is nothing to buy."),
    ("own_app", 400, "Nhà phát triển không mua app của chính mình.", "Developers can't buy their own app."),
    ("payments_unavailable", 503, "Máy chủ chưa cấu hình cổng thanh toán.", "No payment gateway is configured on this server."),
    ("withdrawals_disabled", 503, "Máy chủ tạm không nhận lệnh rút F-Coin (FECO_WALLET_WITHDRAWALS=false).", "The server isn't accepting F-Coin withdrawals (FECO_WALLET_WITHDRAWALS=false)."),
    ("second_approver_required", 403, "Cần một quản trị viên khác duyệt.", "Another administrator must approve."),
    ("own_withdrawal", 403, "Quản trị viên không xử lý lệnh rút của chính mình.", "Administrators can't act on their own withdrawals."),
    ("rate_change_too_large", 400, "Mỗi lần chỉ đổi tỷ giá tối đa 20%.", "Each rate change is limited to 20%."),
    ("topup_expired", 410, "Lệnh nạp đã hết hạn hoặc tỷ giá đã thay đổi.", "The top-up expired or the rate changed."),
    ("topup_refund_via_withdrawal", 409, "Giao dịch nạp F-Coin chỉ hoàn qua lệnh rút.", "F-Coin top-ups are refunded only through withdrawals."),
    ("conflict_of_interest", 403, "Bạn là một bên liên quan nên không xử lý được việc này.", "You are a party to this, so you can't decide it."),
    ("self_transfer", 400, "Không chuyển F-Coin cho chính mình.", "You can't transfer F-Coin to yourself."),
    ("recipient_not_found", 404, "Không có tài khoản đã xác minh email với địa chỉ này.", "No account with this verified email."),
    ("invalid_form", 400, "Biểu mẫu không hợp lệ, hãy tải lại trang.", "Invalid form; reload the page."),
    ("unbalanced", 500, "Lỗi nội bộ: bút toán không cân. Báo cho đội vận hành.", "Internal error: unbalanced posting. Report it to operations."),
    ("decrypt_failed", 500, "Lỗi nội bộ: không giải mã được dữ liệu. Báo cho đội vận hành.", "Internal error: data could not be decrypted. Report it to operations."),
    ("account_suspended", 403, "Tài khoản nhà phát triển đang bị tạm khóa chờ điều tra.", "The developer account is suspended pending investigation."),
    ("email_unverified", 403, "Cần xác minh email trước.", "Verify your email first."),
    ("second_factor_required", 403, "Cần bật xác thực hai lớp trước.", "Turn on two-factor authentication first."),
    ("txt_not_found", 409, "Chưa thấy bản ghi DNS TXT; DNS có thể cần vài phút.", "The DNS TXT record was not found yet; DNS can take a few minutes."),
    ("not_rejected", 409, "Chỉ kháng nghị được phiên bản đang bị từ chối.", "Only rejected versions can be appealed."),
    ("already_appealed", 409, "Mỗi lần từ chối chỉ được kháng nghị một lần.", "Each rejection can be appealed once."),
    ("no_open_appeal", 409, "Không có kháng nghị đang mở.", "There is no open appeal."),
    ("not_in_manual_review", 409, "Phiên bản không ở hàng chờ duyệt thủ công.", "The version is not in the manual review queue."),
    ("reviewer_is_developer", 403, "Người duyệt là thành viên của nhà phát triển app nên không được tự duyệt.", "The reviewer is a member of the app's developer account and can't approve it."),
    ("csrf", 403, "Biểu mẫu hết hạn; tải lại trang.", "The form expired; reload the page."),
    // Chat (thay đổi chat-and-source-scan, design C2, C4).
    ("chat_not_found", 404, "Không có hội thoại (hoặc tệp đính kèm) này.", "No such conversation (or attachment)."),
    ("chat_forbidden", 403, "Bạn không tham gia hội thoại này, hoặc hội thoại dự án đã đóng.", "You are not part of this conversation, or the project conversation is closed."),
    ("chat_blocked", 409, "Nhà phát triển đã chặn hội thoại này; không gửi được tin.", "The developer blocked this conversation; messages can't be sent."),
    ("chat_too_long", 422, "Tin dài quá 5.000 ký tự.", "The message is longer than 5,000 characters."),
    ("chat_empty", 422, "Tin trống.", "The message is empty."),
    ("chat_rate_limited", 429, "Quá 30 tin mỗi phút hoặc 20 hội thoại mới mỗi ngày. Chờ rồi thử lại.", "More than 30 messages a minute or 20 new conversations a day. Wait and retry."),
    ("chat_attachment_too_large", 413, "Tệp đính kèm lớn hơn 20 MB.", "The attachment is larger than 20 MB."),
    ("chat_attachment_pending", 409, "Tệp đang được kiểm tra mã độc; thử lại sau ít phút.", "The attachment is still being scanned; retry in a few minutes."),
    ("chat_attachment_blocked", 403, "Tệp có mã độc nên bị chặn tải.", "The attachment contains malware and can't be downloaded."),
    ("chat_not_developer", 422, "Người này không phải nhà phát triển nên không nhận hội thoại trực tiếp.", "This person isn't a developer, so they can't receive direct conversations."),
    ("chat_self", 422, "Không nhắn cho chính mình.", "You can't message yourself."),
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
            include_str!("source_scan.rs"),
            include_str!("entitlements.rs"),
            include_str!("security.rs"),
            include_str!("wallet.rs"),
            include_str!("wallet_web.rs"),
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
