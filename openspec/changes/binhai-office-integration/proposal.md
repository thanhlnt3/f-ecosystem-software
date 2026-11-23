# Proposal

## Why

Chủ dự án yêu cầu (23/11/2026) đưa vào kho ứng dụng app https://github.com/binhtran89/Demo_CraftAnything ("BinhAI Office"), sau khi kiểm tra lỗi bảo mật.

BinhAI Office là web app tĩnh (React, Vite, three.js), chạy hoàn toàn trong trình duyệt với dữ liệu mô phỏng. Repo chưa có giấy phép; chủ dự án xác nhận tác giả cho phép F-Eco tích hợp. Chủ dự án chọn dựng từ mã nguồn thay vì dùng `app/dist` dựng sẵn.

## What Changes

- **Quét trước khi tích hợp** (spec source-scan, "Quét trước khi tích hợp app bên ngoài"):
  - `feco-codescan`: 0 phát hiện, 236 thư viện npm, OSV sạch;
  - rà tay: bí mật, gọi mạng, chèn HTML, dữ liệu cá nhân trong tệp dự án, giấy phép thư viện;
  - kết quả trong `integrations/binhai-office/SCAN.md`.
- **Thư mục `integrations/binhai-office/`**:
  - `README.md` (repo, commit, cách dựng và chạy), `NOTICE.md`, `SCAN.md`, `scan-allow.toml`;
  - `build.sh`: dựng từ mã nguồn tại commit đã ghi, cài thư viện với `npm ci --ignore-scripts`; với `--verify-dist` thì kiểm bản tự dựng trùng từng tệp với `app/dist` của tác giả.
- **CI**:
  - job `binhai-office` dựng và kiểm `dist`;
  - `scripts/scan-integrations-all.sh` thêm BinhAI Office, nên job `integration-scan` và workflow hằng tuần quét lại.
- **Danh mục dev**: `scripts/dev-apps.sh binhai` ký gói app web (địa chỉ `http://localhost:5180/`) vào kho TUF và ghi danh mục F-Store.

## Capabilities

### Modified Capabilities
- `app-catalog`: app web BinhAI Office.

## Impact

- `integrations/binhai-office/` (mới), `scripts/scan-integrations-all.sh`, `scripts/dev-apps.sh`, `.github/workflows/ci.yml`, README, CHANGELOG, QUESTIONS.md.
- Không đổi mã của feco-server, F-Launcher hay app di động.
