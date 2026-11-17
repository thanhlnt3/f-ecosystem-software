# Design

## M1. Vì sao là ứng dụng đồng hành

- **iOS**: chỉ cài app qua App Store. App không tải được tệp chạy khác, cũng không chạy được tệp đó.
- **Android**: app tự cài APK khác cần quyền `REQUEST_INSTALL_PACKAGES`, quyền này Google Play hạn chế. Hơn nữa, các app trong danh mục F-Eco hiện là app desktop.
- **Kết luận**: bản di động không có kho TUF, không cài và không chạy app con. Bản di động làm những việc không cần cài app:
  - tài khoản và đăng nhập;
  - danh mục;
  - mở app web;
  - ví và mua app;
  - "Ứng dụng của tôi" (đồng bộ với máy chủ);
  - thông báo.

Khi danh mục có app Android hay iOS thật, bản di động sẽ mở trang của app trên Google Play hoặc App Store (ngoài phạm vi thay đổi này).

## M2. Công nghệ

| Phần | Lựa chọn |
|---|---|
| Khung giao diện | Flutter 3.50 (Dart 3.14), Material 3 |
| Đăng nhập | `flutter_appauth` (AppAuth-Android, AppAuth-iOS): authorization code + PKCE, mở trình duyệt hệ thống (Custom Tabs, ASWebAuthenticationSession), không có ô mật khẩu trong app |
| Lưu token | `flutter_secure_storage` (Keychain, Android Keystore). Chỉ lưu refresh token; access token giữ trong bộ nhớ |
| Gọi API | `http`, header `Authorization: Bearer`. Gặp 401 thì refresh một lần rồi gọi lại |
| Mở liên kết | `url_launcher`, chế độ ứng dụng ngoài (trình duyệt) |
| Ngôn ngữ | bảng chuỗi trong mã (vi, en, fr, ja), như launcher desktop; test bảo đảm đủ bốn thứ tiếng |

**Phiên bản hệ điều hành**:
- Android: `minSdk 29` (Android 10), `targetSdk` và `compileSdk 36` (Android 16, Google Play yêu cầu).
- iOS: `IPHONEOS_DEPLOYMENT_TARGET 18.0`.

**Định danh**:
- gói và bundle id: `vn.feco.launcher`;
- địa chỉ chuyển hướng OAuth: `vn.feco.launcher:/oauth2redirect`.

## M3. Cấu hình

Địa chỉ máy chủ được đặt lúc dựng (`--dart-define`), không cấu hình trong app:

| Biến | Mặc định | Ghi chú |
|---|---|---|
| `FECO_SERVER` | `http://10.0.2.2:8080/` | Máy chủ F-Eco. `10.0.2.2` là máy chủ của Android emulator. Bản phát hành phải là HTTPS |
| `FECO_ISSUER` | `http://10.0.2.2:4444/` | Hydra |
| `FECO_CLIENT_ID` | `feco-mobile` | Client OAuth của app di động |
| `FECO_DEMO` | `false` | `true`: dữ liệu mẫu, không gọi mạng. Dùng cho integration test trên emulator và Simulator và để xem giao diện |

Cho phép HTTP chỉ với `10.0.2.2`, `localhost`, `127.0.0.1`:
- Android: `network_security_config`;
- iOS: `NSAllowsLocalNetworking`.

## M4. Máy chủ

- **`FECO_MOBILE_CLIENT_ID`** (tùy chọn): client công khai, PKCE.
  - `launch::launcher_subject` nhận token của cả F-Launcher desktop lẫn app di động.
  - Màn hình đồng ý bỏ qua cho cả hai, vì đều là app bên thứ nhất.
  - `POST /v1/launch-codes` vẫn chỉ cho F-Launcher desktop.
- **`GET /v1/library`** (token launcher hoặc di động) trả từng mục:
  - `id`, `name`, `has_icon`, `installed_version`, `purchased`, `latest`, `web_url`;
  - `update_available`: có bản mới.
- **"Thêm" app web trên điện thoại**: gọi `POST /v1/catalog/installs` với phiên bản mới nhất, như launcher desktop báo lượt cài. Nhờ đó app web xuất hiện trong "Ứng dụng của tôi" trên web, desktop và điện thoại.

## M5. Kiểm thử

- **Trên máy dev** (Windows, đã có Flutter SDK): `flutter analyze`, unit test và widget test chạy với API giả:
  - phân tích JSON danh mục, ví, thư viện;
  - `validWebUrl`;
  - tính số F-Coin còn thiếu;
  - đủ chuỗi bốn thứ tiếng;
  - năm tab hiện đúng;
  - mua app khi thiếu F-Coin.
- **Trên CI**:
  - Ubuntu: dựng APK, chạy `integration_test` trên Android emulator API 29 và API 36 (chế độ `FECO_DEMO`);
  - macOS: dựng cho iOS Simulator và chạy `integration_test` trên Simulator mới nhất của runner. Mục tiêu dựng iOS 18 bảo đảm app chạy được từ iOS 18.
- **Máy chủ**: E2E Rust kiểm token client di động gọi `/v1/wallet`, `/v1/library`, `/v1/catalog/installs`, và bị từ chối ở `/v1/launch-codes`.

Đăng nhập thật qua trình duyệt trên thiết bị không tự động hóa được trong CI. Hướng dẫn kiểm tay ở README; đăng nhập đã được E2E máy chủ kiểm bằng cùng luồng PKCE.

## M6. Phát hành

- **Android**: `feco-mobile-<phiên bản>.apk`, ký bằng khóa debug của Flutter, chỉ để cài thử. Bản cho Google Play cần khóa tải lên và tài khoản Google Play Console của chủ dự án.
- **iOS**: `feco-mobile-<phiên bản>-ios-simulator.zip` chạy trên Simulator. Bản cho iPhone và App Store cần tài khoản Apple Developer, chứng thư và provisioning profile.
