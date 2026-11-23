# Tích hợp VPdf với F-Eco

VPdf là ứng dụng đọc PDF desktop (Rust, egui, PDFium), chạy offline. Các tính năng chính:
- mở file, kể cả file có mật khẩu hoặc file hỏng;
- cuộn, phóng 10%–6400%, ảnh thu nhỏ, mục lục, nhiều tab;
- sao chép chữ;
- tìm chữ khi đang gõ, gõ không dấu vẫn tìm ra chữ có dấu.

VPdf là app của chủ dự án F-Eco.

## Mã nguồn

| Phần | Repo | Tag | Commit đã dùng |
|---|---|---|---|
| App desktop Windows x64 | https://github.com/thanhlnt3/vpdf | `v0.2.0` | `2b5971bf5a6cb6a2457e4e64a477dcc0f76ebbff` |

- Bản hiện tại chỉ hỗ trợ Windows 10/11 x64; macOS đang tạm hoãn ở repo VPdf.
- Trong F-Eco, VPdf là **app desktop**: F-Launcher trên Windows cài, mở, cập nhật và gỡ VPdf như mọi app con.
- VPdf không cần tài khoản nên chưa đăng nhập bằng F-Eco.

## Dựng gói

F-Eco tự dựng gói từ mã nguồn, không dùng gói trên Releases của VPdf. Cần Windows, Rust và Git Bash.

```bash
integrations/vpdf/build.sh ~/.feco-dev-ext/vpdf/pkg
scripts/dev-apps.sh vpdf
```

`build.sh` làm các bước sau:
1. lấy repo ở đúng commit trên;
2. tải PDFium `chromium/8138` và qpdf 12.4.2 bằng `scripts/fetch-native.sh` của VPdf, kiểm SHA-256 ghim trong script;
3. chạy `cargo build --release --locked -p vpdf-app`;
4. sắp gói gồm `vpdf-app.exe`, `pdfium.dll`, các DLL của qpdf, và thư mục `licenses/`.

`scripts/dev-apps.sh vpdf` ký gói vào kho TUF dev và ghi danh mục F-Store. Job CI `vpdf` dựng gói trên Windows ở mỗi lần chạy.

## Quét bảo mật và giấy phép

- Kết quả quét: `SCAN.md`. Job CI `integration-scan` và workflow `integration-scan.yml` (mỗi thứ Hai) quét lại commit trên.
- Giấy phép và các ngoại lệ đã duyệt: `NOTICE.md`.
