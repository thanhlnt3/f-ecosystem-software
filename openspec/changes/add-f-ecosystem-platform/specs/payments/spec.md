# Spec Delta

## Purpose

Thu tiền an toàn cho đơn xây app theo yêu cầu (đặt cọc và theo mốc) qua các phương thức phổ biến ở Việt Nam, hoàn tiền khi cần, xuất hóa đơn điện tử, và đối soát chính xác với cổng thanh toán.

## ADDED Requirements

### Requirement: Phương thức thanh toán
Nền tảng SHALL hỗ trợ: chuyển khoản bằng mã VietQR, thẻ ATM nội địa, ví điện tử phổ biến, và thẻ quốc tế Visa, Mastercard, JCB, thông qua cổng thanh toán được cấp phép tại Việt Nam. Đơn vị tiền tệ của phiên bản đầu MUST là VND, số tiền là số nguyên.

#### Scenario: Chọn phương thức
- **WHEN** khách hàng mở trang thanh toán mốc đặt cọc
- **THEN** trang hiện số tiền, nội dung mốc, và các phương thức: QR chuyển khoản, thẻ ATM, ví điện tử, thẻ quốc tế

### Requirement: Không lưu dữ liệu thẻ
Nhập số thẻ SHALL diễn ra trên trang hoặc thành phần nhúng của cổng thanh toán. Máy chủ và cơ sở dữ liệu của F-Eco MUST không bao giờ nhận, ghi nhật ký hay lưu số thẻ, CVV hay ngày hết hạn; nền tảng chỉ lưu mã giao dịch và 4 số cuối do cổng trả về.

#### Scenario: Kiểm tra nhật ký
- **WHEN** quét toàn bộ nhật ký và cơ sở dữ liệu sau một giao dịch thẻ thử
- **THEN** không tìm thấy số thẻ đầy đủ hay CVV ở bất kỳ đâu

### Requirement: Vòng đời giao dịch
Mỗi yêu cầu thanh toán SHALL có trạng thái: Chờ thanh toán → Thành công, Thất bại hoặc Hết hạn. Liên kết thanh toán MUST hết hạn sau 15 phút. Trạng thái Thành công MUST chỉ được đặt khi có xác nhận đã xác minh chữ ký từ cổng (webhook hoặc truy vấn chủ động), không dựa vào trang chuyển hướng phía trình duyệt.

#### Scenario: Khách đóng trình duyệt sau khi trả tiền
- **GIVEN** khách hàng đã thanh toán thành công nhưng đóng trình duyệt trước khi được chuyển hướng về
- **WHEN** cổng thanh toán gửi webhook đã ký báo thành công
- **THEN** giao dịch chuyển sang Thành công và đơn hàng được cập nhật như bình thường

#### Scenario: Trang chuyển hướng bị giả
- **WHEN** ai đó mở trực tiếp đường dẫn trả về kèm tham số "thành công" mà không có webhook hợp lệ
- **THEN** giao dịch vẫn ở "Chờ thanh toán" cho tới khi nền tảng truy vấn cổng và nhận kết quả

#### Scenario: Liên kết hết hạn
- **WHEN** khách hàng không thanh toán trong 15 phút
- **THEN** giao dịch chuyển sang Hết hạn và khách hàng tạo được liên kết mới cho cùng mốc

### Requirement: Không thu tiền hai lần
Mỗi mốc SHALL chỉ được thu thành công đúng một lần. Mọi lời gọi tạo giao dịch và xử lý webhook MUST idempotent theo khóa (mã mốc, lần thử). Khi phát hiện hai giao dịch thành công cho cùng một mốc, nền tảng MUST tự tạo hoàn tiền cho giao dịch thứ hai và báo quản trị viên.

#### Scenario: Webhook đến hai lần
- **WHEN** cổng gửi lặp lại cùng một webhook "thành công" ba lần
- **THEN** đơn hàng chỉ được ghi nhận thanh toán một lần

#### Scenario: Khách trả hai lần
- **WHEN** khách hàng trả thành công cùng một mốc bằng QR rồi bằng thẻ
- **THEN** giao dịch thứ hai được hoàn tiền tự động và quản trị viên nhận cảnh báo

### Requirement: Hoàn tiền
Quản trị viên SHALL hoàn tiền toàn bộ hoặc một phần một giao dịch thành công, về đúng phương thức thanh toán ban đầu, kèm lý do. Tổng số tiền hoàn MUST không vượt số tiền đã thu. Hoàn tiền từ 10.000.000 ₫ trở lên MUST cần hai quản trị viên phê duyệt.

#### Scenario: Hoàn một phần
- **WHEN** quản trị viên hoàn 4.000.000 ₫ của giao dịch 12.000.000 ₫ sau tranh chấp
- **THEN** cổng nhận lệnh hoàn 4.000.000 ₫, khách hàng nhận email, và giao dịch hiện "Đã hoàn một phần"

#### Scenario: Hoàn vượt số đã thu
- **WHEN** quản trị viên cố hoàn 13.000.000 ₫ cho giao dịch 12.000.000 ₫
- **THEN** thao tác bị từ chối

#### Scenario: Hoàn tiền lớn
- **WHEN** một quản trị viên tạo lệnh hoàn 15.000.000 ₫
- **THEN** lệnh ở trạng thái "Chờ phê duyệt" cho tới khi một quản trị viên khác phê duyệt

### Requirement: Hóa đơn điện tử
Sau mỗi giao dịch thành công, nền tảng SHALL phát hành hóa đơn điện tử theo quy định hiện hành của Việt Nam, dùng thông tin xuất hóa đơn của khách (cá nhân hoặc doanh nghiệp có mã số thuế), và gửi qua email. Khi hoàn tiền, nền tảng MUST lập hóa đơn điều chỉnh hoặc thay thế theo quy định.

#### Scenario: Khách doanh nghiệp
- **GIVEN** khách hàng đã nhập tên công ty và mã số thuế trong thông tin xuất hóa đơn
- **WHEN** thanh toán mốc đặt cọc thành công
- **THEN** khách hàng nhận email kèm hóa đơn điện tử mang tên công ty và mã số thuế đó

### Requirement: Lịch sử thanh toán
Khách hàng SHALL xem lịch sử mọi giao dịch, hoàn tiền và hóa đơn của mình, và tải lại được hóa đơn.

#### Scenario: Tải lại hóa đơn
- **WHEN** khách hàng mở Lịch sử thanh toán và chọn một giao dịch
- **THEN** khách hàng tải được hóa đơn điện tử của giao dịch đó

### Requirement: Đối soát
Hằng ngày, nền tảng SHALL đối chiếu giao dịch nội bộ với báo cáo của cổng thanh toán. Mọi chênh lệch (thiếu, thừa, sai số tiền) MUST tạo cảnh báo cho quản trị viên.

#### Scenario: Giao dịch thiếu webhook
- **GIVEN** cổng ghi nhận một giao dịch thành công nhưng nền tảng chưa nhận được webhook
- **WHEN** chạy đối soát hằng ngày
- **THEN** giao dịch được cập nhật thành Thành công, đơn hàng được cập nhật, và quản trị viên nhận báo cáo chênh lệch

### Requirement: Môi trường thanh toán thử
Môi trường phát triển và staging SHALL chỉ dùng chế độ sandbox của cổng thanh toán và thẻ thử của cổng. Khóa thật của cổng MUST chỉ có ở môi trường production.

#### Scenario: Staging dùng khóa thật
- **WHEN** cấu hình staging chứa khóa production của cổng thanh toán
- **THEN** kiểm tra cấu hình lúc khởi động thất bại và dịch vụ không chạy
