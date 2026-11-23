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
