# Spec Delta

## Purpose

Bảo đảm mọi app con của bên thứ ba đều an toàn, đúng với những gì đã khai báo và tuân thủ chính sách trước khi tới tay người dùng, đồng thời gỡ được nhanh khi phát hiện vấn đề.

## ADDED Requirements

### Requirement: Kiểm tra tự động
Mỗi phiên bản nộp lên SHALL qua kiểm tra tự động: quét mã độc bằng ít nhất hai engine, xác minh chữ ký mã, so manifest với gói (tệp chạy có tồn tại, kiến trúc khớp), và phát hiện gói đã bị làm rối (packer) thường gặp ở mã độc. Phiên bản không qua kiểm tra tự động MUST không được chuyển sang duyệt thủ công.

#### Scenario: Phát hiện mã độc
- **WHEN** một engine quét phát hiện gói chứa trojan
- **THEN** phiên bản bị đánh dấu "Bị từ chối: mã độc", tài khoản nhà phát triển bị tạm khóa chờ điều tra, và quản trị viên nhận cảnh báo

#### Scenario: Kiến trúc không khớp
- **WHEN** manifest khai báo `windows-x64` nhưng tệp chạy là ARM64
- **THEN** phiên bản bị từ chối kèm lỗi chỉ rõ chỗ không khớp

### Requirement: Duyệt thủ công
Phiên bản đầu tiên của mỗi app, và mọi phiên bản xin thêm scope dữ liệu hoặc thêm quyền hệ thống, SHALL qua duyệt thủ công. Người duyệt kiểm tra việc dùng dữ liệu đúng mục đích đã khai báo, mô tả đúng sự thật, và tuân thủ chính sách nội dung. Các phiên bản khác MUST được tự động duyệt nếu qua kiểm tra tự động.

#### Scenario: Bản cập nhật không đổi quyền
- **GIVEN** SoundLab 1.2.0 đã được duyệt
- **WHEN** nhà phát triển nộp 1.2.1 với cùng scope và quyền, và gói qua kiểm tra tự động
- **THEN** 1.2.1 được duyệt tự động

#### Scenario: Thêm scope
- **WHEN** SoundLab 1.3.0 thêm scope `phone`
- **THEN** 1.3.0 vào hàng chờ duyệt thủ công

### Requirement: Thời gian duyệt
Duyệt thủ công SHALL hoàn tất trong 3 ngày làm việc với 90% số phiên bản. Kết quả từ chối MUST nêu rõ điều khoản chính sách bị vi phạm và cách khắc phục; nhà phát triển MUST kháng nghị được một lần cho mỗi lần từ chối.

#### Scenario: Bị từ chối
- **WHEN** người duyệt từ chối vì app dùng email cho tiếp thị trong khi khai báo "chỉ để đăng nhập"
- **THEN** nhà phát triển nhận email nêu điều khoản bị vi phạm và cách sửa, kèm nút Kháng nghị

### Requirement: Chính sách nội dung
Nền tảng SHALL công bố chính sách cấm: mã độc, thu thập dữ liệu trái khai báo, đào tiền mã hóa ẩn, vượt DRM hay bản quyền, giả mạo thương hiệu hoặc sản phẩm của người khác, nội dung vi phạm pháp luật Việt Nam. App dựng lại từ phần mềm khác MUST không dùng tên, logo hay tài sản của sản phẩm gốc.

#### Scenario: Giả mạo thương hiệu
- **WHEN** một app nộp lên với tên và biểu tượng giống một phần mềm thương mại nổi tiếng
- **THEN** người duyệt từ chối với lý do "Giả mạo thương hiệu"

### Requirement: Gỡ khẩn cấp
Quản trị viên SHALL thu hồi được một phiên bản hoặc toàn bộ một app ngay lập tức, kèm lý do. Thu hồi MUST lan tới danh mục trong 5 phút và tới launcher theo capability `launcher`, và nhà phát triển MUST nhận thông báo ngay.

#### Scenario: Thu hồi toàn bộ app
- **WHEN** quản trị viên thu hồi app "FakeCleaner" vì lừa đảo
- **THEN** app biến khỏi danh mục, launcher chặn mở mọi phiên bản của nó, và nhà phát triển nhận email kèm lý do

### Requirement: Kiểm tra lại định kỳ
Các gói đang phát hành SHALL được quét lại bằng cơ sở dữ liệu mã độc mới ít nhất mỗi 7 ngày. Phát hiện mới MUST tạo cảnh báo cho quản trị viên, kèm đề xuất thu hồi.

#### Scenario: Phát hiện muộn
- **WHEN** lần quét lại phát hiện mã độc trong một gói đã phát hành 20 ngày
- **THEN** quản trị viên nhận cảnh báo kèm số người dùng đang cài phiên bản đó
