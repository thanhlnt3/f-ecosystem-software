# Spec Delta

## Purpose

Bổ sung cho capability `launcher`: API dành cho launcher phục vụ cả F-Launcher di động.

## ADDED Requirements

### Requirement: Client di động là app bên thứ nhất
Máy chủ SHALL coi client OAuth `FECO_MOBILE_CLIENT_ID` là app bên thứ nhất như F-Launcher desktop:
- không hiện màn hình đồng ý;
- token của client này được gọi `/v1/wallet`, `/v1/wallet/purchases`, `/v1/catalog/installs`, `/v1/notifications`, `/v1/library`.

`POST /v1/launch-codes` MUST vẫn chỉ nhận token của F-Launcher desktop.

#### Scenario: Ví từ điện thoại
- **GIVEN** người dùng đăng nhập bằng app di động
- **WHEN** app gọi `GET /v1/wallet`
- **THEN** máy chủ trả số dư của người dùng

#### Scenario: Mã khởi chạy từ điện thoại
- **WHEN** app di động gọi `POST /v1/launch-codes`
- **THEN** máy chủ trả 403 `launcher_only`

### Requirement: Ứng dụng của tôi dạng JSON
`GET /v1/library` (token của F-Launcher desktop hoặc di động) SHALL trả các app đã cài, đã thêm hoặc đã mua của người dùng. Mỗi app gồm `id`, `name`, `has_icon`, `installed_version`, `purchased`, `latest`, `web_url`, `update_available`.

#### Scenario: Thư viện có app web
- **GIVEN** người dùng đã thêm ThesisAI
- **WHEN** app gọi `GET /v1/library`
- **THEN** kết quả có ThesisAI kèm `web_url`
