# Tasks

## 1. Máy chủ

- [ ] 1.1 `FECO_MOBILE_CLIENT_ID`, client di động trong API dành cho launcher và màn hình đồng ý, `GET /v1/library`, client di động trong `scripts/dev-launcher.sh setup`; xác minh bằng E2E scenario "Ví từ điện thoại", "Mã khởi chạy từ điện thoại", "Thư viện có app web"

## 2. Ứng dụng Flutter

- [ ] 2.1 Dự án `clients/feco-mobile` (Android 10+, iOS 18+, `vn.feco.launcher`, logo), cấu hình `--dart-define`, chế độ `FECO_DEMO`; xác minh bằng `flutter analyze` và dựng trên CI
- [ ] 2.2 Đăng nhập AppAuth, lưu token, làm mới khi 401; xác minh bằng unit test scenario "Token hết hạn" và kiểm tay scenario "Đăng nhập lần đầu"
- [ ] 2.3 Năm tab: Ứng dụng của tôi, Cửa hàng, Ví (mua app), Thông báo, Tài khoản; bốn thứ tiếng; xác minh bằng widget test scenario "Chưa đăng nhập", "Mở app web", "App desktop", "Không đủ F-Coin", "Thêm app web", "Đổi ngôn ngữ"

## 3. CI và phát hành

- [ ] 3.1 Job CI dựng và test Android (emulator API 29, 36) và iOS (Simulator); release.yml đính kèm APK và gói iOS Simulator; xác minh bằng scenario "Android thấp nhất và mới nhất", "iOS"
- [ ] 3.2 README, CHANGELOG, QUESTIONS.md; phát hành 1.10.0
