# Spec Delta

## Purpose

Bổ sung cho capability `mobile-launcher`: thông báo lỗi tải dữ liệu đúng nguyên nhân và tự thử lại.

## ADDED Requirements

### Requirement: Thông báo lỗi tải dữ liệu đúng nguyên nhân
App di động MUST chỉ báo "Không kết nối được máy chủ F-Eco" khi không kết nối được. Khi máy chủ có trả lời nhưng báo lỗi, app SHALL báo máy chủ đang gặp lỗi. Khi người dùng quay lại app mà lần tải trước lỗi, app SHALL tự tải lại.

#### Scenario: Máy chủ trả lỗi
- **GIVEN** máy chủ trả lỗi 503 khi tải "Ứng dụng của tôi"
- **WHEN** app di động tải dữ liệu
- **THEN** app báo "Máy chủ F-Eco đang gặp lỗi", không báo mất kết nối

#### Scenario: Quay lại sau lỗi
- **GIVEN** lần tải trước bị lỗi
- **WHEN** người dùng quay lại app và máy chủ đã hoạt động
- **THEN** app tải lại và thông báo lỗi biến mất
