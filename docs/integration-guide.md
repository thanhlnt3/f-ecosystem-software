# Hướng dẫn tích hợp app con vào F-Eco

Tài liệu này dành cho nhóm phát triển muốn đưa một ứng dụng desktop (Windows, macOS) vào hệ sinh
thái F-Eco: người dùng tìm thấy app trên F-Store, cài và mở bằng F-Launcher, đăng nhập một lần
bằng tài khoản F-Eco, và app nhận đúng những dữ liệu người dùng đã đồng ý chia sẻ.

Áp dụng cho F-Eco 1.1. So với 1.0:
- manifest có thêm `source`, `terminal`, `website`, `features`, `release_notes`;
- có lệnh `f-dev pack`;
- F-Launcher hiện app theo lưới hoặc danh sách, và có trang chi tiết cho từng app.

Mã lỗi đầy đủ và địa chỉ máy chủ có ở trang `/developer/docs` của máy chủ F-Eco bạn dùng.

Muốn đưa một app mã nguồn mở có sẵn (ví dụ trên GitHub) vào F-Eco, xem thêm
[`github-apps.md`](github-apps.md).

## Mục lục

1. [App con chạy trong F-Eco như thế nào](#1-app-con-chạy-trong-f-eco-như-thế-nào)
2. [Chuẩn bị tài khoản nhà phát triển](#2-chuẩn-bị-tài-khoản-nhà-phát-triển)
3. [Tạo app và lấy client ID](#3-tạo-app-và-lấy-client-id)
4. [Viết manifest `f-app.toml`](#4-viết-manifest-f-apptoml)
5. [Đăng nhập trong app bằng F-SDK](#5-đăng-nhập-trong-app-bằng-f-sdk)
6. [Gọi API của F-Eco](#6-gọi-api-của-f-eco)
7. [Nhận sự kiện qua webhook](#7-nhận-sự-kiện-qua-webhook)
8. [Đóng gói, ký mã và nộp bằng `f-dev`](#8-đóng-gói-ký-mã-và-nộp-bằng-f-dev)
9. [Kiểm duyệt và phát hành](#9-kiểm-duyệt-và-phát-hành)
10. [Tích hợp không dùng F-SDK (OIDC chuẩn)](#10-tích-hợp-không-dùng-f-sdk-oidc-chuẩn)
11. [Thử trên máy của bạn](#11-thử-trên-máy-của-bạn)
12. [Danh sách kiểm tra trước khi nộp](#12-danh-sách-kiểm-tra-trước-khi-nộp)
13. [App hiển thị thế nào trong F-Launcher](#13-app-hiển-thị-thế-nào-trong-f-launcher)

## 1. App con chạy trong F-Eco như thế nào

```
Nhà phát triển          F-Eco (máy chủ)                    Máy người dùng
──────────────          ───────────────                    ──────────────
f-app.toml + gói ──f-dev──▶ Cổng nhà phát triển
                            │ kiểm tra tự động, kiểm duyệt
                            ▼
                         Phát hành ── ký target TUF ──▶ F-Launcher đồng bộ, kiểm chữ ký,
                         F-Store (danh mục)               cài vào thư mục người dùng
                                                          │ người dùng bấm "Mở"
                         /v1/launch-codes ◀── xin mã ─────┤ (launcher đã đăng nhập F-Eco)
                                                          ▼
                         /v1/launch-codes/exchange ◀── App con (F-SDK) đổi mã lấy token
                         /v1/me, /v1/entitlements  ◀── gọi API bằng access token
                         webhook (đã ký) ──────────▶ máy chủ của app (nếu có)
```

- **Cài đặt an toàn:** gói của app được ký thành target TUF khi phát hành; F-Launcher chỉ cài gói
  có chữ ký hợp lệ, đúng SHA-256, và chặn bản bị thu hồi.
- **Đăng nhập một lần:** người dùng đăng nhập F-Launcher một lần. Khi mở app, launcher xin một
  **mã khởi chạy** dùng một lần (sống 60 giây) cho đúng client ID của app và truyền qua biến môi
  trường. App đổi mã lấy access token và refresh token của riêng nó. Launcher không bao giờ đưa
  token của chính nó cho app.
- **Đồng ý theo từng scope:** lần đầu, người dùng thấy màn hình đồng ý liệt kê từng loại dữ liệu
  và mục đích bạn khai trong manifest. Scope tùy chọn có thể bị từ chối. Từ lần sau, mở app từ
  launcher không hỏi lại, trừ khi bạn thêm scope **bắt buộc** mới.
- **Định danh riêng cho từng app (pairwise):** `sub` app nhận được khác với mọi app khác. Hai app
  không thể ghép dữ liệu của cùng một người dùng qua `sub`.

## 2. Chuẩn bị tài khoản nhà phát triển

1. Tạo tài khoản F-Eco, **xác minh email** và **bật xác thực hai lớp** (ứng dụng TOTP) trong Cài
   đặt. Cổng nhà phát triển từ chối tài khoản thiếu một trong hai (`email_unverified`,
   `second_factor_required`).
2. Mở `/developer`, đăng ký **cá nhân** hoặc **tổ chức** và chấp nhận Thỏa thuận nhà phát triển.
   Tổ chức có thể xác minh tên miền bằng bản ghi DNS TXT `feco-domain-verification=<mã>` để app
   hiện dấu "Đã xác minh".
3. Mời thành viên vào nhóm nếu cần. Vai trò: Chủ sở hữu, Quản trị (phát hành được), Phát triển
   viên (nộp phiên bản, không phát hành).
4. Tạo **token CLI** (hiện một lần) cho `f-dev` và CI của bạn.

## 3. Tạo app và lấy client ID

Trên `/developer`, tạo app với:

| Trường | Ý nghĩa |
|---|---|
| Mã app (`app_id`) | Chữ thường, số, `.`, `_`, `-`; tối đa 63 ký tự. Trùng với `id` trong manifest. |
| Loại | **Desktop**: client công khai, dùng PKCE, không có client secret (hầu hết app). **Có máy chủ riêng**: có client secret, xoay vòng được (hiện một lần). |
| Địa chỉ chuyển hướng | Địa chỉ callback khi đăng nhập qua trình duyệt. F-SDK mặc định dùng `http://127.0.0.1:47280/callback`; khai đúng địa chỉ này nếu bạn không đổi. |

Trang app hiện **client ID**. Bạn cũng đặt địa chỉ webhook và nhận khóa ký webhook ở đây.

## 4. Viết manifest `f-app.toml`

Mỗi phiên bản nộp kèm một manifest. Ví dụ đầy đủ:

```toml
id = "quanlykho"
name = "Quản Lý Kho"
version = "1.2.0"                       # SemVer, phải lớn hơn bản đã phát hành
license = "MIT"
privacy_policy = "https://kho.example/privacy"
webhook = "https://kho.example/feco/webhook"   # không bắt buộc
summary = "Quản lý nhập, xuất, tồn kho"         # không bắt buộc
description = "Mô tả dài hiện trên trang chi tiết F-Store"
category = "productivity"               # mã nhóm, xem mục 13
source = "https://github.com/kho/quanlykho"    # không bắt buộc; có thì gói phải kèm LICENSE
terminal = false                        # true: launcher mở app trong cửa sổ terminal
website = "https://kho.example"         # không bắt buộc, phải là https://
features = [                            # không bắt buộc, tối đa 12 dòng, mỗi dòng ≤ 120 ký tự
  "Nhập, xuất kho bằng mã vạch",
  "Báo cáo tồn kho theo ngày",
]
release_notes = "Thêm báo cáo tồn kho"  # thay đổi của phiên bản này, ≤ 2.000 ký tự

[[platforms]]
os = "windows"          # windows | macos
arch = "x64"            # x64 | arm64
entry = "QuanLyKho.exe" # đường dẫn tệp chạy, tính từ gốc gói

[[platforms]]
os = "macos"
arch = "arm64"
entry = "QuanLyKho.app/Contents/MacOS/QuanLyKho"

[scopes.profile]
required = true
purpose = "Hiển thị tên người nhập kho"
purpose_en = "Shows who entered the stock"   # không bắt buộc

[scopes.purchases]
purpose = "Kiểm tra bản quyền"

[permissions]
network = true
camera = false
microphone = false
user_folders = ["documents"]   # documents, downloads, pictures, music, videos, desktop
```

**Scope dữ liệu** (mỗi scope phải có `purpose`, hiện nguyên văn trên màn hình đồng ý và F-Store):

| Scope | Trường app nhận trong `/v1/me` |
|---|---|
| `profile` | `name` (tên hiển thị, ảnh đại diện) |
| `email` | `email` |
| `locale` | `locale` (ngôn ngữ, múi giờ) |
| `phone` | `phone_number` |
| `billing` | `billing` (thông tin xuất hóa đơn) |
| `purchases` | quyền đọc `/v1/entitlements` của chính app |

`openid` và `offline_access` là scope giao thức, không cần khai. Chỉ xin scope bạn thật sự dùng:
người duyệt đối chiếu scope với chức năng của app (điều khoản "Thu thập dữ liệu không khai báo").

Kiểm tra manifest trước khi nộp:

```bash
f-dev validate --manifest f-app.toml
```

Mọi lỗi được liệt kê cùng lúc, mỗi lỗi chỉ rõ trường sai (ví dụ `scopes.email.purpose là bắt buộc`).

## 5. Đăng nhập trong app bằng F-SDK

F-SDK có cho Rust (`fsdk`), C (`fsdk-c`, ABI ổn định), Python (`fsdk`, gọi qua C ABI) và
TypeScript/Node.js (`@f-eco/sdk`, Node ≥ 22.6). Mọi bản đều làm cùng một việc:

1. Nếu app được F-Launcher mở (`FECO_LAUNCH_CODE` có trong môi trường): đổi mã khởi chạy lấy token.
2. Nếu không (chạy thẳng, hoặc người dùng chưa đồng ý scope): mở trình duyệt đăng nhập (OAuth 2.0
   PKCE), nhận callback ở `127.0.0.1`.
3. Tự làm mới access token (15 phút) bằng refresh token.

**Biến môi trường F-Launcher đặt khi mở app** (mọi biến `FECO_*` khác của launcher bị gỡ):

| Biến | Nội dung |
|---|---|
| `FECO_LAUNCH_CODE` | Mã khởi chạy dùng một lần, sống 60 giây |
| `FECO_CLIENT_ID` | Client ID của app |
| `FECO_SERVER_URL` | Máy chủ F-Eco (API `/v1/...`) |
| `FECO_ISSUER_URL` | Máy chủ OAuth 2.0 / OIDC |
| `FECO_APP_DATA_DIR` | Thư mục dữ liệu riêng của app (giữ lại khi cập nhật) |
| `FECO_LAUNCHER_URL`, `FECO_LAUNCHER_SECRET` | API cục bộ của launcher để xin mã khởi chạy mới |

Biến riêng của F-SDK (không bắt buộc): `FSDK_REDIRECT_URI`, `FSDK_SCOPES` (mặc định
`openid offline_access profile`; thêm scope bạn đã khai trong manifest).

**Rust**

```rust
let config = fsdk::Config::from_env()?.scopes("openid offline_access profile purchases");
let session = fsdk::session(&config).await?;      // launcher hoặc trình duyệt
let me = session.me().await?;                     // sub, name...
let licenses = session.entitlements().await?;     // cần scope purchases
```

Chạy ngoài launcher thì tạo `Config::new(client_id, issuer, server)` thay cho `from_env()`.
Lưu `session.refresh_token()` vào kho bảo mật của hệ điều hành để lần sau dùng
`Session::resume(&config, &refresh_token)`.

**C**

```c
#include "fsdk.h"
fsdk_session *s = getenv("FECO_LAUNCH_CODE")
    ? fsdk_session_from_launch_code(NULL, NULL, NULL, NULL, NULL, NULL)  /* NULL: đọc biến môi trường */
    : fsdk_login_browser(NULL, NULL, NULL, NULL, NULL);
if (!s) { fprintf(stderr, "%s\n", fsdk_last_error()); return 1; }
char *me = fsdk_me_json(s);          /* JSON, giải phóng bằng fsdk_string_free */
fsdk_string_free(me);
fsdk_session_free(s);
```

Kiểm tra `fsdk_abi_version() == FSDK_ABI_VERSION` khi khởi động. Xem `sdk/c/examples/login.c`.

**Python**

```python
import fsdk
with fsdk.Session.auto() as session:     # launcher hoặc trình duyệt
    me = session.me()
    licenses = session.entitlements()   # cần scope purchases
```

Đặt `FSDK_LIBRARY` trỏ tới thư viện `fsdk_c` đi kèm app. Xem `sdk/python/examples/login.py`.

**TypeScript / Node.js**

```ts
import { Session, configFromEnv } from "@f-eco/sdk";
const session = await Session.auto(configFromEnv());
const me = await session.me();
```

Xem `sdk/typescript/examples/login.ts`.

**Xin mã khởi chạy mới:** mã chỉ sống 60 giây. Nếu app cần đăng nhập lại khi đang chạy (ví dụ
người dùng đăng xuất trong app), gọi API cục bộ của launcher:

```
POST {FECO_LAUNCHER_URL}v1/launch-code
Authorization: Bearer {FECO_LAUNCHER_SECRET}
{"client_id": "<client ID của app>"}
→ {"code": "...", "expires_in": 60}
```

Launcher chỉ cấp mã cho đúng app nó đã mở (xin cho app khác bị 403 và ghi sự kiện bảo mật).

## 6. Gọi API của F-Eco

Gọi bằng `Authorization: Bearer <access token>` của app.

| Endpoint | Scope | Nội dung |
|---|---|---|
| `GET /v1/me` | `profile`, `email`, ... | Thông tin người dùng theo scope đã được cấp; `sub` là định danh pairwise của app. `?fields=name,email` để lấy bớt. |
| `GET /v1/entitlements` | `purchases` | Quyền sử dụng của người dùng với app: `owner`, `license`, `subscription` (kèm nguồn, ngày cấp, hạn). |
| `POST /v1/apps/crashes` | (bất kỳ) | Báo app gặp sự cố: `{"version": "1.2.0"}`. Chỉ đếm theo phiên bản, không gửi dữ liệu người dùng. F-SDK: `report_crash`. |
| `GET /v1/catalog/{app}/update` | (công khai) | Phiên bản thiết bị này được nhận (phát hành dần). |

**Lỗi** luôn là JSON:

```json
{"error": "insufficient_scope", "message": "...", "message_vi": "...", "request_id": "..."}
```

Các mã hay gặp: `invalid_token` (401, làm mới token), `insufficient_scope` (403, khai scope và
xin đồng ý), `consent_required` (403, F-SDK tự mở trình duyệt), `invalid_grant` (400, mã khởi
chạy sai hoặc quá 60 giây), `rate_limited` (429, chờ theo `Retry-After`). Giới hạn mặc định:
120 yêu cầu/phút cho mỗi người dùng, 1.200 yêu cầu/phút cho mỗi app và mỗi IP. Danh sách đầy đủ
ở `/developer/docs`. Ghi lại `request_id` khi liên hệ hỗ trợ.

## 7. Nhận sự kiện qua webhook

Đặt địa chỉ webhook (HTTPS) trên trang app ở Cổng nhà phát triển; lưu khóa ký hiện một lần.
F-Eco gửi `POST` JSON:

```json
{"event": "user.revoked", "client_id": "...", "sub": "<pairwise sub>", "occurred_at": "2026-11-13T08:00:00Z"}
```

| Sự kiện | Bạn cần làm |
|---|---|
| `user.revoked` | Người dùng thu hồi quyền của app: ngừng dùng token, xóa phiên. |
| `user.deleted` | Người dùng xóa tài khoản F-Eco: xóa dữ liệu cá nhân của họ trong hệ thống của bạn. |
| `entitlement.changed` | Quyền sử dụng thay đổi: đọc lại `/v1/entitlements`. |
| `app.version.revoked` | Một phiên bản của app bị gỡ khẩn cấp: kiểm tra thông báo kiểm duyệt. |

**Kiểm chữ ký** trước khi xử lý: header
`F-Eco-Signature: t=<giây unix>,v1=<hex>`, với `v1 = HMAC-SHA256(khóa ký, "<t>.<thân yêu cầu>")`.
Từ chối nếu `t` lệch quá 5 phút. F-SDK có sẵn hàm: `fsdk::verify_webhook` (Rust),
`fsdk.verify_webhook` (Python), `verifyWebhook` (TypeScript).

Trả 2xx khi đã nhận. Không nhận được 2xx, F-Eco gửi lại sau 1, 5, 30 phút rồi giãn dần, trong tối
đa 72 giờ. Xử lý idempotent: cùng một sự kiện có thể tới hơn một lần.

## 8. Đóng gói, ký mã và nộp bằng `f-dev`

**Gói** là thư mục (hoặc tệp `.zip`) chứa tệp chạy và mọi tài nguyên. `entry` trong manifest là
đường dẫn tệp chạy tính từ gốc gói. Nhiều nền tảng: thư mục con hoặc tệp zip tên `<os>-<arch>`:

```
dist/
├── windows-x64/QuanLyKho.exe, data/...
└── macos-arm64/QuanLyKho.app/...
```

Một nền tảng thì trỏ thẳng vào thư mục gói. Gói tối đa 2 GB. Không đặt đường dẫn kiểu `../` trong
zip: launcher từ chối gói như vậy.

**Ký mã là bắt buộc:** tệp chạy Windows ký Authenticode, app macOS ký Developer ID (nên notarize để Gatekeeper không cảnh báo).
Gói chưa ký bị từ chối (`invalid_package`).

**Nộp:**

```bash
export FDEV_TOKEN=<token CLI>          # hoặc --token
export FDEV_SERVER=https://<máy chủ F-Eco>   # hoặc --server
f-dev publish --manifest f-app.toml --package dist/
# → "Chờ duyệt", kèm mã phiên bản
```

`f-dev` kiểm manifest, nén từng nền tảng, tải lên kèm SHA-256 và gửi duyệt. Dùng được trong CI.

Muốn xem trước gói sẽ nộp mà chưa tải lên, dùng `f-dev pack --manifest f-app.toml --package build/
--out dist/`. Lệnh này tạo `dist/<os>-<arch>.zip` và chạy đúng các bước kiểm tra như `publish`.
Manifest có `source` thì mỗi gói phải chứa file giấy phép gốc (`LICENSE*`, `COPYING*` hoặc
`NOTICE*`), không thì bị từ chối.
Bạn cũng có thể tải gói và gửi duyệt trên trang app ở Cổng nhà phát triển.

## 9. Kiểm duyệt và phát hành

1. **Kiểm tra tự động** (vài phút): quét mã độc bằng hai engine, kiểm chữ ký mã, kiến trúc tệp
   chạy so với manifest, phát hiện packer.
2. **Duyệt thủ công** khi: phiên bản đầu tiên, thêm scope hoặc quyền mới, hoặc kiểm tra tự động
   cần người xem. Người duyệt thấy rõ khác biệt scope và quyền so với bản trước.
3. **Từ chối** luôn kèm điều khoản trong Chính sách nội dung (`/policy/content`) và cách sửa. Mỗi
   lần từ chối được kháng nghị một lần.
4. **Phát hành** (vai trò Quản trị hoặc Chủ sở hữu): chọn kênh `stable` hoặc `beta` và phần trăm
   phát hành dần; tăng dần tới 100% hoặc tạm dừng. Phát hành sẽ ký target TUF; F-Launcher của
   người dùng nhận bản mới ở lần đồng bộ kế tiếp (tối đa 1 giờ) và người đã cài nhận thông báo
   cập nhật (nếu họ bật).
5. Mọi phiên bản được quét lại định kỳ 7 ngày. Phát hiện vi phạm nghiêm trọng thì bản đó bị thu hồi
   khẩn cấp: gỡ khỏi F-Store, launcher chặn mở, bạn nhận email và webhook `app.version.revoked`.

Số liệu tổng hợp (lượt cài, người dùng hoạt động 7 và 30 ngày, lỗi khi chạy, đánh giá) ở trang
app; F-Eco không bao giờ cho nhà phát triển biết danh tính người dùng.

## 10. Tích hợp không dùng F-SDK (OIDC chuẩn)

F-Eco là nhà cung cấp OpenID Connect chuẩn; dùng được bất kỳ thư viện OIDC nào:

- Discovery: `{FECO_ISSUER_URL}.well-known/openid-configuration`
- Luồng: Authorization Code + PKCE (app desktop), hoặc kèm client secret (app có máy chủ).
- Scope như mục 4; `sub` trong ID token là pairwise.
- Khi được launcher mở, đổi mã khởi chạy bằng
  `POST {FECO_SERVER_URL}v1/launch-codes/exchange` với JSON `{"code", "client_id", "redirect_uri",
  "code_challenge", "scope"}` (`code_challenge` là S256 của PKCE verifier do app tạo) để nhận
  authorization code, rồi đổi code lấy token ở token endpoint kèm verifier như bình thường.
  (F-SDK làm sẵn các bước này.)

Ví dụ chạy được với Authlib (Python): `examples/standard-oidc/authlib_login.py`.

## 11. Thử trên máy của bạn

Kho mã có sẵn hạ tầng dev (Kratos, Hydra, PostgreSQL, Mailpit) và script dựng launcher dev; xem
README phần "Hướng dẫn chạy hệ thống":

1. Chạy hạ tầng và `feco-server` (README bước 2–3).
2. Tạo tài khoản nhà phát triển và app trên `http://127.0.0.1:8080/developer`.
3. `scripts/dev-launcher.sh setup` tạo kho TUF dev và client của launcher; `scripts/dev-launcher.sh
   publish` ký gói của bạn vào kho dev để launcher dev cài được (README bước 6).
4. Mở app từ F-Launcher dev: app nhận các biến ở mục 5. Hoặc chạy thẳng app với
   `FECO_CLIENT_ID`, `FECO_ISSUER_URL=http://127.0.0.1:4444/`,
   `FECO_SERVER_URL=http://127.0.0.1:8080/` để thử đăng nhập qua trình duyệt.
5. App mẫu:
   - `examples/apps/f-notes` (Ghi chú) và `examples/apps/f-timer` (Bấm giờ): app cửa sổ egui
     dùng F-SDK, tự đăng nhập khi được launcher mở, chạy chế độ khách khi mở trực tiếp;
   - `tools/feco-demo-app` (Rust) và `sdk/*/examples`.

   `scripts/dev-apps.sh samples` ký hai app mẫu vào kho dev. `scripts/dev-apps.sh github ...`
   dựng và ký một app mã nguồn mở từ GitHub (xem [`github-apps.md`](github-apps.md)).

## 12. Danh sách kiểm tra trước khi nộp

- [ ] Email đã xác minh, bật xác thực hai lớp, đã chấp nhận Thỏa thuận nhà phát triển.
- [ ] `id` trong manifest trùng mã app; `version` lớn hơn bản đã phát hành.
- [ ] Mỗi scope có `purpose` rõ ràng; chỉ xin scope thật sự dùng; scope không thiết yếu để
      `required = false`.
- [ ] `privacy_policy` là trang HTTPS truy cập được.
- [ ] Tệp chạy đã ký Authenticode / Developer ID; `entry` đúng đường dẫn trong gói.
- [ ] App dùng F-SDK (hoặc OIDC chuẩn) để đăng nhập; không tự hỏi mật khẩu F-Eco.
- [ ] Lưu refresh token trong kho bảo mật của hệ điều hành, không ghi ra tệp thường.
- [ ] Dữ liệu riêng của app nằm trong `FECO_APP_DATA_DIR`.
- [ ] Webhook kiểm chữ ký, xử lý idempotent; xóa dữ liệu khi nhận `user.deleted`.
- [ ] Có `summary` ngắn gọn và `category` là một mã ở mục 13, để app hiện đúng nhóm trong launcher.
- [ ] App chạy trong terminal có `terminal = true`. App dựng từ mã nguồn bên ngoài có `source`, và
      gói kèm giấy phép gốc.

## 13. App hiển thị thế nào trong F-Launcher

F-Launcher hiện app ở Thư viện (app đã cài) và Cửa hàng, theo **lưới** hoặc **danh sách**. Người
dùng tìm (không phân biệt dấu), lọc theo nhóm, và sắp xếp theo tên, lần mở gần nhất, ngày cài,
nhóm hoặc dung lượng.

Bấm vào một app (ô hoặc hàng) để mở **trang chi tiết**. Trang có:
- biểu tượng, tên, nhà phát triển, nhóm, mô tả ngắn;
- giới thiệu (`description`) và tính năng (`features`);
- "Có gì mới trong bản …" (`release_notes`) kèm ngày phát hành;
- bảng thông tin: phiên bản mới nhất, phiên bản đã cài, dung lượng, các nền tảng có gói, giấy phép, mã nguồn, website;
- nút Cài, Cập nhật, Mở, Gỡ, Quay lại bản trước;
- nút và phím mũi tên để chuyển sang app trước hoặc sau.

Mục nào không có dữ liệu thì không hiện. `release_notes` cũng là lịch sử thay đổi của phiên bản
trên F-Store.

Mọi thông tin hiển thị đều lấy từ metadata TUF mà máy chủ ký khi phát hành:

| Hiển thị | Lấy từ |
|---|---|
| Tên | `name` |
| Mô tả ngắn | `summary` |
| Nhóm | `category` |
| Nhà phát triển | tên trong danh mục F-Store |
| Nút "Mã nguồn" | `source` |
| "Mở (terminal)" | `terminal = true` |
| Giới thiệu, tính năng | `description`, `features` |
| Website | `website` |
| Có gì mới, ngày phát hành | `release_notes`, thời điểm máy chủ ký phiên bản |
| Giấy phép | `license` |
| Chạy trên | các nền tảng `[[platforms]]` có gói đã ký |

Biểu tượng là ô chữ cái đầu của tên, màu cố định theo mã app. Hiện chưa có icon do nhà phát
triển tải lên.

Mã nhóm nên dùng (giá trị khác được hiện nguyên văn):

| Mã | Tên hiển thị | Mã | Tên hiển thị |
|---|---|---|---|
| `productivity` | Năng suất | `games` | Trò chơi |
| `tools` | Công cụ | `graphics` | Đồ họa |
| `system` | Hệ thống | `music` | Âm nhạc |
| `development` | Lập trình | `video` | Video |
| `education` | Giáo dục | `business` | Kinh doanh |

App không khai `[scopes]` được coi là không đăng nhập bằng F-Eco: target không có `client_id`,
launcher mở app mà không xin mã khởi chạy. Người dùng chỉnh giao diện launcher (chủ đề, màu nhấn,
cỡ chữ, bố cục, cỡ ô lưới) ở trang Cài đặt. Các cài đặt này chỉ lưu trên máy họ.
- [ ] Xử lý `rate_limited` theo `Retry-After`, `invalid_token` bằng làm mới token.
- [ ] `f-dev validate` không báo lỗi.
