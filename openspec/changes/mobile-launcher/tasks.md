# Tasks

## 1. Máy chủ

- [x] 1.1 `FECO_MOBILE_CLIENT_ID`, client di động trong API dành cho launcher và màn hình đồng ý, `GET /v1/library`, client di động trong `scripts/dev-launcher.sh setup`; xác minh bằng E2E scenario "Ví từ điện thoại", "Mã khởi chạy từ điện thoại", "Thư viện có app web"
  - Xác minh: E2E `mobile_client_uses_launcher_apis`:
    - client di động không hỏi đồng ý, nhận refresh token;
    - `/v1/wallet`, `/v1/notifications`, `/v1/account/role` trả 200;
    - "Thêm" app web qua `/v1/catalog/installs` thì `/v1/library` có app kèm `web_url`;
    - `/v1/launch-codes` trả 403 `launcher_only`; token sai bị 401.

## 2. Ứng dụng Flutter

- [x] 2.1 Dự án `clients/feco-mobile` (Android 10+, iOS 18+, `vn.feco.launcher`, logo), cấu hình `--dart-define`, chế độ `FECO_DEMO`; xác minh bằng `flutter analyze` và dựng trên CI
  - Xác minh: `flutter analyze` không có vấn đề. CI dựng được APK release (`minSdk 29`, `targetSdk 36`) và bản iOS Simulator (mục tiêu 18.0). Giấy phép của 85 gói Dart: BSD-3-Clause, Apache-2.0, MIT và gói của Flutter SDK.
- [x] 2.2 Đăng nhập AppAuth, lưu token, làm mới khi 401; xác minh bằng unit test scenario "Token hết hạn" và kiểm tay scenario "Đăng nhập lần đầu"
  - Xác minh:
    - unit test `scenario_token_het_han`: làm mới bằng refresh token đã lưu, lưu refresh token xoay vòng, làm mới thất bại thì về chưa đăng nhập và xóa token;
    - `api_retries_once_after_401`: gặp 401 thì làm mới một lần rồi gọi lại;
    - đăng nhập qua trình duyệt thật chưa kiểm tay trên điện thoại, vì máy dev không có emulator (chủ dự án chưa cho tải). Phía máy chủ đã kiểm luồng PKCE của client di động bằng E2E.
- [x] 2.3 Năm tab: Ứng dụng của tôi, Cửa hàng, Ví (mua app), Thông báo, Tài khoản; bốn thứ tiếng; xác minh bằng widget test scenario "Chưa đăng nhập", "Mở app web", "App desktop", "Không đủ F-Coin", "Thêm app web", "Đổi ngôn ngữ"
  - Xác minh: widget test `scenario_chua_dang_nhap`, `scenario_mo_app_web_va_app_desktop`, `scenario_khong_du_f_coin` ("Còn thiếu 39 F-Coin", nút Nạp mở `/wallet`; đủ F-Coin thì mua, số dư còn 51), `scenario_them_app_web`, `scenario_doi_ngon_ngu`; unit test `every_key_has_four_languages`, `web_urls_are_https_or_dev_hosts`, `json_from_the_server`, `purchase_errors_are_reported`.

## 3. CI và phát hành

- [x] 3.1 Job CI dựng và test Android (emulator API 29, 36) và iOS (Simulator); release.yml đính kèm APK và gói iOS Simulator; xác minh bằng scenario "Android thấp nhất và mới nhất", "iOS"
  - Xác minh: CI chạy 44025591488. Integration test `app_runs_on_device` qua trên:
    - Android emulator API 29 (Android 10) và API 36 (Android 16);
    - iPhone 17 Pro Simulator iOS 26.5.

    Runner macOS không có Simulator iOS 18, nên iOS 18 được bảo đảm bằng mục tiêu dựng 18.0. Job `mobile` của release.yml đính kèm APK và gói Simulator.
- [x] 3.2 README, CHANGELOG, QUESTIONS.md; phát hành 1.10.0
  - Xác minh: `scripts/check-docs-i18n.py`; README mục 6.16, bảng cấu hình và API; `clients/feco-mobile/README.md`; CHANGELOG 1.10.0; QUESTIONS.md 101–103; release.yml của tag `v1.10.0`.
