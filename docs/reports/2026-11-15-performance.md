# Biên bản kiểm thử hiệu năng, 15/11/2026

Theo spec `platform-support` ("Hiệu năng", scenario "Kiểm thử tải"; tasks.md 15.2).

## Môi trường

- Một máy dev Windows 10: Intel Core i9-10900X (10 lõi, 20 luồng), 32 GB RAM. Máy tạo tải và máy chủ chạy chung trên máy này.
- feco-server 1.5.0 bản release; PostgreSQL 17, Kratos 1.3.1 và Hydra 2.3.0 chạy trực tiếp, Kratos và Hydra dùng SQLite.
- Kết quả trên máy dev chưa thay được phép đo trên staging giống production (QUESTIONS 84).

## 1. Tải `GET /v1/me`

Lệnh chạy:

```bash
FECO_E2E=1 cargo test --release -p feco-server --test e2e_auth -- --ignored load_v1_me --nocapture
```

Cách đo:
- Token thật của một app, lấy qua luồng OAuth đầy đủ.
- Một máy chủ feco-server riêng, giới hạn tần suất đặt rất cao, để đo hiệu năng chứ không đo bộ chặn.
- Tải mở (open-loop): gửi đều 1.000 yêu cầu mỗi giây trong 30 giây, không chờ yêu cầu trước. Mỗi yêu cầu đi qua toàn bộ đường xử lý: kiểm token với Hydra, đọc danh tính từ Kratos, đọc cơ sở dữ liệu.

| Chỉ số | Giá trị |
|---|---|
| Tốc độ đặt ra | 1.000 yêu cầu/giây |
| Thời gian | 30,0 giây |
| Đã gửi | 30.000 |
| Thành công (200) | 30.000 |
| Lỗi | 0 (0%) |
| Tốc độ đạt được | 1.000 yêu cầu/giây |
| p50 | 2,2 ms |
| p95 | 2,9 ms |
| p99 | 3,8 ms |
| Lâu nhất | 20,2 ms |

Kết luận: **đạt** (yêu cầu p95 dưới 300 ms, lỗi dưới 0,1%).

## 2. Thời gian khởi động F-Launcher

Lệnh chạy:

```powershell
powershell -File scripts\measure-launcher-startup.ps1 -Exe <feco-launcher.exe bản release> -Runs 5
```

Launcher ghi thời gian từ lúc tiến trình bắt đầu tới khung hình đầu tiên. Khung hình đầu tiên đã hiện Thư viện (App của tôi) từ trạng thái lưu trên máy, việc đồng bộ chạy nền sau đó.

| Lần | Khung hình đầu (ms) |
|---|---|
| 1 (khởi động nguội) | 469 |
| 2 | 367 |
| 3 | 353 |
| 4 | 356 |
| 5 | 349 |

Trung vị 356 ms, lâu nhất 469 ms. Kết luận: **đạt** (yêu cầu dưới 3 giây). Máy đo có 32 GB RAM, trên mức 8 GB của spec. Launcher không tải dữ liệu lớn lúc khởi động, nên kết quả ít phụ thuộc vào RAM; vẫn nên đo lại trên một máy 8 GB.
