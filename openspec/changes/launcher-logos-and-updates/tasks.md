# Tasks

## 1. Logo

- [x] 1.1 `TargetMeta::icon`, `icon_from_png` (PNG, tối đa 64 KB), `Repo::set_icon`, `feco-tuf publish --icon` và `set-icon`; xác minh bằng test scenario "Logo không phải PNG" và "App có logo"
  - Xác minh: test `app_icon_is_signed_with_its_targets` (crates/feco-tuf). Logo được ký cùng target, client xác minh rồi đọc được; target của app khác giữ nguyên; tệp GIF và tệp quá 64 KB bị từ chối.
- [x] 1.2 Logo trên F-Store: migration `0016_app_icons.sql`, `catalog::icon`, `/store/{id}/icon.png`, logo trong danh sách và trang app, `/favicon.svg`, `feco-catalog add-app` với `icon_file`; xác minh bằng E2E scenario "Logo app trên F-Store", "App không có logo"
  - Xác minh: E2E `app_logos_and_web_library`. PNG trả đúng byte và `image/png`; cập nhật app không kèm logo thì giữ logo; app không logo trả 404; `/favicon.svg` là logo F-Launcher.
- [x] 1.3 Logo trong launcher (giải mã PNG bằng `png`, texture theo app, chữ cái đầu khi không có), logo F-Launcher làm biểu tượng cửa sổ và logo thanh bên; logo dev (`assets/logos/`, `scripts/dev-apps.sh icons`); xác minh bằng unit test và chạy thử scenario "Biểu tượng cửa sổ"
  - Xác minh:
    - unit test `logos_decode`: logo F-Launcher 256 px và logo kibi trong metadata giải mã được; dữ liệu không phải PNG bị bỏ qua;
    - `dev-apps.sh icons` đặt logo cho 5 app dev;
    - chạy thử launcher 1.6.0 trên Windows 10.

## 2. Cửa sổ

- [x] 2.1 Launcher mở giữa màn hình (`centered`); Cài đặt và chi tiết app mở giữa màn hình, là cửa sổ chặn (lớp phủ, đưa cửa sổ con lên trước); xác minh bằng unit test scenario "Cửa sổ ở giữa màn hình" và chạy thử scenario "Bấm vào cửa sổ chính khi đang mở chi tiết"
  - Xác minh:
    - unit test `child_windows_are_centered`: màn hình 1920 × 1080, cửa sổ 760 × 700 mở tại (580, 190); màn hình nhỏ hơn cửa sổ thì đặt ở (0, 0);
    - chạy thử launcher 1.6.0.

## 3. Đồng bộ tính năng và gợi ý cập nhật

- [x] 3.1 Gợi ý cập nhật (`update_suggestions`, hộp thoại, Bỏ qua bản này lưu ở `settings.json`, băng nhắc ở Thư viện); xác minh bằng unit test scenario "Phát hiện bản mới", "Bỏ qua phiên bản"
  - Xác minh: unit test `suggests_updates_except_skipped_versions`. Gợi ý f-notes và kibi kèm ghi chú phát hành; bỏ qua kibi 0.3.3 thì không gợi ý lại; có 0.3.4 thì gợi ý lại.
- [x] 3.2 Trang "Ứng dụng của tôi" trên web (`/library`), menu "F-Eco trên web" trong launcher; xác minh bằng E2E scenario "Ứng dụng của tôi trên web", "App đã mua chưa cài"
  - Xác minh: E2E `app_logos_and_web_library`:
    - cài 1.0.0, rồi phát hành 1.1.0: app có nhãn "Có bản mới" và trang có băng "Mở F-Launcher để cập nhật";
    - app đã mua chưa cài có nhãn "Đã mua";
    - unit test so phiên bản `catalog::newer`.

## 4. Phát hành

- [x] 4.1 README, CHANGELOG, QUESTIONS.md (FPT đồng ý dùng nhận diện thương hiệu; cổng chi hộ); phát hành 1.6.0 cùng thay đổi fcoin-wallet-and-marketplace
