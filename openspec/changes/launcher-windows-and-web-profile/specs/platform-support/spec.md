# Spec Delta

## Purpose

Bổ sung cho capability `platform-support`: tiếng Pháp trên web (QUESTIONS 79).

## ADDED Requirements

### Requirement: Tiếng Pháp trên web
Trang web của F-Eco SHALL có giao diện tiếng Pháp bên cạnh tiếng Việt, tiếng Anh và tiếng Nhật, chọn được ở thanh trên cùng và ở trang Giao diện. Trình duyệt ưu tiên tiếng Pháp SHALL được tự nhận. Mọi chuỗi giao diện web MUST có bản tiếng Pháp. Nội dung do hệ thống tạo chưa có bản tiếng Pháp SHALL hiện bằng tiếng Anh.

#### Scenario: Trình duyệt tiếng Pháp
- **GIVEN** trình duyệt gửi `Accept-Language: fr-FR`
- **WHEN** người dùng mở trang đăng nhập lần đầu
- **THEN** trang hiện tiếng Pháp ("Se connecter") với `<html lang="fr">`

#### Scenario: Thiếu bản dịch tiếng Pháp
- **WHEN** một lập trình viên thêm khóa chuỗi giao diện web mà không có bản tiếng Pháp
- **THEN** kiểm thử tự động báo lỗi và chỉ ra khóa còn thiếu
