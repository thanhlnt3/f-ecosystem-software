# Tasks

## 1. Kho phân phối

- [x] 1.1 `Repo::maintain` trong `feco-tuf`; xác minh bằng test scenario "Timestamp sắp hết hạn", "Máy chủ tắt nhiều ngày"
  - Xác minh: `feco-tuf` test `maintain_renews_metadata_before_it_expires`: vừa phát hành thì `Nothing`; 40 phút sau (timestamp còn 20 phút) thì `Refreshed`, gọi lại ngay thì `Nothing`; metadata ký 72 giờ trước khiến client lỗi `ExpiredMetadata`, `maintain` sửa thành `Refreshed` và client đồng bộ được; targets còn dưới 30 ngày thì `Resigned` (thiếu khóa targets thì lỗi).
- [x] 1.2 feco-server chạy `maintain` lúc khởi động và mỗi 5 phút; xác minh bằng E2E và kiểm thủ công trên máy dev (`timestamp.json` được gia hạn)
  - Xác minh: E2E `developer_portal_end_to_end`: `maintain_tuf` trả `Nothing` sau phát hành; metadata ký lùi 72 giờ khiến client mới lỗi `ExpiredMetadata`; `maintain_tuf` trả `Refreshed` và client đồng bộ được. Máy dev (22/11/2026): `timestamp.json` hết hạn từ 19/11 (phiên bản 29); máy chủ 1.16 vừa khởi động ghi log "đã ký lại metadata phân phối done=Refreshed", phiên bản 30, hạn mới 1 giờ sau.

## 2. F-Launcher máy tính

- [x] 2.1 `LauncherError::ExpiredMetadata`, `distribution_error`, không coi là ngoại tuyến; xác minh bằng test scenario "Metadata hết hạn", "Máy chủ đã gia hạn"
  - Xác minh: `tests/launcher.rs::expired_metadata_is_not_offline_and_clears_after_renewal` (vẫn `is_online`, thông báo bắt đầu bằng "Dữ liệu phân phối của máy chủ đã hết hạn", không chứa `ExpiredMetadata`/`Tuf`, chi tiết vẫn trong `detail()` và sự kiện `expired_metadata`, danh mục cũ giữ nguyên; `maintain` rồi đồng bộ lại thì hết thông báo); `fake_manifest_is_rejected` cập nhật (không còn coi là ngoại tuyến, thông báo không có chi tiết kỹ thuật). Bản dịch Anh, Pháp, Nhật trong `i18n.rs`, `i18n_ja.rs`.

## 3. Di động

- [x] 3.1 `error_server` và tải lại khi quay lại app; xác minh bằng widget test scenario "Máy chủ trả lỗi", "Quay lại sau lỗi"
  - Xác minh: widget test `scenario_loi_may_chu_va_quay_lai`: `ApiException(503)` báo "Máy chủ F-Eco đang gặp lỗi…", không báo mất kết nối; lỗi mạng vẫn báo "Không kết nối được máy chủ F-Eco."; quay lại app (vòng đời paused → resumed) khi máy chủ ổn thì tải lại và hết lỗi. `flutter analyze` sạch, `flutter test` 32 qua.

## 4. Email thật

- [x] 4.1 `dev-local.sh kratos` đọc `FECO_SMTP_URL`/`.dev/smtp-url`, `FECO_EMAIL_FROM`/`.dev/smtp-from`; `FECO_SMTP_URL_FILE` cho feco-server; xác minh bằng test cấu hình và scenario "Chưa cấu hình SMTP", "Đăng ký với email Gmail" (với SMTP thử)
  - Xác minh: `feco-config` test `smtp_url_from_secret_file` (đọc `smtps://…smtp.gmail.com:465` từ tệp, `FECO_SMTP_URL` được ưu tiên, nội dung sai không lộ trong thông báo lỗi). Máy dev: Kratos chạy với `FECO_SMTP_URL=smtp://127.0.0.1:2525` (Mailpit thử thứ hai thay cho Gmail) và `FECO_EMAIL_FROM`; đăng ký qua API Kratos thì email "Xác minh email tài khoản F-Eco" tới máy chủ SMTP đó với đúng địa chỉ gửi, Mailpit dev không nhận thư nào; log không chứa URL SMTP. Không có cấu hình thì Kratos dùng Mailpit như trước. Chưa thử với Gmail thật: cần mật khẩu ứng dụng của chủ tài khoản.

## 5. Tài liệu và phát hành

- [x] 5.1 README hai thứ tiếng, CHANGELOG, QUESTIONS.md; phát hành 1.16.0
  - Xác minh: README hai thứ tiếng (mục "Gửi email tới hòm thư thật", bảng cấu hình, xử lý sự cố), `scripts/check-docs-i18n.py`; CHANGELOG 1.16.0; QUESTIONS.md 128–129; clippy sạch, `cargo test --workspace` qua, E2E `e2e_auth` 44, `e2e_chat` 5, `e2e_source_scan` 1; CI và release.yml của tag `v1.16.0`.
