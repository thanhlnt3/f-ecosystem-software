# F-Launcher di động

Ứng dụng đồng hành của F-Eco trên điện thoại (thay đổi OpenSpec `mobile-launcher`):
- Android 10 (API 29) trở lên, nhắm Android 16 (API 36);
- iOS 18 trở lên.

Ứng dụng gồm năm tab:
- **Ứng dụng của tôi**: app đã thêm hoặc đã mua, đồng bộ với web và F-Launcher trên máy tính;
- **Cửa hàng**: F-Store; app web mở bằng trình duyệt, app desktop ghi "Dùng trên máy tính qua F-Launcher";
- **Ví**: số dư F-Coin, mua app, mở trang nạp và rút trên web;
- **Thông báo**;
- **Tài khoản**: tên, vai trò, ngôn ngữ (vi, en, fr, ja), đăng xuất.

Đăng nhập bằng tài khoản F-Eco qua trình duyệt hệ thống (OIDC, PKCE, thư viện AppAuth). Refresh token lưu trong Keychain (iOS) hoặc Keystore (Android).

## Dựng và chạy

Cần Flutter 3.50 (Dart 3.14). Dựng cho Android cần Android SDK; dựng cho iOS cần macOS và Xcode.

```bash
flutter pub get
flutter analyze
flutter test
```

Chạy với dữ liệu mẫu (không cần máy chủ):

```bash
flutter run --dart-define=FECO_DEMO=true
```

Chạy với F-Eco dev trên Android emulator (máy dev là `10.0.2.2`). Client ID lấy từ `scripts/dev-launcher.sh setup` (tệp `.dev/mobile-client-id`):

```bash
flutter run --dart-define=FECO_CLIENT_ID=<client id> --dart-define=FECO_SERVER=http://10.0.2.2:8080/ --dart-define=FECO_ISSUER=http://10.0.2.2:4444/
```

Máy chủ F-Eco phải chạy với `FECO_MOBILE_CLIENT_ID=<client id>`.

## Kiểm thử

- **Unit test và widget test**: `flutter test`.
- **Integration test**: `flutter test integration_test --dart-define=FECO_DEMO=true`, cần emulator hoặc Simulator. CI chạy trên Android emulator API 29 và API 36, và trên iOS Simulator.

## Phát hành

Bản phát hành trên GitHub kèm hai gói:
- `feco-mobile-<phiên bản>.apk`: ký bằng khóa debug, chỉ để cài thử;
- `feco-mobile-<phiên bản>-ios-simulator.zip`: chạy trên iOS Simulator.

Đưa lên Google Play và App Store cần tài khoản nhà phát triển, khóa ký và provisioning profile (QUESTIONS.md).
