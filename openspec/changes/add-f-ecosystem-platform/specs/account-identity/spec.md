# Spec Delta

## Purpose

Cho phép người dùng tạo một tài khoản F-Eco duy nhất, đăng nhập an toàn trên web và launcher, rồi dùng chính tài khoản đó để đăng nhập một lần vào mọi app con.

## ADDED Requirements

### Requirement: Đăng ký bằng email
Nền tảng SHALL cho đăng ký bằng email và mật khẩu, bắt buộc xác minh email qua liên kết có hạn 24 giờ trước khi mua hàng, đặt đơn xây app hay đăng ký nhà phát triển. Mật khẩu MUST dài ít nhất 10 ký tự và không nằm trong danh sách mật khẩu đã bị lộ.

#### Scenario: Đăng ký thành công
- **WHEN** người dùng đăng ký bằng email `lan@example.com` và một mật khẩu hợp lệ
- **THEN** tài khoản được tạo ở trạng thái "chưa xác minh" và một email xác minh được gửi đi

#### Scenario: Mật khẩu đã bị lộ
- **WHEN** người dùng chọn mật khẩu "Password123!"
- **THEN** nền tảng từ chối và báo "Mật khẩu này đã xuất hiện trong các vụ rò rỉ dữ liệu"

#### Scenario: Email đã được dùng
- **WHEN** người dùng đăng ký bằng một email đã có tài khoản
- **THEN** nền tảng hiện cùng một thông báo như khi đăng ký thành công, và gửi email cho chủ tài khoản đó, để không lộ việc email đã tồn tại

#### Scenario: Chưa xác minh email
- **GIVEN** tài khoản chưa xác minh email
- **WHEN** người dùng bấm Thanh toán cho một đơn xây app
- **THEN** nền tảng yêu cầu xác minh email trước

### Requirement: Đăng nhập bằng tài khoản bên ngoài
Nền tảng SHALL cho đăng nhập bằng Google và GitHub qua OpenID Connect hoặc OAuth 2.0. Khi email từ nhà cung cấp đã được xác minh và trùng một tài khoản có sẵn, nền tảng MUST yêu cầu người dùng đăng nhập bằng cách cũ một lần để liên kết, không tự động gộp tài khoản.

#### Scenario: Đăng nhập Google lần đầu
- **WHEN** người dùng chưa có tài khoản chọn "Đăng nhập bằng Google" và đồng ý
- **THEN** một tài khoản F-Eco mới được tạo với email đã xác minh từ Google

#### Scenario: Liên kết với tài khoản có sẵn
- **GIVEN** đã có tài khoản email `lan@example.com`
- **WHEN** người dùng đăng nhập bằng GitHub có cùng email đó
- **THEN** nền tảng yêu cầu nhập mật khẩu tài khoản cũ để liên kết, rồi mới cho đăng nhập

### Requirement: Xác thực hai lớp
Nền tảng SHALL hỗ trợ xác thực hai lớp bằng ứng dụng TOTP và passkey (WebAuthn), kèm 10 mã khôi phục dùng một lần. Xác thực hai lớp MUST bắt buộc với tài khoản nhà phát triển, tài khoản quản trị, và trước mọi thao tác đổi email, đổi mật khẩu hay xem mã khôi phục.

#### Scenario: Bật TOTP
- **WHEN** người dùng quét mã QR bằng ứng dụng xác thực và nhập đúng mã 6 số
- **THEN** xác thực hai lớp được bật và 10 mã khôi phục hiện ra một lần

#### Scenario: Nhà phát triển chưa bật hai lớp
- **WHEN** một tài khoản nhà phát triển chưa bật xác thực hai lớp nộp phiên bản app
- **THEN** nền tảng chặn và yêu cầu bật xác thực hai lớp trước

### Requirement: Phiên đăng nhập
Phiên đăng nhập SHALL dùng access token sống tối đa 15 phút và refresh token xoay vòng, sống tối đa 30 ngày. Người dùng MUST xem được danh sách thiết bị đang đăng nhập (loại thiết bị, vị trí gần đúng, lần dùng cuối) và đăng xuất từng thiết bị hoặc tất cả.

