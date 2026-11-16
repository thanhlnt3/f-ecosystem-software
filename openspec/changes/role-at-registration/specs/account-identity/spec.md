# Spec Delta

## Purpose

Bổ sung cho capability `account-identity`: chọn vai trò khi đăng ký tài khoản.

## ADDED Requirements

### Requirement: Chọn vai trò khi đăng ký
Trang đăng ký SHALL có ô "Vai trò" với năm lựa chọn: Người dùng phổ thông (mặc định), Khách hàng cá nhân, Khách hàng doanh nghiệp, Lập trình viên cá nhân, Công ty phát triển phần mềm. Tài khoản mới SHALL luôn bắt đầu là Người dùng phổ thông. Chọn vai trò khác thì F-Eco SHALL tạo yêu cầu vai trò đang chờ quản trị viên duyệt, ghi nguồn "Chọn khi đăng ký".

#### Scenario: Đăng ký làm Khách hàng doanh nghiệp
- **WHEN** Mai đăng ký với vai trò Khách hàng doanh nghiệp, tên pháp lý "Công ty TNHH Hoa Mai", mã số thuế 0312345678
- **THEN** tài khoản của Mai là Người dùng phổ thông, có một yêu cầu vai trò Khách hàng doanh nghiệp đang chờ với đúng tên pháp lý và mã số thuế, và quản trị viên nhận cảnh báo

#### Scenario: Đăng ký làm Người dùng phổ thông
- **WHEN** Nam đăng ký và giữ vai trò mặc định
- **THEN** tài khoản được tạo và không có yêu cầu vai trò nào

### Requirement: Thông tin tổ chức khi đăng ký
Vai trò Khách hàng doanh nghiệp và Công ty phát triển phần mềm MUST có tên pháp lý và mã số thuế hợp lệ (10 số, hoặc 10 số và `-` 3 số). Thiếu hoặc sai thì F-Eco MUST không tạo tài khoản và SHALL hiện lại trang đăng ký với lỗi dưới ô sai, giữ các giá trị đã nhập trừ mật khẩu. Vai trò không có trong danh sách MUST bị từ chối.

#### Scenario: Công ty thiếu mã số thuế
- **WHEN** người dùng đăng ký với vai trò Công ty phát triển phần mềm, có tên pháp lý nhưng để trống mã số thuế
- **THEN** trang đăng ký hiện lại với lỗi ở ô Mã số thuế, giữ email và tên pháp lý, và không có tài khoản nào được tạo

#### Scenario: Vai trò lạ
- **WHEN** biểu mẫu đăng ký gửi `feco_role=admin`
- **THEN** F-Eco báo lỗi và không tạo tài khoản

### Requirement: Trang sau khi đăng ký không lộ email đã dùng
Khi chọn vai trò cần duyệt, trang "Đã gửi email xác minh" SHALL nói thêm rằng vai trò đó cần quản trị viên duyệt. Trang MUST giống hệt nhau dù email mới hay đã có tài khoản. Email đã có tài khoản MUST không tạo yêu cầu vai trò mới.

#### Scenario: Email đã có tài khoản
- **GIVEN** Mai đã có tài khoản
- **WHEN** ai đó đăng ký lại bằng email của Mai với vai trò Lập trình viên cá nhân
- **THEN** trang hiện ra giống hệt trang của một đăng ký mới với cùng vai trò, và không có yêu cầu vai trò mới cho Mai
