# Spec Delta

## Purpose

Capability mới `software-projects`: khách hàng gửi yêu cầu làm phần mềm cho lập trình viên cá nhân hoặc công ty phát triển phần mềm, thương lượng giá, ký quỹ bằng F-Coin, nhận bàn giao rồi trả tiền; hợp đồng giữa các bên.

## ADDED Requirements

### Requirement: Hồ sơ nhận việc
Lập trình viên cá nhân và Công ty phát triển phần mềm SHALL tạo được hồ sơ nhận việc: tên hiển thị, giới thiệu, kỹ năng, đang nhận việc hay không. Danh sách người nhận việc MUST chỉ hiện hồ sơ đang nhận việc, kèm loại (cá nhân hoặc công ty), và MUST không hiện email. Vai trò khác MUST không tạo được hồ sơ.

#### Scenario: Khách hàng tìm người nhận việc
- **GIVEN** Lan (Lập trình viên cá nhân) bật hồ sơ "Lan Mobile", kỹ năng "Flutter"
- **WHEN** khách hàng mở danh sách người nhận việc và tìm "flutter"
- **THEN** thấy "Lan Mobile" (Lập trình viên cá nhân), không thấy email của Lan

### Requirement: Gửi yêu cầu làm phần mềm
Khách hàng cá nhân và Khách hàng doanh nghiệp SHALL gửi được yêu cầu cho một người nhận việc: tiêu đề, mô tả, ngân sách đề xuất bằng F-Coin (tùy chọn), thời hạn mong muốn. Người nhận việc MUST nhận thông báo. Vai trò khác MUST không gửi được yêu cầu.

#### Scenario: Người dùng phổ thông gửi yêu cầu
- **WHEN** người dùng phổ thông mở trang tạo yêu cầu
- **THEN** trang báo cần vai trò Khách hàng, kèm lối tắt xin vai trò

### Requirement: Thương lượng giá
Hai bên SHALL đưa được đề nghị giá (F-Coin) và thời hạn (ngày), kèm ghi chú. Đề nghị mới của một bên thay đề nghị đang mở trước đó. Mỗi bên MUST chỉ chấp nhận được đề nghị của bên kia. Khi chấp nhận, giá và thời hạn được chốt. Trước khi ký quỹ, người nhận việc từ chối được yêu cầu và hai bên hủy được dự án.

#### Scenario: Thương lượng hai vòng
- **GIVEN** khách hàng gửi yêu cầu với ngân sách 500 F-Coin
- **WHEN** Lan đề nghị 800 F-Coin trong 20 ngày, khách đề nghị 650 F-Coin trong 20 ngày, rồi Lan chấp nhận
- **THEN** dự án ở trạng thái "Đã chốt giá" với giá 650 F-Coin, 20 ngày

#### Scenario: Tự chấp nhận đề nghị của mình
- **WHEN** khách hàng chấp nhận đề nghị do chính mình đưa ra
- **THEN** thao tác bị từ chối

### Requirement: Ký quỹ
Sau khi chốt giá, khách hàng SHALL chuyển đúng giá đã chốt vào ký quỹ, và dự án chuyển sang "Đang làm". Mọi bước của dự án MUST được ghi vào lịch sử dự án, và bên kia MUST nhận thông báo.

#### Scenario: Không đủ F-Coin để ký quỹ
- **GIVEN** khách có 100 F-Coin, giá chốt 650 F-Coin
- **WHEN** khách bấm ký quỹ
- **THEN** thao tác bị từ chối kèm số F-Coin còn thiếu và lối tắt nạp, dự án vẫn "Đã chốt giá"

### Requirement: Bàn giao và trả tiền
Người nhận việc SHALL bàn giao kèm ghi chú và đường dẫn hoặc mã app. Sau đó:
- Khách hàng chấp nhận bàn giao: F-Coin trong ký quỹ MUST được chuyển cho người nhận việc (trừ phí dự án), và dự án hoàn thành.
- Khách hàng có thể yêu cầu sửa kèm lý do: dự án quay lại "Đang làm".
- Người nhận việc rút lui khi đang làm: F-Coin ký quỹ MUST về lại ví khách hàng.

