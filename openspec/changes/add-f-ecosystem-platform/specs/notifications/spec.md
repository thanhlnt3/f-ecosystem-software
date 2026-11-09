# Spec Delta

## Purpose

Gửi cho người dùng và nhà phát triển những thông báo cần thiết (bảo mật, đơn hàng, thanh toán, cập nhật app) qua email và trong launcher, tôn trọng tùy chọn nhận thông báo của họ.

## ADDED Requirements

### Requirement: Kênh thông báo
Nền tảng SHALL gửi thông báo qua email và qua trung tâm thông báo trong F-Launcher và cổng web. Mỗi thông báo MUST có tiêu đề, nội dung theo ngôn ngữ người dùng chọn, và liên kết tới đúng màn hình liên quan.

#### Scenario: Báo giá mới
- **WHEN** đội F-Eco gửi báo giá cho một đơn
- **THEN** khách hàng nhận email và thông báo trong launcher, bấm vào thì mở đúng trang báo giá

### Requirement: Thông báo bắt buộc và tùy chọn
Thông báo bảo mật, thanh toán, hóa đơn, thay đổi trạng thái đơn hàng và thay đổi điều khoản SHALL luôn được gửi. Thông báo về cập nhật app, gợi ý app mới và bản tin MUST tắt được theo từng loại, và bản tin MUST mặc định là tắt.

#### Scenario: Tắt thông báo cập nhật
- **WHEN** người dùng tắt "Thông báo khi app có bản cập nhật"
- **THEN** người dùng không nhận email về cập nhật app nữa, nhưng vẫn nhận email bảo mật

#### Scenario: Hủy đăng ký từ email
- **WHEN** người dùng bấm "Hủy đăng ký" trong một email bản tin
- **THEN** bản tin bị tắt mà không cần đăng nhập

### Requirement: Không lộ thông tin nhạy cảm qua thông báo
Thông báo SHALL không chứa mật khẩu, mã xác thực đầy đủ, số thẻ, hay nội dung trao đổi trong đơn hàng. Email MUST gửi từ tên miền có SPF, DKIM và DMARC.

#### Scenario: Tin nhắn mới trong đơn
- **WHEN** đội F-Eco gửi tin nhắn trong đơn hàng
- **THEN** email chỉ báo "Bạn có tin nhắn mới trong đơn #1024" kèm liên kết, không chép nội dung tin nhắn
