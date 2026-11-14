# Spec Delta

## Purpose

Bổ sung cho capability `launcher`: giao diện tiếng Nhật.

## ADDED Requirements

### Requirement: Tiếng Nhật trong launcher
F-Launcher SHALL có tiếng Nhật (日本語) trong danh sách ngôn ngữ ở Cài đặt, cùng tiếng Việt, tiếng Anh và tiếng Pháp. Mọi chuỗi giao diện launcher MUST có bản tiếng Nhật. Launcher MUST nạp phông chữ tiếng Nhật có sẵn của hệ điều hành, và không đóng gói phông chữ.

#### Scenario: Đổi sang tiếng Nhật
- **WHEN** người dùng chọn 日本語 trong Cài đặt
- **THEN** thanh bên hiện "ライブラリ", "ストア", "通知", "設定" ngay, chữ hiện đúng, và lựa chọn được nhớ cho lần mở sau
