# Spec Delta

## Purpose

Bổ sung cho capability `account-identity`: trang hồ sơ của người dùng đã đăng nhập.

## ADDED Requirements

### Requirement: Trang hồ sơ
Trang chủ của người đã đăng nhập SHALL là trang hồ sơ. Phần đầu SHALL có ảnh đại diện chữ cái, tên, email kèm trạng thái xác minh, vai trò nền tảng, ngày tham gia, nút Chỉnh sửa hồ sơ và Đăng xuất. Dưới đó SHALL có thẻ số liệu (ứng dụng đã kết nối, thiết bị đang đăng nhập, đơn hàng, thông báo chưa đọc), mỗi thẻ dẫn tới trang tương ứng. Số liệu không lấy được MUST hiện "—", và trang vẫn mở.

#### Scenario: Người dùng mở trang hồ sơ
- **GIVEN** người dùng đã đăng nhập, đã kết nối một app và chưa có đơn hàng
- **WHEN** người dùng mở `/`
- **THEN** trang hiện email, vai trò "Người sử dụng ứng dụng", thẻ "Ứng dụng đã kết nối" ghi 1 và thẻ "Đơn hàng" ghi 0

### Requirement: Lối tắt theo nhóm trên trang hồ sơ
Trang hồ sơ SHALL có các nhóm lối tắt:
- Bảo mật: mật khẩu và xác thực hai lớp (kèm trạng thái đã bật hay chưa), thiết bị;
- Dữ liệu và quyền riêng tư: ứng dụng đã kết nối, tải dữ liệu, xóa tài khoản;
- Mua sắm và thanh toán;
- Công cụ: thông báo, giao diện.

Lối tắt Cổng nhà phát triển MUST chỉ hiện với Lập trình viên và Quản trị viên; lối tắt Bảng quản trị MUST chỉ hiện với Quản trị viên.

#### Scenario: Lập trình viên thấy lối tắt Cổng nhà phát triển
- **GIVEN** người dùng có vai trò Lập trình viên
- **WHEN** người dùng mở `/`
- **THEN** trang có lối tắt Cổng nhà phát triển và vai trò "Lập trình viên", không có lối tắt Bảng quản trị

#### Scenario: Trạng thái xác thực hai lớp
- **GIVEN** người dùng đã thêm passkey
- **WHEN** người dùng mở `/`
- **THEN** nhóm Bảo mật ghi "Đang bật xác thực hai lớp"
