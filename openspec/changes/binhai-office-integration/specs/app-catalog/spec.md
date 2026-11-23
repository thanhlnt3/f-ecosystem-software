# Spec Delta

## Purpose

Bổ sung cho capability `app-catalog`: app web BinhAI Office trong F-Store, dựng từ mã nguồn đã quét.

## ADDED Requirements

### Requirement: App web BinhAI Office
F-Store SHALL có app web BinhAI Office. Gói app chỉ chứa thông tin app và địa chỉ web; ứng dụng chạy trong trình duyệt. Bản được phục vụ MUST được F-Eco dựng từ mã nguồn tại commit đã quét, với thư viện cài theo lockfile và không chạy script cài đặt của thư viện. Commit đó MUST được quét lại mỗi khi CI chạy và mỗi tuần.

#### Scenario: Mở BinhAI Office
- **GIVEN** BinhAI Office đã được đưa vào kho
- **WHEN** người dùng mở trang BinhAI Office trên F-Store
- **THEN** trang có nút "Mở ứng dụng web" trỏ tới địa chỉ của app

#### Scenario: Bản dựng sẵn khác mã nguồn
- **GIVEN** `app/dist` của tác giả khác bản F-Eco tự dựng từ mã nguồn
- **WHEN** job CI `binhai-office` chạy
- **THEN** job thất bại và liệt kê các tệp khác nhau
