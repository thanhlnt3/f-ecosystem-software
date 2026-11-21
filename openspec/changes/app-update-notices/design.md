# Design

## U1. Khi nào gửi thông báo cập nhật

Một phiên bản được báo khi nó là **bản phát hành cho mọi người**:

- `catalog_versions.status = 'released'`;
- `channel = 'stable'`;
- `rollout_percent = 100`;
- app đang `listed`.

Hàm `notifications::announce_updates(state, app: Option<&str>)` làm như sau:

1. Lấy các bản phát hành cho mọi người của app (hoặc của mọi app khi `app` là `None`).
2. Chọn bản mới nhất theo semver cho mỗi app. Không báo bản trung gian: app đi từ 1.0 lên 1.2 thì chỉ báo 1.2.
3. `INSERT INTO app_update_notices (app_id, version) … ON CONFLICT DO NOTHING`. Đã có dòng thì thôi, đây là chỗ chống gửi trùng.
4. Mới chèn thì gửi `Category::AppUpdates` cho người có dòng `catalog_installs` mà `version` là NULL hoặc cũ hơn (`catalog::newer`).

Hàm được gọi ở hai chỗ:

- **`developer::release`**: thay cho `app_updated`. Phát hành 10% thì chưa báo. Nâng lên 100% (gọi lại `release`) thì báo.
- **Việc định kỳ 10 phút trong `main.rs`**, với `None`. Nó bắt bản phát hành bằng `feco-catalog release`, công cụ vận hành không có mailer.

Migration `0022` chèn sẵn mọi bản đang phát hành cho mọi người vào `app_update_notices`. Nhờ vậy, lần chạy đầu của việc định kỳ không gửi lại thông báo cho các bản cũ.

Bản beta không có thông báo. F-Launcher chưa có lựa chọn kênh beta (xem QUESTIONS.md).

## U2. "Bản mới nhất" trong "Ứng dụng của tôi"

`catalog::library` lấy `latest` từ các bản phát hành cho mọi người (U1), chọn theo `released_at`.

- Bản đang phát hành dần, bản tạm dừng (`rollout_percent = 0`) và bản beta không làm hiện "Có bản mới" trên web hay di động.
- Người thuộc nhóm 10% đã cập nhật trên F-Launcher thì có phiên bản mới hơn `latest`, nên cũng không thấy "Có bản mới".

## U3. F-Launcher báo máy chủ

`report_install(app, version)` đã có (`POST /v1/catalog/installs`). Giờ nó được gọi thêm sau khi:

- `Update` thành công;
- `UpdateAll` và tự cập nhật: cho từng app cập nhật thành công;
- `Rollback`: phiên bản sau khi quay lại.

Gỡ app thì gọi `remove_install(app)`, tức `DELETE /v1/catalog/installs/{app}`:

- Máy chủ xóa dòng `catalog_installs` của người dùng và trả 204, kể cả khi không có dòng nào.
- Endpoint chỉ nhận token của F-Launcher (`launch::launcher_subject`), như `POST`.
- App đã mua vẫn còn trong "Ứng dụng của tôi" với nhãn "Đã mua".
- Báo lỗi thì không chặn việc gỡ, chỉ ghi log, như `report_install`.

Một tài khoản dùng F-Launcher trên hai máy: gỡ ở một máy thì xóa bản ghi chung, dù máy kia vẫn cài. Máy kia sẽ báo lại khi cập nhật lần tới. Điểm này được ghi vào QUESTIONS.md.

## U4. Bản đã bỏ qua trong F-Launcher

- `Launcher::update_all(now, skipped)` nhận danh sách `app → phiên bản đã bỏ qua` (`prefs.skipped_updates`) và không cài các bản đó.
- Huy hiệu Thư viện và nút "Cập nhật tất cả (N)" đếm như `update_suggestions`.
- Mục "Cập nhật lên X" trong menu của từng app vẫn cài được bản đã bỏ qua. Người dùng chọn tận tay.

## U5. Di động

- `AppState.refreshLibrary()` tải lại `/v1/library` và `/v1/notifications` mà không hiện vòng chờ.
- `AppLifecycleListener.onResume` gọi nó cùng `refreshChat()`.
- `LibraryItem.desktopUpdate` (`updateAvailable && webUrl == null`) dùng cho nhãn, dòng tóm tắt và huy hiệu. App web luôn mở bản mới nhất nên không có nhãn.
- Khóa i18n mới `updates_banner`, đủ bốn thứ tiếng.

Di động chưa có thông báo đẩy. Nếu thêm thì cần thư viện mới, Firebase hoặc APNs, và khóa dịch vụ (QUESTIONS.md).
