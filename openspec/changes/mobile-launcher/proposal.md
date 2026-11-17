# Proposal

## Why

Chủ dự án muốn có F-Launcher trên điện thoại, cho Android và iOS, hỗ trợ các phiên bản đang có nhiều người dùng nhất.

Số liệu tháng 10–11/2026:
- **Android**: Android 16 chiếm nhiều nhất (khoảng 25%). Android 11 tới 15 mỗi bản khoảng 10–15%. Android 10 trở về trước mỗi bản dưới 6%.
- **iOS**: iOS 26 khoảng 55%, iOS 27 khoảng 36%, iOS 18 khoảng 7%.

Chủ dự án chọn:
- công nghệ Flutter;
- hỗ trợ Android 10 trở lên và iOS 18 trở lên;
- dựng iOS trên GitHub Actions; chưa ký và chưa đưa lên App Store vì chưa có tài khoản Apple Developer.

Giới hạn của nền tảng di động:
- iOS không cho cài app ngoài App Store.
- Google Play hạn chế app tự cài app khác.
- Các app F-Eco hiện là app desktop (Windows, macOS, Linux) hoặc app web.

Vì vậy F-Launcher di động là **ứng dụng đồng hành**: đăng nhập F-Eco, xem F-Store, mở app web, ví F-Coin, mua app, xem "Ứng dụng của tôi" và thông báo. Bản di động không cài app desktop.

## What Changes

- **Ứng dụng mới `clients/feco-mobile`** (Flutter 3.50, Dart 3.14), gói `vn.feco.launcher`:
  - **Đăng nhập**: dùng tài khoản F-Eco theo OIDC (authorization code + PKCE qua trình duyệt hệ thống) bằng thư viện AppAuth. Refresh token lưu trong Keychain (iOS) hoặc Keystore (Android).
  - **Năm tab**:
    - Ứng dụng của tôi: app đã thêm hoặc đã mua, báo bản mới; app web mở bằng trình duyệt;
    - Cửa hàng: tìm, lọc nhóm, logo, giá F-Coin. App web có nút "Thêm" và "Mở"; app desktop ghi "Dùng trên máy tính qua F-Launcher";
    - Ví: số dư, mua app có giá (xác nhận, báo số còn thiếu), mở trang nạp và rút trên web;
    - Thông báo;
    - Tài khoản: tên, vai trò, ngôn ngữ, đăng xuất, lối tới F-Eco trên web.
  - **Ngôn ngữ**: tiếng Việt, English, Français, 日本語.
  - **Logo**: dùng logo F-Launcher làm biểu tượng app.
  - **Phiên bản hệ điều hành**: Android 10 (API 29) trở lên, nhắm Android 16 (API 36); iOS 18 trở lên.
- **Máy chủ**:
  - client OAuth của app di động (`FECO_MOBILE_CLIENT_ID`) là app bên thứ nhất như F-Launcher desktop: không hỏi đồng ý, được gọi các API chỉ dành cho launcher (`/v1/wallet`, `/v1/wallet/purchases`, `/v1/catalog/installs`, `/v1/notifications`);
  - API mới `GET /v1/library` ("Ứng dụng của tôi" dạng JSON).
  - Mã khởi chạy vẫn chỉ cấp cho F-Launcher desktop, vì điện thoại không chạy app con.
- **Môi trường dev**: `scripts/dev-launcher.sh setup` tạo thêm client OAuth di động, chuyển hướng về `vn.feco.launcher:/oauth2redirect`.
- **CI và phát hành**:
  - job `mobile`: `flutter analyze`, unit test và widget test;
  - dựng APK, chạy integration test trên Android emulator API 29 và API 36;
  - dựng iOS cho Simulator và chạy integration test trên iOS Simulator;
  - bản phát hành kèm `feco-mobile-<phiên bản>.apk` (ký bằng khóa debug, chỉ để thử) và gói iOS Simulator.

## Capabilities

### New Capabilities

- `mobile-launcher`: F-Launcher trên Android và iOS.

### Modified Capabilities

- `launcher`: API dành cho launcher nhận cả client di động; `GET /v1/library`.

## Impact

- **Thư mục mới**: `clients/feco-mobile` (dự án Flutter, không thuộc Cargo workspace).
- **Thư viện Dart** (license được phép):
  - `flutter_appauth` (BSD-3-Clause, dùng AppAuth Apache-2.0);
  - `flutter_secure_storage` (BSD-3-Clause);
  - `http` (BSD-3-Clause);
  - `url_launcher` (BSD-3-Clause).
- **Máy chủ**: `feco-config` (`FECO_MOBILE_CLIENT_ID`), `launch.rs`, `oauth.rs`, `store_web.rs` (`GET /v1/library`).
- **CI**: `.github/workflows/ci.yml` (job `mobile-android`, `mobile-ios`), `release.yml`.
- **Chưa có**: ký bản phát hành, đưa lên Google Play và App Store. Cần tài khoản nhà phát triển của Google và Apple (QUESTIONS.md).
