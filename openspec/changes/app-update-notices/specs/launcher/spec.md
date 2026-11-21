# Spec Delta

## Purpose

Bổ sung cho capability `launcher`: F-Launcher giữ cho máy chủ biết phiên bản đang cài, và đếm bản cập nhật nhất quán.

## ADDED Requirements

### Requirement: Báo máy chủ phiên bản đang cài
F-Launcher SHALL báo máy chủ phiên bản đang cài sau mỗi lần cài, cập nhật (một app, tất cả, hay tự cập nhật) hoặc quay lại bản trước. F-Launcher SHALL báo máy chủ khi gỡ app. Báo lỗi MUST không làm việc cài hay gỡ thất bại.

#### Scenario: Cập nhật rồi báo máy chủ
- **WHEN** người dùng cập nhật app từ 1.0.0 lên 1.1.0
- **THEN** F-Launcher báo máy chủ phiên bản 1.1.0

#### Scenario: Gỡ rồi báo máy chủ
- **WHEN** người dùng gỡ app
- **THEN** F-Launcher báo máy chủ app đã bị gỡ

### Requirement: Bản đã bỏ qua không được đếm
Huy hiệu của Thư viện, nút "Cập nhật tất cả" và tự cập nhật MUST không tính phiên bản người dùng đã chọn "Bỏ qua bản này". Người dùng SHALL vẫn cài được bản đó bằng mục "Cập nhật lên" của từng app.

#### Scenario: Cập nhật tất cả bỏ qua bản đã bỏ qua
- **GIVEN** app A có bản 2.0.0 mà người dùng đã bỏ qua, app B có bản mới
- **WHEN** người dùng bấm "Cập nhật tất cả"
- **THEN** chỉ app B được cập nhật, và nút ghi "Cập nhật tất cả (1)"
