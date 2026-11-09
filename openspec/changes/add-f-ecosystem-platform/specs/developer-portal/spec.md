# Spec Delta

## Purpose

Cổng nhà phát triển giúp lập trình viên và công ty bên thứ ba đăng ký, khai báo app, nộp phiên bản, theo dõi kiểm duyệt và phát hành app con trên F-Eco.

## ADDED Requirements

### Requirement: Đăng ký nhà phát triển
Người dùng đã xác minh email SHALL đăng ký làm nhà phát triển dưới dạng cá nhân hoặc tổ chức, chấp nhận Thỏa thuận nhà phát triển (ghi lại phiên bản và thời điểm chấp nhận), và bật xác thực hai lớp. Tổ chức MUST cung cấp tên pháp lý, mã số doanh nghiệp và tên miền; nền tảng xác minh tên miền bằng bản ghi DNS TXT.

#### Scenario: Xác minh tên miền
- **WHEN** tổ chức "Acme" thêm đúng bản ghi TXT do nền tảng cấp vào `acme.vn`
- **THEN** tài khoản nhà phát triển của Acme nhận dấu "Đã xác minh"

#### Scenario: Chưa chấp nhận thỏa thuận
- **WHEN** người dùng chưa chấp nhận Thỏa thuận nhà phát triển cố tạo app
- **THEN** nền tảng yêu cầu đọc và chấp nhận thỏa thuận trước

### Requirement: Nhóm và vai trò
Tài khoản tổ chức SHALL mời được thành viên với vai trò Chủ sở hữu, Quản trị, Phát triển hoặc Chỉ xem. Chỉ Chủ sở hữu và Quản trị MUST phát hành được phiên bản, và chỉ Chủ sở hữu MUST xóa được app.

#### Scenario: Phát triển viên phát hành
- **WHEN** thành viên có vai trò Phát triển bấm Phát hành
- **THEN** nền tảng từ chối và báo cần vai trò Quản trị hoặc Chủ sở hữu

### Requirement: Manifest app
Mỗi phiên bản SHALL kèm file `f-app.toml` khai báo: mã app duy nhất, tên, phiên bản SemVer, hệ điều hành và kiến trúc hỗ trợ, tệp chạy chính, scope dữ liệu cần dùng cùng mục đích từng scope, quyền hệ thống (mạng, camera, micro, thư mục người dùng), địa chỉ webhook, license, liên kết chính sách quyền riêng tư. Manifest sai cú pháp hoặc thiếu trường bắt buộc MUST bị từ chối kèm thông báo chỉ rõ lỗi.

#### Scenario: Thiếu mục đích của scope
- **WHEN** manifest khai báo scope `email` nhưng không có mục đích
- **THEN** việc nộp bị từ chối với lỗi "scopes.email.purpose là bắt buộc"

#### Scenario: Phiên bản không tăng
- **GIVEN** đã phát hành 1.2.0
- **WHEN** nhà phát triển nộp manifest phiên bản 1.1.9
- **THEN** việc nộp bị từ chối vì phiên bản phải lớn hơn bản đã phát hành

### Requirement: Nộp gói phiên bản
Nhà phát triển SHALL tải gói lên cho từng hệ điều hành và kiến trúc qua cổng web hoặc CLI `f-dev publish`, kèm SHA-256. Gói cho Windows MUST được ký Authenticode, và gói cho macOS MUST được ký Developer ID và notarize, bằng chứng thư của chính nhà phát triển. Gói tối đa 2 GB.

#### Scenario: Gói Windows chưa ký
- **WHEN** nhà phát triển nộp gói `.exe` cho Windows chưa được ký
- **THEN** việc nộp bị từ chối với lỗi "Gói Windows phải được ký Authenticode"

#### Scenario: Nộp bằng CLI
- **WHEN** chạy `f-dev publish --manifest f-app.toml --package dist/` với token nhà phát triển hợp lệ
- **THEN** phiên bản được tạo ở trạng thái "Chờ duyệt" và lệnh in ra mã phiên bản

### Requirement: Kênh phát hành và phát hành dần
Nhà phát triển SHALL chọn kênh `stable` hoặc `beta` cho mỗi phiên bản đã duyệt, và phát hành dần theo phần trăm người dùng (ví dụ 10%, 50%, 100%). Nhà phát triển MUST dừng được việc phát hành dần bất kỳ lúc nào.

#### Scenario: Phát hành 10%
- **WHEN** nhà phát triển phát hành SoundLab 1.3.0 cho 10% người dùng `stable`
- **THEN** khoảng 10% launcher đang có SoundLab nhận bản cập nhật, phần còn lại vẫn ở 1.2.0

### Requirement: Thông tin đăng nhập cho app
Mỗi app SHALL có client ID và các địa chỉ chuyển hướng đã đăng ký. App desktop MUST dùng client công khai với PKCE, không có client secret nhúng trong gói. App có máy chủ riêng MUST nhận client secret xoay vòng được.

#### Scenario: Địa chỉ chuyển hướng lạ
- **WHEN** luồng đăng nhập dùng một địa chỉ chuyển hướng chưa đăng ký cho app
- **THEN** máy chủ xác thực từ chối và không cấp mã

### Requirement: Số liệu cho nhà phát triển
Cổng SHALL hiện số lượt cài, số người dùng hoạt động trong 7 và 30 ngày, phân bố phiên bản, số lần app bị lỗi khi chạy (nếu app dùng F-SDK để báo), và điểm đánh giá. Số liệu MUST là số tổng hợp, không cho nhà phát triển thấy danh tính người dùng.

#### Scenario: Xem số liệu
- **WHEN** nhà phát triển mở trang số liệu của SoundLab
- **THEN** trang hiện các số tổng hợp và không có email hay tên người dùng nào

### Requirement: Tài liệu và môi trường thử
Cổng SHALL cung cấp tài liệu API và SDK, app mẫu, và môi trường sandbox có tài khoản người dùng thử. Dữ liệu trong sandbox MUST tách hẳn khỏi dữ liệu thật.

#### Scenario: Thử đăng nhập trong sandbox
- **WHEN** nhà phát triển cấu hình app trỏ tới sandbox và đăng nhập bằng tài khoản thử
- **THEN** app nhận token của sandbox, và token này không dùng được với API thật
