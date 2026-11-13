# Tasks

## 1. Ngôn ngữ giao diện

- [x] 1.1 `i18n.rs`: bảng dịch vi, en, fr; `t`, `tf`; ngôn ngữ hiện hành; xác minh bằng unit test (thiếu bản dịch, sai số `{}`, chuỗi có dấu nằm ngoài `t`)
  - Đã xác minh, unit test:
    - `table_is_consistent`: không trùng khóa, đủ bản dịch, đúng số `{}`;
    - `every_ui_string_has_translations` (scenario "Thiếu bản dịch"): quét mọi lời gọi `t`/`tf` trong `main.rs`, `library.rs`, `prefs.rs`, `style.rs`;
    - `no_untranslated_literals_in_main`: không còn chuỗi có dấu nằm ngoài `t`/`tf`;
    - `french_labels_from_the_spec`, `translate_and_fill`.
  - Test đã bắt được hai chỗ sót khi chạy thử: gợi ý ô tìm kiếm và chữ khi di chuột.
- [x] 1.2 Mọi chuỗi giao diện trong `main.rs`, nhãn trong `library.rs`, `prefs.rs` đi qua bảng dịch; `settings.json` có `language`; xác minh bằng unit test và chạy thử ba ngôn ngữ
  - Đã xác minh:
    - unit test `language_and_style_are_saved`: settings 1.1 vẫn mở tiếng Việt, giá trị lạ về mặc định, lưu và đọc lại, khôi phục mặc định;
    - chạy thử trên Windows, có chụp màn hình, với tiếng Anh (Store, Library, Install, All categories, Sort: Name) và tiếng Pháp (Boutique, Installer, Toutes les catégories, Productivité, trang Paramètres).

## 2. Phong cách giao diện

- [x] 2.1 `style.rs`: Chân phương, Trang nhã, tám phong cách họa sĩ với bảng màu, bo góc, dải họa tiết; xác minh bằng unit test tương phản WCAG
  - Đã xác minh: unit test `every_style_is_readable` (scenario "Kiểm tra tương phản"): chữ ≥ 4,5:1, chữ mờ ≥ 3:1, cảnh báo và lỗi ≥ 4,5:1 trên nền cửa sổ, thẻ và thanh bên; chữ trên nút nhấn ≥ 4,5:1; cờ nền tối đúng. Cùng `styles_are_distinct_and_described`, `contrast_math`.
- [x] 2.2 Áp phong cách vào egui, dải họa tiết ở thanh bên, trang Cài đặt có ô xem trước; tắt chủ đề và màu nhấn với phong cách họa sĩ; `settings.json` có `style`; xác minh bằng chạy thử trên Windows
  - Đã chạy thử trên Windows, có chụp màn hình, các phong cách Vincent van Gogh, Bùi Xuân Phái, Nguyễn Gia Trí, Trang nhã (nút hình viên thuốc) và Claude Monet.
  - Trang Cài đặt có 10 ô xem trước; với phong cách họa sĩ, ô chủ đề và màu nhấn bị tắt kèm lời giải thích.
  - Khoảng cách làm tròn tới điểm ảnh nguyên. Bản debug của egui báo "Unaligned" khi khoảng cách lẻ.

## 3. Tài liệu và phát hành

- [x] 3.1 README, CHANGELOG, QUESTIONS.md (tên họa sĩ, lỗi kỹ thuật chưa dịch); phát hành 1.2.0
  - Đã xác minh: README bước 6 mục 12, CHANGELOG 1.2.0, QUESTIONS.md mục 75 và 76. Phiên bản 1.2.0; `Cargo.lock` của cả hai workspace khớp (`cargo metadata --locked`).
