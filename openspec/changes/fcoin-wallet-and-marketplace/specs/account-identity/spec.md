# Spec Delta

## Purpose

Bổ sung cho capability `account-identity`: sáu vai trò nền tảng thay ba vai trò của thay đổi platform-roles.
- Requirement "Vai trò nền tảng" của platform-roles được thay bằng "Sáu vai trò nền tảng" dưới đây.
- Các requirement về phân quyền và chặn theo vai trò vẫn giữ, và áp dụng cho sáu vai trò.

## ADDED Requirements

### Requirement: Sáu vai trò nền tảng
Mỗi tài khoản SHALL có đúng một trong sáu vai trò nền tảng: Người dùng phổ thông (mặc định), Khách hàng cá nhân, Khách hàng doanh nghiệp, Lập trình viên cá nhân, Công ty phát triển phần mềm, Quản trị viên. Lập trình viên có từ trước MUST trở thành Lập trình viên cá nhân, không mất quyền nào.

#### Scenario: Lập trình viên có từ trước
- **GIVEN** Lan là Lập trình viên trước bản 1.6.0
- **WHEN** máy chủ nâng cấp lên 1.6.0
- **THEN** vai trò của Lan là "Lập trình viên cá nhân" và Lan vẫn vào được Cổng nhà phát triển

### Requirement: Quyền theo vai trò
Mọi vai trò SHALL có ví F-Coin và mua được app. Thêm vào đó:
- Khách hàng (cá nhân, doanh nghiệp): gửi yêu cầu làm phần mềm, thương lượng, ký quỹ, trả tiền.
- Lập trình viên cá nhân, Công ty: Cổng nhà phát triển, nhận yêu cầu làm phần mềm.
- Doanh nghiệp, Công ty: lập và ký hợp đồng.
- Quản trị viên: bảng quản trị, Cổng nhà phát triển.

#### Scenario: Công ty vào Cổng nhà phát triển
- **GIVEN** tài khoản có vai trò Công ty phát triển phần mềm
- **WHEN** mở `/developer`
- **THEN** thấy biểu mẫu đăng ký tài khoản nhà phát triển như Lập trình viên cá nhân

### Requirement: Xin vai trò
Người dùng SHALL xin được một trong bốn vai trò ở trang Vai trò (`/account/role`), kèm ghi chú: Khách hàng cá nhân, Khách hàng doanh nghiệp, Lập trình viên cá nhân, Công ty phát triển phần mềm.
- Vai trò doanh nghiệp và công ty MUST khai tên pháp lý và mã số thuế hợp lệ (10 hoặc 13 số).
- Mỗi người chỉ có một yêu cầu đang chờ.
- Quản trị hệ thống duyệt hoặc từ chối trong bảng quản trị; người gửi nhận thông báo.

#### Scenario: Xin vai trò Khách hàng doanh nghiệp
- **WHEN** Minh xin vai trò Khách hàng doanh nghiệp với tên "Công ty TNHH Minh An" và mã số thuế "0101234567", rồi quản trị viên duyệt
- **THEN** Minh có vai trò Khách hàng doanh nghiệp và nhận thông báo "Đã được cấp vai trò Khách hàng doanh nghiệp"

#### Scenario: Thiếu mã số thuế
- **WHEN** Minh xin vai trò Công ty phát triển phần mềm mà không nhập mã số thuế
- **THEN** yêu cầu bị từ chối với thông báo cần mã số thuế
