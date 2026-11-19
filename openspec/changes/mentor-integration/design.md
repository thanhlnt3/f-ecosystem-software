# Design

## M1. Luồng đăng nhập bằng F-Eco

```
App Mentor (web hoặc desktop)            Gateway Mentor                       F-Eco (Hydra, feco-server)
  "Đăng nhập bằng F-Eco"  ──────────────► GET /api/auth/feco/login?return=<origin của app>
                                           kiểm return, lưu vào cookie ký
                                           Authlib: state, nonce, PKCE ─────► /oauth2/auth
                                                                              đăng nhập, đồng ý (lần đầu)
                                           GET /api/auth/feco/callback ◄───── code, state
                                           đổi mã bằng client secret, kiểm ID token (JWKS)
                                           GET /v1/me (tên)
                                           tìm hoặc tạo tài khoản, tạo mã dùng một lần
  <origin>/?feco_code=<mã> ◄──────────────  303
  POST /api/auth/feco/exchange {code} ───► xóa mã, cấp phiên như /api/auth/login
  lưu token, vào app ◄───────────────────  {token, user}
```

- **Client phía máy chủ** (`client_secret_basic`, `subject_type: pairwise`), giống ThesisAI. Client secret chỉ nằm ở gateway.
- **Authlib** lo state, nonce, PKCE, đổi mã, kiểm chữ ký ID token. State lưu trong cookie `mentor_feco` của Starlette `SessionMiddleware`:
  - ký bằng `FECO_SESSION_SECRET`; không đặt thì sinh ngẫu nhiên mỗi lần chạy;
  - sống 10 phút, `SameSite=Lax`, `Secure` khi gateway chạy HTTPS.
- **Địa chỉ quay về** (`return`) MUST nằm trong danh sách origin của gateway (`Settings.allowed_origins`): app web (`WEB_ORIGIN`), webview Tauri (`http://tauri.localhost`, `tauri://localhost`, `https://tauri.localhost`), `CORS_ORIGINS`. Địa chỉ khác bị từ chối (400 `bad_return`), nên mã không bị gửi tới trang lạ. Callback không có cookie của `/login` thì trả 400 `feco_state`.
- **Mã dùng một lần thay vì token trong địa chỉ.** ThesisAI đặt JWT sau dấu `#`. Mentor dùng `HashRouter`, nên phần `#` đã là đường dẫn trong app. Vì vậy gateway trả mã ngẫu nhiên trong query `?feco_code=`:
  - mã lưu dạng băm, sống 120 giây, dùng một lần;
  - app đổi mã lấy token phiên rồi xóa mã khỏi thanh địa chỉ bằng `history.replaceState`;
  - token phiên (sống lâu) không bao giờ nằm trong địa chỉ;
  - callback xóa các mã đã hết hạn.

## M2. Ánh xạ tài khoản

- Tài khoản Mentor liên kết qua `users.feco_subject` (`sub` pairwise của F-Eco, unique).
  - Cơ sở dữ liệu cũ nhận cột mới bằng cơ chế `_ADDED_COLUMNS` sẵn có của Mentor, cùng chỉ mục unique `ix_users_feco_subject`.
  - Mentor không dùng Alembic.
- **Lần đầu đăng nhập** thì tạo tài khoản **học sinh**:
  - tên tài khoản `feco-<12 ký tự đầu SHA-256 của sub>`;
  - tên hiển thị lấy từ `GET /v1/me`, không có thì là "F-Eco";
  - mật khẩu ngẫu nhiên không ai biết, nên tài khoản chỉ đăng nhập qua F-Eco.
- **Mentor không có email**, nên không liên kết theo email. Scope chỉ `openid profile`.
- **Giáo viên** không bao giờ được tạo qua F-Eco. Quản trị Mentor nâng vai trò bằng:

  ```
  python -m mentor_gateway.cli set-role --username <tên> --role teacher
  ```

  Lệnh này kết thúc các phiên đang mở của tài khoản. Tài khoản giáo viên có `feco_subject` thì đăng nhập F-Eco vào thẳng giao diện giáo viên.
- **Phiên** cấp đúng như đăng nhập bằng mật khẩu (`issue_session`, tách từ `/api/auth/login`): lần đầu học sinh đăng nhập thì tạo không gian học tập.
- **Đăng nhập bằng mật khẩu vẫn giữ.**

## M3. App web và app desktop trong F-Eco

- **Một app `mentor`, hai target TUF**:
  - `web`: `web_url` là `http://localhost:8100/api/auth/feco/login?return=http://localhost:5173` trong dev. Bấm "Mở" thì vào thẳng luồng đăng nhập F-Eco.
  - `windows-x64`: gói desktop.

  Launcher trên Windows dùng gói desktop khi có. Launcher di động và F-Store trên web dùng bản web.
- **Target desktop không có `client_id`.** Launcher không cấp mã khởi chạy, vì app Mentor không dùng F-SDK. Trong app desktop, nút "Đăng nhập bằng F-Eco" đưa webview Tauri qua gateway và F-Eco rồi quay về `http://tauri.localhost/?feco_code=...`.
- **Dựng app desktop trên CI**, vì crate Rust của Tauri chưa được phép tải về máy dev. Job `mentor-desktop` (Windows):
  1. tải Mentor đúng commit trong `integrations/mentor/MENTOR_COMMIT`, áp bản vá;
  2. chạy pytest của gateway và vitest của app;
  3. `pnpm tauri build --no-bundle` với `VITE_API_BASE=http://localhost:8100/api`, và tệp cấu hình `tauri.feco.conf.json` bỏ địa chỉ cập nhật của Mentor (F-Launcher lo cập nhật qua TUF);
  4. đóng gói `Mentor.exe`, NOTICE, SOURCE.txt thành `mentor-<phiên bản>-windows-x64.zip`.
- **Cổng dev**: gateway Mentor 8100 (8000 thường bị app khác dùng), app web 5173.

## M4. Giấy phép

- Repo Mentor chưa có LICENSE. Chủ dự án xác nhận tác giả cho phép tích hợp. F-Store ghi giấy phép "Chưa công bố giấy phép (tác giả cho phép F-Eco tích hợp)", giống ThesisAI.
- Mentor chứa mã DeepTutor (Apache-2.0) và tám tệp kỹ năng CC BY-SA 4.0. `integrations/mentor/NOTICE.md` và gói desktop ghi nguồn các phần này.
