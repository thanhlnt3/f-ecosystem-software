# Quét mã nguồn Mentor trước khi tích hợp

Thay đổi chat-and-source-scan (design S3). Quét bằng `feco-codescan` qua `scripts/scan-integration.sh`;
job CI `integration-scan` và `build-desktop.sh` quét lại mỗi lần chạy với `scan-allow.toml`.

- **Ngày quét**: 19/11/2026
- **Repo**: https://github.com/katD2707/agent-mentor-maximum-experience
- **Commit**: `0a3b8b708cf435f730a77453b8203fbff280d198` (lần quét đầu; `MENTOR_COMMIT` hiện là bản 0.2.2, xem mục dưới)
- **Lệnh**:

  ```bash
  scripts/scan-integration.sh katD2707/agent-mentor-maximum-experience \
    "$(cat integrations/mentor/MENTOR_COMMIT)" integrations/mentor/scan-allow.toml
  ```

- **Phạm vi**: 1.282 tệp (khoảng 27 MB), bỏ 1 tệp lớn hơn 2 MB; 848 thư viện đọc từ lockfile
  (`uv.lock`: 168 PyPI; `Cargo.lock` của app Tauri: 510 crates.io; `pnpm-lock.yaml`: 170 npm); OSV.dev đã tra.

## Cập nhật lên Mentor 0.2.2 (23/11/2026)

- **Commit mới**: `5ad3c9fd7f1bb6344c874ec6707787d661db6ee6` (App 0.2.2; 3 commit, 11 tệp so với `0a3b8b7`). Hai bản vá của F-Eco áp được không cần sửa.
- **Quét commit gốc** (`scripts/scan-integration.sh`): critical 0, high 5, medium 10. Vẫn là các lỗ hổng thư viện của bản cũ.
- **Bản vá của F-Eco nâng thư viện có lỗ hổng**:

  | Thư viện | Trước | Sau | Lỗ hổng vá |
  |---|---|---|---|
  | `pypdf` (`uv.lock`) | 6.21.3 | 6.21.4 | GHSA-5849-834g-hvw5, GHSA-cggf-277g-3q54, GHSA-mfr5-77xh-9j7h (HIGH: vòng lặp vô hạn, tốn bộ nhớ khi đọc PDF) |
  | `soupsieve` (`uv.lock`, Python ≥ 3.11.5) | 4.0.1 | 4.1.3 | GHSA-mmhr-3c5m-f64f |
  | `opentelemetry-api` (`uv.lock`) | 1.45.1 | 1.45.2 | GHSA-mf5v-w3w3-f6cg (MODERATE) |
  | `url` (`apps/app/src-tauri/Cargo.lock`) | 2.5.8 | 2.5.9 | RUSTSEC-2026-0592 |

- **Quét bản đã áp bản vá**: critical 0, high 2, medium 9.
- **Còn lại**: `soupsieve` 2.10, chỉ dùng khi chạy bằng Python dưới 3.11.5 (soupsieve 3 trở lên đòi Python mới hơn). F-Eco dựng và chạy Mentor bằng Python 3.14, nên dùng 4.1.3 đã vá. Cách hết hẳn: Mentor nâng `requires-python` lên `>=3.11.5`. Đề nghị tác giả Mentor làm việc này.
- Workflow `integration-scan.yml` quét lại commit này mỗi thứ Hai.

## Số phát hiện

| Mức | Còn lại | Đã cho phép (`scan-allow.toml`) |
|---|---|---|
| critical | 0 | 0 |
| high | 5 | 1 |
| medium | 9 | 0 |
| info | 0 | 1 |

## Kết luận

**Đạt để tích hợp.** Không có phát hiện `critical`: không có bí mật lộ (khóa riêng, token, khóa
AWS, tệp `.env` có giá trị), không có mã đáng ngờ (đào tiền ảo, đọc kho mật khẩu trình duyệt, tắt
Defender, móc bàn phím). Đã đối chiếu thêm bằng tìm kiếm tay các mẫu khóa phổ biến (`sk-…`,
`AIza…`, `ghp_…`, `AKIA…`, khóa PEM, webhook Slack và Discord) và các phép gán
`password`/`secret`/`api_key`/`token`: chỉ có nhãn giao diện trong `apps/app/src/i18n/en.ts` và
một khóa giả rõ ràng trong `services/gateway/tests/test_gw_research_sources.py`. Các tệp `.env*` trong repo đều là tệp mẫu (`.example`).

## Phát hiện mức high đáng chú ý

Lỗ hổng thư viện thật, không ghi vào `scan-allow.toml`. Theo design S1, lỗ hổng thư viện không
tự chặn; nên nâng phiên bản ở Mentor:

