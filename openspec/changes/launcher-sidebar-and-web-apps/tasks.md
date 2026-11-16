# Tasks

## 1. Thanh bên thu gọn

- [x] 1.1 `Prefs::sidebar_collapsed`, `sidebar_width`, `badge_text`, icon `Menu`; thanh bên hai chế độ, nút ẩn/hiện, Ctrl+B, tự thu gọn khi cửa sổ hẹp; xác minh bằng unit test scenario "Thu gọn thanh bên", "Huy hiệu thông báo", "Cửa sổ hẹp", "Cài đặt cũ" và chạy thử scenario "Mở lại launcher"
  - Xác minh:
    - unit test `sidebar_choice_is_saved`: `settings.json` cũ không có trường mới thì thanh bên đầy đủ, các cài đặt khác giữ nguyên; lưu rồi đọc lại có `"sidebar_collapsed": true`;
    - unit test `sidebar_collapses_when_chosen_or_narrow`: 210 px và 56 px; cửa sổ 700 px thu gọn mà không đổi lựa chọn, 1000 px thì đầy đủ;
    - unit test `badge_counts`: 12 thông báo thì huy hiệu "9+";
    - `every_icon_paints_inside_its_box` có icon `Menu` và `PanelLeft`;
    - chạy thử launcher 1.7.0 trên Windows 10: thu gọn, mở rộng, mở lại launcher vẫn thu gọn.

## 2. App web

- [x] 2.1 `TargetMeta::web_url`, `valid_web_url`, `feco-tuf publish --web-url`; xác minh bằng test scenario "Địa chỉ không an toàn"
  - Xác minh: test `web_app_url_is_signed_and_checked` (crates/feco-tuf). Từ chối `http://example.com/app`, `javascript:`, `file:`, `user:pass@`, target `web` thiếu địa chỉ, và địa chỉ web trên target desktop; kho không đổi. `http://localhost:3000/auth/feco` được ký và client đọc được sau khi xác minh.
- [x] 2.2 Launcher: catalog nhận nền tảng `web`, nhãn "App web", "Thêm", mở bằng trình duyệt; xác minh bằng unit test scenario "App web trên Windows", "Mở app web"
  - Xác minh:
    - test `web_apps_show_everywhere_and_open_their_url` (clients/feco-launcher/tests): app web hiện trên nền tảng Windows; app có cả gói Windows và bản web thì dùng gói Windows; "Thêm" tải, kiểm và ghi vào Thư viện; `prepare_launch` trả `web_url` mà không cần tệp chạy;
    - `platform_labels_and_dates`: nền tảng `web` ghi "Trình duyệt web".
- [x] 2.3 F-Store: migration `0017_web_apps.sql`, `web_url` trong danh mục, trang app, bộ lọc nền tảng, "Ứng dụng của tôi", `feco-catalog add-app`; xác minh bằng E2E scenario "Trang app web", "Lọc theo Windows", "Ứng dụng của tôi"
  - Xác minh: E2E `web_apps_and_server_side_login`:
    - trang app có nút "Mở ứng dụng web" tới `web_url`, nền tảng "Web", không có hướng dẫn cài;
    - địa chỉ không an toàn bị bỏ qua, địa chỉ cũ giữ nguyên;
    - lọc Windows, macOS, Linux đều có app web; JSON danh mục có `web_url`;
    - `/library` có nút "Mở" tới `web_url` sau khi launcher báo đã thêm.
- [x] 2.4 Đăng nhập bằng F-Eco cho client phía máy chủ; xác minh bằng E2E scenario "ThesisAI đăng nhập bằng F-Eco", "Sai client secret", "Chuyển hướng lạ"
  - Xác minh: E2E `web_apps_and_server_side_login`:
    - client `client_secret_basic` đi hết luồng authorization code + PKCE, về `http://localhost:8000/api/auth/feco/callback` đúng `state`;
    - `sub` pairwise khác mã danh tính; `/v1/me` trả tên, email, `email_verified`;
    - lần sau không hỏi lại đồng ý; sai secret trả 401 `invalid_client`;
    - `redirect_uri` chưa đăng ký không nhận được mã.

## 3. ThesisAI

- [x] 3.1 `integrations/thesisai/` (README, NOTICE, bản vá backend và frontend), logo, `scripts/dev-apps.sh thesisai`; xác minh bằng `py_compile` bản vá backend và chạy `dev-apps.sh thesisai` trên môi trường dev
  - Xác minh:
    - hai bản vá áp sạch (`git apply --check`) lên bản sao mới tại commit `d890595` (backend) và `8f3735e` (frontend);
    - các tệp Python của backend qua `python -m py_compile`;
    - `scripts/dev-apps.sh thesisai` tạo client phía máy chủ, ký `thesisai-0.1.0-web.zip` (nền tảng `web`, logo), ghi danh mục;
    - chưa chạy ThesisAI thật vì máy dev chưa có thư viện và mô hình AI (QUESTIONS.md 92).

## 4. Tài liệu và phát hành

- [x] 4.1 Chuỗi giao diện bốn thứ tiếng, README, CHANGELOG, QUESTIONS.md; phát hành 1.7.0
  - Xác minh: test `every_ui_string_has_translations` (launcher) và `every_key_has_japanese`, `every_key_has_french` (máy chủ); `scripts/check-docs-i18n.py`; README mục 6.14, 6.15; CHANGELOG 1.7.0; QUESTIONS.md 91–93; CI và release.yml của tag `v1.7.0`.
