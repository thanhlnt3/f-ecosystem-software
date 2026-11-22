# Spec Delta

## Purpose

Bổ sung cho capability `notifications`: email xác minh và thông báo gửi tới hòm thư thật qua máy chủ SMTP được cấu hình.

## ADDED Requirements

### Requirement: Gửi email qua máy chủ SMTP thật
Nền tảng SHALL gửi email xác minh tài khoản (Kratos) và email thông báo (feco-server) qua cùng một máy chủ SMTP được cấu hình. Thông tin đăng nhập SMTP MUST đọc được từ tệp bí mật không nằm trong repo, và MUST không bị ghi ra log. Khi chưa cấu hình SMTP trên máy dev, email SHALL vào hòm thư thử Mailpit như trước.

#### Scenario: Đăng ký với email Gmail
- **GIVEN** máy dev đã cấu hình SMTP của Gmail bằng tệp `.dev/smtp-url`
- **WHEN** người dùng đăng ký tài khoản với một địa chỉ Gmail
- **THEN** email xác minh được gửi qua máy chủ SMTP đã cấu hình, không vào Mailpit

#### Scenario: Chưa cấu hình SMTP
- **WHEN** máy dev chưa có `.dev/smtp-url` hay `FECO_SMTP_URL`
- **THEN** email xác minh vào Mailpit như trước
