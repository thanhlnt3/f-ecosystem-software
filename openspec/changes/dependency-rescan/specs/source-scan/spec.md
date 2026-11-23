# Spec Delta

## Purpose

Bổ sung cho capability `source-scan`: phát hiện lỗ hổng thư viện công bố sau khi app đã phát hành.

## ADDED Requirements

### Requirement: Quét lại mã nguồn định kỳ
Nền tảng SHALL quét lại mã nguồn của mọi phiên bản đang phát hành ít nhất 7 ngày một lần, gồm tra lại lỗ hổng thư viện trên OSV.dev. Khi có phát hiện mức `high` hoặc `critical` chưa có ở lần quét trước, nền tảng SHALL gửi email cho nhà phát triển, nêu thư viện, mã lỗ hổng và cách sửa, và SHALL tạo cảnh báo cho quản trị viên kèm số người đang cài. Mỗi lỗ hổng MUST chỉ được báo một lần cho mỗi phiên bản. Nền tảng MUST không tự thu hồi phiên bản chỉ vì lỗ hổng thư viện.

#### Scenario: Lỗ hổng mới công bố
- **GIVEN** phiên bản 1.0.0 đã phát hành, lần quét trước không có phát hiện `high`
- **WHEN** OSV.dev công bố một lỗ hổng HIGH cho một thư viện trong lockfile, và việc quét lại chạy
- **THEN** nhà phát triển nhận email nêu mã lỗ hổng
- **AND** quản trị viên có cảnh báo `new_vulnerability`
- **AND** phiên bản vẫn đang phát hành

#### Scenario: Không báo lại
- **GIVEN** lỗ hổng đã được báo ở lần quét trước
- **WHEN** việc quét lại chạy lần nữa
- **THEN** không có email hay cảnh báo mới

#### Scenario: Báo cáo trên trang phiên bản
- **WHEN** việc quét lại xong
- **THEN** trang phiên bản của nhà phát triển hiện báo cáo mới

### Requirement: Quét lại app tích hợp định kỳ
App tích hợp từ bên ngoài (Mentor, ThesisAI) SHALL được quét lại ở commit đã ghi mỗi tuần, kể cả khi repo F-Eco không có thay đổi. Phát hiện `critical` chưa được cho phép MUST làm lần quét thất bại để chủ repo được báo.

#### Scenario: Quét hằng tuần
- **WHEN** tới thứ Hai hằng tuần
- **THEN** workflow `integration-scan` quét Mentor và ThesisAI, và báo cáo được đính kèm
