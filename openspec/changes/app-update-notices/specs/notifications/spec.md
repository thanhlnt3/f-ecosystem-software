# Spec Delta

## Purpose

Bổ sung cho capability `notifications`: khi nào thông báo cập nhật app được gửi và gửi cho ai.

## ADDED Requirements

### Requirement: Thông báo cập nhật app đúng lúc, đúng người
Nền tảng SHALL gửi thông báo cập nhật app khi một phiên bản được phát hành trên kênh ổn định cho mọi thiết bị (100%). Mỗi phiên bản MUST được báo nhiều nhất một lần. Thông báo MUST chỉ gửi cho người đang cài bản cũ hơn. Phiên bản đang phát hành dần, bản trên kênh beta và bản đã tạm dừng MUST không tạo thông báo. Phiên bản phát hành bằng công cụ vận hành SHALL được báo trong vòng 10 phút. Người đã tắt loại "Cập nhật app" SHALL vẫn thấy thông báo trong trung tâm thông báo nhưng không nhận email.

#### Scenario: Phát hành dần chưa báo
- **GIVEN** người dùng đã cài app ở bản 1.0.0
- **WHEN** nhà phát triển phát hành bản 1.1.0 cho 10% thiết bị
- **THEN** người dùng chưa nhận thông báo cập nhật

#### Scenario: Phát hành cho mọi người
- **GIVEN** bản 1.1.0 đang phát hành cho 10% thiết bị
- **WHEN** nhà phát triển nâng lên 100%
- **THEN** người đang cài bản 1.0.0 nhận đúng một thông báo "có bản cập nhật 1.1.0"

#### Scenario: Không báo hai lần
- **GIVEN** bản 1.1.0 đã được báo
- **WHEN** nhà phát triển phát hành lại bản 1.1.0
- **THEN** không có thông báo thứ hai

#### Scenario: Người đã có bản mới
- **GIVEN** người dùng đã cập nhật lên 1.1.0 trong lúc phát hành dần
- **WHEN** bản 1.1.0 được phát hành cho mọi người
- **THEN** người dùng không nhận thông báo cập nhật

#### Scenario: Kênh beta
- **WHEN** nhà phát triển phát hành bản 1.2.0 trên kênh beta
- **THEN** không ai nhận thông báo cập nhật

#### Scenario: Phát hành bằng công cụ vận hành
- **WHEN** quản trị phát hành một phiên bản bằng `feco-catalog release`
- **THEN** người đang cài bản cũ hơn nhận thông báo cập nhật trong vòng 10 phút
