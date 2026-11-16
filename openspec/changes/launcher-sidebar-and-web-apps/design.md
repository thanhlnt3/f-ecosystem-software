# Design

## H1. Thanh bên hai chế độ

| | Đầy đủ (mặc định) | Thu gọn |
|---|---|---|
| Độ rộng | 210 px | 56 px |
| Mục điều hướng | icon và chữ | chỉ icon, tooltip ghi tên |
| Thông báo chưa đọc, số bản cập nhật | trong chữ, ví dụ "Thông báo (3)" | huy hiệu số ở góc icon |
| Hàng nút dưới cùng (đồng bộ, web, quản trị, nhà phát triển, đăng xuất) | một hàng ngang | một cột dọc |
| Ví F-Coin | icon và số dư | icon, tooltip ghi số dư |
| Tài khoản | vai trò và tên | ô chữ cái đầu, tooltip ghi tên và vai trò |

- **Nút ẩn/hiện** nằm ngay dưới logo: icon `ChevronLeft` khi đầy đủ, icon `Menu` (ba gạch) khi thu gọn. Phím tắt Ctrl+B (Cmd+B trên macOS).
- **Nhớ lựa chọn**: trường `sidebar_collapsed` trong `settings.json`, đọc từng trường như các trường khác; tệp cũ không có trường này thì là `false`.
- **Cửa sổ hẹp**: chiều rộng cửa sổ dưới 720 px thì thanh bên hiện thu gọn, không đổi lựa chọn đã lưu. Cửa sổ rộng lại thì trở về lựa chọn đã lưu.
- **Hàm thuần**: `sidebar_width(collapsed, window_width)` trả độ rộng; `badge_text(n)` trả "1"…"9", "9+" (0 thì không có huy hiệu). Test được không cần cửa sổ.

## H2. App web

**Phân phối.** Có hai cách đưa app web vào launcher:
- Danh mục máy chủ liệt kê thẳng địa chỉ web: launcher phải tin thêm một nguồn ngoài TUF.
- **Target TUF nền tảng `web`**: chọn cách này. Địa chỉ app web nằm trong metadata đã ký, như logo (G1 của launcher-logos-and-updates). Gói zip rất nhỏ, chỉ chứa `webapp.json` (tên, địa chỉ) và `NOTICE.txt`. Nhờ vậy dùng lại toàn bộ luồng thêm, cập nhật, gỡ, thu hồi, gợi ý cập nhật, mua bằng F-Coin, và báo lượt cài về máy chủ.

**Kiểm địa chỉ** (`feco_tuf::valid_web_url`, dùng ở cả `feco-tuf` và launcher):
- `https://` với tên máy bất kỳ;
- `http://` chỉ với `localhost` hoặc `127.0.0.1` (môi trường dev);
- không có thông tin đăng nhập trong địa chỉ (`user:pass@`).

Launcher bỏ qua `web_url` không hợp lệ: app đó không mở được và báo lỗi.

**Launcher**:
- `catalog::build` nhận target cùng nền tảng của máy hoặc nền tảng `web`. App có cả gói desktop và bản web thì dùng gói desktop.
- Nhãn "App web" ở ô và cửa sổ chi tiết; "Chạy trên: Trình duyệt web".
- Nút "Thêm" thay "Cài" (vẫn là tải và kiểm gói), "Mở" gọi `open::that(web_url)`.
- Không cấp mã khởi chạy (`FECO_LAUNCH_CODE` chỉ dùng cho tiến trình con). App web tự đăng nhập bằng OIDC trong trình duyệt.

**Đăng nhập.** App web là client OAuth phía máy chủ: `token_endpoint_auth_method = client_secret_basic`, `subject_type = pairwise`, luồng authorization code + PKCE, chuyển hướng về backend của app. Người dùng đã đăng nhập F-Eco trong trình duyệt (launcher đăng nhập qua trình duyệt mặc định) thì Hydra không hỏi lại mật khẩu, chỉ hỏi đồng ý chia sẻ dữ liệu lần đầu.

**F-Store** (máy chủ):
- `catalog_apps.web_url` (migration `0017_web_apps.sql`, cột mới; không sửa migration cũ).
- `Listing::web_url`. Trang app: nút "Mở ứng dụng web" (`rel="noopener"`), dòng nền tảng ghi "Web".
- Bộ lọc nền tảng `os=windows` (hoặc `macos`, `linux`) vẫn hiện app có phiên bản nền tảng `web`.
- Trang "Ứng dụng của tôi": app web có nút "Mở" tới `web_url`.

## H3. ThesisAI

