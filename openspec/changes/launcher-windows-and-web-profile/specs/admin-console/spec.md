# Spec Delta

## Purpose

Bổ sung cho capability `admin-console`: đếm người dùng trên dashboard có bộ nhớ đệm (QUESTIONS 80).

## ADDED Requirements

### Requirement: Đếm người dùng có bộ nhớ đệm
Thẻ số người dùng trên dashboard quản trị SHALL dùng con số đếm gần nhất nếu con số đó chưa quá 10 phút. Mở dashboard nhiều lần trong 10 phút MUST không gọi lại dịch vụ tài khoản để liệt kê danh tính.

#### Scenario: Mở dashboard hai lần liền
- **GIVEN** dashboard vừa đếm được N người dùng
- **WHEN** có thêm một người đăng ký, rồi quản trị viên mở lại dashboard trong vòng 10 phút
- **THEN** thẻ vẫn ghi N; sau 10 phút thẻ ghi N + 1
