# Spec Delta

## Purpose

Bổ sung cho capability `platform-support`: tiếng Nhật và tùy chỉnh giao diện trên web.

## ADDED Requirements

### Requirement: Tiếng Nhật trên web
Trang web của F-Eco SHALL có giao diện tiếng Nhật bên cạnh tiếng Việt và tiếng Anh, chọn được ở thanh trên cùng và ở trang Giao diện. Trình duyệt ưu tiên tiếng Nhật SHALL được tự nhận. Mọi chuỗi giao diện web MUST có bản tiếng Nhật. Nội dung do hệ thống tạo mà chưa có bản tiếng Nhật SHALL hiện bằng tiếng Anh.

#### Scenario: Chọn tiếng Nhật
- **WHEN** người dùng chọn 日本語
- **THEN** trang đăng nhập hiện tiêu đề tiếng Nhật, và lựa chọn được nhớ cho các trang sau

#### Scenario: Trình duyệt tiếng Nhật
- **GIVEN** trình duyệt gửi `Accept-Language: ja-JP`
- **WHEN** người dùng mở trang chủ lần đầu
- **THEN** trang hiện tiếng Nhật

#### Scenario: Thiếu bản dịch tiếng Nhật
- **WHEN** một lập trình viên thêm khóa chuỗi giao diện web mà không có bản tiếng Nhật
- **THEN** kiểm thử tự động báo lỗi và chỉ ra khóa còn thiếu

### Requirement: Tùy chỉnh giao diện web
Trang Giao diện (`/preferences`) SHALL cho người dùng chọn:
- ngôn ngữ;
- chủ đề: theo hệ thống, sáng hoặc tối;
- màu nhấn;
- phong cách: 10 phong cách giống F-Launcher;
- mật độ: thường hoặc gọn;
- cỡ chữ: 90–125%.

Tùy chỉnh MUST lưu trên trình duyệt (cookie) và áp ngay từ lần vẽ đầu của mọi trang, không cần JavaScript. Giá trị lạ hoặc hỏng MUST về mặc định.

#### Scenario: Đổi sang chủ đề tối và phong cách Trời sao
- **WHEN** người dùng chọn phong cách "Trời sao" ở trang Giao diện rồi lưu
- **THEN** trang tiếp theo có thẻ `<html>` mang `data-style="starry_night"`, và nền trang dùng bảng màu Trời sao

#### Scenario: Cookie hỏng
- **GIVEN** cookie `feco_ui` có `style=khong-ton-tai&size=900`
- **WHEN** người dùng mở một trang
- **THEN** trang dùng phong cách Chân phương và cỡ chữ 125%

### Requirement: Bảng màu chung và đọc được
F-Launcher và trang web SHALL dùng chung một bảng màu cho 10 phong cách. Mọi phong cách MUST đạt độ tương phản WCAG 2.1:
- chữ trên nền: ít nhất 4,5:1;
- chữ mờ: ít nhất 3:1;
- chữ trên nút nhấn: ít nhất 4,5:1.

#### Scenario: CSS khớp bảng màu
- **WHEN** kiểm thử tự động so CSS phong cách của web với bảng màu chung
- **THEN** mọi màu nền, chữ và màu nhấn trùng khớp
