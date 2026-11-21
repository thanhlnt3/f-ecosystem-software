# Tasks

## 1. Máy chủ

- [x] 1.1 Migration `0022_app_update_notices.sql`, `notifications::announce_updates`, gọi trong `developer::release` và việc định kỳ 10 phút; xác minh bằng E2E scenario "Phát hành dần chưa báo", "Phát hành cho mọi người", "Không báo hai lần", "Người đã có bản mới", "Kênh beta", "Phát hành bằng công cụ vận hành"
  - Xác minh: E2E `developer_portal_end_to_end` (cổng nhà phát triển thật): cài 1.2.0, phát hành 1.3.0 cho 10% và tạm dừng thì không có thông báo; nâng 100% thì đúng một thông báo "có bản cập nhật 1.3.0"; phát hành lại và chạy `announce_updates(None)` không có thông báo thứ hai. E2E `app_update_notices`: kênh beta không báo; bản lên stable bằng `feco-catalog` (chỉ `release_version`) được việc định kỳ báo cho người cài 1.0.0, không báo người đã có 1.1.0.
- [x] 1.2 `catalog::library` chỉ tính bản phát hành cho mọi người; `DELETE /v1/catalog/installs/{app}`; xác minh bằng E2E scenario "Bản đang phát hành dần", "Bản đã tạm dừng", "Đã cập nhật trên máy tính", "Gỡ app"
  - Xác minh: cùng hai E2E: `/v1/library` `update_available` false khi bản mới đang 10% hoặc tạm dừng, true khi 100%; F-Launcher báo 1.3.0 thì hết "Có bản mới"; `DELETE /v1/catalog/installs/{app}` trả 204 (cả lần hai), app rời `/v1/library` và `/library`, không nhận thông báo 1.2.0; không có token trả 401; app đã mua vẫn còn với "Đã mua". `app_logos_and_web_library`, `mobile_client_uses_launcher_apis` vẫn qua.

## 2. F-Launcher máy tính

- [x] 2.1 Báo máy chủ sau khi cập nhật, cập nhật tất cả, tự cập nhật, quay lại bản trước và gỡ (`Auth::remove_install`); xác minh bằng test `tests/auth.rs` và scenario "Cập nhật rồi báo máy chủ", "Gỡ rồi báo máy chủ"
  - Xác minh: `tests/auth.rs::reports_installed_version_and_uninstall` (máy chủ giả nhận `POST /v1/catalog/installs` với phiên bản mới và `DELETE /v1/catalog/installs/soundlab`, kèm access token). `report_install` gọi sau `Update`, `Rollback`; `report_updates` sau `UpdateAll` và tự cập nhật; `report_uninstall` sau `Uninstall`.
- [x] 2.2 `update_all` bỏ qua bản đã bỏ qua; huy hiệu và nút "Cập nhật tất cả" đếm như hộp thoại; xác minh bằng test scenario "Cập nhật tất cả bỏ qua bản đã bỏ qua"
  - Xác minh: `tests/launcher.rs::update_all_leaves_skipped_versions` (bản 2.0.0 đã bỏ qua không được cài, bản 2.1.0 thì có); huy hiệu và nút dùng `update_suggestions` (test `suggests_updates_except_skipped_versions`).

## 3. Di động

- [x] 3.1 `refreshLibrary` khi quay lại app, nhãn chỉ cho app máy tính, dòng tóm tắt, huy hiệu tab; xác minh bằng widget test scenario "App máy tính có bản mới", "App web", "Quay lại app"
  - Xác minh: widget test `scenario_bao_ban_moi_tren_dien_thoai`: kibi 0.3.0 có nhãn, dòng "Số app có bản mới: 1…", huy hiệu 1 trên tab; ThesisAI (app web) không có nhãn; cập nhật rồi giả lập quay lại app (vòng đời paused → resumed) thì hết nhãn và dòng tóm tắt. `flutter analyze` sạch, `flutter test` 28 qua.

## 4. Tài liệu và phát hành

- [x] 4.1 README hai thứ tiếng, CHANGELOG, QUESTIONS.md; phát hành 1.14.0
  - Xác minh: README hai thứ tiếng (mục 12 bước 6, bảng API), `scripts/check-docs-i18n.py`; CHANGELOG 1.14.0; QUESTIONS.md 109 (cập nhật), 121–124; clippy sạch, `cargo test --workspace` qua; E2E `e2e_auth` 43 qua (1 bỏ qua theo thiết kế), `e2e_chat` 5, `e2e_source_scan` 1; CI và release.yml của tag `v1.14.0`.
