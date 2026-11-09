# Spec Delta

## Purpose

Bảng quản trị cho đội vận hành F-Eco: duyệt app, xử lý báo cáo, quản lý đơn xây app, hoàn tiền, hỗ trợ người dùng, với phân quyền chặt và mọi thao tác được ghi nhật ký.

## ADDED Requirements

### Requirement: Vai trò quản trị
Bảng quản trị SHALL có các vai trò: Người duyệt app, Hỗ trợ, Quản lý dự án (đơn xây app), Tài chính, Quản trị hệ thống. Mỗi vai trò MUST chỉ thấy và làm được những gì thuộc phạm vi của nó. Truy cập bảng quản trị MUST cần xác thực hai lớp bằng passkey hoặc khóa bảo mật phần cứng.

#### Scenario: Hỗ trợ cố hoàn tiền
- **WHEN** tài khoản có vai trò Hỗ trợ mở chức năng hoàn tiền
- **THEN** chức năng bị ẩn, và lời gọi API trực tiếp bị từ chối với mã 403

### Requirement: Hàng chờ duyệt app
Người duyệt SHALL thấy hàng chờ phiên bản cần duyệt thủ công, sắp theo thời gian chờ, kèm kết quả kiểm tra tự động, khác biệt về scope và quyền so với phiên bản trước, và công cụ chạy thử app trong máy ảo cô lập. Quyết định duyệt hay từ chối MUST kèm lý do theo điều khoản chính sách.

#### Scenario: Xem khác biệt quyền
- **WHEN** người duyệt mở SoundLab 1.3.0
- **THEN** màn hình hiện rõ "Thêm scope: phone (mục đích: gửi mã xác nhận)" so với 1.2.0

### Requirement: Quản lý đơn xây app
Quản lý dự án SHALL tạo và gửi báo giá, cập nhật trạng thái mốc, bàn giao bản build, trao đổi với khách hàng, và thấy các đơn sắp trễ hạn. Đơn còn dưới 2 ngày tới hạn mốc mà chưa bàn giao MUST được đánh dấu cảnh báo.

#### Scenario: Đơn sắp trễ
- **GIVEN** mốc 2 của một đơn còn 1 ngày tới hạn và chưa bàn giao
- **WHEN** quản lý dự án mở danh sách đơn
- **THEN** đơn đó được đánh dấu màu cảnh báo, đứng đầu danh sách

### Requirement: Hỗ trợ người dùng
Hỗ trợ SHALL tìm người dùng theo email hoặc mã đơn, xem thông tin tài khoản đã che bớt (email che một phần, không thấy mật khẩu hay token), gửi lại email xác minh, và đăng xuất mọi phiên của người dùng theo yêu cầu. Mọi lần xem hồ sơ người dùng MUST được ghi vào nhật ký kiểm toán kèm lý do.

#### Scenario: Xem hồ sơ
- **WHEN** nhân viên Hỗ trợ mở hồ sơ một người dùng
- **THEN** nền tảng yêu cầu nhập lý do (ví dụ mã phiếu hỗ trợ) và ghi lần xem vào nhật ký kiểm toán

### Requirement: Không giả danh người dùng
Bảng quản trị SHALL không có chức năng đăng nhập thay người dùng hay xem dữ liệu bên trong app con của bên thứ ba.

#### Scenario: Tìm chức năng giả danh
- **WHEN** kiểm tra mọi API của bảng quản trị
- **THEN** không có API nào cấp token đứng tên một người dùng cho quản trị viên