#### Scenario: Đăng xuất thiết bị khác
- **GIVEN** người dùng đang đăng nhập trên laptop và máy văn phòng
- **WHEN** người dùng chọn đăng xuất máy văn phòng từ laptop
- **THEN** mọi token của máy văn phòng bị thu hồi trong vòng 60 giây

#### Scenario: Refresh token bị dùng lại
- **WHEN** một refresh token đã xoay vòng được dùng lại lần nữa
- **THEN** toàn bộ chuỗi phiên của thiết bị đó bị thu hồi và người dùng nhận email cảnh báo

### Requirement: Khôi phục tài khoản
Nền tảng SHALL cho đặt lại mật khẩu qua liên kết email dùng một lần, có hạn 30 phút. Đặt lại mật khẩu MUST đăng xuất mọi phiên khác. Tài khoản đã bật hai lớp MUST vẫn yêu cầu yếu tố thứ hai hoặc mã khôi phục.

#### Scenario: Quên mật khẩu
- **WHEN** người dùng yêu cầu đặt lại mật khẩu và mở liên kết trong vòng 30 phút
- **THEN** người dùng đặt được mật khẩu mới và mọi phiên khác bị đăng xuất

#### Scenario: Liên kết hết hạn
- **WHEN** người dùng mở liên kết đặt lại mật khẩu sau 31 phút
- **THEN** nền tảng báo liên kết đã hết hạn và cho yêu cầu liên kết mới

### Requirement: Chống dò mật khẩu
Nền tảng SHALL làm chậm dần các lần đăng nhập sai liên tiếp của một tài khoản, và yêu cầu thử thách chống bot sau 5 lần sai trong 15 phút. Nền tảng MUST không khóa cứng tài khoản, để kẻ xấu không khóa được tài khoản của người khác.

#### Scenario: Sai nhiều lần
- **WHEN** có 5 lần đăng nhập sai vào cùng một tài khoản trong 15 phút
- **THEN** lần thử tiếp theo phải vượt qua thử thách chống bot, và chủ tài khoản nhận email cảnh báo

### Requirement: Đăng nhập một lần cho app con
Khi người dùng đã đăng nhập launcher, app con SHALL nhận được phiên đăng nhập qua luồng OAuth 2.0 Authorization Code + PKCE do launcher làm trung gian, không cần nhập lại mật khẩu. App con MUST không bao giờ nhận được mật khẩu hay refresh token của tài khoản F-Eco.

#### Scenario: Mở app con khi đã đăng nhập
- **GIVEN** người dùng đã đăng nhập F-Launcher và đã đồng ý cho app "SoundLab"
- **WHEN** người dùng mở SoundLab từ launcher
- **THEN** SoundLab nhận access token dành riêng cho nó và hiện tên người dùng, không hỏi mật khẩu

#### Scenario: Đăng xuất launcher
- **WHEN** người dùng đăng xuất khỏi F-Launcher
- **THEN** mọi token đã cấp cho các app con trên thiết bị đó bị thu hồi

### Requirement: Xóa tài khoản
Người dùng SHALL tự xóa được tài khoản sau khi xác thực lại. Tài khoản chuyển sang trạng thái chờ xóa 30 ngày (khôi phục được bằng cách đăng nhập lại), sau đó dữ liệu cá nhân MUST bị xóa hoặc ẩn danh, trừ dữ liệu pháp luật buộc lưu giữ như hóa đơn.

#### Scenario: Xóa tài khoản
- **WHEN** người dùng xác nhận xóa tài khoản và không đăng nhập lại trong 30 ngày
- **THEN** hồ sơ, dữ liệu đã cấp cho app con và lịch sử F-Builder bị xóa; hóa đơn được giữ ở dạng tách khỏi hồ sơ

#### Scenario: Còn đơn hàng đang thực hiện
- **GIVEN** người dùng có một đơn xây app chưa nghiệm thu
- **WHEN** người dùng yêu cầu xóa tài khoản
- **THEN** nền tảng báo cần hủy hoặc hoàn tất đơn hàng trước