- `pypdf` 6.21.3 (`uv.lock`): GHSA-5849-834g-hvw5, GHSA-cggf-277g-3q54, GHSA-mfr5-77xh-9j7h (vòng
  lặp vô hạn và dùng nhiều bộ nhớ khi đọc PDF độc). Gateway đọc PDF người dùng tải lên nên nên
  nâng `pypdf`.
- `soupsieve` 2.10 (`uv.lock`): GHSA-6655-6qjp-jx7x, GHSA-mmhr-3c5m-f64f (ReDoS khi dịch bộ chọn
  CSS). Chỉ khai thác được nếu bộ chọn CSS do người dùng nhập.

Mức medium: `glib`, `zvariant`, `zbus_names`, `proc-macro-error`, `chrono` (RUSTSEC "unmaintained"
hoặc unsound, phụ thuộc của Tauri trên Linux), `opentelemetry-api` và một lỗi MODERATE khác của
`soupsieve`.

## Phát hiện đã xem xét và cho phép

| Luật | Tệp | Lý do |
|---|---|---|
| `suspicious.download_exec` (high) | `infra/linux-app.Dockerfile:24` | Cài Rust bằng trình cài chính thức `https://sh.rustup.rs` trong image dựng bản Linux; chỉ chạy lúc dựng, F-Eco không dùng Dockerfile này. |
| `suspicious.download_exec` (info) | `README.md:35` | Hướng dẫn cài uv bằng trình cài chính thức của Astral trong tài liệu. |

## Giấy phép

Không có tệp `LICENSE` ở gốc repo và không manifest gói nào ở gốc khai `license` (khớp
`NOTICE.md`: quyền dùng do tác giả xác nhận với chủ dự án F-Eco).

## Mentor 0.2.4: gói desktop tự chứa (25/11/2026)

Thay đổi `mentor-desktop-0.2.4`. Chủ dự án yêu cầu tích hợp gói
`https://github.com/katD2707/agent-mentor-maximum-experience/releases/download/v0.2.4/mentor-0.2.4-windows-x64.zip`.

- **Mã nguồn**: tag `v0.2.4`, commit `c7ac6f22c60c157efa78eccbb727e34a9a9b8184` (15 commit, 300 tệp so với `5ad3c9f`).
- **Quét** (`scripts/scan-integration.sh`): 2.078 tệp, 681 thư viện, OSV đã tra; **critical 0, high 0, medium 9**, đã cho phép 2 (như trước). Hai lỗ hổng mức cao còn lại của bản 0.2.2 (`soupsieve` 2.10 cho Python dưới 3.11.5) không còn.
- **9 phát hiện medium**, đều trong `apps/app/src-tauri/Cargo.lock` của app desktop:
  - `url` 2.5.8 (RUSTSEC-2026-0592) và `cookie` 0.18.2 (RUSTSEC-2026-0594): đã có bản vá (2.5.9, 0.18.3), tác giả chưa nâng;
  - `glib` 0.18.5 (RUSTSEC-2024-0429): chỉ dùng khi dựng cho Linux, không có trong gói Windows;
  - `chrono`, `proc-macro-error`, `zvariant`, `zbus_names`, `zvariant_derive`: ngừng bảo trì, không phải lỗ hổng.
- **Gói**: `mentor-0.2.4-windows-x64.zip`, 160.452.772 byte, SHA-256 `3032e27ff7923265906b5341dfe955f5765d649599c592bc6f7e9189b50ae369`, khớp mã băm GitHub công bố cho tệp của bản phát hành.
  - 948 tệp, giải nén 197 MB: `Mentor.exe`, `gateway/` (gateway PyInstaller, 105 tệp `.exe`, `.dll`, `.pyd`), `THIRD_PARTY_NOTICES.md`. Không có đường dẫn tuyệt đối hay `..`.
  - Windows Defender (`MpCmdRun -Scan -ScanType 3`): không phát hiện mối đe dọa.
  - **Chưa ký Authenticode** (`Mentor.exe`, `gateway\mentor-gateway.exe`).
- **Kết nối mạng của gói** (theo `integrations/feco/README.md` của Mentor): tải khóa học từ danh mục công khai trên Google Drive của chủ dự án Mentor (`MENTOR_REMOTE_CATALOG_URL`, kiểm mã băm); gọi nhà cung cấp AI bằng khóa do người dùng nhập. Không có đăng nhập bằng F-Eco.
- **Giấy phép**: repo Mentor chưa có tệp LICENSE (`f-app.toml` của tác giả ghi `license = "TODO"`). F-Eco ghi như bản trước: "Chưa công bố giấy phép (tác giả cho phép F-Eco tích hợp)".
- **Chưa làm**: dựng lại gói từ mã nguồn để so (cần PyInstaller và Tauri trên Windows). F-Eco dùng nguyên gói của tác giả, ghim bằng SHA-256.
