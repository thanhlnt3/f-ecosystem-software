# Tasks

## 1. Máy chủ

- [x] 1.1 Module `listing` (phân tích tham số, bỏ dấu, lọc, sắp xếp) và template `admin_toolbar.html`; xác minh bằng unit test `listing::tests`
  - Xác minh: `listing::tests` (5 test): bỏ dấu (`Công ty ÂU LẠC` → `cong ty au lac`, `Đ` → `d`), tham số lạ bị bỏ, `q` tối đa 200 ký tự, tìm nhiều từ, giữ thứ tự mặc định, mới/cũ nhất, người gửi A–Z/Z–A, trạng thái ẩn mặc định, `all`, khoảng ngày.
- [x] 1.2 Thanh công cụ ở hàng chờ duyệt, đơn xây app, hoàn tiền, phân quyền, dự án, báo cáo tin nhắn, ví; giao diện điện thoại; xác minh bằng E2E scenario "Tìm theo người gửi", "Sắp xếp theo người gửi", "Lọc theo thời gian", "Lọc trạng thái", "Đơn đã xong", "Xóa lọc"
  - Xác minh: E2E `admin_list_tools`: tìm "au lac" chỉ còn app của "Công ty Âu Lạc", "Đang hiện 1/…" và nút Xóa lọc; trang không điều kiện có cả hai app; đơn đã hoàn thành ẩn mặc định, hiện với `status=all` và `status=completed`; `sort=sender` xếp an < binh < chi, `sender_desc` ngược lại; `from`=`to`=hôm qua chỉ còn đơn hôm qua và ô ngày giữ giá trị; báo cáo tin nhắn lọc `handled`, tìm "lua dao", tìm theo người báo cáo không dấu, `sort=old`; dự án không khớp ghi "Đang hiện 0/…"; CSS bảng cuộn ngang dưới 800px. `admin_console_end_to_end`, `admin_transfers_f_coin` và toàn bộ `e2e_auth` vẫn qua.
- [x] 1.3 `admin_sections` trong `GET /v1/account/role`; xác minh bằng E2E scenario "Người duyệt app"
  - Xác minh: cùng E2E: Người duyệt app nhận `admin_sections` = `[{"key": "reviews", "url": ".../admin/reviews"}]`; người dùng thường nhận `[]`.

## 2. F-Launcher máy tính

- [x] 2.1 `AccountRole.admin_sections`, menu "Bảng quản trị"; xác minh bằng unit test và scenario "Mở hàng chờ duyệt"
  - Xác minh: unit test `auth::notification_tests::admin_sections_are_parsed_safely` (bỏ khóa lạ, `javascript:`, mục thiếu khóa; máy chủ cũ không có trường) và `tests::admin_section_labels`; menu dùng `egui::Popup::menu` như menu "F-Eco trên web", mỗi dòng gửi `Action::OpenLink`.

## 3. Di động

- [x] 3.1 `Account.adminSections`, thẻ "Bảng quản trị" ở tab Tài khoản; xác minh bằng widget test scenario "Quản trị viên mở báo cáo tin nhắn", "Người dùng thường"
  - Xác minh: unit test `admin_sections_from_the_server`; widget test `scenario_bang_quan_tri_tren_dien_thoai` (thẻ "Bảng quản trị", "Hàng chờ duyệt app", bấm "Báo cáo tin nhắn" mở đúng địa chỉ) và `scenario_nguoi_dung_thuong_khong_co_bang_quan_tri`; `flutter analyze` sạch, `flutter test` 31 qua.

## 4. Tài liệu và phát hành

- [x] 4.1 README hai thứ tiếng, CHANGELOG, QUESTIONS.md; phát hành 1.15.0
  - Xác minh: README hai thứ tiếng (mục 11 bước 2), `scripts/check-docs-i18n.py`; CHANGELOG 1.15.0; QUESTIONS.md 125–127; clippy sạch, `cargo test --workspace` qua, E2E `e2e_auth`, `e2e_chat`, `e2e_source_scan` qua; CI và release.yml của tag `v1.15.0`.
