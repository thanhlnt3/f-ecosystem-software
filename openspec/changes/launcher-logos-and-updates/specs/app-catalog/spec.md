# Spec Delta

## Purpose

Bổ sung cho capability `app-catalog`: logo app trên F-Store và trang "Ứng dụng của tôi" trên web.

## ADDED Requirements

### Requirement: Logo app trên F-Store
Danh mục F-Store SHALL lưu logo PNG của app (tối đa 64 KB) và phục vụ ở `/store/{id}/icon.png`. Danh sách và trang app hiện logo đó; app không có logo hiện ô chữ cái đầu. Cập nhật thông tin app mà không kèm logo MUST giữ logo cũ. Trang web SHALL dùng logo F-Launcher làm biểu tượng trang (`/favicon.svg`) và logo trên thanh trên cùng.

#### Scenario: Logo app trên F-Store
- **GIVEN** app có logo trong danh mục
- **WHEN** người dùng mở trang app trên F-Store
- **THEN** trang hiện ảnh `/store/{id}/icon.png`, và ảnh đó là PNG của app

#### Scenario: App không có logo
- **WHEN** trình duyệt hỏi `/store/{id}/icon.png` của app không có logo
- **THEN** trả `404`, và trang app hiện ô chữ cái đầu

### Requirement: Ứng dụng của tôi trên web
Web SHALL có trang "Ứng dụng của tôi" (`/library`), cùng nội dung với Thư viện của F-Launcher:
- liệt kê app người dùng đã cài qua launcher (theo lượt cài launcher báo về) và app đã mua bằng F-Coin;
- mỗi app có phiên bản đã cài và phiên bản mới nhất;
- app có bản mới hơn bản đã cài MUST có nhãn "Có bản mới", và trang báo "Mở F-Launcher để cập nhật".

#### Scenario: Ứng dụng của tôi trên web
- **GIVEN** người dùng đã cài app bản 1.0.0 qua launcher
- **WHEN** app phát hành bản 1.1.0 và người dùng mở `/library`
- **THEN** app có nhãn "Có bản mới", trang báo "Mở F-Launcher để cập nhật"

#### Scenario: App đã mua chưa cài
- **GIVEN** người dùng đã mua app bằng F-Coin nhưng chưa cài
- **WHEN** người dùng mở `/library`
- **THEN** app có trong danh sách với nhãn "Đã mua"
