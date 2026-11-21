# Spec Delta

## Purpose

Bổ sung cho capability `app-catalog`: "Ứng dụng của tôi" khớp với những gì F-Launcher cài được, và app đã gỡ không còn trong danh sách.

## ADDED Requirements

### Requirement: Bản mới nhất theo kênh ổn định
"Ứng dụng của tôi" trên web và `GET /v1/library` SHALL tính "Có bản mới" theo phiên bản mới nhất được phát hành trên kênh ổn định cho mọi thiết bị. Bản đang phát hành dần, bản beta và bản đã tạm dừng MUST không làm hiện "Có bản mới".

#### Scenario: Bản đang phát hành dần
- **GIVEN** người dùng đã cài app ở bản 1.0.0
- **WHEN** bản 1.1.0 đang phát hành cho 10% thiết bị
- **THEN** "Ứng dụng của tôi" chưa hiện "Có bản mới"

#### Scenario: Bản đã tạm dừng
- **GIVEN** bản 1.1.0 đã phát hành cho mọi người rồi bị tạm dừng
- **WHEN** người dùng mở "Ứng dụng của tôi"
- **THEN** app không hiện "Có bản mới"

### Requirement: F-Launcher báo cập nhật và gỡ app
Nền tảng SHALL nhận báo từ F-Launcher khi người dùng gỡ một app (`DELETE /v1/catalog/installs/{app}`). Sau đó app MUST không còn trong "Ứng dụng của tôi", trừ khi người dùng đã mua app. Người dùng đó MUST không còn nhận thông báo cập nhật của app.

#### Scenario: Đã cập nhật trên máy tính
- **GIVEN** "Ứng dụng của tôi" hiện "Có bản mới" cho app
- **WHEN** F-Launcher cập nhật app và báo phiên bản mới
- **THEN** "Ứng dụng của tôi" trên web và di động không còn hiện "Có bản mới"

#### Scenario: Gỡ app
- **WHEN** người dùng gỡ app trong F-Launcher
- **THEN** app không còn trong "Ứng dụng của tôi"
- **AND** người dùng không nhận thông báo cập nhật của app nữa
