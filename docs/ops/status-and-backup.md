# Trang trạng thái, sao lưu và khôi phục

Tài liệu vận hành cho spec `platform-support` ("Độ sẵn sàng", "Sao lưu và khôi phục"; tasks.md 15.3).

## 1. Trang trạng thái

- **Địa chỉ:**
  - `/status`: trang công khai, có đủ 4 ngôn ngữ;
  - `/status.json`: cho công cụ giám sát. Trường `state` là `operational` hoặc `outage`; mỗi thành phần có `uptime_24h` và `uptime_30d`.
- **Thành phần được kiểm tra** mỗi phút (tác vụ nền của feco-server):

  | Thành phần | Cách kiểm tra |
  |---|---|
  | Tài khoản và đăng nhập | Kratos `GET /admin/health/alive` |
  | Đăng nhập cho app và API `/v1/me` | Hydra `GET /health/alive` |
  | Cơ sở dữ liệu | `SELECT 1` |

- **Sự cố:**
  - hai lần kiểm tra lỗi liên tiếp mở một sự cố, lần đạt đầu tiên đóng sự cố;
  - sự cố hiện trên trang trong khoảng 2 phút (spec yêu cầu trong vòng 10 phút);
  - kết quả kiểm tra giữ 90 ngày.
- **Giới hạn:** trang chạy trong feco-server. Khi máy chủ hoặc cơ sở dữ liệu ngừng hẳn, trang cũng không mở được. Ở production cần thêm một dịch vụ giám sát bên ngoài, ví dụ một máy khác gọi `/healthz` và `/status.json` mỗi phút rồi báo cho người trực. Việc chọn dịch vụ giám sát còn mở (QUESTIONS.md).

## 2. Sao lưu liên tục

Mục tiêu theo spec:
- mất dữ liệu không quá 5 phút (RPO);
- khôi phục không quá 4 giờ (RTO);
- khôi phục được về bất kỳ thời điểm nào trong 14 ngày.

Cấu hình Postgres ở production:

```conf
wal_level = replica
archive_mode = on
archive_command = '<chép %p sang kho lưu trữ, ví dụ object storage>'
archive_timeout = 300          # tối đa 5 phút giữa hai lần lưu trữ WAL: RPO 5 phút
```

- Sao lưu nền (`pg_basebackup` hoặc công cụ tương đương) mỗi ngày.
- Giữ bản sao lưu nền và WAL 14 ngày.
- Kho lưu trữ đặt ở vùng khác máy chủ cơ sở dữ liệu. Vị trí phải theo yêu cầu lưu trữ dữ liệu trong nước (xem `docs/legal/data-inventory.md`).
- Kratos và Hydra ở production dùng chung cụm Postgres này, nên được sao lưu cùng.

## 3. Diễn tập khôi phục (mỗi quý)

```bash
PG_BIN=<thư mục chương trình Postgres> scripts/backup-drill.sh docs/ops/drills/<ngày>-backup-drill.md
```

Script chạy trên một cụm Postgres tạm, không đụng cơ sở dữ liệu đang dùng:
1. bật lưu trữ WAL;
2. nạp bản sao dữ liệu (`pg_dump`);
3. sao lưu nền;
4. tiếp tục ghi dữ liệu;
5. dừng ngang cụm chính;
6. khôi phục từ bản sao lưu nền và WAL đã lưu trữ;
7. so sánh dữ liệu.

Kết quả ghi thành bảng Markdown. Script thoát với mã lỗi nếu số dòng không khớp, mất dữ liệu quá chu kỳ lưu trữ WAL, hoặc khôi phục quá 4 giờ.

Biên bản các lần diễn tập: `docs/ops/drills/`.
