# Tasks

## 1. Metadata và manifest (`developer-portal`)

- [x] 1.1 `TargetMeta` thêm `summary`, `category`, `developer`, `source`, `terminal` (không bắt buộc); `feco-tuf publish` nhận cờ tương ứng; xác minh metadata cũ vẫn đọc được
  - Đã xác minh:
    - unit test `display_fields_are_optional` (`crates/feco-tuf`): metadata của bản 1.0 vẫn đọc được, trường rỗng không được ghi ra;
    - `feco-tuf publish --summary --category --developer --source --terminal` ký năm app của danh mục dev.
  - `TargetMeta` không từ chối trường lạ, nên launcher 1.0 bỏ qua các trường mới. Chưa chạy thử launcher 1.0 với kho mới.
- [x] 1.2 `f-app.toml` thêm `source` (HTTPS) và `terminal`; xác minh bằng unit test của `feco-manifest`
  - Đã xác minh: unit test `source_and_terminal` gồm `source` http bị từ chối với đúng thông báo của spec, và `terminal` sai kiểu.
- [x] 1.3 Máy chủ ghi thông tin hiển thị khi ký target; xác minh bằng E2E phát hành rồi đọc target TUF
  - Đã xác minh: E2E `child_app_integration_end_to_end`. Manifest có `summary`, `category`, `source`; sau khi duyệt và phát hành, launcher đồng bộ và đọc được đủ các trường từ target đã ký, kể cả `developer` lấy từ danh mục.
  - App không khai scope thì target không có `client_id` (`developer.rs`, `sign_target`).
- [x] 1.4 `f-dev pack` từ chối gói có `source` mà thiếu giấy phép gốc; xác minh bằng unit test
  - Đã xác minh, unit test của `f-dev`:
    - `source_package_needs_upstream_license`: gói chỉ có `kibi.exe` bị từ chối, không tạo zip; có `licenses/LICENSE-*` thì đạt;
    - `license_file_names`;
    - `terminal_app_with_scopes_on_macos_warns`.
  - E2E 1.3 nộp gói kèm `LICENSE` qua cùng đường kiểm tra.

## 2. Thư viện, cửa hàng và cài đặt (`launcher`)

- [x] 2.1 `library::arrange`: tìm không dấu, lọc nhóm, sắp xếp theo tên, mở gần đây, ngày cài, nhóm, dung lượng; `last_opened` ghi khi mở app; xác minh bằng unit test cho từng scenario
  - Đã xác minh:
    - unit test trong `library.rs` cho các scenario "Tìm không dấu", "Không có kết quả", "Sắp xếp theo lần mở gần nhất", "Lọc theo nhóm", "Biểu tượng ổn định", cùng sắp xếp theo tên, ngày cài, nhóm, dung lượng;
    - E2E 1.3 kiểm `mark_opened` ghi `last_opened`.
- [x] 2.2 `prefs.rs`: `settings.json` ghi nguyên tử, giá trị hỏng về mặc định, cỡ chữ kẹp 80–150%; xác minh bằng unit test
  - Đã xác minh: unit test `saved_prefs_survive_restart`, `broken_file_falls_back_to_defaults` (file không phải JSON; cỡ chữ 900%; trường sai kiểu; kiểu sắp xếp không hợp trang), `reset_restores_defaults`.
- [x] 2.3 Giao diện mới: thanh bên (Thư viện, Cửa hàng, Thông báo, Cài đặt), thanh công cụ (tìm, nhóm, sắp xếp, lưới/danh sách), ô lưới và hàng danh sách, biểu tượng chữ cái đầu; xác minh bằng chạy thử trên Windows
  - Đã xác minh trên Windows 10 với kho TUF dev có 6 app, chụp lại từng màn hình:
    - Cửa hàng ở bố cục lưới và danh sách;
    - đổi bố cục được nhớ trong `settings.json`;
    - tìm "ghi chu" chỉ ra "Ghi chú", kèm nút "Xóa bộ lọc";
    - hộp thoại cài có mô tả, nhà phát triển, mã nguồn;
    - Thư viện báo bản cập nhật và "Cập nhật tất cả".
  - Trong lúc thử phát hiện eframe 0.37 với renderer glow vẽ cửa sổ trắng trên máy này (cả bản 1.0.0). Đã chuyển sang wgpu (QUESTIONS.md mục 71).
