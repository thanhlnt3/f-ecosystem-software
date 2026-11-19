# Tích hợp Mentor với F-Eco

Mentor là ứng dụng học tập cho học sinh từ lớp 1 tới đại học và cho giáo viên: khóa học, luyện tập, hỏi đáp, kế hoạch học. Mã nguồn: https://github.com/katD2707/agent-mentor-maximum-experience (commit trong `MENTOR_COMMIT`).

Mentor có hai phần:
- **app** (`apps/app`): React 19 + Vite. Chạy trên trình duyệt (cổng 5173), hoặc đóng gói thành app desktop bằng Tauri 2;
- **gateway** (`services/gateway`): FastAPI, giữ tài khoản, khóa học và gọi AI.

Trong F-Eco, Mentor có cả **app web** và **app desktop**:
- F-Store có trang Mentor với nút "Mở ứng dụng web".
- F-Launcher trên Windows cài và mở app desktop `Mentor.exe` (khi kho có gói desktop). Trên máy khác, và trên F-Launcher di động, Mentor là app web.
- Người dùng đăng nhập Mentor bằng tài khoản F-Eco (OIDC). Đăng nhập bằng mật khẩu của Mentor vẫn giữ.

Thư mục này chỉ có bản vá, không có mã Mentor (NOTICE.md).

## Tệp

| Tệp | Nội dung |
|---|---|
| `MENTOR_COMMIT` | Commit của Mentor mà bản vá áp vào |
| `mentor-gateway.patch` | Gateway: route `/api/auth/feco/*` (Authlib), cột `users.feco_subject`, bảng `feco_login_codes`, cấu hình `FECO_*`, lệnh `set-role`, test `tests/test_gw_feco_auth.py`, thư viện `authlib` và `itsdangerous` |
| `mentor-app.patch` | App: nút "Đăng nhập bằng F-Eco", đổi mã dùng một lần lấy phiên, thông báo lỗi tiếng Việt và tiếng Anh, test `src/feco-login.test.tsx` |
| `tauri.feco.conf.json` | Cấu hình Tauri thêm khi F-Eco dựng app desktop: bỏ địa chỉ cập nhật của Mentor, vì F-Launcher cập nhật app qua TUF |

Áp bản vá:

```bash
git clone https://github.com/katD2707/agent-mentor-maximum-experience mentor
cd mentor
git checkout "$(cat ../f-ecosystem/integrations/mentor/MENTOR_COMMIT)"
git apply ../f-ecosystem/integrations/mentor/mentor-gateway.patch
git apply ../f-ecosystem/integrations/mentor/mentor-app.patch
```

## Luồng đăng nhập

1. Người dùng bấm "Đăng nhập bằng F-Eco" trong app, hoặc bấm "Mở" Mentor trong F-Launcher. Trình duyệt (hoặc webview của app desktop) mở `GET {gateway}/api/auth/feco/login?return=<origin của app>`.
2. Gateway kiểm `return`: phải là origin được phép của Mentor (`WEB_ORIGIN`, webview Tauri, `CORS_ORIGINS`). Sau đó gateway chuyển tới F-Eco theo authorization code + PKCE. `state`, `nonce` và mã PKCE nằm trong cookie ký `mentor_feco`, sống 10 phút.
3. F-Eco đăng nhập người dùng, hỏi đồng ý chia sẻ tên (lần đầu), rồi chuyển về `GET /api/auth/feco/callback`.
4. Gateway đổi mã bằng client secret và kiểm chữ ký ID token. Sau đó gateway lấy tên qua `GET {FECO_SERVER_URL}v1/me` và tìm tài khoản theo `feco_subject` (`sub` pairwise):
   - chưa có tài khoản thì tạo tài khoản **học sinh** tên `feco-<12 ký tự>`, mật khẩu ngẫu nhiên;
   - giáo viên do quản trị Mentor nâng vai trò (bên dưới).
5. Gateway chuyển về `<origin>/?feco_code=<mã>`. Mã ngẫu nhiên, dùng một lần, sống 120 giây. App xóa mã khỏi thanh địa chỉ, gọi `POST /api/auth/feco/exchange` để lấy token phiên như đăng nhập bằng mật khẩu, rồi vào app.

