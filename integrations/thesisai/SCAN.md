# Quét mã nguồn ThesisAI trước khi tích hợp

Thay đổi chat-and-source-scan (design S3). Quét bằng `feco-codescan` qua `scripts/scan-integration.sh`;
job CI `integration-scan` quét lại cả hai repo ở commit trong bảng của `README.md`, với `scan-allow.toml`.

- **Ngày quét**: 19/11/2026
- **Lệnh**:

  ```bash
  scripts/scan-integration.sh justAchillGuy8386/thesisai-fe 8f3735e5e630c4aedb12bc888894ed26e8a07fa3 integrations/thesisai/scan-allow.toml
  scripts/scan-integration.sh justAchillGuy8386/thesisai-be d890595fbb0777e6a269203bcfecbb8af5eaf3a3 integrations/thesisai/scan-allow.toml
  ```

| Repo | Commit | Tệp | Thư viện | OSV |
|---|---|---|---|---|
| thesisai-fe (Next.js) | `8f3735e5e630c4aedb12bc888894ed26e8a07fa3` | 42 | 652 (npm, `package-lock.json`) | đã tra |
| thesisai-be (FastAPI) | `d890595fbb0777e6a269203bcfecbb8af5eaf3a3` | 100 | 0 (xem ghi chú) | đã tra |

## Số phát hiện

| Mức | thesisai-fe | thesisai-be | Đã cho phép |
|---|---|---|---|
| critical | 0 | 0 | 0 |
| high | 10 | 0 | 0 |
| medium | 19 | 0 | 0 |
| info | 0 | 0 | 0 |

## Kết luận

**Đạt để tích hợp**, kèm khuyến nghị nâng thư viện của thesisai-fe. Không có phát hiện `critical`:
không có bí mật lộ, không có mã đáng ngờ. Đã đối chiếu thêm bằng tìm kiếm tay các mẫu khóa phổ
biến và phép gán `password`/`secret`/`api_key`/`token`: không thấy gì. Tệp `.env` duy nhất là
`thesisai-be/.env.example` (giá trị mẫu cho máy dev: `localhost`, mật khẩu Postgres mặc định), đúng
là tệp mẫu. Không có phát hiện sai nên `scan-allow.toml` không có mục nào.

## Phát hiện mức high đáng chú ý (thesisai-fe)

Lỗ hổng thư viện thật, không ghi vào `scan-allow.toml`:

- **`next` 16.4.0**: GHSA-2464-fhr4-w9xw và GHSA-673q-mjm6-r5xr là mức **CRITICAL** trên OSV (thực
  thi mã từ xa không cần đăng nhập: Image Optimization với tệp SVG, và hàm `use cache` trả mảng có
  kiểu); GHSA-p3m5-59mp-jv5r (HIGH, từ chối dịch vụ qua Server Actions). Theo design S1, lỗ hổng
  CRITICAL thành phát hiện `high`, nên không làm CI thất bại; nhưng **nên nâng Next.js lên bản đã
  vá trước khi đưa ThesisAI ra ngoài môi trường dev**. Bản vá đã nâng lên 16.4.6 (mục dưới).
- `undici` 7.30.0: 4 lỗ hổng HIGH (bỏ qua kiểm tra chứng thư TLS, đầu độc bộ nhớ đệm, từ chối dịch vụ).
- `body-parser` 2.3.0, `zod` 3.25.76, `systeminformation` 5.33.15: mỗi thư viện một lỗ hổng HIGH
  (từ chối dịch vụ, chèn lệnh Windows trong `services()`).

Mức medium: `hono`, `@hono/node-server`, `fast-uri`, `minimatch`, `qs`, `undici`, `next` (MODERATE/LOW).

## Sau khi áp bản vá (21/11/2026)

`thesisai-fe.patch` nâng `next` và `eslint-config-next` từ 16.4.0 lên 16.4.6 (bản 16.4.x mới nhất, OSV
không ghi lỗ hổng nào; bản sửa đầu tiên là 16.4.1). Bản vá cũng nâng trong `package-lock.json`, vẫn
trong khoảng phiên bản mà `package.json` cho phép: `undici` 7.30.2, `body-parser` 2.4.0,
`systeminformation` 5.33.23, `hono` 4.13.19, `@hono/node-server` 2.1.11, `fast-uri` 3.1.9, `qs` 6.17.1.

Quét lại thư mục đã áp bản vá:

```bash
feco-codescan <thesisai-fe đã áp bản vá> --allow integrations/thesisai/scan-allow.toml
```

| Mức | Trước bản vá | Sau bản vá |
|---|---|---|
| critical | 0 | 0 |
| high | 10 | 1 |
| medium | 19 | 1 |

Còn lại: `zod` 3.25.76 (GHSA-mrhx-c7xc-m3w5, HIGH). Bản sửa chỉ có từ zod 4.6.6, mà `@shadcn/registry`
cần zod 3. Thư viện này thuộc công cụ dòng lệnh `shadcn`; app chỉ dùng `shadcn/tailwind.css`, không
chạy mã của nó khi phục vụ người dùng. Job CI `thesisai-fe` áp bản vá lên commit trong `README.md`,
kiểm `next` là 16.4.6, rồi chạy `npm ci` và `next build`.

## Ghi chú

- **thesisai-be không ghim phiên bản**: `requirements.txt` chỉ có ràng buộc `>=` (ví dụ
  `fastapi>=0.115.0`, `pypdf>=5.0.0`), không có lockfile, nên bộ quét không biết phiên bản sẽ cài và
  không tra OSV được (theo design, chỉ dòng ghim `==` được tra). Nên đề nghị tác giả ghim phiên bản
  (hoặc dùng `uv.lock`) để bản dựng tái lập được và quét được lỗ hổng.
- **Giấy phép**: hai repo chưa có tệp `LICENSE` (khớp `NOTICE.md`, QUESTIONS.md mục 91).
