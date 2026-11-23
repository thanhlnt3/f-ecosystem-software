# Quét BinhAI Office trước khi tích hợp

Quét bằng `feco-codescan` qua `scripts/scan-integration.sh`, theo quy trình của thay đổi chat-and-source-scan (design S3). Job CI `integration-scan` và workflow `integration-scan.yml` quét lại commit này, dùng `scan-allow.toml`.

- **Ngày quét**: 23/11/2026
- **Repo**: https://github.com/binhtran89/Demo_CraftAnything
- **Commit**: `86a1fa3fc40b18730be9acf1db6a104f570491d3`
- **Lệnh**:

  ```bash
  scripts/scan-integration.sh binhtran89/Demo_CraftAnything 86a1fa3fc40b18730be9acf1db6a104f570491d3 integrations/binhai-office/scan-allow.toml
  ```

- **Phạm vi**: 94 tệp (6,4 MB), 236 thư viện npm đọc từ `app/apps/web/package-lock.json`, đã tra OSV.dev.

## Số phát hiện

| Mức | Còn lại | Đã cho phép |
|---|---|---|
| critical | 0 | 0 |
| high | 0 | 0 |
| medium | 0 | 0 |
| info | 0 | 0 |

## Rà tay

- **Bí mật**: không có khóa API, token, khóa riêng. Đã tìm các mẫu `sk-ant-`, `sk-…`, `AKIA…`, `ghp_…`, `AIza…`, khóa PEM và webhook trong mọi tệp, kể cả tệp dự án của công cụ dựng app.
- **Gọi mạng**:
  - mã nguồn (`app/apps/web/src`) không có `fetch`, `XMLHttpRequest`, WebSocket hay `sendBeacon`;
  - bản dựng chỉ nhắc tới địa chỉ không gian tên XML (Office Open XML, W3C) và liên kết tài liệu;
  - app không gửi dữ liệu ra ngoài.
- **Thực thi mã, chèn HTML**:
  - không có `eval` hay `new Function`;
  - chỗ duy nhất dùng `dangerouslySetInnerHTML` (`ui/common.tsx`) nhận HTML từ `lib/markdown.ts`, và hàm này thoát `& < > "` trước khi dựng thẻ, nên không chèn được HTML hay script.
- **Dữ liệu**: lưu trong `localStorage` của trình duyệt (`store.ts`), không có máy chủ.
- **Tệp dự án của công cụ dựng app** (`craft.json`, `investigation.cany/`, `checks/`, `feed.json`, `questions.json`):
  - không có email hay dữ liệu cá nhân;
  - các địa chỉ WhatsApp, Teams, FPT Chat chỉ nằm trong mô tả một ý tưởng app khác, không phải phiên trình duyệt bị chụp lại;
  - F-Eco không dùng các tệp này.
  - `app/README.md` có đường dẫn trên máy của tác giả (`/Users/…`). Không ảnh hưởng bảo mật.
- **Bản dựng sẵn `app/dist`**: F-Eco dựng lại từ mã nguồn (`npm ci --ignore-scripts`, `npm run build`). Bản tự dựng trùng từng tệp với `app/dist` của tác giả: `index-Bg1nCOme.js`, `Office-DsSJV5HI.js`, `index-6yEMx81e.css` và phông chữ. Nghĩa là bản dựng sẵn khớp mã nguồn. Job CI `binhai-office` kiểm lại mỗi lần chạy (`build.sh --verify-dist`).

## Giấy phép

- **Repo**: chưa có tệp giấy phép. Tác giả cho phép F-Eco tích hợp (`NOTICE.md`).
- **Thư viện**:

  | Giấy phép | Số thư viện |
  |---|---|
  | MIT | 203 |
  | MPL-2.0 | 12 |
  | ISC | 8 |
  | Apache-2.0 | 7 |
  | BSD-3-Clause | 3 |
  | OFL-1.1 | 1 (phông Be Vietnam Pro) |

  - `webgl-constants` 1.1.1 không ghi giấy phép trong lockfile, nhưng gói có tệp LICENSE là MIT.
- **Ngoài danh sách cho phép của F-Eco**, chỉ dùng lúc dựng, không có trong bản phục vụ:
  - `caniuse-lite` (CC-BY-4.0, dữ liệu trình duyệt của công cụ dựng);
  - `graceful-fs` (BlueOak-1.0.0).

## Kết luận

**Đạt để tích hợp** như một app web tĩnh dựng từ mã nguồn. Không có phát hiện nào. Cần theo dõi:
- giấy phép của repo;
- `three-mesh-bvh` 0.7.8, bị npm đánh dấu ngừng hỗ trợ (đi kèm `@react-three/drei`).
