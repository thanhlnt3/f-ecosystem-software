# Spec Delta

## Purpose

Bổ sung cho capability `mobile-launcher`: điện thoại báo app có bản mới rõ ràng như trên web.

## ADDED Requirements

### Requirement: Báo bản mới trên điện thoại
App di động SHALL hiện nhãn "Có bản mới" cho app máy tính có bản mới, cùng một dòng tóm tắt "Số app có bản mới: N. Mở F-Launcher trên máy tính để cập nhật." và một huy hiệu trên tab "Ứng dụng của tôi". App web MUST không có nhãn này, vì app web luôn mở bản mới nhất. Khi người dùng quay lại app từ nền, app di động SHALL tải lại "Ứng dụng của tôi" và thông báo.

#### Scenario: App máy tính có bản mới
- **GIVEN** máy chủ báo app máy tính có bản mới
- **WHEN** người dùng mở tab "Ứng dụng của tôi"
- **THEN** app có nhãn "Có bản mới", có dòng "Số app có bản mới: 1", và tab có huy hiệu 1

#### Scenario: App web
- **GIVEN** máy chủ báo app web có phiên bản mới hơn bản đã thêm
- **WHEN** người dùng mở tab "Ứng dụng của tôi"
- **THEN** app web không có nhãn "Có bản mới"

#### Scenario: Quay lại app
- **GIVEN** app di động đang chạy nền
- **WHEN** người dùng cập nhật app trên máy tính rồi quay lại app di động
- **THEN** "Ứng dụng của tôi" được tải lại và không còn hiện "Có bản mới" cho app đó
