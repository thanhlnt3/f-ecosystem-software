# Spec Delta

## Purpose

Bổ sung cho capability `admin-console`: tài khoản quản trị viên mặc định luôn hoạt động; quản trị viên chuyển F-Coin cho người dùng.

## ADDED Requirements

### Requirement: Tài khoản quản trị viên mặc định
Máy chủ SHALL bảo đảm tài khoản `FECO_ADMIN_EMAIL` tồn tại, đang hoạt động, email đã xác minh và có đủ năm vai trò quản trị. Việc này chạy mỗi lần khởi động và mỗi 10 phút. Mật khẩu ban đầu MUST đọc từ tệp `FECO_ADMIN_PASSWORD_FILE`, tối thiểu 12 ký tự, và MUST không có trong mã nguồn hay nhật ký. Ở production, thiếu email, thiếu tệp hoặc mật khẩu ngắn thì máy chủ MUST không khởi động. Ở dev, máy chủ SHALL tự sinh mật khẩu ngẫu nhiên vào tệp mặc định nếu tệp chưa có.

#### Scenario: Lần khởi động đầu
- **GIVEN** chưa có tài khoản `admin@f-eco.local`
- **WHEN** máy chủ khởi động
- **THEN** tài khoản được tạo với email đã xác minh, là Quản trị viên với đủ năm vai trò, và nhật ký kiểm toán có `admin.bootstrap`

#### Scenario: Tài khoản bị tắt
- **GIVEN** tài khoản quản trị viên mặc định bị chuyển sang `inactive` trong Kratos
- **WHEN** lượt kiểm tra kế tiếp chạy
- **THEN** tài khoản hoạt động lại và vẫn đủ năm vai trò

#### Scenario: Production thiếu mật khẩu
- **WHEN** máy chủ production khởi động với `FECO_ADMIN_EMAIL` nhưng không có `FECO_ADMIN_PASSWORD_FILE`
- **THEN** máy chủ báo thiếu biến và không khởi động

### Requirement: Bảo vệ tài khoản quản trị viên mặc định
Tài khoản quản trị viên mặc định MUST không bị hạ vai trò, bỏ vai trò Quản trị hệ thống, hay xóa, dù qua bảng quản trị, công cụ dòng lệnh hay trang xóa tài khoản.

#### Scenario: Hạ vai trò tài khoản mặc định
- **WHEN** một Quản trị hệ thống khác đổi vai trò của tài khoản mặc định thành Người dùng phổ thông
- **THEN** thao tác bị từ chối với thông báo "Không thể hạ vai trò của tài khoản quản trị viên mặc định" và vai trò không đổi

### Requirement: Quản trị viên chuyển F-Coin
Quản trị viên Tài chính SHALL chuyển F-Coin cho một người dùng khác ở trang Ví F-Coin, chọn một trong hai nguồn:
- **Ví của tôi**: ghi ngay;
- **Quỹ nền tảng**: phát hành thêm F-Coin. Dưới 1.000.000 ₫ thì ghi ngay; từ 1.000.000 ₫ trở lên MUST được một quản trị viên Tài chính khác duyệt.


#### Scenario: Chuyển từ ví của quản trị viên
- **GIVEN** quản trị viên Tài chính có 200 F-Coin
- **WHEN** chuyển 50 F-Coin từ "Ví của tôi" cho Lan với lý do "Hoàn tiền hỗ trợ"
- **THEN** ví quản trị viên còn 150, Lan có thêm 50, Lan nhận email "Bạn nhận được 50 F-Coin", và nhật ký có `wallet.admin_transfer`

#### Scenario: Ví của quản trị viên không đủ
- **GIVEN** quản trị viên có 10 F-Coin
- **WHEN** chuyển 50 F-Coin từ "Ví của tôi"
- **THEN** thao tác bị từ chối "còn thiếu 40 F-Coin" và không ví nào đổi

#### Scenario: Cấp từ quỹ nền tảng
- **WHEN** quản trị viên cấp 100 F-Coin (100.000 ₫) từ "Quỹ nền tảng" cho Lan với lý do "Khuyến mại ra mắt"
- **THEN** Lan có thêm 100 F-Coin, tài khoản `issuance` giảm 100, sổ cái vẫn cân

#### Scenario: Cấp lớn cần người thứ hai
- **WHEN** quản trị viên A cấp 2.000 F-Coin (2.000.000 ₫) từ "Quỹ nền tảng" cho Lan
- **THEN** yêu cầu cấp đang chờ, Lan chưa nhận gì; A không tự duyệt được; quản trị viên Tài chính B duyệt thì Lan nhận 2.000 F-Coin

### Requirement: Ràng buộc khi chuyển F-Coin
Lý do MUST bắt buộc. Người nhận MUST khác người gửi, MUST có tài khoản và ví không bị khóa. Gửi lại cùng biểu mẫu MUST không chuyển hai lần. Người nhận SHALL nhận thông báo và email, và nhật ký kiểm toán SHALL ghi mọi lần chuyển.

#### Scenario: Gửi lại biểu mẫu
- **WHEN** quản trị viên gửi cùng một biểu mẫu chuyển 50 F-Coin hai lần
- **THEN** Lan chỉ nhận 50 F-Coin

#### Scenario: Chuyển cho chính mình
- **WHEN** quản trị viên cấp F-Coin từ "Quỹ nền tảng" cho chính email của mình
- **THEN** thao tác bị từ chối và không bút toán nào được ghi
