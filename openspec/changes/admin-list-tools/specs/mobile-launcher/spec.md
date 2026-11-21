# Spec Delta

## Purpose

Bổ sung cho capability `mobile-launcher`: quản trị viên vào bảng quản trị từ điện thoại.

## ADDED Requirements

### Requirement: Bảng quản trị trên điện thoại
Với tài khoản có vai trò quản trị, tab Tài khoản của app di động SHALL có thẻ "Bảng quản trị" liệt kê các mục mà tài khoản được vào. Chọn một mục SHALL mở trang web tương ứng, nơi có lọc, sắp xếp và tìm kiếm. Tài khoản không có vai trò quản trị MUST không thấy thẻ này.

#### Scenario: Quản trị viên mở báo cáo tin nhắn
- **GIVEN** quản trị viên có vai trò Hỗ trợ
- **WHEN** quản trị viên chọn "Báo cáo tin nhắn" trong thẻ "Bảng quản trị"
- **THEN** trình duyệt mở trang báo cáo tin nhắn của bảng quản trị

#### Scenario: Người dùng thường
- **WHEN** người dùng không có vai trò quản trị mở tab Tài khoản
- **THEN** không có thẻ "Bảng quản trị"
