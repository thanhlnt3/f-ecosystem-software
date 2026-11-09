# Spec Delta

## Purpose

F-SDK và API công khai cho phép app con của bên thứ ba đăng nhập người dùng, đọc dữ liệu người dùng trong phạm vi được đồng ý, kiểm tra quyền sử dụng và nhận sự kiện từ nền tảng.

## ADDED Requirements

### Requirement: Chuẩn mở cho xác thực
API xác thực SHALL theo OAuth 2.0 và OpenID Connect, công bố tài liệu khám phá tại `/.well-known/openid-configuration` và khóa ký tại JWKS. Một app con MUST tích hợp được bằng thư viện OIDC chuẩn bất kỳ, không bắt buộc dùng F-SDK.

#### Scenario: Dùng thư viện OIDC chuẩn
- **WHEN** một app viết bằng Python dùng thư viện OIDC phổ biến trỏ tới địa chỉ khám phá của F-Eco
- **THEN** app đăng nhập được và kiểm tra được chữ ký ID token qua JWKS

### Requirement: F-SDK chính thức
F-Eco SHALL cung cấp F-SDK cho Rust, C (ABI ổn định), TypeScript/Node.js và Python, gói gọn: nhận mã khởi chạy từ launcher, đổi lấy token, tự làm mới token, gọi API dữ liệu người dùng, kiểm tra quyền sử dụng, và báo lỗi khi app gặp sự cố (tùy chọn). F-SDK MUST có license MIT OR Apache-2.0.

#### Scenario: Đăng nhập bằng F-SDK
- **WHEN** app Rust gọi `fsdk::session_from_launcher()` khi được launcher khởi chạy
- **THEN** app nhận một phiên có access token hợp lệ và thông tin `profile` theo scope đã đồng ý

#### Scenario: Chạy ngoài launcher
- **WHEN** người dùng mở app trực tiếp, không qua launcher
- **THEN** F-SDK mở trình duyệt để đăng nhập bằng PKCE, rồi trả về phiên như bình thường

### Requirement: API dữ liệu người dùng
API SHALL cung cấp endpoint `GET /v1/me` trả về chỉ các trường thuộc scope mà token mang. API MUST có phiên bản trong đường dẫn, và một phiên bản API MUST được giữ ít nhất 12 tháng sau khi công bố ngừng hỗ trợ.

#### Scenario: Token chỉ có profile
- **WHEN** app gọi `GET /v1/me` bằng token chỉ có scope `profile`
- **THEN** phản hồi chỉ có `sub`, `name` và `picture`, không có `email`

### Requirement: Kiểm tra quyền sử dụng
API SHALL có endpoint `GET /v1/entitlements` cho app biết người dùng hiện tại có quyền gì với chính app đó (ví dụ app được bàn giao từ đơn xây app theo yêu cầu). App MUST không xem được quyền của người dùng ở app khác.

#### Scenario: App được xây theo yêu cầu
- **GIVEN** app "QuanLyKho" được bàn giao cho khách hàng qua đơn xây app theo yêu cầu
- **WHEN** QuanLyKho gọi `GET /v1/entitlements` cho khách hàng đó
- **THEN** phản hồi có quyền `owner` với QuanLyKho

### Requirement: Sự kiện cho app
Nền tảng SHALL gửi webhook đã ký cho app các sự kiện `user.revoked`, `user.deleted`, `app.version.revoked` và `entitlement.changed`. Webhook MUST được gửi lại theo backoff tăng dần trong 72 giờ khi app không trả mã 2xx.

#### Scenario: Máy chủ app tạm lỗi
- **WHEN** máy chủ của app trả mã 503 cho webhook `user.revoked`
- **THEN** nền tảng gửi lại sau 1, 5, 30 phút rồi giãn dần, tối đa trong 72 giờ

### Requirement: Mã lỗi thống nhất
Mọi lỗi API SHALL trả JSON gồm `error` (mã lỗi máy đọc được), `message` (tiếng Anh), `message_vi` và `request_id`. Tài liệu MUST liệt kê mọi mã lỗi kèm cách xử lý.

#### Scenario: Token hết hạn
- **WHEN** app gọi API bằng access token đã hết hạn
- **THEN** API trả mã 401 với `error: "invalid_token"` và một `request_id`
