# Spec Delta

## Purpose

F-Launcher là ứng dụng desktop để người dùng đăng nhập một lần, rồi cài đặt, cập nhật, mở và gỡ các app con của F-Eco một cách an toàn.

## ADDED Requirements

### Requirement: Đăng nhập trong launcher
F-Launcher SHALL đăng nhập qua trình duyệt mặc định của hệ điều hành bằng OAuth 2.0 Authorization Code + PKCE, với địa chỉ chuyển hướng loopback hoặc custom URI scheme. Launcher MUST không có ô nhập mật khẩu riêng, và MUST lưu refresh token trong kho bảo mật của hệ điều hành (Windows Credential Manager, macOS Keychain).

#### Scenario: Đăng nhập lần đầu
- **WHEN** người dùng bấm Đăng nhập trong launcher
- **THEN** trình duyệt mở trang đăng nhập F-Eco, và sau khi đăng nhập xong launcher hiện tên người dùng

#### Scenario: Khởi động lại máy
- **GIVEN** người dùng đã đăng nhập launcher
- **WHEN** máy khởi động lại rồi người dùng mở launcher
- **THEN** launcher vẫn ở trạng thái đăng nhập mà không cần làm lại

### Requirement: Duyệt danh mục trong launcher
F-Launcher SHALL hiện danh mục app (xem capability `app-catalog`) cùng danh sách "App của tôi" gồm các app đã cài và phiên bản của chúng. Launcher MUST vẫn mở được các app đã cài khi không có mạng.

#### Scenario: Không có mạng
- **GIVEN** máy không có kết nối mạng
- **WHEN** người dùng mở launcher
- **THEN** "App của tôi" vẫn hiện và mở được các app đã cài, phần danh mục báo "Đang ngoại tuyến"

### Requirement: Cài đặt có xác minh
Trước khi cài hoặc chạy một gói, F-Launcher SHALL tải gói qua HTTPS từ máy chủ phân phối của F-Eco, kiểm tra SHA-256 khớp với manifest đã được nền tảng ký, và kiểm tra chữ ký của nền tảng trên manifest. Gói không khớp MUST bị xóa và không được chạy.

#### Scenario: Cài thành công
- **WHEN** người dùng bấm Cài cho app "SoundLab" 1.2.0
- **THEN** launcher tải gói, xác minh mã băm và chữ ký, cài vào thư mục app của người dùng, rồi hiện nút Mở

#### Scenario: Gói bị sửa
- **WHEN** gói tải về có SHA-256 khác với manifest đã ký
- **THEN** launcher xóa gói, báo "Gói cài đặt không hợp lệ" và ghi sự kiện bảo mật

#### Scenario: Manifest giả
- **WHEN** launcher nhận một manifest không có chữ ký hợp lệ của nền tảng
- **THEN** launcher từ chối manifest đó

### Requirement: Cập nhật app con
F-Launcher SHALL kiểm tra bản cập nhật khi khởi động và mỗi 6 giờ, cho cập nhật từng app hoặc tất cả, và tùy chọn tự động cập nhật. Cập nhật MUST cài kiểu nguyên tử: lỗi giữa chừng thì giữ nguyên bản cũ chạy được. Người dùng MUST quay lại được bản ngay trước đó.

#### Scenario: Cập nhật lỗi giữa chừng
- **WHEN** mất mạng trong lúc tải bản cập nhật của SoundLab
- **THEN** SoundLab bản cũ vẫn mở được và launcher thử lại ở lần kiểm tra sau

#### Scenario: Quay lại bản trước
- **WHEN** người dùng chọn "Quay lại 1.1.0" sau khi cập nhật lên 1.2.0
- **THEN** SoundLab 1.1.0 được khôi phục mà không cần tải lại

### Requirement: Mở app con và truyền phiên đăng nhập
Khi mở app con, F-Launcher SHALL khởi chạy app với một mã dùng một lần, sống 60 giây. App đổi mã này lấy token theo capability `account-identity`. Mã MUST chỉ dùng được một lần và chỉ cho đúng app đó.

#### Scenario: Mã bị dùng lại
- **WHEN** một tiến trình khác thử dùng lại mã khởi chạy đã dùng
- **THEN** máy chủ từ chối và ghi sự kiện bảo mật

### Requirement: Gỡ app con
F-Launcher SHALL gỡ app con khỏi máy, và hỏi người dùng có xóa luôn dữ liệu cục bộ của app không (mặc định là giữ). Gỡ app MUST không tự thu hồi đồng ý dữ liệu; launcher MUST gợi ý liên kết tới trang "Ứng dụng đã kết nối".

#### Scenario: Gỡ app
- **WHEN** người dùng gỡ SoundLab và chọn xóa dữ liệu cục bộ
- **THEN** thư mục cài đặt và thư mục dữ liệu của SoundLab bị xóa, các app khác không bị ảnh hưởng

### Requirement: Gỡ khẩn cấp
Khi nền tảng đánh dấu một phiên bản app là nguy hiểm (xem capability `app-review`), F-Launcher SHALL chặn mở phiên bản đó trong vòng 1 giờ kể từ lần kết nối mạng kế tiếp. Launcher MUST báo lý do cho người dùng và đề xuất cập nhật hoặc gỡ app.

#### Scenario: Phiên bản bị thu hồi
- **GIVEN** SoundLab 1.2.0 đã cài trên máy
- **WHEN** nền tảng thu hồi SoundLab 1.2.0 vì phát hiện mã độc
- **THEN** sau lần đồng bộ kế tiếp, launcher không cho mở SoundLab 1.2.0 và hiện lý do thu hồi

### Requirement: Launcher tự cập nhật
F-Launcher SHALL tự cập nhật chính nó, với bản cập nhật được ký mã (Authenticode trên Windows, Developer ID và notarize trên macOS). Bản cập nhật không có chữ ký hợp lệ MUST bị từ chối.

#### Scenario: Có bản launcher mới
- **WHEN** có phiên bản F-Launcher mới
- **THEN** launcher tải về, kiểm tra chữ ký, và cài khi người dùng khởi động lại launcher

### Requirement: App con chạy với quyền người dùng thường
F-Launcher và app con SHALL chạy với quyền của người dùng thường, cài vào thư mục riêng của người dùng, không yêu cầu quyền quản trị. Launcher MUST lưu token của từng app thành mục riêng trong kho bảo mật của hệ điều hành, và MUST không trao cho app này token, mã khởi chạy hay dữ liệu mà launcher giữ cho app khác.

#### Scenario: Cài không cần quyền quản trị
- **WHEN** một người dùng không có quyền quản trị cài launcher và một app con
- **THEN** việc cài đặt thành công và không hiện hộp thoại yêu cầu quyền quản trị

#### Scenario: App xin token của app khác
- **WHEN** app A gọi API cục bộ của launcher để xin token kèm client ID của app B
- **THEN** launcher từ chối, vì client ID không khớp với app đã được nó khởi chạy

### Requirement: Công bố mức cô lập
Trang chi tiết app và màn hình cài đặt SHALL nói rõ app con chạy với quyền đầy đủ của người dùng trên máy, giống một ứng dụng desktop thông thường, chưa bị sandbox của hệ điều hành giới hạn. Nền tảng MUST không quảng bá là app con được cô lập khi điều đó chưa đúng.

#### Scenario: Màn hình cài đặt
- **WHEN** người dùng bấm Cài một app con
- **THEN** hộp thoại xác nhận có dòng "App này chạy với quyền như mọi ứng dụng khác trên máy bạn"
