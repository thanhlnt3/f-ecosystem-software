# Spec Delta

## Purpose

Xác định nền tảng hệ điều hành, trình duyệt, ngôn ngữ, cùng các ngưỡng hiệu năng và độ sẵn sàng mà F-Eco phải đạt.

## ADDED Requirements

### Requirement: Hệ điều hành cho launcher và F-Builder
F-Launcher và F-Builder SHALL chạy trên Windows 10 (22H2) và Windows 11 bản x64, và trên macOS 13 trở lên cho cả Apple silicon lẫn Intel. Bộ cài MUST được ký mã, và trên macOS MUST được notarize.

#### Scenario: Windows 11
- **WHEN** cài F-Launcher trên máy Windows 11 x64 mới cài, không có quyền quản trị
- **THEN** cài đặt thành công và launcher đăng nhập được

#### Scenario: macOS Apple silicon
- **WHEN** chạy F-Launcher trên Mac M1 với macOS 13
- **THEN** launcher chạy native arm64 và Gatekeeper không cảnh báo

### Requirement: Trình duyệt cho cổng web
Cổng web (người dùng, nhà phát triển, quản trị) SHALL hoạt động trên 2 phiên bản mới nhất của Chrome, Edge, Firefox và Safari, và dùng được trên màn hình rộng từ 360 px trở lên.

#### Scenario: Thanh toán trên điện thoại
- **WHEN** khách hàng mở trang thanh toán trên điện thoại rộng 375 px
- **THEN** trang hiển thị đủ, không cuộn ngang, và quét được mã QR hoặc mở được ví điện tử

### Requirement: Ngôn ngữ
Toàn bộ giao diện, email và tài liệu cho người dùng SHALL có tiếng Việt và tiếng Anh, mặc định theo ngôn ngữ của hệ điều hành hoặc trình duyệt. Số tiền MUST hiển thị theo định dạng Việt Nam (ví dụ 9.000.000 ₫), ngày giờ theo múi giờ của người dùng.

#### Scenario: Định dạng tiền
- **WHEN** khách hàng xem báo giá bằng giao diện tiếng Việt
- **THEN** số tiền hiện dạng "9.000.000 ₫"

### Requirement: Độ sẵn sàng
Dịch vụ tài khoản, xác thực và API dữ liệu người dùng SHALL đạt độ sẵn sàng 99,9% theo tháng. Danh mục, cổng nhà phát triển và đơn hàng MUST đạt 99,5%. Nền tảng MUST có trang trạng thái công khai.

#### Scenario: Sự cố dịch vụ
- **WHEN** dịch vụ xác thực gặp sự cố
- **THEN** trang trạng thái hiện sự cố trong vòng 10 phút, và các app con đã có token còn hạn vẫn chạy được

### Requirement: Hiệu năng
Với 95% yêu cầu, API xác thực và `GET /v1/me` SHALL trả lời dưới 300 ms, và trang danh mục SHALL tải xong nội dung chính dưới 2 giây trên kết nối 4G. F-Launcher MUST khởi động tới khi hiện "App của tôi" dưới 3 giây trên máy 8 GB RAM.

#### Scenario: Kiểm thử tải
- **WHEN** chạy kiểm thử tải 1.000 yêu cầu mỗi giây vào `GET /v1/me`
- **THEN** 95% yêu cầu trả lời dưới 300 ms và tỉ lệ lỗi dưới 0,1%

### Requirement: Sao lưu và khôi phục
Cơ sở dữ liệu SHALL được sao lưu liên tục, có thể khôi phục về bất kỳ thời điểm nào trong 14 ngày gần nhất, với mục tiêu mất dữ liệu không quá 5 phút và thời gian khôi phục không quá 4 giờ. Việc khôi phục MUST được diễn tập mỗi quý.

#### Scenario: Diễn tập khôi phục
- **WHEN** đội vận hành khôi phục bản sao lưu vào môi trường riêng
- **THEN** dữ liệu khôi phục đầy đủ trong vòng 4 giờ, và kết quả diễn tập được ghi lại

### Requirement: Lưu trữ dữ liệu
Dữ liệu cá nhân của người dùng SHALL được lưu và xử lý theo yêu cầu về lưu trữ dữ liệu của pháp luật Việt Nam. Việc chuyển dữ liệu ra nước ngoài (ví dụ cho nhà cung cấp AI hay email) MUST được nêu trong chính sách quyền riêng tư và thực hiện đủ thủ tục pháp luật yêu cầu.

#### Scenario: Gửi dữ liệu cho nhà cung cấp email ở nước ngoài
- **WHEN** nền tảng gửi email qua nhà cung cấp có máy chủ ở nước ngoài
- **THEN** việc này đã được nêu trong chính sách quyền riêng tư, và đã có hồ sơ chuyển dữ liệu ra nước ngoài theo quy định
