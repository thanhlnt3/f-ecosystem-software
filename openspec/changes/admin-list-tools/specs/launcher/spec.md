# Spec Delta

## Purpose

Bổ sung cho capability `launcher`: quản trị viên mở thẳng từng mục quản trị từ F-Launcher.

## ADDED Requirements

### Requirement: Menu mục quản trị
Với tài khoản có vai trò quản trị, nút "Bảng quản trị" của F-Launcher SHALL mở một menu gồm trang chủ quản trị và các mục mà tài khoản được vào. Chọn một mục SHALL mở trang đó trong trình duyệt. F-Launcher MUST chỉ mở địa chỉ `https` hoặc của máy cục bộ.

#### Scenario: Mở hàng chờ duyệt
- **GIVEN** quản trị viên có vai trò Người duyệt app
- **WHEN** quản trị viên chọn "Hàng chờ duyệt" trong menu "Bảng quản trị"
- **THEN** trình duyệt mở trang hàng chờ duyệt của bảng quản trị
