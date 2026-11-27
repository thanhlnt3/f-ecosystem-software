# Tasks

## 1. Phông chữ

- [x] 1.1 Đọc phông giao diện của hệ điều hành theo danh sách của F-Launcher, kiểm bằng `skrifa` (đọc được, đủ chữ tiếng Việt) trước khi nạp, `F_BUILDER_UI_FONT` để chọn phông khác; xác minh bằng unit test và chạy app bằng tiếng Việt, scenario "Chữ tiếng Việt đủ dấu", "Phông chỉ định bị hỏng"
  - Xác minh (Windows 10, 27/11/2026):
    - Nguyên nhân: DM Sans thiếu 70 chữ tiếng Việt và mũi tên; Segoe UI có đủ (đếm bằng bảng `cmap`).
    - Test trong `theme.rs`:
      - `dm_sans_alone_cannot_show_vietnamese`;
      - `a_broken_or_unsuitable_font_is_skipped`: tệp rác, tệp không có, DM Sans đều bị bỏ qua; bản đậm hỏng không được nạp; không panic;
      - `vietnamese_comes_from_one_system_face`: mọi họ phông bắt đầu bằng phông hệ thống, phông đó có đủ 146 chữ;
      - `without_a_system_font_dm_sans_leads`;
      - `the_system_font_is_found_where_the_launcher_finds_one`: tìm thấy Segoe UI và Segoe UI Semibold.
    - Ảnh chụp `--lang vi` (dự án, bước AI của lần chạy đầu, trang Tạo, bàn làm việc): mọi chữ cùng một phông, "Cài đặt → AI" không còn ô vuông.
    - Chưa thử: phông trên macOS và Linux trong cửa sổ thật.

## 2. Bảng màu và hình khối

- [x] 2.1 Bảng màu sáng/tối, bo góc Trang nhã, nút chính màu F-Eco, mặc định giao diện sáng; xác minh bằng test tương phản WCAG, test giao diện và ảnh chụp app, scenario "Giống F-Launcher"
  - Xác minh:
    - `contrast_meets_wcag_aa_in_both_themes`; `main_buttons_keep_their_contrast_in_every_state` (chữ trắng trên màu F-Eco lúc thường, di chuột, bấm); `theme_ids_round_trip` (mặc định sáng, giá trị lạ là sáng).
    - Test giao diện `ui-egui`: 105 qua. Có điều chỉnh vì phông hệ thống cao hơn DM Sans khoảng 2%:
      - trang Tạo bớt 10 px khoảng trống, để nút Bắt đầu vẫn trong khung ở cửa sổ cao 900 px;
      - test thêm AI bằng địa chỉ cuộn tới nút Thêm trước khi bấm;
      - test đổi chủ đề bấm "Studio" (tối), vì mặc định đã là sáng.
    - Ảnh chụp: nền `#f6f8fc`, thanh bên trắng, mục đang chọn, nút chính và bước đã xong màu `#0f6b5c`.
    - Đã chạy clippy, layers, locks, secrets, i18n-coverage (0 thiếu).
    - Chưa thử: giao diện tối trong cửa sổ thật.

## 3. Tài liệu và phát hành

- [x] 3.1 README hai thứ tiếng, CHANGELOG, tài liệu F-Builder; phát hành
  - Xác minh:
    - README hai thứ tiếng (tóm tắt tính năng, phiên bản 1.22.0); `scripts/check-docs-i18n.py` qua.
    - CHANGELOG 1.22.0; QUESTIONS.md 139.
    - `clients/f-builder/docs/STATUS.md` và `localization.md`.
    - CI và `release.yml` của tag `v1.22.0`.
