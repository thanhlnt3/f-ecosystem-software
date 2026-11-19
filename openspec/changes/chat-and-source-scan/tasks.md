# Tasks

## 1. Chat trên máy chủ

- [x] 1.1 Migration `0019_chat.sql` (chuyển `project_messages`), module `chat.rs` (hội thoại, tin, đã đọc, chặn, báo cáo, giới hạn, đính kèm và quét), `pg_notify` + `PgListener` + `broadcast`; xác minh bằng E2E scenario "Nhắn từ trang app", "Không phải nhà phát triển", "Quá nhiều hội thoại mới", "Tin cũ được giữ", "Người ngoài", "Tin quá dài", "Đã đọc", "Tệp có mã độc", "Bị chặn"
  - Xác minh: `tests/e2e_chat.rs`.
    - `direct_conversations_web_and_launcher_api`:
      - Nhắn từ trang app, Không phải nhà phát triển, Quá nhiều hội thoại mới;
      - Người ngoài, Tin quá dài, Đã đọc, Bị chặn;
      - thêm: giới hạn 30 tin mỗi phút, `after`/`limit`, đủ trường JSON của C4.
    - `migration_keeps_old_project_messages`: Tin cũ được giữ. Chạy trên cơ sở dữ liệu tạm tới 0018, có ba tin cũ, rồi áp 0019: đúng thứ tự, đúng người gửi, chưa đọc 0 cho cả hai bên.
    - `attachments_are_scanned_before_download`: Tệp có mã độc.
      - EICAR có trạng thái `infected`, tải trả 403 `chat_attachment_blocked`, có cảnh báo `chat_malware` và email quản trị;
      - thêm: tệp sạch tải được, tệp đang quét trả 409, tệp trên 20 MB trả 413.
- [x] 1.2 Trang web `/messages`, SSE `/messages/events`, ô chat trong trang dự án, nút "Nhắn nhà phát triển", `/admin/chat-reports`, bốn thứ tiếng; xác minh bằng E2E scenario "Hai trình duyệt", "Quản trị giải quyết tranh chấp"
  - Xác minh:
    - `realtime_and_grouped_notifications`: Hai trình duyệt. Sự kiện SSE `message` tới trang của nhà phát triển, và `/messages/{id}/after/{last}` trả "Xin chào". Sự kiện chỉ tới người tham gia.
    - `project_conversation_and_dispute`: Quản trị giải quyết tranh chấp; hai bên thấy tin của quản trị trên web và qua API.
    - Báo cáo hội thoại: Hỗ trợ xem chỉ đọc và đánh dấu đã xử lý (trong `direct_conversations_web_and_launcher_api`).
    - Unit test: mọi khóa chat có bốn thứ tiếng.
- [x] 1.3 API `/v1/chat/*`; xác minh bằng E2E scenario "Launcher gửi tin"
  - Xác minh: `direct_conversations_web_and_launcher_api` gửi tin bằng token của launcher máy tính và di động: 201, `mine: true`, bên kia thấy trên web. Tên trường của `ConversationJson` và `MessageJson` khớp với phần đọc JSON của launcher (`chat.rs`) và app di động (`chat.dart`).
- [x] 1.4 Nhóm thông báo `messages`, không chép nội dung, gộp 15 phút; bỏ trích nội dung ở thông báo dự án; xác minh bằng E2E scenario "Không lộ nội dung", "Gộp thông báo"
  - Xác minh:
    - `project_conversation_and_dispute`: Không lộ nội dung. Thông báo và email chỉ có "Tin nhắn mới từ Minh Khách" và đường dẫn, không có "abc123".
    - `realtime_and_grouped_notifications`: Gộp thông báo.
      - 5 tin cho đúng 1 thông báo và 1 email; quá 15 phút thì có thông báo tiếp;
      - không báo khi đang mở hội thoại;
      - nhóm "Tin nhắn" bật mặc định và tắt được.

## 2. Chat trong F-Launcher

- [x] 2.1 F-Launcher máy tính: mục "Tin nhắn", huy hiệu, danh sách, luồng tin, gửi, hỏi 5 giây; xác minh bằng test scenario "Huy hiệu chưa đọc", "Trả lời trong launcher"
  - Xác minh: `clients/feco-launcher/tests/chat.rs`, với máy chủ `/v1/chat` giả:
    - `unread_badge_after_sync`: Huy hiệu chưa đọc. 3 tin chưa đọc thì huy hiệu "3", lần hỏi 60 giây sau thì "4"; token sai được coi là hết phiên.
    - `reply_in_launcher`: Trả lời trong launcher.
      - 250 tin tải hai trang, rồi đánh dấu đã đọc;
      - "Em gửi bản sửa nhé" ở cuối luồng và máy chủ đã lưu;
      - lần hỏi sau lấy được tin trả lời mà không trùng tin vừa gửi.

    Thêm: unit test đọc JSON, gộp trang, lịch hỏi 5 giây và 60 giây, thông báo lỗi; `every_icon_paints_inside_its_box` có icon `Chat`, `Send`. feco-launcher: 74 test qua.
