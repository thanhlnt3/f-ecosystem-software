# NOTICE

VPdf (https://github.com/thanhlnt3/vpdf) được phát hành theo **MIT hoặc Apache-2.0**, tùy người dùng chọn (`LICENSE-MIT`, `LICENSE-APACHE` trong gói).

Gói VPdf trên F-Store có kèm thư viện native của bên thứ ba. Giấy phép của từng thành phần nằm trong `licenses/` của gói:

| Thành phần | Giấy phép | Trong danh sách cho phép của F-Eco |
|---|---|---|
| PDFium (bblanchon/pdfium-binaries `chromium/8138`) | BSD-3-Clause | có |
| Abseil, Highway (trong PDFium) | Apache-2.0 | có |
| fast_float, Little CMS (trong PDFium) | MIT | có |
| ICU (trong PDFium) | Unicode-3.0 | có |
| Dragonbox (trong PDFium) | Apache-2.0 WITH LLVM-exception OR BSL-1.0 | có (chọn BSL-1.0) |
| qpdf 12.4.2 | Apache-2.0 | có |
| Noto Sans (nhúng trong `vpdf-app.exe`) | OFL-1.1 | có |
| Anti-Grain Geometry 2.3 (trong PDFium) | LicenseRef-AGG-2.3 | ngoại lệ |
| FreeType (trong PDFium) | FTL | ngoại lệ |
| HarfBuzz (trong PDFium) | MIT-Modern-Variant | ngoại lệ |
| libjpeg-turbo (trong PDFium) | IJG | ngoại lệ |
| libpng (trong PDFium) | libpng-2.0 | ngoại lệ |
| LLVM libc (trong PDFium) | Apache-2.0 WITH LLVM-exception | ngoại lệ |
| zlib, libjpeg (liên kết tĩnh trong `qpdf30.dll`) | Zlib, IJG | ngoại lệ (IJG) |
| Microsoft Visual C++ runtime (DLL kèm qpdf) | LicenseRef-Microsoft-VC-Redist | ngoại lệ |

## Ngoại lệ đã duyệt

- **Người duyệt**: chủ dự án F-Eco, cũng là chủ VPdf (tài khoản GitHub `thanhlnt3`).
- **Ngày duyệt**: 23/11/2026.
- **Phạm vi**: tám ngoại lệ trong `third_party/license-exceptions.toml` của VPdf (đi kèm gói, thư mục `licenses/`), để phân phối VPdf trên F-Store. Lý do từng mục ghi trong tệp đó. Các giấy phép này đều kiểu BSD/MIT/zlib, hoặc là giấy phép phân phối lại của Microsoft; không có điều khoản copyleft.
- **Việc còn lại ở repo VPdf**: điền `reviewer` và `date` trong `third_party/license-exceptions.toml`, để `vpdf-license-check` của VPdf qua.
