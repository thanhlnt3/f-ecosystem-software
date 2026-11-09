# Spec Delta

## Purpose

Cho phép khách hàng trả tiền để đội ngũ F-Eco xây hộ một app con theo yêu cầu, qua một vòng đời đơn hàng minh bạch: yêu cầu, báo giá, thanh toán theo mốc, bàn giao, nghiệm thu.

## ADDED Requirements

### Requirement: Gửi yêu cầu xây app
Khách hàng đã xác minh email SHALL gửi yêu cầu gồm: tên dự án, mô tả, nền tảng mục tiêu, thời hạn mong muốn, ngân sách dự kiến (tùy chọn), và tệp đính kèm (spec, bản vẽ, tài liệu; tổng tối đa 200 MB). Trước khi gửi, khách hàng MUST chấp nhận Điều khoản dịch vụ xây app (ghi lại phiên bản và thời điểm).

#### Scenario: Gửi yêu cầu
- **WHEN** khách hàng gửi yêu cầu "App quản lý kho cho cửa hàng" kèm file spec
- **THEN** đơn chuyển sang trạng thái "Đã gửi" và khách hàng nhận email xác nhận kèm mã đơn

#### Scenario: Tệp quá lớn
- **WHEN** tổng tệp đính kèm vượt 200 MB
- **THEN** nền tảng từ chối tải lên và báo giới hạn

### Requirement: Trạng thái đơn hàng
Đơn SHALL đi theo các trạng thái: Nháp → Đã gửi → Đã báo giá → Đang thực hiện → Đã bàn giao → Hoàn tất. Ngoài ra có Đã hủy và Đang tranh chấp. Mọi lần chuyển trạng thái MUST được ghi lại kèm người thực hiện và thời điểm, và khách hàng MUST xem được toàn bộ lịch sử này.

#### Scenario: Chuyển trạng thái không hợp lệ
- **WHEN** hệ thống cố chuyển một đơn từ "Đã gửi" thẳng sang "Hoàn tất"
- **THEN** thao tác bị từ chối vì không phải bước chuyển hợp lệ

### Requirement: Báo giá theo mốc
Đội F-Eco SHALL gửi báo giá trong 3 ngày làm việc, gồm: phạm vi công việc, những gì không bao gồm, các mốc (mỗi mốc có sản phẩm bàn giao, số tiền bằng VND đã gồm thuế, thời hạn), số lượt sửa miễn phí của mỗi mốc (mặc định 2), và hạn hiệu lực của báo giá (mặc định 7 ngày). Mốc đầu tiên là tiền đặt cọc MUST không vượt quá 30% tổng giá.

#### Scenario: Khách chấp nhận báo giá
- **GIVEN** báo giá 30.000.000 ₫ gồm ba mốc 9.000.000 ₫, 12.000.000 ₫ và 9.000.000 ₫
- **WHEN** khách hàng chấp nhận báo giá
- **THEN** nền tảng tạo yêu cầu thanh toán 9.000.000 ₫ cho mốc đặt cọc

#### Scenario: Báo giá hết hạn
- **WHEN** khách hàng mở báo giá sau hạn hiệu lực
- **THEN** nút Chấp nhận bị vô hiệu và khách hàng thấy nút "Yêu cầu báo giá lại"

#### Scenario: Thương lượng
- **WHEN** khách hàng bấm "Đề nghị điều chỉnh" và ghi chú yêu cầu
- **THEN** đơn vẫn ở trạng thái "Đã báo giá" và đội F-Eco nhận được đề nghị để gửi báo giá mới

### Requirement: Bắt đầu thực hiện sau khi đặt cọc
Đơn SHALL chuyển sang "Đang thực hiện" chỉ sau khi thanh toán đặt cọc thành công theo capability `payments`. Thời hạn của các mốc MUST tính từ ngày đặt cọc thành công.

#### Scenario: Đặt cọc thành công
- **WHEN** cổng thanh toán xác nhận đã thu tiền đặt cọc
- **THEN** đơn chuyển sang "Đang thực hiện" và khách hàng nhận email kèm lịch các mốc

