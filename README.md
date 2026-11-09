# F-EcoSystem-Software (F-Eco)

Nền tảng mở để nhà phát triển bên thứ ba phát hành **app con** qua một launcher chung, với tài khoản người dùng dùng chung, công cụ tự xây app (**F-Builder**) và dịch vụ **xây app theo yêu cầu** có thanh toán. Ý tưởng lấy cảm hứng từ mô hình launcher + bộ ứng dụng của ArtCraft, nhưng mở cho bên thứ ba.

> **Trạng thái: đang xây dựng.** Đã chạy được (nhóm task 1–5):
> - Tài khoản: đăng ký, xác minh email, đăng nhập, chống dò mật khẩu, xác thực hai lớp (TOTP, mã khôi phục), thiết bị đang đăng nhập, khôi phục mật khẩu, xóa tài khoản sau 30 ngày chờ.
> - Đăng nhập một lần cho app con qua OAuth 2.0/OIDC, màn hình đồng ý theo scope, API `/v1/me`, trang ứng dụng đã kết nối, thu hồi quyền, webhook, xuất dữ liệu cá nhân.
> - Giao diện tiếng Việt và tiếng Anh.
> - Bảo mật: HSTS và header bảo vệ, mã hóa dữ liệu nhạy cảm khi lưu (AES-256-GCM), giới hạn tần suất, nhật ký kiểm toán chỉ ghi thêm, cảnh báo đăng nhập từ thiết bị mới. Quy trình xử lý sự cố: [`docs/security/incident-response.md`](docs/security/incident-response.md).
> - Chuỗi tin cậy cho gói cài đặt theo TUF (dùng `tough`): ký phiên bản đã duyệt, thu hồi tới launcher trong 1 giờ, chặn gói hoặc metadata bị sửa, rollback, đóng băng; xoay khóa targets và root (ngưỡng 2/3). Công cụ `feco-tuf`, quy trình giữ khóa: [`docs/security/tuf-keys.md`](docs/security/tuf-keys.md).
>
> Chưa có: launcher, danh mục app, cổng nhà phát triển, F-Builder, đơn hàng, thanh toán. Chưa dùng được cho người dùng cuối.

## Mục lục

