# Tích hợp Mentor với F-Eco

Mentor là ứng dụng học tập cho học sinh từ lớp 1 tới đại học và cho giáo viên: khóa học, luyện tập, hỏi đáp, kế hoạch học. Mã nguồn: https://github.com/katD2707/agent-mentor-maximum-experience (commit trong `MENTOR_COMMIT`).

Mentor có hai phần:
- **app** (`apps/app`): React 19 + Vite. Chạy trên trình duyệt (cổng 5173), hoặc đóng gói thành app desktop bằng Tauri 2;
- **gateway** (`services/gateway`): FastAPI, giữ tài khoản, khóa học và gọi AI.

Trong F-Eco, Mentor có cả **app web** và **app desktop**, ở hai phiên bản khác nhau:

| Dạng | Phiên bản | Nguồn | Đăng nhập |
|---|---|---|---|
| App web | 0.2.2 (`MENTOR_COMMIT`) | F-Eco áp bản vá trong thư mục này | Tài khoản F-Eco (OIDC), giữ cả mật khẩu Mentor |
| App desktop Windows | 0.2.4 (`DESKTOP_PACKAGE`) | Gói tự chứa do tác giả dựng, F-Eco ghim mã băm | Tài khoản Mentor tạo ngay trên máy; **chưa** đăng nhập bằng F-Eco |

- F-Store có trang Mentor với nút "Mở ứng dụng web".
- F-Launcher trên Windows cài và mở app desktop `Mentor.exe`. Trên máy khác, và trên F-Launcher di động, Mentor là app web.

Thư mục này chỉ có bản vá, script và ghi chép, không có mã Mentor (NOTICE.md).

## Tệp

| Tệp | Nội dung |
|---|---|
| `MENTOR_COMMIT` | Commit của Mentor mà bản vá áp vào (bản web) |
| `DESKTOP_PACKAGE` | Gói desktop đã ghim: phiên bản, tag, commit, địa chỉ tải, SHA-256 |
| `fetch-desktop.sh` | Quét mã nguồn ở commit của tag, tải gói desktop, kiểm SHA-256 và cấu trúc gói |
| `build-desktop.sh` | Dựng app desktop 0.2.2 từ mã gốc và bản vá (CI dùng để kiểm bản vá; không còn đính vào bản phát hành) |
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
   - có `MENTOR_DESKTOP_ZIP` (gói `mentor-<phiên bản>-windows-x64.zip`, lấy bằng `fetch-desktop.sh`) thì ký thêm target `windows-x64`, với phiên bản `MENTOR_DESKTOP_VERSION` (mặc định bằng bản web). `MENTOR_SKIP_WEB=1` chỉ đăng bản desktop;
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

Từ F-Eco 1.21.0, app desktop là **gói tự chứa Mentor 0.2.4** do tác giả dựng bằng `integrations/feco/build.ps1` trong repo Mentor (tag `v0.2.4`, commit `c7ac6f2`):

- `Mentor.exe` (Tauri) tự bật `gateway\mentor-gateway.exe` (gateway đóng gói bằng PyInstaller) trên một cổng localhost trống. Không cần chạy gateway hay máy chủ Mentor riêng.
- Dữ liệu (tài khoản, tiến độ, nhật ký) nằm trong `FECO_APP_DATA_DIR` do F-Launcher đặt, giữ nguyên khi cập nhật.
- Lần đầu mở, người dùng tạo tài khoản Mentor ngay trên máy (học sinh hoặc giáo viên). Gói **chưa có** đăng nhập bằng F-Eco: hai bản vá trong thư mục này không áp được vào 0.2.4, vì tác giả đã viết lại giao diện (Làng gà) và gateway.
- Kết nối mạng: khóa học tải từ danh mục công khai của chủ dự án Mentor trên Google Drive (kiểm mã băm, giữ lại để học khi mất mạng); tính năng AI gọi nhà cung cấp AI bằng khóa do người dùng nhập.
- `Mentor.exe` và gateway **chưa ký Authenticode**. Kho dev nhận gói này; kiểm duyệt thật của F-Eco sẽ từ chối cho tới khi tác giả ký.

Job `mentor` trong `release.yml` chạy `fetch-desktop.sh`: quét mã nguồn ở commit của tag, tải gói từ bản phát hành của Mentor, so SHA-256 với `DESKTOP_PACKAGE`, kiểm cấu trúc gói, rồi đính gói nguyên vẹn vào bản phát hành F-Eco. Kết quả quét và kiểm tra: `SCAN.md`, mục "Mentor 0.2.4".

Đưa bản desktop vào kho dev:

```bash
integrations/mentor/fetch-desktop.sh ~/.feco-dev-ext/mentor-desktop
MENTOR_DESKTOP_ZIP=~/.feco-dev-ext/mentor-desktop/mentor-0.2.4-windows-x64.zip MENTOR_DESKTOP_VERSION=0.2.4 MENTOR_SKIP_WEB=1 scripts/dev-apps.sh mentor
```

Job `mentor-desktop` trong `ci.yml` vẫn dựng bản 0.2.2 từ mã gốc và bản vá, để kiểm bản vá của bản web còn áp được và test còn qua.

## Kiểm thử

```bash
uv run pytest services/gateway/tests/test_gw_feco_auth.py services/gateway/tests/test_gw_auth.py
pnpm -C apps/app exec vitest run src/feco-login.test.tsx src/app.test.tsx
```
