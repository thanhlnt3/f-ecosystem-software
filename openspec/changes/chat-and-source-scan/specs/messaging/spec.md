# messaging

## Purpose

Chat giữa khách hàng và nhà phát triển, trên web, F-Launcher máy tính và F-Launcher di động.

## ADDED Requirements

### Requirement: Hội thoại trực tiếp
Người dùng đã xác minh email SHALL bắt đầu được hội thoại với một nhà phát triển, từ trang app trong F-Store hoặc từ danh sách nhà phát triển trong Dự án. Mỗi cặp khách hàng và nhà phát triển SHALL có tối đa một hội thoại trực tiếp; bấm lại thì mở hội thoại đã có.

#### Scenario: Nhắn từ trang app
- **GIVEN** khách hàng đã đăng nhập và xác minh email
- **WHEN** khách hàng bấm "Nhắn nhà phát triển" ở trang app Mentor
- **THEN** hội thoại trực tiếp với chủ tài khoản nhà phát triển của Mentor được mở, ghi ngữ cảnh app Mentor

### Requirement: Ai nhận được hội thoại trực tiếp
Bên nhận hội thoại trực tiếp MUST là người có vai trò lập trình viên cá nhân hoặc công ty phát triển phần mềm, hoặc chủ tài khoản nhà phát triển sở hữu app. Người dùng MUST không nhắn cho chính mình, và MUST không tạo quá 20 hội thoại trực tiếp mới trong 24 giờ.

#### Scenario: Không phải nhà phát triển
- **WHEN** người dùng gọi `POST /v1/chat/conversations` với một người không có vai trò nhà phát triển
- **THEN** máy chủ trả 422 `chat_not_developer`

#### Scenario: Quá nhiều hội thoại mới
- **GIVEN** người dùng đã tạo 20 hội thoại trực tiếp trong 24 giờ qua
- **WHEN** người dùng bắt đầu hội thoại thứ 21
- **THEN** máy chủ trả 429 `chat_rate_limited`

### Requirement: Hội thoại dự án
Mỗi dự án SHALL có một hội thoại giữa khách hàng và bên nhận dự án. Tin nhắn cũ của dự án MUST được chuyển sang hội thoại và không thành "chưa đọc". Quản trị dự án SHALL đọc và gửi tin trong hội thoại dự án với tư cách quản trị.

#### Scenario: Tin cũ được giữ
- **GIVEN** dự án có ba tin nhắn trước bản 1.12.0
- **WHEN** máy chủ chạy migration và khách hàng mở dự án
- **THEN** ba tin hiện đúng thứ tự và số chưa đọc của dự án là 0

#### Scenario: Quản trị giải quyết tranh chấp
- **WHEN** quản trị dự án giải quyết tranh chấp
- **THEN** hội thoại dự án có tin của quản trị, và cả hai bên thấy tin đó

### Requirement: Gửi và nhận tin
Người tham gia SHALL gửi tin văn bản 1–5.000 ký tự, kèm tối đa một tệp 20 MB. Chỉ người tham gia (và quản trị với hội thoại dự án hoặc bị báo cáo) MUST đọc được hội thoại. Mỗi người MUST không gửi quá 30 tin mỗi phút. Mở hội thoại SHALL đánh dấu đã đọc; danh sách hội thoại và thanh trên cùng SHALL hiện số chưa đọc.

#### Scenario: Người ngoài
- **WHEN** người không tham gia gọi `GET /v1/chat/conversations/{id}/messages`
- **THEN** máy chủ trả 403 `chat_forbidden`

#### Scenario: Tin quá dài
- **WHEN** người tham gia gửi tin 5.001 ký tự
- **THEN** máy chủ trả 422 `chat_too_long` và không lưu tin

#### Scenario: Đã đọc
- **GIVEN** nhà phát triển có 2 tin chưa đọc trong một hội thoại
- **WHEN** nhà phát triển mở hội thoại
- **THEN** `GET /v1/chat/unread` của nhà phát triển giảm 2

### Requirement: Tin đến gần như tức thời
Trang hội thoại trên web SHALL hiện tin mới của bên kia mà không cần tải lại trang, qua Server-Sent Events. Khi có nhiều máy chủ, sự kiện SHALL đi qua PostgreSQL `LISTEN/NOTIFY`. F-Launcher máy tính và di động SHALL hỏi tin mới mỗi 5 giây khi đang mở hội thoại.

#### Scenario: Hai trình duyệt
- **GIVEN** khách hàng và nhà phát triển cùng mở một hội thoại trên web
- **WHEN** khách hàng gửi "Xin chào"
- **THEN** trang của nhà phát triển nhận sự kiện `message` của hội thoại đó và hiện "Xin chào" mà không tải lại trang

### Requirement: Tệp đính kèm được quét
Tệp đính kèm MUST được quét mã độc bằng các engine của duyệt app trước khi tải về được. Đang quét thì nút tải hiện "Đang kiểm tra". Tệp có mã độc MUST bị chặn tải, và quản trị nhận cảnh báo.

#### Scenario: Tệp có mã độc
- **WHEN** nhà phát triển gửi tệp chứa chuỗi thử EICAR
- **THEN** tệp có trạng thái `infected`, tải về trả 403 `chat_attachment_blocked`, và quản trị có cảnh báo `chat_malware`

### Requirement: Chặn và báo cáo
Nhà phát triển SHALL chặn và bỏ chặn được hội thoại trực tiếp; khi bị chặn, khách hàng MUST không gửi được tin. Người tham gia SHALL báo cáo được hội thoại; quản trị hỗ trợ xem hội thoại bị báo cáo ở chế độ chỉ đọc và đánh dấu đã xử lý.

#### Scenario: Bị chặn
- **GIVEN** nhà phát triển đã chặn hội thoại
- **WHEN** khách hàng gửi tin
- **THEN** máy chủ trả 409 `chat_blocked`

### Requirement: API chat cho launcher
Máy chủ SHALL có `/v1/chat/unread`, `/v1/chat/conversations` (GET, POST), `/v1/chat/conversations/{id}/messages` (GET với `after`, POST), `/v1/chat/conversations/{id}/read`, nhận token của F-Launcher máy tính và di động, theo design C4.

#### Scenario: Launcher gửi tin
- **WHEN** F-Launcher gọi `POST /v1/chat/conversations/{id}/messages` với `{"body": "Chào anh"}`
- **THEN** máy chủ trả 201 kèm tin có `mine: true`, và bên kia thấy tin trên web