**Kiến trúc của ThesisAI** (đọc từ hai repo):
- Frontend Next.js 16, cổng 3000. Token lưu ở `localStorage` (`thesisai_token`, `thesisai_user`). Gọi backend ở `NEXT_PUBLIC_API_URL` (mặc định `http://localhost:8000/api`).
- Backend FastAPI, cổng 8000. Tài khoản riêng (bảng `users`: email, `password_hash`, vai trò `student | lecturer | admin`), JWT HS256 sống 7 ngày.
- PostgreSQL có pgvector; Ollama với `qwen2.5:7b`, `bge-m3`; faster-whisper.

**Luồng "Đăng nhập bằng F-Eco"**:

1. Launcher mở `http://localhost:3000/auth/feco` (trang mới của frontend).
2. Trang đó chuyển sang `GET {API}/auth/feco/login` (backend).
3. Backend dùng Authlib (`authlib.integrations.starlette_client`) chuyển tới Hydra của F-Eco: authorization code + PKCE, `state` và `nonce` lưu trong cookie phiên ký bằng `SECRET_KEY` (`SessionMiddleware`).
4. Hydra chuyển về `GET {API}/auth/feco/callback`. Authlib đổi mã lấy token bằng client secret và kiểm chữ ký ID token qua JWKS. Backend gọi `GET {FECO_SERVER_URL}v1/me` bằng access token để lấy tên, email, `email_verified` (ID token của F-Eco chỉ có `sub`).
5. Backend tìm người dùng theo `feco_subject` (cột mới, `sub` pairwise của F-Eco). Chưa có thì:
   - nếu `email_verified` và đã có tài khoản cùng email: liên kết tài khoản đó;
   - nếu không: tạo tài khoản mới, vai trò `student`, mật khẩu ngẫu nhiên không dùng được (chỉ đăng nhập qua F-Eco).
6. Backend cấp JWT của ThesisAI như đăng nhập thường, rồi chuyển về `http://localhost:3000/auth/feco#token=...`. Token đặt trong fragment nên không đi vào nhật ký máy chủ hay header Referer.
7. Trang `/auth/feco` đọc fragment, xóa nó khỏi thanh địa chỉ, gọi `/auth/me`, lưu như đăng nhập thường và vào `/dashboard`.

Trang đăng nhập của ThesisAI có thêm nút "Đăng nhập bằng F-Eco". Đăng nhập bằng email và mật khẩu vẫn giữ.

**Cấu hình mới của backend ThesisAI**: `FECO_ISSUER_URL`, `FECO_SERVER_URL`, `FECO_CLIENT_ID`, `FECO_CLIENT_SECRET`, `FECO_REDIRECT_URI`, `FRONTEND_URL`. Không có `FECO_CLIENT_ID` thì không bật đăng nhập F-Eco.

**Trong repo F-Eco** (`integrations/thesisai/`):
- `README.md`: cách chạy ThesisAI cùng F-Eco dev, biến môi trường, luồng đăng nhập.
- `thesisai-be.patch`, `thesisai-fe.patch`: bản vá tạo bằng `git diff` trên commit đã ghi trong README.
- `NOTICE.md`: nguồn, commit, tác giả, việc tác giả đồng ý (theo xác nhận của chủ dự án).

Mã của ThesisAI không được chép vào repo F-Eco, chỉ có bản vá. Bản dựng và mã tải về nằm ngoài repo (`$FECO_EXT_DIR`).

**Môi trường dev**:
- `scripts/dev-apps.sh thesisai`: tạo client OAuth (lưu client ID và secret trong `.dev/`, không commit), gói app web với `web_url = http://localhost:3000/auth/feco`, logo, danh mục, rồi in các biến cần đặt cho backend ThesisAI.
- Chạy được ThesisAI cần PostgreSQL có pgvector, Ollama và mô hình (khoảng 6 GB), thư viện Python và npm. Việc tải những thứ đó cần chủ dự án cho phép riêng; F-Eco không tải.

## H4. Kiểm thử

- Thanh bên: unit test `sidebar_width`, `badge_text`, prefs cũ không có trường mới; chạy thử hai chế độ trên Windows.
- `feco-tuf`: test `web_url` được ký cùng target; địa chỉ `http://` lạ, `javascript:`, có `user:pass@` bị từ chối.
- Launcher: unit test catalog nhận target `web` trên mọi nền tảng và ưu tiên gói desktop; `prepare_launch` của app web trả địa chỉ, không cần tệp chạy.
- Máy chủ: E2E app web trên F-Store (trang app, bộ lọc nền tảng, trang "Ứng dụng của tôi") và luồng OIDC của client phía máy chủ kiểu ThesisAI (secret, PKCE, email qua `/v1/me`, sai secret bị từ chối, chuyển hướng lạ bị chặn).
- Bản vá ThesisAI: `python -m py_compile` các tệp Python; chạy thật cần thư viện của ThesisAI (QUESTIONS.md).
