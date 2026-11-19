# Spec Delta

## Purpose

Bổ sung cho capability `account-identity`: cách bật nhà cung cấp đăng nhập ngoài, và cách F-Eco nhận biết refresh token bị dùng lại.

## ADDED Requirements

### Requirement: Nhà cung cấp đăng nhập ngoài
Nền tảng SHALL hiện nút đăng nhập và đăng ký cho mỗi nhà cung cấp được khai báo khi triển khai (Google, GitHub), bằng bốn thứ tiếng. Client ID và secret MUST không nằm trong mã nguồn. Nền tảng MUST chỉ nhận email mà nhà cung cấp đã xác minh. Email đó SHALL được tính là đã xác minh trên F-Eco. Đăng nhập bằng nhà cung cấp ngoài MUST được ghi nhật ký kiểm toán và được kiểm tra thiết bị mới như đăng nhập bằng mật khẩu.

#### Scenario: Nút đăng nhập Google
- **GIVEN** nhà cung cấp Google đã được khai báo
- **WHEN** người dùng mở trang đăng nhập
- **THEN** trang có nút "Đăng nhập bằng Google"

#### Scenario: Email chưa xác minh ở nhà cung cấp
- **WHEN** nhà cung cấp trả về một email chưa được xác minh
- **THEN** nền tảng không tạo tài khoản với email đó và yêu cầu người dùng nhập email để xác minh

#### Scenario: Nhật ký đăng nhập bằng Google
- **WHEN** người dùng đăng nhập bằng Google
- **THEN** nhật ký kiểm toán có một mục `auth.login` với phương thức `oidc`

### Requirement: Nhận biết refresh token bị dùng lại
Nền tảng SHALL nhận yêu cầu đổi token tại điểm nhận token của chính F-Eco, được công bố trong tài liệu discovery của OpenID Connect. Nền tảng MUST chỉ lưu băm của refresh token. Khi một refresh token đã được cấp, chưa hết hạn và chưa bị thu hồi chủ động lại bị từ chối, nền tảng SHALL gửi đúng một cảnh báo bảo mật cho mỗi chuỗi phiên (email và thông báo). Cảnh báo ghi tên app, thời điểm, thiết bị và IP. Nền tảng MUST không đăng xuất các thiết bị khác của người dùng.

#### Scenario: Dùng lại token đã xoay vòng
- **GIVEN** F-Launcher đã đổi refresh token A lấy refresh token B
- **WHEN** refresh token A được gửi lại
- **THEN** yêu cầu bị từ chối, refresh token B hết hiệu lực, và người dùng nhận một email cảnh báo

#### Scenario: Gửi lại lần nữa
- **GIVEN** đã có cảnh báo cho chuỗi phiên đó
- **WHEN** refresh token A được gửi lại thêm lần nữa
- **THEN** yêu cầu bị từ chối và không có email thứ hai

#### Scenario: Gỡ quyền app không gây cảnh báo
- **GIVEN** người dùng đã gỡ quyền của app ở "Ứng dụng đã kết nối"
- **WHEN** app gửi refresh token cũ
- **THEN** yêu cầu bị từ chối và không có cảnh báo