Lỗi quay về dạng `?feco_error=` (`feco_denied`, `feco_failed`, `feco_state`, `feco_disabled`, `feco_unavailable`). App hiện câu thông báo tương ứng ở màn hình đăng nhập.

**Giáo viên.** Đăng nhập bằng F-Eco không tạo tài khoản giáo viên. Quản trị Mentor nâng vai trò tài khoản đã đăng nhập bằng F-Eco:

```bash
uv run python -m mentor_gateway.cli set-role --username feco-xxxxxxxxxxxx --role teacher
```

Lệnh này kết thúc các phiên đang mở của tài khoản. Lần đăng nhập F-Eco sau vào giao diện giáo viên.

## Chạy cùng F-Eco dev

1. Chạy F-Eco dev (README của F-Eco, các bước 1–7).
2. Đưa Mentor vào kho TUF và danh mục dev:

   ```bash
   scripts/dev-apps.sh mentor
   ```

   Lệnh này:
   - tạo client OAuth phía máy chủ (scope `openid profile`, chuyển hướng `http://localhost:8100/api/auth/feco/callback`). Client ID và secret lưu trong `.dev/mentor-client-id` và `.dev/mentor-client-secret` (không commit);
   - ký target `web` của Mentor, `web_url` là `http://localhost:8100/api/auth/feco/login?return=http://localhost:5173`;
   - có `MENTOR_DESKTOP_ZIP` (gói `mentor-<phiên bản>-windows-x64.zip` từ bản phát hành F-Eco) thì ký thêm target `windows-x64`;
   - ghi mục F-Store kèm logo;
   - in các biến cần đặt cho gateway Mentor.
3. Chạy gateway Mentor (trong thư mục Mentor đã áp bản vá) với các biến script in ra:

   ```bash
   uv sync --all-packages
   MENTOR_ENV=dev PUBLIC_BASE_URL=http://localhost:8100 WEB_ORIGIN=http://localhost:5173 \
     FECO_ISSUER_URL=http://127.0.0.1:4444/ FECO_SERVER_URL=http://127.0.0.1:8080/ \
     FECO_CLIENT_ID=<client id> FECO_CLIENT_SECRET=<nội dung .dev/mentor-client-secret> \
     uv run uvicorn mentor_gateway.app:app --port 8100
   ```

   Các tính năng AI (hỏi đáp, soạn khóa học) cần khóa FPT AI Factory (`FPT_API_KEY`, README của Mentor). Đăng nhập, khóa học có sẵn và luyện tập chạy được khi không có khóa.
4. Chạy app web Mentor:

   ```bash
   cd apps/app
   pnpm install
   VITE_API_BASE=http://localhost:8100/api pnpm dev
   ```

5. Trong F-Launcher: Cửa hàng → Mentor → Thêm → Mở. Hoặc mở http://localhost:5173 rồi bấm "Đăng nhập bằng F-Eco".

## App desktop

CI của F-Eco (job `mentor-desktop` trong `.github/workflows/ci.yml` và `release.yml`) dựng app desktop Windows:

1. Tải Mentor đúng `MENTOR_COMMIT`, áp hai bản vá.
2. Chạy pytest của gateway và vitest của app.
3. Dựng app:

   ```bash
   pnpm -C apps/app tauri build --no-bundle --config src-tauri/tauri.feco.conf.json
   ```

   với `VITE_API_BASE=http://localhost:8100/api`.
4. Đóng gói `Mentor.exe`, NOTICE và SOURCE.txt thành `mentor-<phiên bản>-windows-x64.zip`. Bản phát hành F-Eco đính kèm gói này.

App desktop gọi gateway ở `http://localhost:8100`, nên gateway phải chạy trên cùng máy, như bản dùng thử của Mentor. Nút "Đăng nhập bằng F-Eco" đưa webview của app qua gateway và F-Eco, rồi quay về `http://tauri.localhost/?feco_code=...`.

## Kiểm thử

```bash
uv run pytest services/gateway/tests/test_gw_feco_auth.py services/gateway/tests/test_gw_auth.py
pnpm -C apps/app exec vitest run src/feco-login.test.tsx src/app.test.tsx
```
