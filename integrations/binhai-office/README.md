# Tích hợp BinhAI Office với F-Eco

BinhAI Office là bản thử giao diện của một "phòng AI" làm việc trên máy tính:

- người dùng ra lệnh bằng tiếng Việt;
- 18 agent chia việc theo 8 quy trình, tự kiểm tra rồi gửi kết quả chờ duyệt;
- văn phòng 3D (react-three-fiber) phản ánh trạng thái công việc;
- xuất kết quả ra `.md`, `.docx`, `.xlsx`.

Bản này chạy hoàn toàn trong trình duyệt:
- "backend" là mô phỏng (`store.ts`), lưu vào `localStorage`;
- kết quả là văn bản mẫu (MOCK), không gọi dịch vụ AI nào;
- không cần tài khoản, nên chưa đăng nhập bằng F-Eco.

## Mã nguồn

| Phần | Repo | Commit đã dùng |
|---|---|---|
| Web app (React + Vite, tĩnh) | https://github.com/binhtran89/Demo_CraftAnything | `86a1fa3fc40b18730be9acf1db6a104f570491d3` |

- Mã nguồn nằm ở `app/apps/web/`.
- Repo còn có tệp dự án của công cụ dựng app (`craft.json`, `investigation.cany/`, `checks/`). F-Eco không dùng các tệp này.
- Thư mục này chỉ có hướng dẫn và script dựng, không chép mã của tác giả.

Trong F-Eco, BinhAI Office là **app web**:
- F-Store có trang BinhAI Office với nút "Mở ứng dụng web";
- F-Launcher máy tính và điện thoại mở app bằng trình duyệt.

## Dựng từ mã nguồn

F-Eco không phục vụ `app/dist` dựng sẵn của tác giả. `build.sh` làm các bước sau:
1. lấy repo ở đúng commit trên;
2. cài thư viện npm theo `package-lock.json` với `--ignore-scripts`;
3. chạy `npm run build` (`tsc -b && vite build`);
4. chép bản dựng ra thư mục đích.

Với `--verify-dist`, script kiểm thêm: bản tự dựng phải trùng từng tệp (tên và SHA-256) với `app/dist` của tác giả. Job CI `binhai-office` chạy kiểm tra này.

```bash
integrations/binhai-office/build.sh ~/.feco-dev-ext/binhai-office/site --verify-dist
```

## Chạy trên máy dev

1. Dựng như trên, rồi phục vụ thư mục tĩnh ở cổng 5180:

   ```bash
   python -m http.server 5180 --bind 127.0.0.1 --directory ~/.feco-dev-ext/binhai-office/site
   ```

2. Đưa app vào kho TUF và danh mục dev:

   ```bash
   scripts/dev-apps.sh binhai
   ```

3. Trong F-Launcher, mở Cửa hàng, chọn BinhAI Office, bấm "Thêm" rồi "Mở".

## Quét bảo mật và giấy phép

- Kết quả quét trước khi tích hợp: `SCAN.md`.
- Job CI `integration-scan` và workflow `integration-scan.yml` (mỗi thứ Hai) quét lại commit trên.
- Repo chưa có tệp giấy phép. Chủ dự án F-Eco xác nhận tác giả cho phép F-Eco tích hợp (23/11/2026), xem `NOTICE.md`.