- [Hướng dẫn chạy hệ thống](#hướng-dẫn-chạy-hệ-thống)
  - [1. Chuẩn bị](#1-chuẩn-bị)
  - [2. Chạy hạ tầng](#2-chạy-hạ-tầng)
  - [3. Chạy feco-server](#3-chạy-feco-server)
  - [4. Dùng thử tài khoản](#4-dùng-thử-tài-khoản)
  - [5. Dùng thử đăng nhập một lần với app mẫu](#5-dùng-thử-đăng-nhập-một-lần-với-app-mẫu)
  - [6. Dừng và xóa dữ liệu](#6-dừng-và-xóa-dữ-liệu)
- [Địa chỉ các dịch vụ](#địa-chỉ-các-dịch-vụ)
- [Cấu hình](#cấu-hình)
- [Kiểm thử](#kiểm-thử)
- [Xử lý sự cố](#xử-lý-sự-cố)
- [Yêu cầu và thiết kế](#yêu-cầu-và-thiết-kế)
- [Cấu trúc thư mục](#cấu-trúc-thư-mục)
- [Giấy phép](#giấy-phép)

## Hướng dẫn chạy hệ thống

Hệ thống gồm `feco-server` (Rust, cổng 8080) và bốn dịch vụ hạ tầng: PostgreSQL (dữ liệu của F-Eco), Ory Kratos (tài khoản, mật khẩu, phiên đăng nhập), Ory Hydra (OAuth 2.0/OIDC cho app con) và Mailpit (hộp thư giả để xem email khi phát triển).

```text
 Trình duyệt ──► feco-server :8080 ──► Kratos :4433/:4434 ──► Mailpit :1025/:8025
     │                │
     │                ├──► Hydra :4445 (quản trị)
     │                └──► PostgreSQL :5432
     └──► Hydra :4444 (OAuth cho app con) ──► app con (ví dụ feco-demo-app :9999)
```

### 1. Chuẩn bị

Cần có:

| Công cụ | Phiên bản | Ghi chú |
|---|---|---|
| [Rust](https://rustup.rs) | 1.85 trở lên | `rustc --version` để kiểm tra |
| Git | bất kỳ | Trên Windows nên dùng Git Bash cho các lệnh bash bên dưới |
| **Cách A:** [Docker](https://docs.docker.com/get-docker/) | Docker Compose v2 | Đơn giản nhất: một lệnh chạy cả hạ tầng |
| **Cách B:** chương trình chạy trực tiếp | PostgreSQL 17, [Ory Kratos v1.3.1](https://github.com/ory/kratos/releases/tag/v1.3.1), [Ory Hydra v2.3.0](https://github.com/ory/hydra/releases/tag/v2.3.0), [Mailpit](https://github.com/axllent/mailpit/releases), Python 3 | Khi không có Docker. Trên Windows tải bản `windows_sqlite_64bit` của Kratos và Hydra |

Lấy mã nguồn:

```bash
git clone https://github.com/thanhlnt3/f-ecosystem-software.git
cd f-ecosystem-software
```

### 2. Chạy hạ tầng

**Cách A: Docker**

```bash
docker compose -f infra/docker-compose.yml up -d --wait
```

Lệnh chờ tới khi PostgreSQL, Valkey, Mailpit, Kratos và Hydra đều sẵn sàng. Kiểm tra:

```bash
curl http://127.0.0.1:4433/health/ready   # Kratos: {"status":"ok"}
curl http://127.0.0.1:4444/health/ready   # Hydra:  {"status":"ok"}
```

**Cách B: chạy trực tiếp, không cần Docker**

Mở bốn cửa sổ terminal (Git Bash trên Windows) ở thư mục dự án, mỗi cửa sổ chạy một dịch vụ. Nếu các chương trình chưa nằm trong `PATH`, chỉ đường dẫn qua biến môi trường như ví dụ:

```bash
PG_BIN=/duong/dan/pgsql/bin scripts/dev-local.sh postgres     # lần đầu tự tạo CSDL feco
MAILPIT_BIN=/duong/dan/mailpit scripts/dev-local.sh mailpit
KRATOS_BIN=/duong/dan/kratos scripts/dev-local.sh kratos
HYDRA_BIN=/duong/dan/hydra scripts/dev-local.sh hydra
```

`scripts/dev-local.sh` dùng đúng cấu hình trong `infra/`, nên hành vi giống hệt cách A. Dữ liệu PostgreSQL nằm trong `infra/.data/pg` (không được commit). Kratos và Hydra ở chế độ dev lưu dữ liệu trong bộ nhớ, nên khởi động lại là mất hết tài khoản.

### 3. Chạy feco-server

```bash
FECO_ENV=dev cargo run -p feco-server
```

Trên PowerShell:

```powershell
$env:FECO_ENV = "dev"; cargo run -p feco-server
```

Lần đầu biên dịch mất vài phút. Khi thấy dòng `feco-server đang chạy`, kiểm tra:

```bash
curl http://127.0.0.1:8080/healthz
# {"status":"ok","environment":"dev","version":"0.1.0"}
```

Ở môi trường `dev`, mọi địa chỉ đã có giá trị mặc định trỏ tới hạ tầng ở bước 2, và lược đồ cơ sở dữ liệu được tạo tự động khi khởi động. Xem thêm phần [Cấu hình](#cấu-hình).

### 4. Dùng thử tài khoản

Mở http://127.0.0.1:8080 trong trình duyệt. Giao diện theo ngôn ngữ của trình duyệt; đổi bằng liên kết ở góc phải trên.

1. **Đăng ký** tại http://127.0.0.1:8080/registration. Mật khẩu phải dài từ 10 ký tự và không nằm trong các vụ rò rỉ dữ liệu, nên ví dụ `Password123!` sẽ bị từ chối.
2. **Xác minh email**: mở Mailpit tại http://127.0.0.1:8025. Mọi email của hệ thống dev đều được giữ ở đây, không gửi ra ngoài. Mở email "Xác minh email tài khoản F-Eco" rồi bấm liên kết.
3. **Đăng nhập** tại http://127.0.0.1:8080/login. Trang chủ cho biết email đã xác minh hay chưa.
4. **Cài đặt** tại http://127.0.0.1:8080/settings:
   - sửa hồ sơ (tên, ảnh, số điện thoại, thông tin xuất hóa đơn), đổi mật khẩu;
   - bật **ứng dụng xác thực (TOTP)**: quét mã QR bằng Google Authenticator, Microsoft Authenticator hoặc ứng dụng tương tự, rồi nhập mã 6 số;
   - tạo **mã khôi phục** và lưu lại. Từ lần đăng nhập sau, bạn phải nhập mã TOTP hoặc mã khôi phục.
5. **Thiết bị đang đăng nhập** tại http://127.0.0.1:8080/sessions: đăng nhập thêm bằng một trình duyệt khác để thấy hai thiết bị, rồi thử "Đăng xuất thiết bị này".
6. **Quên mật khẩu** tại http://127.0.0.1:8080/recovery: mã khôi phục 6 số được gửi tới Mailpit.
7. **Cảnh báo thiết bị mới**: đăng nhập bằng một trình duyệt khác (khác trình duyệt hoặc hệ điều hành) để nhận email "Đăng nhập từ thiết bị mới" trong Mailpit. Liên kết "Không phải tôi" trong email đăng xuất mọi thiết bị.
8. **Chống dò mật khẩu**: đăng nhập sai 5 lần liên tiếp. Mỗi lần sai bị làm chậm dần, và từ lần thứ 5 xuất hiện ô "Tôi không phải robot" (thử thách thử nghiệm, chỉ có ở dev). Chủ tài khoản nhận email cảnh báo trong Mailpit.
9. **Xuất dữ liệu** tại http://127.0.0.1:8080/account/export: liên kết tải file ZIP được gửi tới Mailpit.
10. **Xóa tài khoản** tại http://127.0.0.1:8080/account/delete: tài khoản chờ xóa 30 ngày; đăng nhập lại trong thời gian đó để hủy.

### 5. Dùng thử đăng nhập một lần với app mẫu

`tools/feco-demo-app` là một app con mẫu. Nó tự đăng ký với Hydra (khai báo scope `profile` bắt buộc; `email`, `locale`, `phone` không bắt buộc), mở luồng OAuth 2.0 có PKCE, nhận kết quả ở `http://127.0.0.1:9999/cb`, rồi gọi `GET /v1/me`.

```bash
cargo run -p feco-demo-app
```

1. App in ra `client_id` và một địa chỉ bắt đầu bằng `http://127.0.0.1:4444/oauth2/auth?...`. Mở địa chỉ đó trong trình duyệt.
2. Nếu chưa đăng nhập F-Eco, bạn được đưa tới trang đăng nhập, rồi tự quay lại luồng.
3. **Màn hình đồng ý** hiện tên app, nhà phát triển, từng loại dữ liệu kèm mục đích. "Tên hiển thị và ảnh đại diện" là bắt buộc; email là không bắt buộc và mặc định không chọn. Chọn tùy ý rồi bấm **Cho phép** (hoặc **Từ chối**).
4. Quay lại terminal: app in ra scope được cấp và kết quả `/v1/me`, ví dụ:

   ```json
   {
     "email": "ban@example.com",
     "email_verified": true,
     "name": "Lan",
     "picture": null,
     "sub": "PYUOrskLAhLL-pdLQKJqyUHw8B719Zbi7--ZuwdfaSw"
   }
   ```

   `sub` là định danh riêng của app này: mỗi app nhận một giá trị khác nhau cho cùng một người. Nếu không cho phép email, kết quả không có `email`.
5. Mở http://127.0.0.1:8080/apps để thấy "Demo App" trong **Ứng dụng đã kết nối**, kèm dữ liệu đã cấp và lần truy cập gần nhất. Bấm **Thu hồi quyền**: token của app mất hiệu lực ngay, và lần sau app phải hỏi lại.

Chạy lại với cùng app (lần này không hỏi lại các scope đã cấp):

```bash
cargo run -p feco-demo-app -- --client-id <client_id đã in ra lần trước>
cargo run -p feco-demo-app -- --client-id <client_id> --scope "openid profile phone"   # xin thêm scope
```

### 6. Dừng và xóa dữ liệu

- Dừng `feco-server` và app mẫu bằng `Ctrl+C`.
- Cách A: `docker compose -f infra/docker-compose.yml down`. Lệnh này dừng và xóa mọi dữ liệu dev, vì compose không dùng volume.
- Cách B: `Ctrl+C` từng cửa sổ. Muốn xóa dữ liệu PostgreSQL thì xóa thư mục `infra/.data/pg`.

## Địa chỉ các dịch vụ

| Dịch vụ | Địa chỉ | Dùng để |
|---|---|---|
| feco-server | http://127.0.0.1:8080 | Trang tài khoản, màn hình đồng ý, API `/v1/me` |
| Mailpit | http://127.0.0.1:8025 (SMTP 1025) | Xem email của hệ thống dev |
| Kratos public / admin | http://127.0.0.1:4433 / :4434 | Tài khoản (feco-server gọi; không mở trực tiếp) |
| Hydra public / admin | http://127.0.0.1:4444 / :4445 | OAuth 2.0/OIDC cho app con; `/.well-known/openid-configuration` |
| PostgreSQL | 127.0.0.1:5432, người dùng `feco`, CSDL `feco` | Đồng ý, nhật ký truy cập, webhook, xuất dữ liệu, nhật ký kiểm toán |
| Valkey | 127.0.0.1:6379 | Chưa dùng (dành cho bộ đếm dùng chung khi chạy nhiều bản) |

Các API chính của `feco-server`:

| Đường dẫn | Mô tả |
|---|---|
| `GET /healthz` | Kiểm tra sức khỏe |
| `/registration`, `/login`, `/verification`, `/recovery`, `/settings`, `/logout` | Trang tài khoản (qua Kratos) |
| `/sessions`, `/apps`, `/account/export`, `/account/delete`, `/lang/{vi\|en}` | Trang do feco-server xử lý |
| `/security/not-me/{token}` | Nút "Không phải tôi" trong email cảnh báo thiết bị mới |
| `/oauth/login`, `/oauth/consent`, `/oauth/logout` | Login & consent provider của Hydra |
| `GET /v1/me` | Dữ liệu người dùng theo scope của access token (`Authorization: Bearer …`); `?fields=email,phone_number` để chọn trường. Lỗi trả JSON `{error, message, message_vi, request_id}` |

## Cấu hình

`feco-server` đọc cấu hình từ biến môi trường và **từ chối khởi động** (mã thoát 78) nếu cấu hình sai. Ở `dev` và `staging`, các biến dưới đây có giá trị mặc định trỏ tới hạ tầng ở bước 2; ở `production`, những biến đánh dấu "bắt buộc" phải được khai báo.

| Biến | Mặc định (dev) | Ghi chú |
|---|---|---|
| `FECO_ENV` | (bắt buộc) | `dev`, `staging` hoặc `production` |
| `FECO_HTTP_ADDR` | `127.0.0.1:8080` | |
| `FECO_PUBLIC_URL` | `http://127.0.0.1:8080/` | Bắt buộc ở production, phải là `https` |
| `FECO_DATABASE_URL` | `postgres://feco:feco-dev-only@127.0.0.1:5432/feco` | Bắt buộc ở production |
| `FECO_KRATOS_PUBLIC_URL` / `FECO_KRATOS_ADMIN_URL` | `http://127.0.0.1:4433/` / `:4434/` | Bắt buộc ở production |
| `FECO_HYDRA_PUBLIC_URL` / `FECO_HYDRA_ADMIN_URL` | `http://127.0.0.1:4444/` / `:4445/` | Bắt buộc ở production |
| `FECO_PAIRWISE_SECRET` | khóa dev cố định | Bắt buộc ở production, tối thiểu 32 ký tự; đổi khóa thì `sub` của mọi app thay đổi |
| `FECO_SMTP_URL` | `smtp://127.0.0.1:1025` | Bắt buộc ở production |
| `FECO_EMAIL_FROM` | `F-Eco <no-reply@f-eco.example>` | |
| `FECO_CHALLENGE_PROVIDER` | `test` | Production cấm `test`; nhà cung cấp thật chưa được chọn (xem `QUESTIONS.md`) |
| `FECO_DATA_KEY` | khóa dev cố định | Khóa AES-256 (32 byte, base64) mã hóa khóa webhook và file xuất dữ liệu khi lưu. Bắt buộc ở production. Tạo: `openssl rand -base64 32` |
| `FECO_RATE_IP_PER_MIN` | `1200` | Số yêu cầu mỗi phút từ một IP; vượt thì trả 429 kèm `Retry-After` |
| `FECO_RATE_APP_PER_MIN` / `FECO_RATE_USER_PER_MIN` | `1200` / `120` | Số lần gọi `/v1/me` mỗi phút của một app / một người dùng |
| `FECO_VNPAY_*`, `FECO_MOMO_*` | không đặt | Dev/staging chỉ được trỏ tới sandbox của cổng thanh toán |
| `RUST_LOG` | `info` | Mức nhật ký, ví dụ `debug` |

Mọi bí mật (mật khẩu CSDL, khóa) được in ra nhật ký dưới dạng `***`. Không commit bí mật thật vào kho mã; CI có bước quét bí mật.

## Kiểm thử

```bash
cargo test --workspace                          # test đơn vị (không cần hạ tầng)
cargo clippy --workspace --all-targets -- -D warnings
cargo deny check                                # license, lỗ hổng, nguồn crate
cargo run -p feco-license-check                 # license dịch vụ và ảnh container
npx -y @fission-ai/openspec@1.16.1 validate add-f-ecosystem-platform --strict
```

**Kiểm thử đầu-cuối** chạy với Kratos, Hydra, PostgreSQL và Mailpit thật, theo từng tình huống trong spec (đăng ký, xác minh, chống dò mật khẩu, TOTP, thiết bị, xóa tài khoản, khôi phục, đồng ý theo scope, pairwise, thu hồi, webhook, xuất dữ liệu). Cần hạ tầng ở bước 2 đang chạy, và cổng 8080 phải trống (dừng `feco-server` trước):

```bash
FECO_E2E=1 cargo test -p feco-server --test e2e_auth
```

**Chuỗi tin cậy TUF**: `cargo test -p feco-tuf` chạy các tình huống tấn công (gói bị sửa, metadata bị sửa, khóa lạ, rollback, đóng băng) cùng thu hồi và xoay khóa. Diễn tập toàn bộ quy trình bằng công cụ dòng lệnh, in biên bản Markdown:

```bash
scripts/tuf-drill.sh > /tmp/tuf-drill.md
cargo run -p feco-tuf -- help                   # các lệnh: keygen, init, publish, revoke, refresh, resign, rotate, status
```

CI (GitHub Actions) chạy tất cả những bước trên, kể cả kiểm thử đầu-cuối với hạ tầng Docker và diễn tập TUF, ở mỗi lần push.

## Xử lý sự cố

| Triệu chứng | Cách xử lý |
|---|---|
| Windows báo `LINK : fatal error LNK1104: cannot open file ...` khi biên dịch | Đường dẫn thư mục dự án quá dài (giới hạn 260 ký tự). Đặt thư mục build ngắn hơn: `export CARGO_TARGET_DIR=C:/t/feco` (PowerShell: `$env:CARGO_TARGET_DIR="C:\t\feco"`), hoặc clone vào thư mục nông như `C:\src` |
| `feco-server` thoát ngay với "Cấu hình không hợp lệ" | Đọc thông báo: nó chỉ đúng biến bị thiếu hoặc sai. Ở dev chỉ cần `FECO_ENV=dev` |
| "Không cập nhật được lược đồ cơ sở dữ liệu" | PostgreSQL chưa chạy hoặc sai `FECO_DATABASE_URL` |
| Trang báo "Dịch vụ tài khoản đang tạm gián đoạn" | Kratos chưa chạy; kiểm tra `curl http://127.0.0.1:4433/health/ready` |
| Không nhận được email | Mọi email dev nằm ở Mailpit http://127.0.0.1:8025, không gửi ra Internet |
| App mẫu báo "cổng 9999 đang bận" | Một lần chạy app mẫu khác vẫn đang chờ; dừng nó bằng `Ctrl+C` |
| Kiểm thử đầu-cuối báo "cổng 8080 phải trống" | Dừng `feco-server` đang chạy trước khi chạy kiểm thử |
| Kratos báo `database table is locked` | Kratos dev dùng SQLite trong bộ nhớ, không chịu được ghi đồng thời nhiều; các kiểm thử đã chạy lần lượt. Staging và production phải dùng PostgreSQL cho Kratos |
| Mật khẩu hợp lệ vẫn bị từ chối là "quá giống email" | Kratos chặn mật khẩu giống định danh; chọn mật khẩu khác hẳn email |

## Yêu cầu và thiết kế

Toàn bộ yêu cầu viết theo [OpenSpec](https://github.com/Fission-AI/OpenSpec) trong
[`openspec/changes/add-f-ecosystem-platform/`](openspec/changes/add-f-ecosystem-platform/):

- `proposal.md`: lý do, phạm vi
- `specs/*/spec.md`: 15 capability (tài khoản, đồng ý dữ liệu, launcher, danh mục, cổng nhà phát triển, kiểm duyệt, SDK, F-Builder, đơn xây app, thanh toán, quản trị…)
- `design.md`: quyết định kỹ thuật (Rust toàn bộ, Ory Kratos/Hydra, TUF, VNPAY/MoMo…)
- `tasks.md`: kế hoạch theo nhóm, đánh dấu tiến độ và cách đã xác minh

Câu hỏi còn chờ quyết định nằm ở [`QUESTIONS.md`](QUESTIONS.md).

## Cấu trúc thư mục

| Thư mục | Nội dung |
|---|---|
| `crates/feco-core` | Kiểu dữ liệu dùng chung: tiền VND, scope dữ liệu, môi trường |
| `crates/feco-config` | Đọc và kiểm tra cấu hình lúc khởi động |
| `crates/feco-tuf` | Kho TUF (dựng, ký, thu hồi, xoay khóa) và client xác minh cho launcher; công cụ `feco-tuf` |
| `services/feco-server` | Dịch vụ HTTP (axum): trang tài khoản, OAuth login/consent, `/v1/me`, webhook, xuất dữ liệu; `migrations/` là lược đồ PostgreSQL; `templates/` là giao diện |
| `tools/feco-demo-app` | App con mẫu: đăng nhập một lần và gọi `/v1/me` |
| `tools/feco-license-check` | Kiểm tra license dịch vụ và ảnh container |
| `infra/` | Hạ tầng dev: `docker-compose.yml`, cấu hình Kratos (schema, mẫu email) và Hydra |
| `scripts/` | `dev-local.sh`: chạy hạ tầng không cần Docker; `tuf-drill.sh`: diễn tập chuỗi tin cậy TUF |
| `docs/security/` | Quy trình xử lý sự cố, quản lý khóa TUF, biên bản diễn tập |
| `third_party/` | Khai báo license và ngoại lệ đã duyệt |
| `openspec/` | Yêu cầu, thiết kế, kế hoạch |

## Giấy phép

Phát hành theo lựa chọn của bạn một trong hai giấy phép: MIT ([LICENSE-MIT](LICENSE-MIT)) hoặc Apache License 2.0 ([LICENSE-APACHE](LICENSE-APACHE)).
Trừ khi bạn nói rõ khác đi, mọi đóng góp bạn gửi vào dự án được cấp phép theo cả hai giấy phép trên, không kèm điều khoản bổ sung.
