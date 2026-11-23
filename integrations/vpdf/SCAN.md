# Quét VPdf trước khi tích hợp

Quét bằng `feco-codescan` qua `scripts/scan-integration.sh`, theo quy trình của thay đổi chat-and-source-scan (design S3). Job CI `integration-scan` và workflow `integration-scan.yml` quét lại commit này, dùng `scan-allow.toml`.

- **Ngày quét**: 23/11/2026
- **Repo**: https://github.com/thanhlnt3/vpdf, tag `v0.2.0`
- **Commit**: `2b5971bf5a6cb6a2457e4e64a477dcc0f76ebbff` (cũng là `main` lúc quét)
- **Lệnh**:

  ```bash
  scripts/scan-integration.sh thanhlnt3/vpdf 2b5971bf5a6cb6a2457e4e64a477dcc0f76ebbff integrations/vpdf/scan-allow.toml
  ```

- **Phạm vi**: 95 tệp, 455 thư viện crates.io đọc từ `Cargo.lock`, đã tra OSV.dev.

## Số phát hiện

| Mức | Còn lại | Đã cho phép |
|---|---|---|
| critical | 0 | 0 |
| high | 0 | 0 |
| medium | 5 | 0 |
| info | 0 | 0 |

Năm phát hiện medium đều là thư viện trong `Cargo.lock`:

| Thư viện | Phiên bản | Mã | Nội dung |
|---|---|---|---|
| `chrono` | 0.4.45 | RUSTSEC-2026-0543 | ngừng bảo trì |
| `url` | 2.5.8 | RUSTSEC-2026-0592 | tràn stack trong `Url::origin()` với URL `blob:` lồng nhau; đã có bản vá 2.5.9 |
| `zbus_names` | 4.3.5 | RUSTSEC-2026-0371 | ngừng bảo trì |
| `zvariant`, `zvariant_derive` | 5.15.1 | RUSTSEC-2026-0370, RUSTSEC-2026-0372 | ngừng bảo trì |

`zbus` và `zvariant` chỉ dùng trên Linux (D-Bus của winit/egui); bản Windows không gọi tới.

## Rà tay

- **Thư viện native**:
  - VPdf tải PDFium `chromium/8138` (bblanchon/pdfium-binaries) và qpdf 12.4.2 từ GitHub Releases của hai dự án;
  - `scripts/fetch-native.sh` so SHA-256 với giá trị ghim cứng, sai thì dừng;
  - F-Eco dựng gói bằng chính script này (`build.sh`), không dùng gói trên Releases của VPdf.
- **Nạp thư viện**: PDFium và qpdf được tìm theo thứ tự biến môi trường `VPDF_*_DIR`, thư mục chứa `vpdf-app.exe`, rồi `third_party/` khi chạy từ mã nguồn. Bản trong F-Launcher đặt DLL cạnh tệp chạy. qpdf được nạp bằng `LOAD_WITH_ALTERED_SEARCH_PATH`, nên DLL phụ thuộc cũng lấy cạnh `qpdf30.dll`.
- **Mạng**: VPdf chạy offline. Mã nguồn không gọi mạng; chỉ các script `fetch-*` tải tệp lúc dựng và lúc kiểm thử.
- **Bí mật**: không có khóa, token hay tệp `.env`.
- **Giấy phép**:
  - crates Rust được `cargo deny check licenses` của VPdf kiểm;
  - thư viện native có 8 ngoại lệ ngoài danh sách cho phép, đã được chủ dự án duyệt ngày 23/11/2026 (`NOTICE.md`).

## Kết luận

**Đạt để tích hợp.** Không có phát hiện critical hay high. Nên nâng `url` lên 2.5.9 ở bản VPdf tới, và điền người duyệt cho các ngoại lệ giấy phép trong repo VPdf.