#### Scenario: Trả tiền sau khi nhận bàn giao
- **GIVEN** dự án đã chốt 650 F-Coin, khách có 700 F-Coin, phí dự án 0
- **WHEN** khách ký quỹ, Lan bàn giao, khách chấp nhận
- **THEN** ví khách còn 50 F-Coin, Lan nhận 650 F-Coin, ký quỹ của dự án về 0 và dự án hoàn thành

#### Scenario: Yêu cầu sửa
- **WHEN** khách yêu cầu sửa bản bàn giao với lý do "Thiếu màn hình đăng nhập"
- **THEN** dự án quay lại "Đang làm" và F-Coin vẫn trong ký quỹ

### Requirement: Tranh chấp dự án
Khi dự án đang làm hoặc đã bàn giao, khách hàng hoặc người nhận việc SHALL mở được tranh chấp kèm lý do. Quản trị viên có vai trò con Quản lý dự án xử lý theo một trong ba cách:
- giải ngân toàn bộ cho người nhận việc;
- hoàn toàn bộ cho khách hàng;
- chia: chuyển một phần cho người nhận việc, phần còn lại hoàn khách hàng.

Tổng chia MUST bằng số F-Coin trong ký quỹ. Mỗi quyết định MUST có ghi chú và được ghi kiểm toán.

#### Scenario: Chia ký quỹ
- **GIVEN** dự án 650 F-Coin đang tranh chấp
- **WHEN** Quản lý dự án chia 400 F-Coin cho người nhận việc, 250 F-Coin hoàn khách với ghi chú
- **THEN** hai ví nhận đúng số đó, dự án ở trạng thái "Đã xử lý tranh chấp"

### Requirement: Lập hợp đồng
Khi dự án đã chốt giá, Khách hàng doanh nghiệp hoặc Công ty phát triển phần mềm là một bên của dự án SHALL lập được hợp đồng từ mẫu, và thêm được điều khoản bổ sung. Mẫu điền sẵn hai bên, phạm vi, giá F-Coin và quy đổi ra đồng theo tỷ giá lúc lập, thời hạn, bàn giao, nghiệm thu, sở hữu mã nguồn, tranh chấp. Vai trò khác MUST không lập được hợp đồng nhưng vẫn ký được hợp đồng do bên kia lập.

#### Scenario: Khách hàng cá nhân lập hợp đồng
- **WHEN** Khách hàng cá nhân (bên kia là Lập trình viên cá nhân) gửi biểu mẫu lập hợp đồng
- **THEN** thao tác bị từ chối vì cần vai trò có quyền ký kết hợp đồng

### Requirement: Ký hợp đồng
Mỗi bên SHALL ký bằng cách đánh dấu đồng ý và nhập họ tên. F-Eco MUST lưu nội dung, mã băm SHA-256, người ký và thời điểm ký. Hợp đồng đã đủ hai chữ ký MUST không sửa được. Khi dự án có hợp đồng chưa ký đủ, khách hàng MUST không ký quỹ được.

#### Scenario: Doanh nghiệp lập và hai bên ký
- **GIVEN** dự án đã chốt giá giữa một Khách hàng doanh nghiệp và Lan
- **WHEN** doanh nghiệp lập hợp đồng, ký, rồi Lan ký
- **THEN** hợp đồng ở trạng thái "Đã ký", có mã băm, và khách ký quỹ được

#### Scenario: Ký quỹ khi hợp đồng chưa ký đủ
- **GIVEN** hợp đồng mới có chữ ký của doanh nghiệp
- **WHEN** khách ký quỹ
- **THEN** trả lỗi hợp đồng chưa ký đủ, không trừ F-Coin