- [x] 2.4 Trang Cài đặt: chủ đề, màu nhấn, cỡ chữ, bố cục, cỡ ô, trang mở đầu, mô tả ngắn, chế độ gọn, tự động cập nhật, tài khoản, giới thiệu, khôi phục mặc định; xác minh bằng chạy thử và unit test lưu/đọc
  - Đã xác minh khi chạy thử:
    - chọn "Tối" và màu cam thì áp dụng ngay, `settings.json` ghi `dark` và `orange`;
    - "Khôi phục mặc định" ghi lại toàn bộ giá trị mặc định;
    - giờ hiển thị theo múi giờ của máy (bản 1.0 hiện giờ UTC).
- [x] 2.5 Mở app `terminal` trong cửa sổ terminal riêng; xác minh bằng unit test `launch::command` và chạy thử kibi trên Windows
  - Đã xác minh:
    - unit test `terminal_apps_open_in_a_terminal_window` và `non_terminal_apps_run_directly`;
    - trên Windows, kibi cài từ launcher mở trong một cửa sổ console mới.
  - Lần thử đầu chỉ dùng cờ `CREATE_NEW_CONSOLE` thì app thừa hưởng stdio của launcher và báo "handle is invalid", nên đã đổi sang `cmd /C start` (design E5).
  - Chưa thử trên macOS.

## 3. App mới và hướng dẫn

- [x] 3.1 App mẫu dùng F-SDK `examples/apps/f-notes` (Ghi chú) và `examples/apps/f-timer` (Bấm giờ); xác minh bằng unit test và mở từ launcher
  - Đã xác minh:
    - unit test của `f-notes` (thêm, sửa, tìm, xóa, lưu, file hỏng), `f-timer` (bấm giờ, vòng, hẹn giờ chuyển pha, định dạng, thống kê) và `feco-sample-common`;
    - Ghi chú cài từ launcher; chạy ở chế độ khách thì tạo và tự lưu ghi chú.
  - Đăng nhập qua mã khởi chạy dùng `fsdk::session`, cùng đường F-SDK mà E2E `child_app_integration_end_to_end` đã kiểm. Lần đầu chưa đồng ý quyền thì F-SDK mở trình duyệt.
- [x] 3.2 `scripts/dev-apps.sh`: đưa app mẫu và app từ GitHub (tải đúng tag/commit, dựng, đóng gói kèm giấy phép gốc, ký TUF, ghi danh mục) vào môi trường dev; xác minh bằng chạy với kibi, bottom, eframe template
  - Đã xác minh: `scripts/dev-apps.sh all` ký và ghi danh mục cho f-notes, f-timer, kibi 0.3.3, bottom 0.15.0, eframe template (commit `f108546`). Chạy lại thì bỏ qua phiên bản đã có.
  - Mã nguồn dựng ngoài repo (`~/.feco-dev-ext`): dựng trong repo thì Cargo coi là thành viên workspace, và `git describe` của kibi đọc nhầm tag F-Eco.
- [x] 3.3 `docs/github-apps.md` và mục mới trong `docs/integration-guide.md`; README cập nhật
  - Đã xác minh:
    - `docs/github-apps.md` có 11 mục, theo đúng các bước đã làm với ba app;
    - `integration-guide.md` có thêm `source`, `terminal`, `f-dev pack`, mục 13 "App hiển thị thế nào trong F-Launcher";
    - README bước 6 có thêm mục 9 và 10; CHANGELOG có mục "Chưa phát hành".
