# Proposal

## Why

Chủ dự án yêu cầu (27/11/2026) chỉnh giao diện F-Builder cho tương thích với F-Launcher, và sửa lỗi phông chữ tiếng Việt.

- **Phông chữ**: F-Builder dùng DM Sans nhúng sẵn. Bản DM Sans trong `assets/fonts` thiếu 70 chữ tiếng Việt có dấu (ả, ạ, ằ, ơ, ư, ự…) và mũi tên (→ ← ↑ ↓). Bản dựng không có bộ phông tùy chọn `F_BUILDER_FONTS_DIR`, như bản dựng trên máy dev, lấy các chữ đó từ phông mặc định của egui (khác nét, khác độ cao) hoặc hiện ô vuông, ví dụ "Settings □ AI".
- **Giao diện**: F-Builder dùng giao diện "Studio" tối, màu than chì, nút chính tô màu chữ. F-Launcher dùng:
  - phông hệ thống (Segoe UI trên Windows);
  - nền sáng xanh nhạt, thanh bên trắng;
  - phong cách Trang nhã bo góc lớn, nút chính tô màu nhấn xanh lục lam.

  Mở F-Builder từ F-Launcher trông như hai sản phẩm khác nhau.

## What Changes

- **Phông chữ hệ thống như F-Launcher**:
  - F-Builder đọc phông giao diện của hệ điều hành theo cùng danh sách với F-Launcher: Segoe UI, Arial, Tahoma trên Windows; Arial trên macOS; Noto Sans, DejaVu Sans trên Linux. Nếu có, F-Builder đọc thêm bản đậm của cùng họ phông.
  - F-Builder chỉ nhận phông đọc được và có đủ chữ tiếng Việt; phông hỏng thì bỏ qua, không làm app dừng.
  - `F_BUILDER_UI_FONT=<đường dẫn .ttf>` chọn phông khác.
  - DM Sans và bộ phông tùy chọn vẫn là phông dự phòng.
- **Bảng màu và hình khối theo F-Launcher**:
  - Giao diện sáng và tối lấy bảng màu "Chân phương/Trang nhã" của F-Launcher: nền, thanh bên, thẻ, chữ, viền.
  - Bo góc theo phong cách Trang nhã.
  - Màu nhấn là màu F-Eco `#0f6b5c`.
  - Nút chính tô màu nhấn với chữ trắng, như nút "Mở" của F-Launcher.
- **Mặc định giao diện sáng**, như F-Launcher. Người đã chọn giao diện giữ lựa chọn của mình.

## Capabilities

### Modified Capabilities
- `app-builder`: thêm requirement "Giao diện F-Builder đồng bộ với F-Launcher".

## Impact

- `clients/f-builder/crates/ui-egui`: `theme.rs` (bảng màu, phông), `widgets.rs` (nút chính).
- `clients/f-builder/crates/engine`: `settings.rs` (giao diện mặc định).
- `skrifa` (đã có trong `Cargo.lock` qua egui, MIT OR Apache-2.0) thành phụ thuộc trực tiếp của `ui-egui` để kiểm phông trước khi nạp.
- F-Builder không kèm phông hệ thống trong gói, chỉ đọc phông có sẵn trên máy, như F-Launcher.
