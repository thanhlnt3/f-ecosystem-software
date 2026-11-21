# Spec Delta

## Purpose

Bổ sung cho capability `admin-console`: lọc, sắp xếp và tìm kiếm trong các danh sách quản trị, dùng được trên màn hình điện thoại, và cho ứng dụng biết các mục quản trị.

## ADDED Requirements

### Requirement: Lọc, sắp xếp và tìm kiếm danh sách quản trị
Mỗi danh sách của bảng quản trị SHALL có tìm kiếm, lọc theo khoảng ngày, lọc theo trạng thái (khi có) và sắp xếp theo thời gian hoặc người gửi. Tìm kiếm MUST không phân biệt hoa thường và dấu tiếng Việt. Điều kiện lọc SHALL nằm trên địa chỉ trang, và trang SHALL ghi số dòng đang hiện trên tổng số. Khi chưa đặt điều kiện, danh sách MUST giữ thứ tự ưu tiên hiện nay.

#### Scenario: Tìm theo người gửi
- **GIVEN** hàng chờ duyệt có app của "Công ty Âu Lạc" và của "Beta Studio"
- **WHEN** quản trị viên tìm "au lac"
- **THEN** chỉ còn app của "Công ty Âu Lạc", và trang ghi số dòng đang hiện trên tổng số

#### Scenario: Sắp xếp theo người gửi
- **WHEN** quản trị viên chọn sắp xếp "Người gửi A–Z" ở danh sách đơn xây app
- **THEN** các đơn xếp theo email khách hàng tăng dần

#### Scenario: Lọc theo thời gian
- **WHEN** quản trị viên đặt "Từ" và "Đến" là cùng một ngày
- **THEN** chỉ còn các dòng có thời gian trong ngày đó

#### Scenario: Lọc trạng thái
- **GIVEN** có một báo cáo tin nhắn đã xử lý và một báo cáo chưa xử lý
- **WHEN** quản trị viên lọc "Đã xử lý"
- **THEN** chỉ còn báo cáo đã xử lý

#### Scenario: Đơn đã xong
- **GIVEN** một đơn đã hoàn thành
- **WHEN** quản trị viên mở danh sách đơn mà không lọc
- **THEN** đơn đó không có trong danh sách
- **AND** khi lọc "Tất cả" thì đơn đó hiện ra

#### Scenario: Xóa lọc
- **GIVEN** danh sách đang có điều kiện lọc
- **WHEN** quản trị viên bấm "Xóa lọc"
- **THEN** danh sách trở lại đầy đủ theo thứ tự mặc định

### Requirement: Mục quản trị cho ứng dụng
`GET /v1/account/role` SHALL trả danh sách các mục quản trị mà tài khoản được vào (khóa và địa chỉ trang web). Tài khoản không có vai trò quản trị MUST nhận danh sách rỗng.

#### Scenario: Người duyệt app
- **GIVEN** tài khoản chỉ có vai trò Người duyệt app
- **WHEN** F-Launcher hỏi vai trò tài khoản
- **THEN** `admin_sections` chỉ có mục `reviews` với địa chỉ `/admin/reviews`
