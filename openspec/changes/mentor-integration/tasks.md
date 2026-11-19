# Tasks

## 1. Bản vá Mentor

- [x] 1.1 Gateway: `/api/auth/feco/*` (Authlib), `users.feco_subject`, mã dùng một lần, `issue_session`, `set-role`, cấu hình `FECO_*`; xác minh bằng pytest scenario "Lần đầu đăng nhập bằng F-Eco", "Lần sau", "Chưa bật", "Địa chỉ quay về lạ", "Mã dùng lại", "Người dùng hủy", "Nâng vai trò"
  - Xác minh: `tests/test_gw_feco_auth.py` (13 test) của Mentor:
    - lần đầu tạo học sinh, mã dùng một lần;
    - lần sau dùng đúng tài khoản; giáo viên đã liên kết vào vai trò giáo viên;
    - `return` lạ trả 400 `bad_return`; webview Tauri và `CORS_ORIGINS` được nhận;
    - callback không có `/login` trước trả 400 `feco_state`;
    - lỗi `feco_denied`, `feco_state`, `feco_failed`, `feco_disabled` quay về app;
    - mã hết hạn bị từ chối và bị xóa;
    - `set-role` nâng vai trò và xóa phiên;
    - đăng nhập mật khẩu vẫn chạy.

    Cả bộ test gateway: 450 passed, 6 skipped.
- [x] 1.2 App: nút "Đăng nhập bằng F-Eco", đổi mã khi quay về, thông báo lỗi; xác minh bằng vitest scenario "Chưa bật", "Mã dùng lại", "Người dùng hủy"
  - Xác minh: `src/feco-login.test.tsx` (6 test):
    - nút chỉ hiện khi gateway bật;
    - nút đi tới gateway kèm origin của app;
    - đổi mã, lưu phiên, xóa mã khỏi địa chỉ;
    - mã đã dùng hiện "Mã đăng nhập F-Eco đã hết hạn. Thử lại nhé.";
    - `feco_denied` hiện "Bạn đã hủy đăng nhập bằng F-Eco.", lỗi lạ hiện câu chung.

    Cả bộ vitest: 271 passed (gồm 6 test mới); `pnpm build` (tsc + vite) qua.
- [x] 1.3 `integrations/mentor/` (README, NOTICE, bản vá, `MENTOR_COMMIT`, `tauri.feco.conf.json`); xác minh bằng `git apply --check` lên bản sao mới đúng commit
  - Xác minh: hai bản vá áp sạch (`git apply --check`) lên worktree mới tại commit `0a3b8b7`.

## 2. F-Eco

- [x] 2.1 Logo, `scripts/dev-apps.sh mentor`; xác minh bằng chạy lệnh trên môi trường dev và scenario "Mở bản web từ launcher" với F-Eco dev thật
  - Xác minh:
    - `scripts/dev-apps.sh mentor` tạo client phía máy chủ, ký `mentor-0.2.0-web.zip` (logo), ghi danh mục;
    - chạy thật gateway Mentor (cổng 8100, Authlib thật, Hydra dev) và app web (cổng 5173) với tài khoản F-Eco thử mới tạo trên máy dev;
    - "Đăng nhập bằng F-Eco" → trang đăng nhập F-Eco → màn hình đồng ý ghi "Mentor by katD2707", chia sẻ tên → quay về Mentor đã đăng nhập (màn hình chọn ngôn ngữ lần đầu), không còn `feco_code` trong địa chỉ;
    - gateway: `/v1/me` 200, callback 303, exchange 200; tài khoản `feco-…` vai trò học sinh, tên lấy từ F-Eco, đã tạo không gian học tập; bảng mã đăng nhập trống.
- [ ] 2.2 Job CI `mentor-desktop`, `release.yml` đính kèm gói desktop; xác minh bằng scenario "Bản vá còn áp được" và "App desktop"

## 3. Tài liệu và phát hành

- [ ] 3.1 README, CHANGELOG, QUESTIONS.md; phát hành 1.11.0
