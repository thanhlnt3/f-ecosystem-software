# Proposal

## Why

Một collaborator gửi PR #1 "Polish UI across F-Eco apps" (`87abdb6`). PR làm mới giao diện web, F-Launcher và F-Launcher di động, và đổi giao diện mặc định của F-Launcher. Chủ dự án yêu cầu (23/11/2026) gộp PR nếu không xung đột với `main`, và cập nhật README theo phiên bản hiện tại.

README vẫn ghi phiên bản 1.5.0, và phần tóm tắt tính năng thiếu những gì đã làm từ 1.6.0.

## What Changes

- **Gộp PR #1**:
  - web có trang chào cho người chưa đăng nhập, thanh trên cùng và kiểu chữ mới;
  - F-Launcher mặc định phong cách Trang nhã, chủ đề sáng, ô lưới lớn, cửa sổ 1280 × 800;
  - đầu trang Thư viện có phần "Không gian của bạn";
  - bảng màu Chân phương và Trang nhã chuyển sang tông xanh lục lam; F-Launcher di động dùng cùng màu.
- **Spec**: requirement "Phong cách giao diện" của launcher đổi mặc định từ Chân phương sang Trang nhã, cho khớp mã đã gộp.
- **README** hai thứ tiếng lên 1.20.0, bổ sung tóm tắt tính năng từ 1.6.0 tới nay.

## Capabilities

### Modified Capabilities
- `launcher`: phong cách mặc định là Trang nhã.

## Impact

- Người đã lưu cài đặt giữ nguyên lựa chọn; chỉ cài đặt mới và "Khôi phục mặc định" nhận giao diện mới.
- Không đổi API, cơ sở dữ liệu hay giao thức.
