# Spec Delta

## Purpose

Bổ sung cho capability `launcher`: F-Launcher tự cập nhật qua kho phân phối đã ký.

## ADDED Requirements

### Requirement: Tự cập nhật qua kho phân phối
F-Launcher SHALL phát hiện bản F-Launcher mới hơn trong kho phân phối sau mỗi lần đồng bộ, tải về và kiểm tra bằng chuỗi tin cậy của kho (chữ ký, hash, độ dài) rồi chờ cài. F-Launcher SHALL cài bản mới khi khởi động lại, và SHALL giữ bản cũ nếu cài không thành công. Gói không qua kiểm tra MUST bị từ chối và ghi sự kiện bảo mật. Gói F-Launcher MUST không hiện trong Cửa hàng.

#### Scenario: Có bản launcher mới
- **GIVEN** kho phân phối có gói F-Launcher 99.0.0 đã ký cho nền tảng của máy
- **WHEN** F-Launcher đồng bộ
- **THEN** F-Launcher tải về, kiểm tra và báo "Khởi động lại để cập nhật"
- **AND** lần khởi động sau chạy bản 99.0.0

#### Scenario: Gói bị sửa
- **GIVEN** gói F-Launcher trên máy chủ bị sửa sau khi ký
- **WHEN** F-Launcher tải bản mới
- **THEN** gói bị từ chối, không có gì chờ cài, và nhật ký có sự kiện `invalid_package`

#### Scenario: Cài không thành công
- **GIVEN** đã có bản mới chờ cài
- **WHEN** chép tệp mới thất bại lúc khởi động
- **THEN** F-Launcher vẫn chạy bản cũ

#### Scenario: Không hiện trong Cửa hàng
- **WHEN** người dùng mở Cửa hàng
- **THEN** gói F-Launcher không có trong danh sách