- [x] 2.2 F-Launcher di động: tab "Tin nhắn"; xác minh bằng widget test scenario "Gửi tin từ điện thoại", "Chưa đăng nhập"
  - Xác minh:
    - widget test `scenario_gui_tin_tu_dien_thoai` và `scenario_tin_nhan_chua_dang_nhap`;
    - thêm: hỏi 5 giây và dừng khi rời luồng tin, tệp đính kèm chỉ xem, unit test JSON và đường dẫn HTTP, `app_version_matches_pubspec`;
    - `flutter analyze` không có vấn đề; `flutter test`: 27 test qua;
    - `integration_test/app_test.dart` đã đổi cho sáu tab, chạy trên CI.

## 3. Quét mã nguồn

- [x] 3.1 Crate `feco-codescan` (thư viện và lệnh); xác minh bằng unit test scenario "Khóa riêng trong mã", "Khóa giả trong test", "Thư viện có lỗ hổng" (máy chủ OSV giả), "Đào tiền ảo", "Zip độc"
  - Xác minh: 31 unit test và 4 test tích hợp.
    - `rules::tests::private_key_in_code`, `fake_key_in_tests_is_medium`, `miner`;
    - `tests/scan.rs::vulnerable_library_via_osv`: máy chủ OSV giả; kiểm cả việc chỉ gửi hệ sinh thái, tên và phiên bản;
    - `archive::tests::evil_zip_is_refused_without_writing`.
- [x] 3.2 Migration `0020_source_scan.sql`; nộp mã nguồn (web, `PUT /v1/dev/versions/{id}/source`, `[source]` trong `f-app.toml`, `f-dev publish --source`); `run_submit` quét mã nguồn; báo cáo ở trang phiên bản và trang duyệt; xác minh bằng E2E scenario "Bí mật lộ bị từ chối", "Lỗ hổng cần người duyệt", "Không kèm mã nguồn", "Thứ tự kiểm tra"
  - Xác minh: `tests/e2e_source_scan.rs::source_scan_end_to_end`.
    - Bí mật lộ bị từ chối: nộp bằng `f-dev --source`, phiên bản bị từ chối với `reject_code` `source_scan`; email ghi luật và tệp, không có token; trang hiện `ghp_****`.
    - Lỗ hổng cần người duyệt: lý do đúng "Quét mã nguồn: 1 phát hiện cần xem"; bảng phát hiện hiện ở trang app và `/admin/reviews`.
    - Không kèm mã nguồn: `checks.source` ghi "Không có mã nguồn", bản cập nhật vẫn tự duyệt.
    - Thứ tự kiểm tra: gói EICAR cùng mã nguồn có bí mật thì bị từ chối vì `malware`, mã nguồn không được quét.
    - Thêm:
      - lỗi mạng khi tải repo thì xếp hàng lại sau 5 phút;
      - SHA sai, không phải zip, zip có `../../evil.sh` đều trả 422;
      - form repo và form tải lên trên web.
  - Khác design: repo trả 4xx (riêng tư, sai commit) thì từ chối thay vì xếp hàng lại mãi (QUESTIONS.md 110); `app_versions.checks` thành `{"packages": [...], "source": {...}}`, trang quản trị đọc được cả dạng cũ.
- [x] 3.3 `scripts/scan-integration.sh`, `integrations/{mentor,thesisai}/SCAN.md` và `scan-allow.toml`, job CI `integration-scan`, bước quét trong `build-desktop.sh`; xác minh bằng chạy quét thật Mentor và ThesisAI, và scenario "Commit mới có bí mật"
  - Xác minh:
    - `tests/scan.rs::cli_github_target_exit_codes` (Commit mới có bí mật): máy chủ codeload giả, mã thoát 2, báo cáo ghi `deploy/key.pem:1`; mục cho phép thì mã thoát 0.
    - Quét thật ngày 19/11/2026 (`integrations/*/SCAN.md`):
      - Mentor `0a3b8b7`: 0 critical, 5 high (lỗ hổng pypdf, soupsieve), 9 medium;
      - thesisai-fe `8f3735e`: 0 critical, 10 high, trong đó Next.js 16.4.0 có hai lỗ hổng OSV xếp CRITICAL (QUESTIONS.md 109);
      - thesisai-be `d890595`: 0 phát hiện.

## 4. Tài liệu và phát hành

- [x] 4.1 README, CHANGELOG, QUESTIONS.md; phát hành 1.12.0
  - Xác minh:
    - `scripts/check-docs-i18n.py`;
    - README mục 6.18, 7 (quét mã nguồn), 12.7, bảng API, cấu hình, kiểm thử, cấu trúc thư mục;
    - CHANGELOG 1.12.0, QUESTIONS.md 109–116;
    - clippy `--workspace --all-targets -D warnings` sạch, `cargo test --workspace` qua;
    - E2E trên cơ sở dữ liệu mới: `e2e_auth` 40 qua (1 bỏ qua theo thiết kế), `e2e_chat` 5, `e2e_source_scan` 1;
    - CI và release.yml của tag `v1.12.0`.