### Requirement: Bàn giao mốc và nghiệm thu
Mỗi mốc SHALL được bàn giao kèm bản build chạy được qua F-Launcher (dạng app riêng tư chỉ khách hàng thấy), ghi chú thay đổi, và kết quả kiểm tra theo spec. Khách hàng MUST nghiệm thu hoặc yêu cầu sửa trong 7 ngày; quá hạn mà không phản hồi, nền tảng gửi nhắc, và mốc tự nghiệm thu sau 14 ngày.

#### Scenario: Yêu cầu sửa trong giới hạn
- **GIVEN** mốc 2 còn 2 lượt sửa miễn phí
- **WHEN** khách hàng yêu cầu sửa kèm mô tả lỗi
- **THEN** mốc quay lại "Đang thực hiện" và còn 1 lượt sửa miễn phí

#### Scenario: Hết lượt sửa miễn phí
- **GIVEN** mốc đã dùng hết lượt sửa miễn phí
- **WHEN** khách hàng yêu cầu sửa thêm
- **THEN** nền tảng thông báo yêu cầu này cần báo giá bổ sung

#### Scenario: Nghiệm thu mốc
- **WHEN** khách hàng nghiệm thu mốc 2
- **THEN** nền tảng tạo yêu cầu thanh toán cho mốc 3, hoặc chuyển đơn sang "Hoàn tất" nếu đó là mốc cuối

### Requirement: Kênh trao đổi trong đơn
Mỗi đơn SHALL có luồng tin nhắn giữa khách hàng và đội F-Eco, đính kèm được tệp. Mọi tin nhắn MUST được lưu cùng đơn hàng để làm căn cứ khi có tranh chấp.

#### Scenario: Gửi tin nhắn
- **WHEN** đội F-Eco hỏi khách hàng về quy trình nhập kho
- **THEN** khách hàng nhận thông báo và trả lời được ngay trong đơn

### Requirement: Bàn giao cuối và quyền sở hữu
Khi đơn Hoàn tất và đã thanh toán đủ, nền tảng SHALL cho khách hàng tải mã nguồn, spec, tài liệu và bản build. Quyền sở hữu hoặc quyền sử dụng mã nguồn MUST theo Điều khoản dịch vụ đã chấp nhận, và MUST hiện rõ trong báo giá trước khi khách hàng chấp nhận.

#### Scenario: Tải bàn giao cuối
- **GIVEN** đơn đã Hoàn tất và thanh toán đủ
- **WHEN** khách hàng bấm Tải bàn giao
- **THEN** khách hàng tải được file ZIP chứa mã nguồn, spec, tài liệu và bản build

#### Scenario: Còn mốc chưa thanh toán
- **WHEN** đơn còn mốc chưa thanh toán
- **THEN** nút Tải mã nguồn bị khóa; khách hàng chỉ chạy thử được bản build qua launcher

### Requirement: Hủy đơn
Khách hàng SHALL hủy đơn ở mọi trạng thái trước "Hoàn tất". Hủy trước khi đặt cọc MUST không mất phí. Hủy sau khi đặt cọc thì hoàn tiền theo capability `payments`: khách được hoàn các khoản đã trả cho mốc chưa bàn giao, trừ tiền đặt cọc khi công việc đã bắt đầu theo Điều khoản dịch vụ.

#### Scenario: Hủy khi đang ở mốc 2
- **GIVEN** khách hàng đã trả mốc 1 (đặt cọc) và mốc 2, mốc 2 chưa bàn giao
- **WHEN** khách hàng hủy đơn
- **THEN** nền tảng tạo yêu cầu hoàn tiền mốc 2, và giữ tiền đặt cọc theo Điều khoản dịch vụ

### Requirement: Tranh chấp
Khách hàng SHALL mở tranh chấp cho một mốc trong 14 ngày sau khi bàn giao. Đơn chuyển sang "Đang tranh chấp", và quản trị viên MUST phản hồi trong 2 ngày làm việc. Kết quả MUST là một trong: tiếp tục sửa, hoàn tiền một phần, hoàn tiền toàn bộ mốc.

#### Scenario: Mở tranh chấp
- **WHEN** khách hàng mở tranh chấp cho mốc 2 vì không đúng spec
- **THEN** đơn chuyển sang "Đang tranh chấp" và quản trị viên nhận việc cần xử lý kèm hạn 2 ngày làm việc
