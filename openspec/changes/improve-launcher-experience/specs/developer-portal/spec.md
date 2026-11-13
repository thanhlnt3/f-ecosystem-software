# Spec Delta

## Purpose

Bổ sung cho capability `developer-portal` (thay đổi `add-f-ecosystem-platform`): thông tin hiển thị trong metadata phát hành, và cách đưa app mã nguồn mở có sẵn vào F-Eco.

## ADDED Requirements

### Requirement: Thông tin hiển thị trong metadata phát hành
Khi ký một phiên bản đã duyệt, nền tảng SHALL ghi vào metadata TUF của target:
- mô tả ngắn và nhóm, lấy từ manifest;
- nhà phát triển, lấy từ danh mục;
- địa chỉ mã nguồn và cờ `terminal`, lấy từ manifest nếu có.

Các trường này MUST được ký cùng mã băm của gói. Manifest `f-app.toml` SHALL nhận thêm hai trường không bắt buộc:
- `source`: địa chỉ mã nguồn, MUST là HTTPS;
- `terminal`: true hoặc false.

#### Scenario: Phát hành ghi thông tin hiển thị
- **GIVEN** manifest của "Ghi chú" 1.0.0 có `summary` và `category = "productivity"`
- **WHEN** phiên bản được duyệt và phát hành
- **THEN** target TUF của phiên bản có `summary`, `category = "productivity"` và `developer` của nhà phát triển, được ký cùng mã băm

#### Scenario: Địa chỉ mã nguồn không phải HTTPS
- **WHEN** manifest có `source = "http://github.com/a/b"`
- **THEN** manifest bị từ chối với lỗi "source không hợp lệ: phải là địa chỉ https://"

### Requirement: App mã nguồn mở dựng từ mã nguồn bên ngoài
Nhà phát triển SHALL đưa được một app mã nguồn mở có sẵn (ví dụ trên GitHub) vào F-Eco theo cùng quy trình với mọi app con: manifest, nộp, kiểm duyệt, ký TUF.

Khi manifest có `source`:
- gói nộp lên MUST chứa ít nhất một file giấy phép gốc (`LICENSE*`, `COPYING*` hoặc `NOTICE*`) ở bất kỳ thư mục nào trong gói;
- `f-dev pack` MUST từ chối gói thiếu file này.

App không dùng F-SDK MUST không khai báo scope, và launcher mở app mà không cần mã khởi chạy.

#### Scenario: Gói thiếu giấy phép gốc
- **GIVEN** manifest có `source = "https://github.com/ilai-deutel/kibi"`
- **WHEN** nhà phát triển chạy `f-dev pack` với thư mục chỉ có `kibi.exe`
- **THEN** f-dev báo lỗi gói thiếu file giấy phép gốc và không tạo gói

#### Scenario: App ngoài không dùng F-SDK
- **GIVEN** app "bottom" dựng từ mã nguồn GitHub, manifest không có scope
- **WHEN** người dùng cài và bấm Mở trong launcher
- **THEN** app chạy, không có màn hình đồng ý, và launcher không xin mã khởi chạy cho app
