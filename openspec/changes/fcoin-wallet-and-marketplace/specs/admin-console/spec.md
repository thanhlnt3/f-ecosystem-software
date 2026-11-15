# Spec Delta

## Purpose

Bổ sung cho capability `admin-console`: trang Ví F-Coin và trang Dự án trong bảng quản trị, và việc duyệt yêu cầu vai trò.

## ADDED Requirements

### Requirement: Trang Ví F-Coin
Bảng quản trị SHALL có trang Ví F-Coin cho vai trò con Tài chính và Quản trị hệ thống. Trang có:
- tổng F-Coin đang lưu hành, đang ký quỹ, đang chờ rút, phí đã thu;
- nạp và rút trong 30 ngày (F-Coin và ₫), kèm biểu đồ theo ngày;
- tỷ giá hiện hành và lịch sử; biểu mẫu đổi tỷ giá và phí (chỉ Tài chính dùng được);
- tra giao dịch theo email hoặc mã; khóa hoặc mở ví kèm lý do.

#### Scenario: Kiểm duyệt viên mở trang Ví
- **WHEN** quản trị viên chỉ có vai trò con Kiểm duyệt mở trang Ví F-Coin
- **THEN** trả 403

### Requirement: Hàng đợi lệnh rút
Trang Ví F-Coin SHALL có hàng đợi lệnh rút với các nút Duyệt, Từ chối (bắt buộc lý do), Đã chuyển (bắt buộc mã chuyển khoản). Mọi thao tác MUST ghi nhật ký kiểm toán.

#### Scenario: Tài chính duyệt lệnh rút
- **GIVEN** có lệnh rút 60 F-Coin chuyển khoản ngân hàng
- **WHEN** người Tài chính mở trang Ví F-Coin, bấm Duyệt rồi bấm Đã chuyển với mã "FT2611150001"
- **THEN** lệnh ở trạng thái đã trả, tổng "Đang chờ rút" giảm 60, và nhật ký kiểm toán có hai dòng tương ứng

### Requirement: Trang Dự án
Bảng quản trị SHALL có trang Dự án cho vai trò con Quản lý dự án. Trang có:
- danh sách dự án theo trạng thái, tranh chấp lên đầu;
- chi tiết: lịch sử và tin nhắn;
- biểu mẫu xử lý tranh chấp (giải ngân, hoàn, chia), ghi chú bắt buộc.

#### Scenario: Xử lý tranh chấp
- **WHEN** Quản lý dự án hoàn toàn bộ ký quỹ của một dự án tranh chấp, kèm ghi chú
- **THEN** khách hàng nhận lại F-Coin, hai bên nhận thông báo, dự án ở trạng thái đã xử lý tranh chấp

### Requirement: Duyệt yêu cầu vai trò
Trang Người dùng SHALL liệt kê các yêu cầu vai trò đang chờ, gồm vai trò xin, ghi chú, tên pháp lý, mã số thuế. Quản trị hệ thống duyệt hoặc từ chối từ danh sách này.

#### Scenario: Duyệt yêu cầu Công ty
- **WHEN** Quản trị hệ thống duyệt yêu cầu Công ty phát triển phần mềm của Lan
- **THEN** vai trò của Lan đổi sang Công ty phát triển phần mềm và nhật ký kiểm toán có dòng `role.set`
