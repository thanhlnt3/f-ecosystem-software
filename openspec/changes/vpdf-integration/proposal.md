# Proposal

## Why

Chủ dự án yêu cầu (23/11/2026) đưa app VPdf của mình (https://github.com/thanhlnt3/vpdf) vào kho ứng dụng.

VPdf là app đọc PDF desktop (Rust, egui, PDFium), chạy offline trên Windows x64, giấy phép MIT hoặc Apache-2.0. Gói chạy được phải kèm PDFium và qpdf dựng sẵn của bên thứ ba. Các thư viện native này mang tám giấy phép ngoài danh sách cho phép; repo VPdf ghi chúng là ngoại lệ chưa được duyệt.

Chủ dự án chọn dựng gói từ mã nguồn, và duyệt tám ngoại lệ giấy phép.

## What Changes

- **Quét trước khi tích hợp** ở tag `v0.2.0` (`2b5971b`):
  - 0 critical, 0 high, 5 medium (thư viện ngừng bảo trì, `url` 2.5.8);
  - rà tay việc tải và nạp thư viện native, mạng, bí mật, giấy phép;
  - kết quả trong `integrations/vpdf/SCAN.md`.
- **Thư mục `integrations/vpdf/`**:
  - `README.md`, `NOTICE.md` (bảng giấy phép, ngoại lệ đã duyệt), `SCAN.md`, `scan-allow.toml`;
  - `build.sh`: lấy repo đúng commit, tải PDFium và qpdf bằng `scripts/fetch-native.sh` của VPdf (SHA-256 ghim), `cargo build --release --locked`, rồi sắp gói gồm `vpdf-app.exe`, các DLL và `licenses/`.
- **CI**:
  - job `vpdf` (Windows) dựng gói và kiểm gói đủ tệp;
  - `scan-integrations-all.sh` thêm VPdf, nên quét lại ở mỗi lần CI và mỗi thứ Hai.
- **Danh mục dev**: `scripts/dev-apps.sh vpdf` ký gói desktop vào kho TUF và ghi F-Store.

## Capabilities

### Modified Capabilities
- `app-catalog`: app desktop VPdf.

## Impact

- `integrations/vpdf/` (mới), `scripts/scan-integrations-all.sh`, `scripts/dev-apps.sh`, `.github/workflows/ci.yml`, README, CHANGELOG, QUESTIONS.md.
- Không đổi mã của feco-server, F-Launcher hay app di động.
