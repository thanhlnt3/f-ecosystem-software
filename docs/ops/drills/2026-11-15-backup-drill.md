# Biên bản diễn tập sao lưu và khôi phục, 15/11/2026

Diễn tập theo spec `platform-support` ("Sao lưu và khôi phục", scenario "Diễn tập khôi phục"; tasks.md 15.3), bằng `scripts/backup-drill.sh`. Cách vận hành: [docs/ops/status-and-backup.md](../status-and-backup.md).

## Môi trường

- Máy dev Windows 10 (Intel Core i9-10900X, 32 GB RAM), PostgreSQL chạy trực tiếp (`~/.local/feco-devtools/pgsql`).
- Cụm Postgres tạm (cổng 55432 và 55433), không đụng cơ sở dữ liệu dev đang dùng.
- Dữ liệu: bản sao cơ sở dữ liệu dev `feco` (`pg_dump`), cộng bảng đánh dấu `drill_marker`. Bảng đánh dấu được ghi mỗi giây một dòng sau bản sao lưu nền, cho tới lúc sự cố.

## Kết quả

Lần 1, chu kỳ lưu trữ WAL 30 giây (thử nhanh):

| Mục | Kết quả |
|---|---|
| Dòng đánh dấu ghi trước sự cố / sau khôi phục | 135 / 128 |
| Dữ liệu mất | 9 giây (giới hạn 30 giây): đạt |
| Thời gian khôi phục | 3 giây (mục tiêu 4 giờ): đạt |
| Số dòng các bảng nghiệp vụ trước/sau | khớp |

Lần 2, chu kỳ lưu trữ WAL 300 giây (như cấu hình production, RPO 5 phút):

| Mục | Kết quả |
|---|---|
| Dòng đánh dấu ghi trước sự cố / sau khôi phục | 405 / 327 |
| Dữ liệu mất | 86 giây (giới hạn 300 giây): đạt |
| Thời gian khôi phục | 2 giây (mục tiêu 4 giờ): đạt |
| Số dòng các bảng nghiệp vụ trước/sau | khớp |

## Nhận xét

- Dữ liệu mất là phần WAL chưa kịp lưu trữ lúc sự cố, nên luôn nhỏ hơn chu kỳ `archive_timeout`. Đặt 300 giây thì đạt mục tiêu 5 phút.
- Thời gian khôi phục ở đây rất ngắn vì dữ liệu dev nhỏ. Với dữ liệu production, thời gian chủ yếu là chép bản sao lưu nền và phát lại WAL, cần đo lại trên staging khi có dữ liệu thật (QUESTIONS 84).
- Chưa diễn tập khôi phục về một thời điểm cụ thể (`recovery_target_time`) trong 14 ngày, vì máy dev không giữ WAL 14 ngày. Cần làm ở lần diễn tập đầu tiên trên staging.
- Lần diễn tập tiếp theo: quý I/2027.
