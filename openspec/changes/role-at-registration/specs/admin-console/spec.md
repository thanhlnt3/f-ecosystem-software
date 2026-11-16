# Spec Delta

## Purpose

Bổ sung cho capability `admin-console`: quản trị viên xác nhận vai trò người dùng chọn khi đăng ký.

## ADDED Requirements

### Requirement: Xác nhận vai trò chọn khi đăng ký
Trang Phân quyền SHALL liệt kê yêu cầu vai trò chọn khi đăng ký cùng các yêu cầu gửi từ trang Vai trò. Mỗi yêu cầu SHALL ghi nguồn ("Chọn khi đăng ký" hoặc "Trang Vai trò") và trạng thái xác minh email.

Quản trị hệ thống SHALL duyệt hoặc từ chối từng yêu cầu:
- duyệt: gán vai trò;
- từ chối: MUST có lý do.

Cả hai trường hợp người dùng SHALL nhận thông báo và email.

#### Scenario: Duyệt vai trò chọn khi đăng ký
- **GIVEN** Mai đăng ký với vai trò Khách hàng doanh nghiệp và chưa xác minh email
- **WHEN** quản trị hệ thống mở trang Phân quyền
- **THEN** yêu cầu của Mai ghi "Chọn khi đăng ký", "Email chưa xác minh", tên pháp lý và mã số thuế; bấm Duyệt thì Mai là Khách hàng doanh nghiệp và nhận email "Đã được cấp vai trò Khách hàng doanh nghiệp"

#### Scenario: Từ chối vai trò chọn khi đăng ký
- **GIVEN** Nam đăng ký với vai trò Lập trình viên cá nhân
- **WHEN** quản trị hệ thống từ chối với lý do "Chưa có hồ sơ năng lực"
- **THEN** Nam vẫn là Người dùng phổ thông và nhận email có lý do đó

#### Scenario: Từ chối không có lý do
- **WHEN** quản trị hệ thống bấm Từ chối mà không nhập lý do
- **THEN** yêu cầu vẫn đang chờ và trang báo cần lý do
