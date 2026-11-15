# Tasks

## 1. F-Launcher

- [x] 1.1 `icons.rs`: 21 icon vẽ bằng nét, `icons::button` (bắt buộc tooltip), `icons::labeled`, `icons::filled`; thay chữ bằng icon ở các nút công cụ; xác minh bằng unit test scenario "Icon vẽ trong ô của nó" và chạy thử scenario "Chuyển lưới bằng icon"
  - Xác minh: unit test `every_icon_paints_inside_its_box` (mọi icon vẽ ra hình, nằm trong ô); chạy thử launcher 1.5.0 trên Windows 10: tooltip "Lưới"/"Danh sách", bấm icon đổi bố cục.
- [x] 1.2 Cửa sổ riêng cho Cài đặt và chi tiết app (`show_viewport_immediate`, tối đa 6 cửa sổ chi tiết, đưa lên trước khi bấm lại, Esc đóng, hộp thoại ở cửa sổ nơi bấm); xác minh bằng chạy thử scenario "Mở chi tiết trong cửa sổ mới", "Bấm lại app đang mở"
  - Xác minh: chạy thử launcher 1.5.0: bấm app mở cửa sổ "<tên app> · F-Launcher", cửa sổ chính giữ Cửa hàng; bấm lại app đưa cửa sổ lên trước; Cài đặt mở cửa sổ riêng.
- [x] 1.3 Ô lưới đều nhau (`grid_columns`, `tile_height`, vẽ ô vào hình chữ nhật cố định); xác minh bằng unit test scenario "Ô lưới đều nhau"
  - Xác minh: unit test `grid_tiles_fill_rows_evenly` (900 điểm, ô 180, khoảng 12 → 4 cột đều, không tràn).

## 2. Web

- [x] 2.1 Trang hồ sơ (`profile.rs`, `home.html`): phần đầu, thẻ số liệu, nhóm lối tắt, trạng thái xác thực hai lớp; xác minh bằng E2E scenario "Người dùng mở trang hồ sơ", "Lập trình viên thấy lối tắt Cổng nhà phát triển"
  - Xác minh: E2E `profile_page_and_french` (ảnh đại diện "LH", vai trò, thẻ ứng dụng đã kết nối = 1, đơn hàng = 0, chưa bật hai lớp, lối tắt theo vai trò); `admin_console_end_to_end` (người có passkey thấy "Đang bật xác thực hai lớp"); unit test `initials_from_name_or_email`, `avatar_color_is_stable`.
- [x] 2.2 Tiếng Pháp trên web (`Lang::Fr`, `i18n_fr.rs`, nút FR, `Accept-Language: fr`, schema `locale = fr`); xác minh bằng unit test scenario "Thiếu bản dịch tiếng Pháp" và E2E scenario "Trình duyệt tiếng Pháp"
  - Xác minh: unit test `every_key_has_french`, `detects_language`; E2E `profile_page_and_french`.
- [x] 2.3 Đếm người dùng có bộ nhớ đệm 10 phút (`admin::user_count`); xác minh bằng E2E scenario "Mở dashboard hai lần liền"
  - Xác minh: E2E `admin_user_count_is_cached`.

## 3. Môi trường dev và phát hành

- [x] 3.1 Kratos và Hydra dev dùng SQLite trong `infra/.data` (`FECO_DEV_MEMORY=1` để dùng bộ nhớ)
  - Xác minh: khởi động lại Kratos và Hydra, client OAuth của launcher và tài khoản vẫn còn; E2E chạy trên hạ tầng SQLite.
- [x] 3.2 README, CHANGELOG, QUESTIONS.md (79, 80 đã xử lý); phát hành 1.5.0
  - Xác minh: CHANGELOG 1.5.0, README (giao diện launcher, trang hồ sơ, tiếng Pháp, dữ liệu dev), `cargo test --workspace` với `FECO_E2E=1`, CI và release.yml của tag `v1.5.0`.
