# Spec Delta

## Purpose

Bổ sung cho capability `app-catalog`: app desktop VPdf trong F-Store, dựng từ mã nguồn đã quét.

## ADDED Requirements

### Requirement: App desktop VPdf
F-Store SHALL có app desktop VPdf cho Windows x64. Gói MUST được F-Eco dựng từ mã nguồn tại commit đã quét. Thư viện native trong gói MUST khớp SHA-256 ghim trong repo VPdf. Gói MUST kèm giấy phép của VPdf và của mọi thành phần bên thứ ba. Thành phần có giấy phép ngoài danh sách cho phép MUST có ngoại lệ đã duyệt, ghi người duyệt và ngày duyệt.

#### Scenario: Cài và mở VPdf
- **GIVEN** VPdf đã được ký vào kho
- **WHEN** người dùng Windows chọn VPdf trong Cửa hàng của F-Launcher, bấm "Cài" rồi "Mở"
- **THEN** F-Launcher tải, kiểm tra và cài gói, rồi mở `vpdf-app.exe`

#### Scenario: Gói đủ giấy phép
- **WHEN** job CI `vpdf` dựng gói
- **THEN** gói có `vpdf-app.exe`, `pdfium.dll`, `qpdf30.dll`, `licenses/LICENSE-MIT`, `licenses/LICENSE-APACHE`, giấy phép PDFium và danh sách ngoại lệ đã duyệt
