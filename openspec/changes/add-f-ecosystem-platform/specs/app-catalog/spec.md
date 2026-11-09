# Spec Delta

## Purpose

F-Store là danh mục công khai các app con đã được duyệt, giúp người dùng tìm, đánh giá và chọn app để cài qua F-Launcher.

## ADDED Requirements

### Requirement: Chỉ hiện app đã duyệt
Danh mục SHALL chỉ hiện các phiên bản app đã qua kiểm duyệt (capability `app-review`) và đang ở trạng thái phát hành. App bị gỡ hoặc thu hồi MUST biến khỏi danh mục trong vòng 5 phút.

#### Scenario: App bị gỡ
- **WHEN** quản trị viên gỡ app "FakeCleaner" khỏi danh mục
- **THEN** sau tối đa 5 phút, app không còn xuất hiện trong kết quả tìm kiếm hay trang danh mục

### Requirement: Trang chi tiết app
Mỗi app SHALL có trang chi tiết gồm tên, nhà phát triển (kèm dấu "Đã xác minh" nếu có), mô tả, ảnh chụp màn hình, phiên bản và ngày cập nhật, hệ điều hành hỗ trợ, dung lượng, scope dữ liệu app yêu cầu cùng mục đích, license, liên kết chính sách quyền riêng tư, và lịch sử thay đổi.

#### Scenario: Xem scope dữ liệu trước khi cài
- **WHEN** người dùng mở trang chi tiết của SoundLab
- **THEN** trang liệt kê "Tên và ảnh đại diện (bắt buộc): hiển thị trên bản nhạc bạn chia sẻ" trước nút Cài

### Requirement: Tìm kiếm và lọc
Danh mục SHALL tìm được theo tên, mô tả và nhà phát triển, không phân biệt dấu tiếng Việt và hoa thường, và lọc được theo danh mục, hệ điều hành, miễn phí hay trả phí, và nhà phát triển đã xác minh.

#### Scenario: Tìm không dấu
- **GIVEN** có app tên "Sổ tay Học tập"
- **WHEN** người dùng tìm "so tay hoc tap"
- **THEN** app đó có trong kết quả

### Requirement: Đánh giá và nhận xét
Chỉ người dùng đã cài app SHALL được chấm điểm từ 1 đến 5 sao và viết nhận xét. Nhà phát triển MUST trả lời được nhận xét. Nhận xét vi phạm chính sách MUST bị ẩn sau khi quản trị viên xác nhận báo cáo.

#### Scenario: Chưa cài app
- **WHEN** người dùng chưa từng cài SoundLab bấm Viết nhận xét
- **THEN** nền tảng báo "Bạn cần cài app trước khi đánh giá"

#### Scenario: Nhà phát triển trả lời
- **WHEN** nhà phát triển trả lời một nhận xét 2 sao
- **THEN** câu trả lời hiện ngay dưới nhận xét, có nhãn "Nhà phát triển"

### Requirement: Báo cáo vi phạm
Mọi người dùng đã đăng nhập SHALL báo cáo được một app hoặc một nhận xét kèm lý do (mã độc, lừa đảo, vi phạm bản quyền, nội dung không phù hợp, khác). Báo cáo MUST vào hàng chờ của quản trị viên, và người báo cáo MUST nhận được kết quả xử lý.

#### Scenario: Báo cáo vi phạm bản quyền
- **WHEN** người dùng báo cáo app "PhotoKlone" vi phạm bản quyền kèm mô tả
- **THEN** báo cáo xuất hiện trong hàng chờ quản trị, và người báo cáo nhận email khi báo cáo được xử lý

### Requirement: App có sẵn của nền tảng
Danh mục SHALL đánh dấu các app do chính F-Eco phát triển (gồm F-Builder) bằng nhãn "Của F-Eco". Các app này MUST đi qua cùng quy trình kiểm duyệt và hiện cùng thông tin như app của bên thứ ba.

#### Scenario: Nhãn app của nền tảng
- **WHEN** người dùng xem trang chi tiết của F-Builder
- **THEN** trang có nhãn "Của F-Eco" và vẫn hiện đầy đủ scope dữ liệu cùng chính sách quyền riêng tư
