# Tasks

## 1. Quét và tích hợp

- [x] 1.1 Quét repo ở commit `86a1fa3` bằng `scripts/scan-integration.sh` và rà tay; ghi `SCAN.md`, `NOTICE.md`, `scan-allow.toml`; xác minh bằng báo cáo quét
  - Xác minh: `scripts/scan-integration.sh binhtran89/Demo_CraftAnything 86a1fa3…`: 94 tệp, 236 thư viện npm, OSV đã tra, critical/high/medium/info đều 0. Rà tay (thư mục tạm, không chạy gì): không có mẫu khóa bí mật; mã nguồn không gọi mạng; `dangerouslySetInnerHTML` nhận HTML từ `markdown.ts` đã thoát `& < > "`; tệp dự án không có email, các URL WhatsApp/Teams/FPT Chat chỉ là mô tả ý tưởng; giấy phép thư viện: MIT 203, MPL-2.0 12, ISC 8, Apache-2.0 7, BSD-3 3, OFL-1.1 1; ngoài danh sách chỉ có `caniuse-lite`, `graceful-fs` (chỉ lúc dựng).
- [x] 1.2 `build.sh` (dựng từ mã nguồn, `--verify-dist`), job CI `binhai-office`, thêm vào `scan-integrations-all.sh`; xác minh bằng chạy trên máy dev và CI, scenario "Bản dựng sẵn khác mã nguồn"
  - Xác minh: `build.sh --verify-dist` trên máy dev (23/11/2026): lần đầu Git đổi LF→CRLF khi lấy `app/dist` nên 3 tệp lệch và script dừng mã 3, liệt kê tệp (scenario "Bản dựng sẵn khác mã nguồn"); sau khi tắt `core.autocrlf` thì "bản tự dựng khớp app/dist của tác giả". `scan-integrations-all.sh` đọc repo và commit từ README; job CI `binhai-office` và `integration-scan` chạy trên commit phát hành.
- [x] 1.3 `scripts/dev-apps.sh binhai`; xác minh bằng F-Store dev, scenario "Mở BinhAI Office"
  - Xác minh: `CARGO_TARGET_DIR=C:/t/feco-test scripts/dev-apps.sh binhai` ký `binhai-office-0.1.0-web.zip` và ghi danh mục; `http://127.0.0.1:8080/store/binhai-office` trả 200, có nút "Mở ứng dụng web" tới `http://localhost:5180/` (scenario "Mở BinhAI Office"); `/store` liệt kê app; `http://localhost:5180/` trả 200.

## 2. Tài liệu và phát hành

- [x] 2.1 README hai thứ tiếng, CHANGELOG, QUESTIONS.md; phát hành 1.18.0
  - Xác minh: README hai thứ tiếng (bước 18 của F-Launcher), `scripts/check-docs-i18n.py`; CHANGELOG 1.18.0; QUESTIONS.md 133–134; CI và release.yml của tag `v1.18.0`.
