# Spec Delta

## Purpose

Bổ sung cho capability `platform-security`: các quy tắc rút ra từ rà soát bảo mật trước khi phát hành production.

## ADDED Requirements

### Requirement: Liên kết do người dùng nhập chỉ là http hoặc https
Liên kết người dùng nhập và hiển thị cho người khác (ví dụ liên kết bàn giao dự án) MUST là địa chỉ `http://` hoặc `https://` có tên máy. Địa chỉ khác MUST bị từ chối khi lưu và MUST không hiện thành liên kết bấm được.

#### Scenario: Liên kết javascript
- **WHEN** người nhận việc bàn giao với liên kết `javascript:fetch('/projects/x/accept')`
- **THEN** F-Eco từ chối với lỗi biểu mẫu và không lưu liên kết

### Requirement: Địa chỉ IP của người dùng
F-Eco MUST chỉ tin `X-Forwarded-For` khi kết nối tới từ một proxy khai báo trong `FECO_TRUSTED_PROXIES`. Khi đó F-Eco SHALL lấy địa chỉ ngoài cùng bên phải không phải proxy tin cậy. Ngược lại F-Eco SHALL dùng địa chỉ của kết nối. Giới hạn số yêu cầu theo IP và nhật ký kiểm toán dùng địa chỉ này.

#### Scenario: Giả X-Forwarded-For
- **GIVEN** không khai báo proxy tin cậy
- **WHEN** một máy gửi 2.000 yêu cầu trong một phút, mỗi yêu cầu một `X-Forwarded-For` khác nhau
- **THEN** giới hạn theo IP vẫn áp dụng cho địa chỉ thật của máy đó

### Requirement: Không tự duyệt việc của chính mình
Quản trị viên MUST không duyệt hay đánh dấu đã chuyển lệnh rút của chính mình, không xử lý tranh chấp của dự án mình là một bên, và không duyệt app của tài khoản nhà phát triển mình là thành viên.

#### Scenario: Tự duyệt lệnh rút
- **GIVEN** quản trị viên Tài chính A tạo lệnh rút từ ví của mình
- **WHEN** A bấm Duyệt lệnh rút đó
- **THEN** thao tác bị từ chối và lệnh vẫn chờ duyệt

### Requirement: Thao tác tiền chỉ thực hiện một lần
Gửi lệnh hoàn tới cổng, kết thúc lệnh hoàn, và chấp nhận báo giá MUST chỉ thực hiện một lần dù có hai yêu cầu đồng thời. Lệnh hoàn mà cổng không trả lời rõ MUST giữ trạng thái "đang xử lý", không trả F-Coin về ví và không cho hoàn lại lần nữa, cho tới khi có kết quả từ cổng hoặc quản trị viên đối chiếu.

#### Scenario: Cổng không trả lời lệnh hoàn
- **GIVEN** lệnh rút hoàn về giao dịch nạp đã được duyệt
- **WHEN** cổng thanh toán không trả lời khi nhận lệnh hoàn
- **THEN** lệnh hoàn ở trạng thái "đang xử lý", F-Coin vẫn bị giữ, và quản trị viên nhận cảnh báo đối chiếu

### Requirement: Lệnh nạp hết hạn
Lệnh nạp F-Coin MUST chỉ thanh toán được trong 30 phút kể từ khi tạo và khi tỷ giá chưa đổi. Quá hạn thì lệnh nạp bị hủy và người dùng tạo lệnh mới.

#### Scenario: Trả lệnh nạp cũ sau khi tỷ giá tăng
- **GIVEN** lệnh nạp 50.000 F-Coin tạo theo tỷ giá 1.000 ₫
- **WHEN** tỷ giá đổi thành 1.200 ₫ rồi người dùng mở trang thanh toán của lệnh đó
- **THEN** lệnh nạp bị hủy và trang báo "Lệnh nạp đã hết hạn hoặc tỷ giá đã thay đổi"

### Requirement: Gói cài đặt an toàn trên Windows
Gói có hai mục trùng đường dẫn khi bỏ phân biệt hoa thường (hoặc khác nhau chỉ ở `./`) MUST bị từ chối khi tải lên và khi cài. Tệp chạy MUST chỉ gồm `[A-Za-z0-9._/-]`. Logo PNG MUST không quá 1024 × 1024 px.

#### Scenario: Hai mục trùng tên
- **WHEN** nhà phát triển tải lên gói có cả `SoundLab.exe` và `soundlab.exe`
- **THEN** gói bị từ chối, báo hai mục trùng đường dẫn
