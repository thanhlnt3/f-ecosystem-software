# Spec Delta

## Purpose

Quản lý hồ sơ người dùng và cho phép app con (kể cả của bên thứ ba) đọc dữ liệu người dùng chỉ trong phạm vi người dùng đã đồng ý, đúng mục đích đã nêu, thu hồi được, theo pháp luật Việt Nam về bảo vệ dữ liệu cá nhân.

## ADDED Requirements

### Requirement: Hồ sơ người dùng
Nền tảng SHALL lưu hồ sơ gồm tên hiển thị, ảnh đại diện, email, ngôn ngữ, múi giờ, và tùy chọn số điện thoại cùng thông tin xuất hóa đơn (tên, mã số thuế, địa chỉ). Người dùng MUST xem và sửa được mọi trường của hồ sơ.

#### Scenario: Sửa hồ sơ
- **WHEN** người dùng đổi tên hiển thị thành "Lan Nguyễn"
- **THEN** tên mới hiện trên web, trong launcher và trong các app con đã được cấp scope `profile` ở lần gọi API tiếp theo

### Requirement: Danh mục scope dữ liệu
Dữ liệu người dùng SHALL được chia thành các scope cố định, mỗi scope có mô tả bằng tiếng Việt và tiếng Anh: `profile` (tên, ảnh), `email`, `locale`, `phone`, `billing` (thông tin hóa đơn), `purchases` (quyền sử dụng của chính app đó). App con MUST không truy cập được dữ liệu nằm ngoài các scope này.

#### Scenario: Gọi API ngoài scope
- **GIVEN** app chỉ được cấp scope `profile`
- **WHEN** app gọi API lấy email của người dùng
- **THEN** API trả lỗi `insufficient_scope` và không trả về email

### Requirement: Đồng ý theo từng app và từng scope
Lần đầu một app con yêu cầu dữ liệu, nền tảng SHALL hiện màn hình đồng ý gồm tên app, nhà phát triển, từng scope được yêu cầu cùng mục đích sử dụng do nhà phát triển khai báo, và liên kết tới chính sách quyền riêng tư của app. Người dùng MUST chọn được từng scope không bắt buộc riêng lẻ, và ghi nhận đồng ý MUST lưu thời điểm cùng phiên bản nội dung đã hiện.

#### Scenario: Đồng ý một phần
- **GIVEN** app yêu cầu `profile` (bắt buộc) và `email` (không bắt buộc)
- **WHEN** người dùng chỉ đồng ý `profile`
- **THEN** app nhận token chỉ có scope `profile`, và vẫn chạy được

#### Scenario: Từ chối
- **WHEN** người dùng từ chối màn hình đồng ý
- **THEN** app không nhận token nào và launcher báo app cần đăng nhập để dùng

#### Scenario: App xin thêm scope
- **GIVEN** người dùng đã đồng ý `profile` cho app
- **WHEN** phiên bản mới của app yêu cầu thêm `phone`
- **THEN** màn hình đồng ý chỉ hỏi về `phone` khi app thực sự cần tới, các scope cũ giữ nguyên

### Requirement: Xem và thu hồi quyền
Trang "Ứng dụng đã kết nối" SHALL liệt kê mỗi app đã được cấp dữ liệu, các scope đang cấp và lần truy cập gần nhất. Thu hồi MUST vô hiệu mọi token của app đó cho người dùng này trong vòng 60 giây.

#### Scenario: Thu hồi quyền
- **WHEN** người dùng thu hồi quyền của app "SoundLab"
- **THEN** lần gọi API tiếp theo của SoundLab bằng token cũ nhận lỗi `invalid_token`
- **AND** lần mở SoundLab sau đó hiện lại màn hình đồng ý

### Requirement: Định danh riêng cho từng app
Mỗi app con SHALL nhận một mã định danh người dùng riêng cho app đó (pairwise subject identifier). Hai app khác nhau MUST không thể dùng mã định danh để biết hai lượt dùng là của cùng một người.

#### Scenario: Hai app khác nhau
- **WHEN** cùng một người dùng đăng nhập vào app A và app B
- **THEN** mã định danh `sub` mà A nhận được khác với mã mà B nhận được

### Requirement: Nhật ký truy cập dữ liệu
Nền tảng SHALL ghi lại mỗi lần app con đọc dữ liệu người dùng (app, scope, thời điểm). Người dùng MUST xem được nhật ký này cho 90 ngày gần nhất.

#### Scenario: Xem nhật ký
- **WHEN** người dùng mở chi tiết app "SoundLab" trong "Ứng dụng đã kết nối"
- **THEN** danh sách hiện các lần SoundLab đọc `profile` và `email` kèm thời điểm

### Requirement: Xuất dữ liệu cá nhân
Người dùng SHALL yêu cầu xuất toàn bộ dữ liệu cá nhân nền tảng đang lưu (hồ sơ, đồng ý, nhật ký truy cập, đơn hàng, hóa đơn, dự án F-Builder) thành một file ZIP chứa JSON và các file gốc. Liên kết tải MUST sẵn sàng trong vòng 72 giờ và hết hạn sau 7 ngày.

#### Scenario: Xuất dữ liệu
- **WHEN** người dùng yêu cầu xuất dữ liệu
- **THEN** người dùng nhận email có liên kết tải trong vòng 72 giờ, và file chứa đủ các nhóm dữ liệu trên

### Requirement: Mục đích và căn cứ xử lý
Nền tảng SHALL công bố chính sách quyền riêng tư nêu rõ: dữ liệu thu thập, mục đích, căn cứ xử lý, thời hạn lưu, bên nhận dữ liệu (gồm nhà cung cấp thanh toán, email, AI) và quyền của chủ thể dữ liệu. Nền tảng MUST không dùng dữ liệu người dùng để quảng cáo cho bên thứ ba, và MUST không dùng nội dung dự án F-Builder để huấn luyện mô hình AI nếu người dùng chưa đồng ý riêng.

#### Scenario: Đồng ý huấn luyện AI
- **GIVEN** người dùng chưa bật "Cho phép dùng dự án để cải thiện F-Builder"
- **WHEN** hệ thống gom dữ liệu để cải thiện mô hình
- **THEN** dự án của người dùng này không bị đưa vào

### Requirement: Trách nhiệm của app con với dữ liệu
App con SHALL chỉ dùng dữ liệu nhận được cho mục đích đã khai báo, và MUST xóa dữ liệu của người dùng khi nhận sự kiện `user.revoked` hoặc `user.deleted` từ nền tảng, trong vòng 30 ngày. Thỏa thuận nhà phát triển MUST ràng buộc nghĩa vụ này.

#### Scenario: Người dùng xóa tài khoản
- **WHEN** tài khoản người dùng bị xóa hẳn
- **THEN** mọi app con từng được cấp dữ liệu nhận sự kiện `user.deleted` qua webhook đã ký
