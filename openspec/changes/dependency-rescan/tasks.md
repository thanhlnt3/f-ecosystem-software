# Tasks

## 1. Quét lại mã nguồn

- [x] 1.1 `source_scan::scan_submission` và `source_scan::rescan`; `review::run_rescan` báo lỗ hổng mới cho nhà phát triển và quản trị; xác minh bằng E2E scenario "Lỗ hổng mới công bố", "Không báo lại", "Báo cáo trên trang phiên bản" (OSV giả)
  - Xác minh: E2E `source_scan_end_to_end` (OSV giả, cờ `NEW_VULN`): bản 1.0.2 đang phát hành; quét lại khi chưa có lỗ hổng mới thì không có email hay cảnh báo; OSV công bố `GHSA-e2e0-late-0002` cho `late-vuln-e2e` 2.0.0 thì email "Lỗ hổng mới trong thư viện của … 1.0.2" nêu mã đó (không nêu lỗ hổng cũ) và có hướng dẫn osv.dev, có một cảnh báo `new_vulnerability`, phiên bản vẫn `released`, `checks.source.counts.high = 2`, trang phiên bản có 2 dòng phát hiện; quét lại lần nữa vẫn 1 email, 1 cảnh báo.
- [x] 1.2 Mục "Cảnh báo kiểm duyệt" ở `/admin/reviews`; xác minh bằng E2E scenario "Xem cảnh báo lỗ hổng mới"
  - Xác minh: cùng E2E: `/admin/reviews` của người duyệt có dòng `admin-alert` `data-kind="new_vulnerability"` đúng app và phiên bản, có mã lỗ hổng; `?status=new_vulnerability&q=<app>` chỉ còn cảnh báo, không còn hàng chờ. `admin_list_tools` và toàn bộ `e2e_auth` vẫn qua.

## 2. App tích hợp và kiểm thử

- [x] 2.1 Workflow `integration-scan.yml` chạy hằng tuần; xác minh bằng chạy tay (`workflow_dispatch`) trên GitHub
  - Xác minh: `scripts/scan-integrations-all.sh` dùng chung cho job `integration-scan` của `ci.yml` (CI xanh) và `integration-scan.yml` (`schedule` thứ Hai 02:00 UTC, `workflow_dispatch`); chạy tay workflow sau khi đẩy lên.
- [x] 2.2 E2E mặc định dùng CSDL `feco_e2e`; xác minh bằng chạy E2E không đặt `FECO_E2E_DATABASE_URL` và CSDL dev không đổi
  - Xác minh: chạy `e2e_auth -- app_update_notices` không đặt `FECO_E2E_DATABASE_URL`: dữ liệu thử nằm trong `feco_e2e` (2 app `notice-*`), CSDL dev `feco` vẫn 7 app trước và sau.

- [x] 2.3 Cập nhật Mentor lên 0.2.2 (`MENTOR_COMMIT`, bản vá nâng `pypdf`, `soupsieve`, `opentelemetry-api`, `url`) và ThesisAI lên commit mới nhất; xác minh bằng quét lại bản đã vá và job CI `mentor-desktop`, `thesisai-fe`
  - Xác minh: Mentor `5ad3c9f` (0.2.2) áp hai bản vá không cần sửa, `git apply --check` trên commit sạch qua; quét bản đã vá: high 5 → 2 (còn `soupsieve` 2.10 cho Python < 3.11.5); ThesisAI fe `6718221`, be `db66b57` áp bản vá, quét như trước; job CI `mentor-desktop` (test và dựng Mentor) và `thesisai-fe` (dựng Next.js) chạy trên commit mới.

## 3. Tài liệu

- [x] 3.1 README hai thứ tiếng, CHANGELOG, QUESTIONS.md
  - Xác minh: README hai thứ tiếng (tóm tắt kiểm duyệt, kiểm thử: CSDL E2E và Kratos với Mailpit), `scripts/check-docs-i18n.py`; CHANGELOG 1.17.0; QUESTIONS.md 130–132.
