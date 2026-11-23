# Tasks

## 1. Quét và tích hợp

- [x] 1.1 Quét VPdf ở tag `v0.2.0` bằng `scripts/scan-integration.sh` và rà tay; ghi `SCAN.md`, `NOTICE.md` (ngoại lệ đã duyệt), `scan-allow.toml`; xác minh bằng báo cáo quét
  - Xác minh: `scripts/scan-integration.sh thanhlnt3/vpdf 2b5971b…`: 95 tệp, 455 thư viện crates.io, OSV đã tra, 0 critical, 0 high, 5 medium (chrono, url 2.5.8, zbus_names, zvariant, zvariant_derive). Rà tay: `fetch-native.sh` so SHA-256 ghim cho PDFium `chromium/8138` và qpdf 12.4.2; thứ tự tìm DLL (biến môi trường, cạnh tệp chạy, `third_party/`); mã nguồn không gọi mạng; không có bí mật. Chủ dự án duyệt tám ngoại lệ giấy phép ngày 23/11/2026 (`NOTICE.md`).
- [x] 1.2 `build.sh`, job CI `vpdf`, thêm vào `scan-integrations-all.sh`; xác minh bằng dựng trên máy dev và CI, scenario "Gói đủ giấy phép"
  - Xác minh: `build.sh` trên máy dev (23/11/2026): tải và kiểm SHA-256 PDFium/qpdf, `cargo build --release --locked` (1 phút 43 giây), gói gồm `vpdf-app.exe`, `pdfium.dll`, `qpdf30.dll`, 8 DLL runtime VC++ và `licenses/` (VPdf MIT/Apache, Noto OFL, PDFium và 18 tệp giấy phép thành phần, qpdf, `native.toml`, `license-exceptions.toml`, `F-ECO-NOTICE.md`). Lần đầu thư mục giấy phép qpdf trống; đã sửa và thêm bước dừng nếu thiếu. Cùng bộ DLL với gói Releases v0.2.0 của VPdf (so tên và kích thước, gói Releases khớp `SHA256SUMS.txt`). Job CI `vpdf` (Windows) dựng và kiểm gói đủ tệp (scenario "Gói đủ giấy phép"); `scan-integrations-all.sh` quét VPdf.
- [x] 1.3 `scripts/dev-apps.sh vpdf`; xác minh bằng F-Store dev và F-Launcher, scenario "Cài và mở VPdf"
  - Xác minh: `scripts/dev-apps.sh vpdf` ký `vpdf-0.2.0-windows-x64.zip` và ghi danh mục; `/store/vpdf` trả 200; F-Launcher sau khi khởi động lại có VPdf 0.2.0 (windows-x64, `vpdf-app.exe`, 14,4 MB) trong danh mục. Chạy `vpdf-app.exe` từ gói: mở `mau-tieng-viet.pdf` (40 trang) có ảnh thu nhỏ, chuyển tệp từ lần chạy thứ hai sang cửa sổ đang mở được (ảnh chụp cửa sổ). Bước bấm "Cài" trong F-Launcher để người dùng làm trên máy (cài qua TUF giống các app desktop khác). Vài lần thoát sớm lúc đầu không tái hiện được: QUESTIONS.md 136.

## 2. Tài liệu và phát hành

- [x] 2.1 README hai thứ tiếng, CHANGELOG, QUESTIONS.md; phát hành 1.19.0
  - Xác minh: README hai thứ tiếng (bước 19), `scripts/check-docs-i18n.py`; CHANGELOG 1.19.0; QUESTIONS.md 135–136; CI và release.yml của tag `v1.19.0`.
