# Tasks

## 1. Chat trên máy chủ

- [ ] 1.1 Migration `0019_chat.sql` (chuyển `project_messages`), module `chat.rs` (hội thoại, tin, đã đọc, chặn, báo cáo, giới hạn, đính kèm và quét), `pg_notify` + `PgListener` + `broadcast`; xác minh bằng E2E scenario "Nhắn từ trang app", "Không phải nhà phát triển", "Quá nhiều hội thoại mới", "Tin cũ được giữ", "Người ngoài", "Tin quá dài", "Đã đọc", "Tệp có mã độc", "Bị chặn"
- [ ] 1.2 Trang web `/messages`, SSE `/messages/events`, ô chat trong trang dự án, nút "Nhắn nhà phát triển", `/admin/chat-reports`, bốn thứ tiếng; xác minh bằng E2E scenario "Hai trình duyệt", "Quản trị giải quyết tranh chấp"
- [ ] 1.3 API `/v1/chat/*`; xác minh bằng E2E scenario "Launcher gửi tin"
- [ ] 1.4 Nhóm thông báo `messages`, không chép nội dung, gộp 15 phút; bỏ trích nội dung ở thông báo dự án; xác minh bằng E2E scenario "Không lộ nội dung", "Gộp thông báo"

## 2. Chat trong F-Launcher

- [ ] 2.1 F-Launcher máy tính: mục "Tin nhắn", huy hiệu, danh sách, luồng tin, gửi, hỏi 5 giây; xác minh bằng test scenario "Huy hiệu chưa đọc", "Trả lời trong launcher"
- [ ] 2.2 F-Launcher di động: tab "Tin nhắn"; xác minh bằng widget test scenario "Gửi tin từ điện thoại", "Chưa đăng nhập"

## 3. Quét mã nguồn

- [ ] 3.1 Crate `feco-codescan` (thư viện và lệnh); xác minh bằng unit test scenario "Khóa riêng trong mã", "Khóa giả trong test", "Thư viện có lỗ hổng" (máy chủ OSV giả), "Đào tiền ảo", "Zip độc"
- [ ] 3.2 Migration `0020_source_scan.sql`; nộp mã nguồn (web, `PUT /v1/dev/versions/{id}/source`, `[source]` trong `f-app.toml`, `f-dev publish --source`); `run_submit` quét mã nguồn; báo cáo ở trang phiên bản và trang duyệt; xác minh bằng E2E scenario "Bí mật lộ bị từ chối", "Lỗ hổng cần người duyệt", "Không kèm mã nguồn", "Thứ tự kiểm tra"
- [ ] 3.3 `scripts/scan-integration.sh`, `integrations/{mentor,thesisai}/SCAN.md` và `scan-allow.toml`, job CI `integration-scan`, bước quét trong `build-desktop.sh`; xác minh bằng chạy quét thật Mentor và ThesisAI, và scenario "Commit mới có bí mật"

## 4. Tài liệu và phát hành

- [ ] 4.1 README, CHANGELOG, QUESTIONS.md; phát hành 1.12.0
