# F-EcoSystem-Software (F-Eco)

Nền tảng mở để nhà phát triển bên thứ ba phát hành **app con** qua một launcher chung, với tài khoản người dùng dùng chung, công cụ tự xây app (**F-Builder**) và dịch vụ **xây app theo yêu cầu** có thanh toán. Ý tưởng lấy cảm hứng từ mô hình launcher + bộ ứng dụng của ArtCraft, nhưng mở cho bên thứ ba.

> **Trạng thái: đang xây dựng.** Đã chạy được (nhóm task 1–11; 11.6 còn E2E sandbox và gửi đơn):
> - Tài khoản: đăng ký, xác minh email, đăng nhập, chống dò mật khẩu, xác thực hai lớp (TOTP, mã khôi phục), thiết bị đang đăng nhập, khôi phục mật khẩu, xóa tài khoản sau 30 ngày chờ.
> - Đăng nhập một lần cho app con qua OAuth 2.0/OIDC, màn hình đồng ý theo scope, API `/v1/me`, trang ứng dụng đã kết nối, thu hồi quyền, webhook, xuất dữ liệu cá nhân.
> - Giao diện tiếng Việt và tiếng Anh.
> - Bảo mật: HSTS và header bảo vệ, mã hóa dữ liệu nhạy cảm khi lưu (AES-256-GCM), giới hạn tần suất, nhật ký kiểm toán chỉ ghi thêm, cảnh báo đăng nhập từ thiết bị mới. Quy trình xử lý sự cố: [`docs/security/incident-response.md`](docs/security/incident-response.md).
> - Chuỗi tin cậy cho gói cài đặt theo TUF (dùng `tough`): ký phiên bản đã duyệt, thu hồi tới launcher trong 1 giờ, chặn gói hoặc metadata bị sửa, rollback, đóng băng; xoay khóa targets và root (ngưỡng 2/3). Công cụ `feco-tuf`, quy trình giữ khóa: [`docs/security/tuf-keys.md`](docs/security/tuf-keys.md).
> - F-Launcher (Rust + egui): đăng nhập qua trình duyệt, nhớ đăng nhập qua khởi động lại máy, cài/cập nhật/quay lại/gỡ app con có xác minh TUF, chạy được khi ngoại tuyến, chặn phiên bản bị thu hồi, mở app con bằng mã khởi chạy dùng một lần.
> - F-Store (`/store`): danh mục app đã duyệt, tìm không phân biệt dấu tiếng Việt, lọc theo danh mục, hệ điều hành, giá, nhà phát triển đã xác minh; trang chi tiết có scope dữ liệu kèm mục đích, license, chính sách quyền riêng tư, lịch sử thay đổi; đánh giá chỉ cho người đã cài, nhà phát triển trả lời, báo cáo vi phạm.
>
> - Cổng nhà phát triển (`/developer`): đăng ký cá nhân hoặc tổ chức (cần email đã xác minh và TOTP), Thỏa thuận nhà phát triển, xác minh tên miền bằng DNS TXT, nhóm và vai trò, client ID cùng địa chỉ chuyển hướng, manifest `f-app.toml`, nộp gói qua web hoặc CLI `f-dev`, kiểm tra chữ ký mã, kênh stable/beta và phát hành dần theo phần trăm, số liệu tổng hợp.
> - Kiểm duyệt: hàng đợi kiểm tra tự động (quét mã độc bằng engine chữ ký của F-Eco và ClamAV, chữ ký mã, kiến trúc, packer), quy tắc duyệt thủ công hay tự động, chính sách nội dung (`/policy/content`), từ chối kèm điều khoản và cách sửa, kháng nghị, gỡ khẩn cấp, quét lại 7 ngày.
> - F-SDK cho Rust, C (ABI ổn định), Python và TypeScript/Node.js: đăng nhập bằng mã khởi chạy của launcher hoặc qua trình duyệt (PKCE), tự làm mới token, `/v1/me`, kiểm tra quyền sử dụng (`/v1/entitlements`), báo lỗi khi chạy, kiểm chữ ký webhook. App dùng thư viện OIDC chuẩn không cần F-SDK. Mã lỗi thống nhất, liệt kê ở `/developer/docs`.
>
> Chưa có: bảng quản trị (người duyệt tạm dùng công cụ `feco-catalog`), máy ảo chạy thử app, môi trường sandbox, F-Builder, đơn hàng, thanh toán. Chưa dùng được cho người dùng cuối.

