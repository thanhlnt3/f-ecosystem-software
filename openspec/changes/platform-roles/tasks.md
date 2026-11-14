# Tasks

## 1. Vai trò nền tảng (`account-identity`)

- [x] 1.1 Migration `0013_platform_roles.sql` (`role_grants`, `role_requests`, cấp Lập trình viên cho thành viên nhà phát triển có sẵn); `roles.rs` (`role_of`, `set_role`, yêu cầu, duyệt); xác minh bằng unit test và E2E
  - Đã xác minh:
    - unit test `roles_parse_and_permissions`;
    - migration cấp quyền cho thành viên `active` của `developer_members` (scenario "Nhà phát triển có từ trước"). Các E2E của cổng nhà phát triển vẫn đạt sau khi cấp quyền qua `grant_developer`.
- [x] 1.2 Cổng nhà phát triển và API `f-dev` kiểm vai trò; trang "Cần quyền Lập trình viên" và gửi yêu cầu; xác minh bằng E2E scenario "Người dùng mở Cổng nhà phát triển", "Thu hồi quyền khi đang có token", "Gửi và được duyệt", "Gửi trùng"
  - Đã xác minh:
    - E2E `admin_console_end_to_end`: trang "Cần quyền Lập trình viên" không có biểu mẫu đăng ký; gửi yêu cầu, gửi trùng trả 409; quản trị viên nhận cảnh báo; duyệt trên trang Phân quyền; người gửi nhận email "Đã được cấp quyền Lập trình viên", rồi vào được cổng;
    - E2E `child_app_integration_end_to_end`: hạ về Người sử dụng thì token `f-dev` nhận `403 developer_role_required`, cấp lại thì dùng tiếp.
- [x] 1.3 Trang Phân quyền trong bảng quản trị; `feco-catalog role-set`, `role-requests`; xác minh bằng E2E scenario "Cấp quyền Lập trình viên", "Thiếu lý do", "Tự hạ quyền"
  - Đã xác minh, E2E `admin_console_end_to_end`:
    - cấp Quản trị viên (vai trò con Hỗ trợ) kèm lý do;
    - thiếu lý do trả 400, vai trò không đổi;
    - tự hạ quyền trả 400;
    - duyệt yêu cầu ghi `audit_log` hành động `role.set` (`to` = `developer`).
  - Thêm `feco-catalog role-decide`.

## 2. F-Launcher (`launcher`)

- [x] 2.1 `GET /v1/account/role` chỉ cho token của launcher; launcher hiện vai trò và lối tắt; xác minh bằng E2E và chạy thử
  - Đã xác minh:
    - E2E: token launcher của người vừa được duyệt nhận `developer` kèm địa chỉ Cổng nhà phát triển; access token của app con nhận 403 (scenario "Token của app khác");
    - unit test `account_role_is_parsed_safely`: vai trò lạ coi như người sử dụng; liên kết không phải https bị bỏ;
    - launcher lưu vai trò vào `state.json` để dùng khi ngoại tuyến.
- [x] 2.2 Đổi tên phong cách sang tên phổ thông, đọc được mã cũ; xác minh bằng unit test scenario "Cài đặt cũ"
  - Đã xác minh: unit test `styles_are_distinct_and_described` đọc đúng tám mã của bản 1.2 (`davinci` … `nguyen_gia_tri`) ra phong cách mới. Test i18n đủ bản dịch Anh, Pháp cho tên mới.

## 3. Dọn dẹp và phát hành

- [x] 3.1 Gỡ Demo App khỏi danh mục và kho TUF dev (`feco-tuf remove`); `dev-launcher.sh` thôi đưa Demo App vào kho; xác minh bằng unit test `feco-tuf` và chạy thử launcher
  - Đã xác minh:
    - test `removed_app_leaves_the_catalog_without_a_revocation`: app gỡ biến khỏi danh mục, app khác giữ nguyên, không có bản ghi thu hồi;
    - `scripts/dev-launcher.sh remove-demo` gỡ hai phiên bản Demo App khỏi kho TUF dev và danh mục F-Store dev.
- [x] 3.2 README, CHANGELOG, QUESTIONS.md; phát hành 1.3.0
  - Đã xác minh: README bước 6 (Demo App, mục 12, 13), CHANGELOG 1.3.0, QUESTIONS.md mục 75 đã xử lý, spec `launcher-personalization` dùng tên phong cách mới. Phiên bản 1.3.0.
