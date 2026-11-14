# Tasks

## 1. Bảng màu chung và tiếng Nhật

- [x] 1.1 `feco_core::ui_palette` (10 phong cách, bảng màu, độ tương phản); F-Launcher `style.rs` dùng dữ liệu này; xác minh bằng unit test tương phản
  - Xác minh: `feco-core` `every_style_is_readable`, `codes_and_shapes` (mã cũ của 1.2 vẫn đọc được), `accents_have_readable_text`; launcher `accents_match_the_shared_palette`, `styles_have_distinct_translated_names`.
- [x] 1.2 F-Launcher tiếng Nhật: `Language::Ja`, cột `ja` trong bảng dịch, phông chữ Nhật của hệ điều hành; xác minh bằng unit test và chạy thử
  - Xác minh: bảng riêng `i18n_ja.rs` (design đã cập nhật); test `japanese_sidebar_labels` (ライブラリ, ストア, 通知, 設定), `japanese_table_matches` (194 khóa, cùng số `{}`); phông Yu Gothic/Meiryo/MS Gothic nạp làm phông dự phòng; chạy thử launcher 1.4.0 trên Windows 10.
- [x] 1.3 Web tiếng Nhật: `Lang::Ja`, `i18n_ja.rs`, nhận `Accept-Language: ja`, `/lang/ja`; xác minh bằng unit test (đủ khóa) và E2E scenario "Chọn tiếng Nhật", "Trình duyệt tiếng Nhật"
  - Xác minh: unit test `every_key_has_japanese` (545 khóa, cùng chỗ `{…}`), `detects_language`; E2E `web_japanese_and_appearance`. Schema Kratos nhận `locale = ja`.

## 2. Giao diện web

- [x] 2.1 `ui_prefs`: cookie `feco_ui`, middleware gắn thuộc tính vào `<html>`, trang `/preferences`; xác minh bằng unit test và E2E scenario "Đổi sang chủ đề tối và phong cách Trời sao", "Cookie hỏng"
  - Xác minh: unit test `broken_cookie_falls_back`, `cookie_round_trip_and_attrs`, `injects_into_html_tag`, `return_paths_stay_local`; E2E `web_japanese_and_appearance` (CSRF, lưu, khôi phục mặc định, cookie hỏng → Chân phương, 125%).
- [x] 2.2 `base.html` mới: thanh trên cùng, CSS biến theo phong cách sinh từ bảng màu chung; xác minh bằng unit test scenario "CSS khớp bảng màu"
  - Xác minh: unit test `css_matches_the_shared_palette`; E2E kiểm `/ui.css` có `--bg` của Trời sao.
- [x] 2.3 Dashboard quản trị: thanh bên, thẻ số liệu theo vai trò, biểu đồ doanh thu; xác minh bằng E2E scenario "Quản trị hệ thống mở bảng quản trị", "Thẻ theo vai trò"
  - Xác minh: E2E `admin_console_end_to_end` (thẻ `kpi-role-requests` bằng số yêu cầu đang chờ, dẫn tới `/admin/members`; vai trò Hỗ trợ không thấy doanh thu, hoàn tiền); unit test `revenue_chart_has_one_bar_per_day`.
- [x] 2.4 `FECO_ADMIN_PASSKEY_OPTIONAL` chỉ ở dev; xác minh bằng unit test cấu hình và E2E scenario "Dev không có passkey"
  - Xác minh: unit test `admin_passkey_flag_is_dev_only` (staging từ chối); E2E `dev_admin_without_passkey` (máy chủ thường vẫn 403, máy chủ dev có cờ hiện cảnh báo và 14 cột biểu đồ).

## 3. Phát hành

- [x] 3.1 README, CHANGELOG, QUESTIONS.md (rà soát bản dịch tiếng Nhật, tiếng Pháp trên web); phát hành 1.4.0
  - Xác minh: CHANGELOG 1.4.0, README (Giao diện web, `FECO_ADMIN_PASSKEY_OPTIONAL`), QUESTIONS 78–80; `cargo test --workspace` với `FECO_E2E=1`: 289 passed, 0 failed; CI và release.yml của tag `v1.4.0`.