## Mục lục

- [Hướng dẫn chạy hệ thống](#hướng-dẫn-chạy-hệ-thống)
  - [1. Chuẩn bị](#1-chuẩn-bị)
  - [2. Chạy hạ tầng](#2-chạy-hạ-tầng)
  - [3. Chạy feco-server](#3-chạy-feco-server)
  - [4. Dùng thử tài khoản](#4-dùng-thử-tài-khoản)
  - [5. Dùng thử đăng nhập một lần với app mẫu](#5-dùng-thử-đăng-nhập-một-lần-với-app-mẫu)
  - [6. Dùng thử F-Launcher](#6-dùng-thử-f-launcher)
  - [7. Dùng thử cổng nhà phát triển](#7-dùng-thử-cổng-nhà-phát-triển)
  - [8. Viết app con bằng F-SDK](#8-viết-app-con-bằng-f-sdk)
  - [9. Dừng và xóa dữ liệu](#9-dừng-và-xóa-dữ-liệu)
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

### 6. Dùng thử F-Launcher

F-Launcher (`clients/feco-launcher`) là ứng dụng desktop. Bản dev đọc danh mục từ một kho TUF cục
bộ mà script dưới đây dựng sẵn, với Demo App ở bước 5 làm app con.

1. Chuẩn bị (cần Hydra đang chạy). Script tạo khóa và kho TUF trong `.dev/` (không commit), đăng ký
   client OAuth của launcher và của Demo App trong Hydra, ký một phiên bản Demo App, rồi in các
   biến môi trường cần dùng:

   ```bash
   scripts/dev-launcher.sh setup
   ```

2. Chạy lại `feco-server` (bước 3) với `FECO_LAUNCHER_CLIENT_ID` mà script in ra. Biến này bật mã
   khởi chạy và cho launcher đăng nhập không cần màn hình đồng ý (launcher là app của nền tảng).
3. Chạy launcher với hai biến script in ra:

   ```bash
   export FECO_LAUNCHER_CLIENT_ID=<client_id của launcher>
   export FECO_LAUNCHER_TUF=<đường dẫn .dev/tuf/repo>
   cargo run -p feco-launcher
   ```

4. Bấm **Đăng nhập**: trình duyệt mặc định mở trang đăng nhập F-Eco, sau đó launcher hiện tên bạn.
   Refresh token được lưu trong Windows Credential Manager (macOS: Keychain), nên tắt mở lại launcher
   hay khởi động lại máy vẫn còn đăng nhập.
5. Tab **Danh mục** → **Cài** Demo App. Hộp thoại xác nhận có dòng "App này chạy với quyền như mọi
   ứng dụng khác trên máy bạn". Gói được kiểm tra chữ ký và SHA-256 qua TUF rồi cài vào
   `%LOCALAPPDATA%\F-Eco\Launcher\apps` (macOS: `~/Library/Application Support/F-Eco/Launcher`).
6. Lần đầu, cho Demo App quyền qua trình duyệt một lần: `cargo run -p feco-demo-app -- --client-id
   $(cat .dev/demo-client-id)`. Sau đó tab **App của tôi** → **Mở**: launcher xin mã khởi chạy dùng
   một lần (sống 60 giây) và chạy Demo App; Demo App dùng F-SDK (`fsdk::session_from_launch`) đổi
   mã lấy token, không mở trình duyệt, rồi in scope được cấp và kết quả `/v1/me`.
7. Thử cập nhật, quay lại, thu hồi:

   ```bash
   scripts/dev-launcher.sh publish   # ký phiên bản mới → launcher hiện "Cập nhật lên ..."
   scripts/dev-launcher.sh revoke    # thu hồi bản mới nhất → sau lần đồng bộ kế tiếp không mở được, hiện lý do
   scripts/dev-launcher.sh refresh   # timestamp TUF hết hạn sau 1 giờ: chạy lại khi launcher báo metadata hết hạn
   ```

   Bấm **Đồng bộ** để launcher đồng bộ ngay (bình thường mỗi giờ). **Quay lại** khôi phục bản trước
   mà không tải lại. **Gỡ** hỏi có xóa dữ liệu cục bộ không (mặc định giữ) và nhắc rằng quyền dữ liệu
   chỉ thu hồi ở trang Ứng dụng đã kết nối.
8. Ngắt mạng (hoặc dừng thư mục kho) rồi mở lại launcher: "App của tôi" vẫn mở được app đã cài,
   thanh trên cùng báo "Đang ngoại tuyến".

Biến của launcher: `FECO_LAUNCHER_SERVER` (mặc định `http://127.0.0.1:8080/`), `FECO_LAUNCHER_HYDRA`
(`http://127.0.0.1:4444/`), `FECO_LAUNCHER_DATA_DIR` (thư mục dữ liệu), `FECO_LAUNCHER_TRUSTED_ROOT`
(root.json tin cậy; mặc định `metadata/1.root.json` của kho cục bộ), `FECO_LAUNCHER_FONT` (phông chữ
`.ttf` nếu máy không có Segoe UI/Arial). Ngoài dev, các địa chỉ phải là HTTPS.

F-Store: `scripts/dev-launcher.sh setup` cũng đưa Demo App vào danh mục, xem ở
http://127.0.0.1:8080/store. Sau khi cài Demo App bằng launcher (đã đăng nhập), bạn viết được nhận
xét ở trang chi tiết. Cho tới khi có cổng nhà phát triển và bảng quản trị (nhóm 8, 14), quản trị
danh mục bằng công cụ dòng lệnh (đọc cấu hình giống `feco-server`):

```bash
cargo run -p feco-server --bin feco-catalog -- add-app docs/catalog-app.example.json
cargo run -p feco-server --bin feco-catalog -- release <app> <phiên bản> windows-x64 <số byte> <tên target TUF> "<thay đổi>"
cargo run -p feco-server --bin feco-catalog -- remove <app> "<lý do>"           # gỡ khỏi danh mục
cargo run -p feco-server --bin feco-catalog -- reports                          # hàng chờ báo cáo vi phạm
cargo run -p feco-server --bin feco-catalog -- resolve <id> upheld "<ghi chú>"  # xử lý, gửi email cho người báo cáo
```

### 7. Dùng thử cổng nhà phát triển

1. Đăng nhập ở http://127.0.0.1:8080, xác minh email (liên kết trong Mailpit), rồi vào **Cài đặt
   tài khoản** để bật ứng dụng xác thực (TOTP). Cổng yêu cầu cả hai trước khi cho đăng ký.
2. Mở http://127.0.0.1:8080/developer, đăng ký **Cá nhân** hoặc **Tổ chức** và tích chấp nhận Thỏa
   thuận nhà phát triển. Tổ chức được cấp một bản ghi DNS TXT để xác minh tên miền; bản dev không
   có DNS thật nên bỏ qua bước này (app vẫn phát hành được, chỉ không có dấu "Đã xác minh").
3. **Tạo app mới**: mã app, tên, loại (desktop: client công khai dùng PKCE; có máy chủ riêng: có
   client secret, hiện một lần và xoay vòng được) và địa chỉ chuyển hướng. Trang app hiện client ID.
4. **Tạo token** cho `f-dev` (hiện một lần), viết `f-app.toml` (mẫu ở
   http://127.0.0.1:8080/developer/docs), rồi nộp phiên bản:

   ```bash
   cargo run -p f-dev -- validate --manifest f-app.toml
   FDEV_TOKEN=fdev_... cargo run -p f-dev -- publish --manifest f-app.toml --package dist/
   ```

   Lệnh in "Chờ duyệt" và mã phiên bản. Gói Windows có tệp chạy chưa ký Authenticode bị từ chối;
   gói macOS phải được ký Developer ID và đính vé notarize. Cũng nộp được qua trang app (dán
   manifest, tải file `.zip` cho từng nền tảng, bấm **Gửi duyệt**).
5. Kiểm duyệt. `feco-server` chạy hàng đợi kiểm tra tự động mỗi 10 giây (cũng chạy tay được bằng
   `scan`). Bản đầu tiên của app, hoặc bản thêm scope hay quyền hệ thống, vào hàng chờ duyệt thủ
   công; các bản khác qua kiểm tra thì được duyệt tự động. Cho tới khi có bảng quản trị (nhóm 14):

   ```bash
   cargo run -p feco-server --bin feco-catalog -- scan                     # chạy hàng đợi, xếp lịch quét lại 7 ngày
   cargo run -p feco-server --bin feco-catalog -- queue                    # hàng chờ duyệt thủ công
   cargo run -p feco-server --bin feco-catalog -- approve <app> <phiên bản> "Đạt"
   cargo run -p feco-server --bin feco-catalog -- reject <app> <phiên bản> brand_impersonation "<ghi chú>"
   cargo run -p feco-server --bin feco-catalog -- appeals                  # kháng nghị đang mở
   cargo run -p feco-server --bin feco-catalog -- appeal <app> <phiên bản> overturn "<ghi chú>"
   cargo run -p feco-server --bin feco-catalog -- emergency <app> all "<lý do>"   # gỡ khẩn cấp
   cargo run -p feco-server --bin feco-catalog -- alerts                   # cảnh báo mã độc, phát hiện muộn
   ```

   Điều khoản dùng khi từ chối: xem http://127.0.0.1:8080/policy/content. Nhà phát triển nhận
   email (Mailpit) nêu điều khoản và cách sửa, và bấm **Kháng nghị** trên trang app (một lần cho
   mỗi lần từ chối). Không có ClamAV thì chỉ engine chữ ký của F-Eco chạy (đủ để thử với mẫu EICAR).

6. Trên trang app, chọn kênh `stable` hoặc `beta` và phần trăm (10%, 50%, 100%) rồi bấm **Phát
   hành** (cần vai trò Quản trị hoặc Chủ sở hữu). App xuất hiện ở F-Store; scope và mục đích trên
   màn hình đồng ý lấy từ manifest. **Dừng phát hành dần** để các máy chưa nhận không nhận nữa.
7. Muốn launcher cài được bản vừa phát hành, chạy `feco-server` với kho TUF dev để máy chủ ký gói
   khi phát hành (đường dẫn do `scripts/dev-launcher.sh setup` tạo):

   ```bash
   export FECO_TUF_REPO=.dev/tuf/repo
   export FECO_TUF_SIGN_KEYS=.dev/tuf/keys/targets.pk8,.dev/tuf/keys/snapshot.pk8,.dev/tuf/keys/timestamp.pk8
   ```

### 8. Viết app con bằng F-SDK

F-SDK (`sdk/`, license MIT OR Apache-2.0) gói gọn đăng nhập và API cho app con. Khi F-Launcher mở
app, nó truyền `FECO_LAUNCH_CODE`, `FECO_CLIENT_ID`, `FECO_ISSUER_URL` và `FECO_SERVER_URL`; F-SDK
đổi mã khởi chạy lấy token. Khi người dùng mở app trực tiếp, F-SDK mở trình duyệt đăng nhập bằng
PKCE và nhận kết quả ở địa chỉ loopback (mặc định `http://127.0.0.1:47280/callback`, đổi bằng
`FSDK_REDIRECT_URI`; địa chỉ này phải có trong danh sách chuyển hướng của app). Scope mặc định là
`openid offline_access profile`; thêm `purchases` để đọc quyền sử dụng (`FSDK_SCOPES`).

```rust
let config = fsdk::Config::from_env()?;           // hoặc fsdk::Config::new(client_id, issuer, server)
let session = fsdk::session(&config).await?;      // launcher nếu có mã khởi chạy, không thì trình duyệt
let me = session.me().await?;                     // chỉ các trường thuộc scope đã đồng ý
let owner = session.entitlements().await?.iter().any(|e| e.kind == "owner");
```

| Ngôn ngữ | Thư mục | App mẫu |
|---|---|---|
| Rust | `sdk/rust` (crate `fsdk`) | `fsdk::session_from_launcher()`, `fsdk::login_with_browser()` |
| C | `sdk/c` (thư viện `fsdk_c`, header `sdk/c/include/fsdk.h`) | `sdk/c/examples/login.c` |
| Python ≥ 3.9 | `sdk/python` (gói `fsdk`, gọi thư viện C qua `ctypes`) | `sdk/python/examples/login.py` |
| TypeScript, Node.js ≥ 22.6 | `sdk/typescript` (gói `@f-eco/sdk`, viết thuần trên OIDC) | `sdk/typescript/examples/login.ts` |

Chạy thử app mẫu. `scripts/dev-launcher.sh setup` (bước 6) đã tạo sẵn app "QuanLyKho (app mẫu
F-SDK)" trong Hydra; `scripts/dev-launcher.sh env` in các biến cần đặt:

```bash
cargo build -p fsdk-c
eval "$(scripts/dev-launcher.sh env | sed -n '/App mẫu F-SDK/,$p')"     # đặt FECO_CLIENT_ID, FECO_ISSUER_URL, ...
FSDK_LIBRARY=target/debug/libfsdk_c.so python sdk/python/examples/login.py   # Windows: fsdk_c.dll, macOS: libfsdk_c.dylib
node sdk/typescript/examples/login.ts
```

Lần đầu, app mở trình duyệt để bạn đăng nhập và đồng ý cho "QuanLyKho" xem tên và quyền sử dụng;
terminal in `/v1/me` (có `sub` riêng của app, không có email) và `"entitlements": []`. Cấp quyền
`owner` rồi chạy lại để thấy quyền:

```bash
cargo run -p feco-server --bin feco-catalog -- grant "$FECO_CLIENT_ID" <email của bạn> owner custom-build:DEMO-1
```

Muốn tích hợp mà không dùng F-SDK: dùng thư viện OIDC chuẩn bất kỳ trỏ tới
`http://127.0.0.1:4444/.well-known/openid-configuration` (ví dụ Authlib:
`examples/standard-oidc/authlib_login.py`). Nhận sự kiện `user.revoked`, `user.deleted`,
`app.version.revoked`, `entitlement.changed`: nhập địa chỉ webhook trên trang app ở cổng nhà phát
triển, lưu khóa ký (hiện một lần) và kiểm chữ ký bằng `fsdk::verify_webhook` (hoặc `verify_webhook`
của bản Python, `verifyWebhook` của bản TypeScript). Danh sách mã lỗi và chính sách phiên bản API:
http://127.0.0.1:8080/developer/docs.

Cho tới khi có đơn xây app và thanh toán (nhóm 12–13), cấp quyền sử dụng bằng tay:

```bash
cargo run -p feco-server --bin feco-catalog -- grant <app hoặc client ID> <email> owner custom-build:<mã đơn>
cargo run -p feco-server --bin feco-catalog -- ungrant <app hoặc client ID> <email> owner
```

### 9. Dừng và xóa dữ liệu

- Dừng `feco-server` và app mẫu bằng `Ctrl+C`.
- Cách A: `docker compose -f infra/docker-compose.yml down`. Lệnh này dừng và xóa mọi dữ liệu dev, vì compose không dùng volume.
- Cách B: `Ctrl+C` từng cửa sổ. Muốn xóa dữ liệu PostgreSQL thì xóa thư mục `infra/.data/pg`.

## Địa chỉ các dịch vụ

| Dịch vụ | Địa chỉ | Dùng để |
|---|---|---|
| feco-server | http://127.0.0.1:8080 | Trang tài khoản, màn hình đồng ý, F-Store (`/store`), API `/v1/...` |
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
| `POST /v1/launch-codes` | Chỉ token của F-Launcher: `{"client_id"}` → mã khởi chạy dùng một lần, sống 60 giây |
| `/store`, `/store/{id}` | F-Store: danh mục, tìm kiếm (`?q=&category=&os=windows\|macos&price=free\|paid&verified=1`), trang chi tiết; `POST /store/{id}/reviews`, `/store/{id}/reviews/{rid}/reply`, `/store/{id}/report` |
| `GET /v1/catalog`, `GET /v1/catalog/{id}` | Danh mục dạng JSON (cùng tham số tìm kiếm); chi tiết kèm scope, mục đích, phiên bản, nhận xét |
| `POST /v1/catalog/installs` | Chỉ token của F-Launcher: `{"app_id", "version"}` ghi nhận đã cài (điều kiện để đánh giá) |
| `GET /v1/catalog/{id}/update?platform=&device=&channel=` | Phiên bản một launcher nên dùng, theo kênh và phần trăm phát hành dần |
| `/developer`, `/developer/apps/{id}`, `/developer/docs` | Cổng nhà phát triển và tài liệu |
| `POST /v1/dev/apps/{app}/versions`, `PUT /v1/dev/versions/{id}/packages/{platform}`, `POST /v1/dev/versions/{id}/submit` | API cho CLI `f-dev` (token `fdev_...`): nộp `f-app.toml`, tải gói (header `X-Feco-Sha256`, tối đa 2 GB), gửi duyệt |
| `/policy/content` | Chính sách nội dung (các điều khoản dùng khi từ chối phiên bản) |
| `POST /v1/apps/crashes` | App báo một lần lỗi khi chạy (`{"version"}`, access token của app); chỉ dùng cho số liệu tổng hợp |
| `POST /v1/launch-codes/exchange` | App con: `{"code", "client_id", "redirect_uri", "code_challenge", "scope"}` → `authorization_code` để đổi lấy token tại Hydra bằng PKCE. Chỉ cấp lại scope người dùng đã đồng ý; chưa đồng ý thì trả `consent_required` |
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
| `FECO_DNS_SERVERS` | `1.1.1.1:53,8.8.8.8:53` | Máy chủ DNS dùng để tra bản ghi TXT khi xác minh tên miền của nhà phát triển |
| `FECO_PACKAGE_DIR` | `infra/.data/packages` | Thư mục lưu gói nhà phát triển tải lên. Bắt buộc ở production |
| `FECO_CLAMD_ADDR` | không đặt | ClamAV (`clamd`, ví dụ `127.0.0.1:3310`), engine quét thứ hai của khâu kiểm duyệt. Bắt buộc ở production |
| `FECO_ADMIN_EMAILS` | không đặt | Email nhận cảnh báo kiểm duyệt (mã độc, phát hiện muộn), ngăn bởi dấu phẩy. Bắt buộc ở production |
| `FECO_TUF_REPO` / `FECO_TUF_SIGN_KEYS` | không đặt | Kho TUF và các file khóa targets, snapshot, timestamp để ký gói khi phát hành. Không đặt thì bỏ qua bước ký |
| `FECO_LAUNCHER_CLIENT_ID` | không đặt | Client ID của F-Launcher trong Hydra. Có thì bật mã khởi chạy app con (`/v1/launch-codes`) và launcher không phải qua màn hình đồng ý |
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

E2E của F-SDK (`sdk_end_to_end`) còn chạy app mẫu Python (cần `python` và thư viện `fsdk_c`, test tự dựng nếu thiếu) và TypeScript (cần `node` ≥ 22.6); máy không có thì bỏ qua, trừ khi đặt `FECO_E2E_SDKS=1`. `standard_oidc_library_integration` (app Python dùng Authlib, không dùng F-SDK) chạy khi đã `pip install authlib requests`. Test riêng của từng binding:

```bash
cargo build -p fsdk-c
FSDK_LIBRARY=target/debug/libfsdk_c.so python -m unittest discover -s sdk/python/tests
(cd sdk/typescript && node --test test/fsdk.test.ts)
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
| Launcher báo "Metadata phân phối không hợp lệ ... ExpiredMetadata" | Timestamp của kho TUF dev hết hạn sau 1 giờ: `scripts/dev-launcher.sh refresh` rồi bấm **Đồng bộ** |
| Launcher báo "cổng 47201 đang bận" khi đăng nhập | Một lần đăng nhập khác vẫn đang chờ trình duyệt; đóng launcher khác hoặc chờ hết 10 phút |
| Launcher thoát ngay với "không tìm thấy phông chữ hệ thống" | Đặt `FECO_LAUNCHER_FONT` tới một file `.ttf` có dấu tiếng Việt |
| Mở Demo App báo `consent_required` | Người dùng chưa từng đồng ý cho app; chạy app qua trình duyệt một lần (bước 6.6) |
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
| `crates/feco-manifest` | Lược đồ và trình kiểm tra `f-app.toml` (dùng chung cho cổng, CLI và kiểm duyệt) |
| `crates/feco-tuf` | Kho TUF (dựng, ký, thu hồi, xoay khóa) và client xác minh cho launcher; công cụ `feco-tuf` |
| `services/feco-server` | Dịch vụ HTTP (axum): trang tài khoản, OAuth login/consent, `/v1/me`, webhook, xuất dữ liệu, F-Store; công cụ `feco-catalog`; `migrations/` là lược đồ PostgreSQL; `templates/` là giao diện |
| `clients/f-builder` | F-Builder: công cụ tự xây app bằng coding agent, phát triển từ Craft Anything 0.6.0 (Cargo workspace riêng; xem `clients/f-builder/README.md` và `NOTICE`) |
| `clients/feco-launcher` | F-Launcher: thư viện (đăng nhập, cài/cập nhật/gỡ qua TUF, mã khởi chạy, API cục bộ) và giao diện egui |
| `tools/feco-demo-app` | App con mẫu: đăng nhập một lần và gọi `/v1/me`; khi được launcher mở thì đăng nhập bằng mã khởi chạy |
| `sdk/` | F-SDK: `rust` (crate `fsdk`), `c` (C ABI `fsdk_c` và header), `python` (binding `ctypes`), `typescript` (gói `@f-eco/sdk`) |
| `examples/standard-oidc` | App tích hợp bằng thư viện OIDC chuẩn (Authlib), không dùng F-SDK |
| `tools/f-dev` | CLI cho nhà phát triển: `f-dev validate`, `f-dev publish` |
| `tools/feco-license-check` | Kiểm tra license dịch vụ và ảnh container |
| `infra/` | Hạ tầng dev: `docker-compose.yml`, cấu hình Kratos (schema, mẫu email) và Hydra |
| `scripts/` | `dev-local.sh`: chạy hạ tầng không cần Docker; `tuf-drill.sh`: diễn tập chuỗi tin cậy TUF; `dev-launcher.sh`: kho TUF và client OAuth để dùng thử launcher |
| `docs/security/` | Quy trình xử lý sự cố, quản lý khóa TUF, biên bản diễn tập |
| `third_party/` | Khai báo license và ngoại lệ đã duyệt |
| `openspec/` | Yêu cầu, thiết kế, kế hoạch |

## Giấy phép

Phát hành theo lựa chọn của bạn một trong hai giấy phép: MIT ([LICENSE-MIT](LICENSE-MIT)) hoặc Apache License 2.0 ([LICENSE-APACHE](LICENSE-APACHE)).
Trừ khi bạn nói rõ khác đi, mọi đóng góp bạn gửi vào dự án được cấp phép theo cả hai giấy phép trên, không kèm điều khoản bổ sung.
