# Spec Delta

## Purpose

Bổ sung cho capability `account-identity`: vai trò nền tảng của tài khoản và cách cấp, thu hồi vai trò.

## ADDED Requirements

### Requirement: Vai trò nền tảng
Mỗi tài khoản F-Eco SHALL có đúng một vai trò nền tảng:
- **Người sử dụng ứng dụng** (mặc định): đăng nhập, cài và dùng app, đặt đơn xây app;
- **Lập trình viên**: có thêm quyền vào Cổng nhà phát triển, tạo app, nộp và phát hành phiên bản, dùng `f-dev`;
- **Quản trị viên**: có thêm quyền vào bảng quản trị theo vai trò con, và mọi quyền của Lập trình viên.

#### Scenario: Nhà phát triển có từ trước
- **GIVEN** một người đã là thành viên tài khoản nhà phát triển trước khi có phân quyền
- **WHEN** máy chủ nâng cấp
- **THEN** người đó có vai trò Lập trình viên và vẫn vào được Cổng nhà phát triển

### Requirement: Chặn theo vai trò ở Cổng nhà phát triển
Người chưa là Lập trình viên hoặc Quản trị viên MUST không tạo được tài khoản nhà phát triển, app hay phiên bản. API `f-dev` MUST kiểm vai trò của chủ token ở mỗi lần gọi.

#### Scenario: Người dùng mở Cổng nhà phát triển
- **GIVEN** tài khoản chỉ là Người sử dụng ứng dụng
- **WHEN** người dùng mở `/developer`
- **THEN** trang báo "Cần quyền Lập trình viên" kèm nút gửi yêu cầu, không hiện biểu mẫu đăng ký tài khoản nhà phát triển

#### Scenario: Thu hồi quyền khi đang có token
- **GIVEN** một Lập trình viên đã tạo token `f-dev`
- **WHEN** quản trị viên đổi tài khoản đó về Người sử dụng ứng dụng
- **THEN** lần gọi API tiếp theo bằng token đó trả `403` với mã `developer_role_required`

### Requirement: Phân quyền người dùng
Quản trị viên có vai trò con Quản trị hệ thống SHALL đổi được vai trò nền tảng của người dùng trong bảng quản trị, tìm người dùng theo email. Mỗi lần đổi MUST:
- có lý do;
- được ghi vào nhật ký kiểm toán kèm vai trò cũ và mới.

Đặt Quản trị viên MUST chọn ít nhất một vai trò con. Quản trị viên MUST không tự đổi vai trò của chính mình, và MUST không bỏ được Quản trị hệ thống cuối cùng.

#### Scenario: Cấp quyền Lập trình viên
- **WHEN** Quản trị hệ thống đổi vai trò của "lan@example.com" sang Lập trình viên với lý do "Đối tác phát triển app kho"
- **THEN** Lan vào được Cổng nhà phát triển, và nhật ký kiểm toán có dòng `role.set` từ Người sử dụng ứng dụng sang Lập trình viên kèm lý do

#### Scenario: Thiếu lý do
- **WHEN** quản trị viên đổi vai trò mà không nhập lý do
- **THEN** thao tác bị từ chối và vai trò không đổi

#### Scenario: Tự hạ quyền
- **WHEN** Quản trị hệ thống đổi vai trò của chính mình sang Người sử dụng ứng dụng
- **THEN** thao tác bị từ chối với thông báo không được tự đổi vai trò của mình

### Requirement: Yêu cầu quyền Lập trình viên
Người sử dụng ứng dụng SHALL gửi được yêu cầu quyền Lập trình viên từ Cổng nhà phát triển, kèm ghi chú. Mỗi người chỉ có một yêu cầu đang chờ. Quản trị viên MUST nhận cảnh báo khi có yêu cầu mới. Khi yêu cầu được duyệt hay từ chối, người gửi MUST nhận thông báo.

#### Scenario: Gửi và được duyệt
- **GIVEN** Minh là Người sử dụng ứng dụng
- **WHEN** Minh gửi yêu cầu với ghi chú "Tôi muốn phát hành app ghi chú", rồi Quản trị hệ thống duyệt
- **THEN** Minh trở thành Lập trình viên, nhận thông báo "Đã được cấp quyền Lập trình viên", và lần sau mở `/developer` thấy biểu mẫu đăng ký tài khoản nhà phát triển

#### Scenario: Gửi trùng
- **GIVEN** Minh đang có một yêu cầu chờ duyệt
- **WHEN** Minh gửi yêu cầu lần nữa
- **THEN** trang báo yêu cầu đang chờ duyệt, không tạo yêu cầu thứ hai
