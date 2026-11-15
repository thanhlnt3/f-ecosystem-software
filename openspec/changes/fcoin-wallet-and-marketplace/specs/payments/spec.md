# Spec Delta

## Purpose

Bổ sung cho capability `payments`: khoản phải thu của lệnh nạp F-Coin và lệnh hoàn của lệnh rút F-Coin.

## ADDED Requirements

### Requirement: Khoản phải thu của lệnh nạp
Mỗi yêu cầu thanh toán SHALL thuộc về đúng một trong hai: một đơn xây app hoặc một lệnh nạp F-Coin. Lệnh nạp dùng chung với đơn hàng các phần sau: trang chọn phương thức, liên kết 15 phút, truy vấn chủ động, đối soát và hóa đơn. Khi cổng xác nhận đã thu, hệ thống MUST cộng F-Coin vào ví thay vì cập nhật đơn hàng.

#### Scenario: Trang thanh toán của lệnh nạp
- **WHEN** người dùng tạo lệnh nạp 50 F-Coin
- **THEN** trang thanh toán ghi "Nạp 50 F-Coin" và số tiền 50.000 ₫, kèm các phương thức của cổng đã cấu hình

### Requirement: Lệnh hoàn của lệnh rút
Khi lệnh rút "hoàn về giao dịch nạp" được duyệt, hệ thống SHALL tạo một lệnh hoàn loại `withdrawal` cho giao dịch nạp đó. Lệnh hoàn này không cần phê duyệt thêm. Kết quả của lệnh hoàn MUST cập nhật lệnh rút:
- thành công: lệnh rút ở trạng thái đã trả;
- thất bại: F-Coin về lại ví.

#### Scenario: Cổng từ chối hoàn
- **GIVEN** lệnh rút 20 F-Coin hoàn về giao dịch nạp đã được duyệt
- **WHEN** cổng trả lỗi cho lệnh hoàn
- **THEN** lệnh rút ở trạng thái thất bại, 20 F-Coin về lại ví, và quản trị viên nhận cảnh báo
