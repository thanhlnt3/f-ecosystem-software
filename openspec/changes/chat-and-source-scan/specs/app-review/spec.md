# Spec Delta

## Purpose

Bổ sung cho capability `app-review`: kiểm tra tự động gồm cả quét mã nguồn (capability `source-scan`).

## ADDED Requirements

### Requirement: Kiểm tra tự động gồm mã nguồn
Khi phiên bản có mã nguồn, kiểm tra tự động SHALL quét mã nguồn sau khi gói qua quét mã độc và trước khi quyết định duyệt tay. Phiên bản có phát hiện mã nguồn mức `critical` MUST không tới hàng duyệt tay. Phiên bản có phát hiện mức `high` MUST không được tự duyệt.

#### Scenario: Thứ tự kiểm tra
- **GIVEN** gói có mã độc và mã nguồn có bí mật lộ
- **WHEN** kiểm tra tự động chạy
- **THEN** phiên bản bị từ chối vì mã độc (`reject_code` `malware`) và mã nguồn không được quét
