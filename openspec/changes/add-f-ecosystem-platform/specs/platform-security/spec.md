# Spec Delta

## Purpose

Đặt các yêu cầu bảo mật xuyên suốt nền tảng: mã hóa, bí mật, giới hạn tần suất, nhật ký kiểm toán, phát hiện bất thường và xử lý sự cố.

## ADDED Requirements

### Requirement: Mã hóa khi truyền và khi lưu
Mọi kết nối tới nền tảng SHALL dùng TLS 1.2 trở lên, kèm HSTS cho các trang web. Dữ liệu cá nhân, token và bí mật của nhà phát triển MUST được mã hóa khi lưu. Mật khẩu MUST được băm bằng Argon2id.

#### Scenario: Kết nối HTTP thường
- **WHEN** trình duyệt mở `http://` một trang của nền tảng
- **THEN** yêu cầu được chuyển sang `https://` và phản hồi có header HSTS

#### Scenario: Không lưu mật khẩu dạng rõ
- **WHEN** kiểm tra cơ sở dữ liệu tài khoản
- **THEN** không có mật khẩu nào ở dạng rõ hay băm bằng thuật toán nhanh như MD5 hoặc SHA-1

### Requirement: Quản lý bí mật
Khóa API, khóa ký và thông tin đăng nhập cổng thanh toán SHALL nằm trong kho bí mật, không nằm trong mã nguồn hay file cấu hình được commit. CI MUST quét để chặn commit có bí mật.

#### Scenario: Commit có khóa
- **WHEN** một commit chứa chuỗi giống khóa bí mật của cổng thanh toán
- **THEN** bước quét bí mật của CI thất bại và chặn merge

### Requirement: Giới hạn tần suất
Mọi API công khai SHALL có giới hạn tần suất theo người dùng, theo app và theo địa chỉ IP. Khi vượt giới hạn, API MUST trả mã 429 kèm header `Retry-After`.

#### Scenario: App gọi quá nhiều
- **WHEN** một app gọi API dữ liệu người dùng vượt hạn mức của nó trong một phút
- **THEN** các yêu cầu vượt mức nhận mã 429 và header `Retry-After`

### Requirement: Nhật ký kiểm toán
Nền tảng SHALL ghi nhật ký kiểm toán không sửa được cho các thao tác nhạy cảm: đăng nhập, đổi thông tin bảo mật, cấp và thu hồi đồng ý, duyệt và gỡ app, hoàn tiền, thao tác của quản trị viên. Nhật ký MUST lưu ít nhất 1 năm, và MUST không chứa mật khẩu, token hay số thẻ.

#### Scenario: Quản trị viên hoàn tiền
- **WHEN** một quản trị viên hoàn tiền cho một đơn hàng
- **THEN** nhật ký ghi người thực hiện, đơn hàng, số tiền, lý do và thời điểm

#### Scenario: Thử sửa nhật ký
- **WHEN** một tài khoản quản trị cố sửa hay xóa một dòng nhật ký kiểm toán
- **THEN** thao tác bị từ chối và chính lần thử đó được ghi vào nhật ký

### Requirement: Phát hiện đăng nhập bất thường
Khi có đăng nhập thành công từ thiết bị mới hoặc từ quốc gia khác với các lần trước, nền tảng SHALL gửi email báo cho người dùng, kèm nút "Không phải tôi" để đăng xuất mọi phiên và yêu cầu đổi mật khẩu.

#### Scenario: Đăng nhập từ thiết bị lạ
- **WHEN** tài khoản đăng nhập thành công từ một thiết bị chưa từng dùng
- **THEN** người dùng nhận email cho biết thiết bị, vị trí gần đúng và thời điểm

### Requirement: Bảo mật webhook
Mọi webhook nền tảng gửi cho app con SHALL được ký HMAC-SHA256 kèm dấu thời gian. Mọi webhook nhận từ cổng thanh toán MUST được xác minh chữ ký theo tài liệu của cổng đó trước khi xử lý; webhook sai chữ ký MUST bị bỏ qua và ghi cảnh báo.

#### Scenario: Webhook thanh toán giả
- **WHEN** nền tảng nhận một webhook "đã thanh toán" có chữ ký sai
- **THEN** đơn hàng không đổi trạng thái và một cảnh báo bảo mật được ghi lại

### Requirement: Kiểm thử bảo mật
Trước mỗi bản phát hành lớn, nền tảng SHALL được quét lỗ hổng tự động (phụ thuộc, OWASP Top 10). Trước khi ra mắt công khai, nền tảng MUST được kiểm thử xâm nhập bởi bên độc lập; lỗ hổng mức nghiêm trọng hoặc cao MUST được sửa trước khi ra mắt.

#### Scenario: Phụ thuộc có lỗ hổng nghiêm trọng
- **WHEN** quét phụ thuộc phát hiện lỗ hổng mức nghiêm trọng
- **THEN** bản phát hành bị chặn cho tới khi phụ thuộc được nâng cấp hoặc có ngoại lệ đã duyệt

### Requirement: Xử lý sự cố lộ dữ liệu
Nền tảng SHALL có quy trình xử lý sự cố bằng văn bản. Khi xác nhận có lộ dữ liệu cá nhân, nền tảng MUST thông báo cho cơ quan có thẩm quyền và người dùng bị ảnh hưởng trong thời hạn pháp luật Việt Nam quy định.

#### Scenario: Diễn tập sự cố
- **WHEN** đội vận hành diễn tập kịch bản lộ dữ liệu
- **THEN** quy trình xác định được người dùng bị ảnh hưởng, nội dung thông báo và thời hạn phải gửi
