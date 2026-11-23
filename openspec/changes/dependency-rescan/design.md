# Design

## R1. Tách bước quét khỏi bước quyết định

`source_scan::scan_submission(state, version_id, manifest) -> Result<Scanned, String>` gom phần lấy mã nguồn và quét hiện nằm trong `review`. Có bốn kết quả:

| Kết quả | Ý nghĩa |
|---|---|
| `NoSource` | phiên bản không có mã nguồn |
| `Retry(e)` | lỗi mạng, thử lại sau |
| `Failed(why)` | repo riêng tư, commit không có, zip hỏng; ghi `status = failed` như cũ |
| `Done { origin, report }` | quét xong |

`review` gọi hàm này rồi `decide` như cũ, nên hành vi duyệt không đổi.

## R2. Quét lại mã nguồn (`source_scan::rescan`)

1. Đọc báo cáo cũ trong `source_submissions.report`.
2. Gọi `scan_submission`. Repo git được tải lại tại đúng commit; tệp tải lên đã lưu sẵn ở `storage_path`.
3. `Done`: ghi báo cáo mới và `scanned_at`, cập nhật `app_versions.checks.source`.
4. Trả `new_findings`: các phát hiện mức `high` hoặc `critical` có khóa `rule|path|message` chưa có trong báo cáo cũ. Mỗi mã lỗ hổng (GHSA/RUSTSEC) chỉ được báo một lần, vì nó đã nằm trong `message`.
5. `Retry`, `Failed`, `NoSource`: không đổi gì. Lần quét lại sau 7 ngày sẽ thử tiếp.

## R3. `review::run_rescan`

Sau khi quét gói:

- Gói có mã độc thì xử lý như cũ (`late_detection`).
- Có `new_findings` thì:
  - `notify_developer`: email "Lỗ hổng mới trong thư viện của {app} {version}", liệt kê tối đa 20 dòng, kèm cách sửa;
  - `admin_alert("new_vulnerability", …)`, chi tiết gồm `app`, `version`, `findings`, `installs`;
  - kết quả job là `vulnerable`.
- `last_scanned_at` chỉ được cập nhật khi cả quét gói lẫn quét mã nguồn không gặp lỗi mạng.

## R4. Cảnh báo trong bảng quản trị

`review::recent_alerts(db, days)` trả cảnh báo 30 ngày gần nhất, mới nhất trước. Trang `/admin/reviews` hiện mục "Cảnh báo kiểm duyệt" (`#admin-alerts`) dưới hàng chờ:

- mỗi dòng gồm loại, nội dung, app và phiên bản, thời gian;
- thanh công cụ của danh sách lọc cả cảnh báo: người gửi là app, trạng thái là loại cảnh báo.

## R5. CI định kỳ

`.github/workflows/integration-scan.yml`:

- kích hoạt bằng `schedule: cron "0 2 * * 1"` (thứ Hai, 9 giờ sáng giờ Việt Nam) và `workflow_dispatch`;
- chạy cùng lệnh với job `integration-scan` của `ci.yml`.

Có phát hiện `critical` chưa được cho phép thì workflow đỏ, GitHub gửi email cho chủ repo. Báo cáo Markdown được đính kèm làm artifact.
