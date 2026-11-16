# Tasks

## 1. Đăng ký

- [x] 1.1 `RoleChoice` (đọc, kiểm), ô vai trò, tên pháp lý, mã số thuế trên trang đăng ký; kiểm trước khi gửi Kratos; xác minh bằng unit test và E2E scenario "Công ty thiếu mã số thuế", "Vai trò lạ"
  - Xác minh:
    - unit test `role_choice_at_registration`: mặc định là Người dùng phổ thông; `admin`, `root` bị từ chối; vai trò tổ chức thiếu tên pháp lý hoặc mã số thuế sai bị từ chối;
    - unit test `registration_form_has_role_fields`: năm lựa chọn trước ô mật khẩu, giữ giá trị, lỗi dưới ô sai;
    - E2E `role_chosen_at_registration`: Công ty thiếu mã số thuế thì báo lỗi, giữ email và tên pháp lý, không có tài khoản; `feco_role=admin` báo "Vai trò không hợp lệ", không có tài khoản.
- [x] 1.2 Tạo yêu cầu vai trò sau khi Kratos tạo tài khoản; trang "Đã gửi email" giống nhau cho email mới và email đã có; xác minh bằng E2E scenario "Đăng ký làm Khách hàng doanh nghiệp", "Đăng ký làm Người dùng phổ thông", "Email đã có tài khoản"
  - Xác minh: E2E `role_chosen_at_registration`:
    - Mai đăng ký Khách hàng doanh nghiệp: yêu cầu đang chờ đúng tên pháp lý, mã số thuế, ghi chú "Chọn khi đăng ký"; Mai vẫn là Người dùng phổ thông; quản trị viên nhận email cảnh báo;
    - Nam giữ vai trò mặc định: không có yêu cầu;
    - đăng ký lại bằng email của Nam với Lập trình viên cá nhân: thông báo giống hệt đăng ký mới, không có yêu cầu.

## 2. Bảng quản trị

- [x] 2.1 Trang Phân quyền ghi nguồn và trạng thái xác minh email; duyệt, từ chối; xác minh bằng E2E scenario "Duyệt vai trò chọn khi đăng ký", "Từ chối vai trò chọn khi đăng ký", "Từ chối không có lý do"
  - Xác minh:
    - E2E `role_chosen_at_registration`: yêu cầu của Mai ghi "Chọn khi đăng ký", "Email chưa xác minh", tên pháp lý và mã số thuế; duyệt thì Mai là Khách hàng doanh nghiệp, có email "Đã được cấp vai trò Khách hàng doanh nghiệp"; từ chối không lý do thì yêu cầu vẫn chờ; từ chối với lý do "Chưa có hồ sơ năng lực" thì vẫn là Người dùng phổ thông, email có lý do;
    - E2E `six_roles_and_role_requests` (yêu cầu từ trang Vai trò) vẫn qua.

## 3. Tài liệu và phát hành

- [x] 3.1 Chuỗi giao diện bốn thứ tiếng, README, CHANGELOG; phát hành 1.8.0
  - Xác minh: test `every_key_has_japanese`, `every_key_has_french`; `scripts/check-docs-i18n.py`; README bước 4.1; CHANGELOG 1.8.0; QUESTIONS.md 94; CI và release.yml của tag `v1.8.0`.
