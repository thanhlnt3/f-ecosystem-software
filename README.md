# F-EcoSystem-Software (F-Eco)

Nền tảng mở để nhà phát triển bên thứ ba phát hành **app con** qua một launcher chung, với tài khoản người dùng dùng chung, công cụ tự xây app (**F-Builder**) và dịch vụ **xây app theo yêu cầu** có thanh toán. Ý tưởng lấy cảm hứng từ mô hình launcher + bộ ứng dụng của ArtCraft, nhưng mở cho bên thứ ba.

> **Phiên bản 1.5.0** (English: [README.en.md](README.en.md)) ([nhật ký thay đổi](CHANGELOG.md)). Tích hợp app con vào F-Eco: [hướng dẫn cho nhà phát triển](docs/integration-guide.md); đưa app mã nguồn mở từ GitHub vào F-Eco: [docs/github-apps.md](docs/github-apps.md); cấu hình cổng thanh toán, hóa đơn điện tử, email: [docs/payments-setup.md](docs/payments-setup.md). Gói phát hành (chương trình, F-SDK, tài liệu, `THIRD_PARTY_NOTICES`, SBOM) dựng bằng `scripts/package-release.sh`, hoặc tự động khi đẩy tag `v*` (`.github/workflows/release.yml`, tạo bản phát hành nháp).
>
> Đã chạy được (nhóm task 1–14; các mục còn mở ghi ở cuối nhật ký thay đổi):
> - Tài khoản: đăng ký, xác minh email, đăng nhập, chống dò mật khẩu, xác thực hai lớp (TOTP, mã khôi phục), thiết bị đang đăng nhập, khôi phục mật khẩu, xóa tài khoản sau 30 ngày chờ.
> - Đăng nhập một lần cho app con qua OAuth 2.0/OIDC, màn hình đồng ý theo scope, API `/v1/me`, trang ứng dụng đã kết nối, thu hồi quyền, webhook, xuất dữ liệu cá nhân.
> - Giao diện web tiếng Việt, tiếng Anh, tiếng Pháp và tiếng Nhật; trang Giao diện (`/preferences`) chọn ngôn ngữ, chủ đề, màu nhấn, 10 phong cách như F-Launcher, mật độ, cỡ chữ. Trang hồ sơ cho người đã đăng nhập (vai trò, thẻ số liệu, bảo mật, quyền riêng tư). Tài liệu có bản tiếng Anh trong `docs/en/`.
> - Trang trạng thái công khai (`/status`, `/status.json`), diễn tập sao lưu và khôi phục (`scripts/backup-drill.sh`), kiểm thử tải `/v1/me` ([biên bản](docs/reports/2026-11-15-performance.md)).
> - Bảo mật: HSTS và header bảo vệ, mã hóa dữ liệu nhạy cảm khi lưu (AES-256-GCM), giới hạn tần suất, nhật ký kiểm toán chỉ ghi thêm, cảnh báo đăng nhập từ thiết bị mới. Quy trình xử lý sự cố: [`docs/security/incident-response.md`](docs/security/incident-response.md).
> - Chuỗi tin cậy cho gói cài đặt theo TUF (dùng `tough`): ký phiên bản đã duyệt, thu hồi tới launcher trong 1 giờ, chặn gói hoặc metadata bị sửa, rollback, đóng băng; xoay khóa targets và root (ngưỡng 2/3). Công cụ `feco-tuf`, quy trình giữ khóa: [`docs/security/tuf-keys.md`](docs/security/tuf-keys.md).
> - F-Launcher (Rust + egui):
>   - đăng nhập qua trình duyệt, nhớ đăng nhập qua khởi động lại máy;
>   - cài, cập nhật, quay lại, gỡ app con có xác minh TUF; chạy được khi ngoại tuyến; chặn phiên bản bị thu hồi;
>   - mở app con bằng mã khởi chạy dùng một lần; mở app chạy trong terminal ở cửa sổ riêng;
>   - Thư viện và Cửa hàng xem theo lưới (ô đều nhau) hoặc danh sách, có tìm không dấu, lọc nhóm và sắp xếp;
>   - nút công cụ dạng icon kèm tooltip; Cài đặt và trang chi tiết app mở trong cửa sổ riêng;
>   - trang Cài đặt giao diện: ngôn ngữ (tiếng Việt, tiếng Anh, tiếng Pháp, tiếng Nhật), phong cách (Chân
>     phương, Trang nhã, Giấy da, Bích họa, Trời sao, Hồ sen, Lập thể, Hoa huệ, Phố cổ, Sơn mài),
>     chủ đề, màu nhấn, cỡ chữ, bố cục;
>   - hiện vai trò nền tảng, lối tắt tới Cổng nhà phát triển và Bảng quản trị theo vai trò;
>   - trang chi tiết app (giới thiệu, tính năng, có gì mới, thông tin phiên bản, nền tảng, nút
>     cài, mở, gỡ), chuyển sang app trước hoặc sau bằng nút hay phím mũi tên.
> - F-Store (`/store`): danh mục app đã duyệt, tìm không phân biệt dấu tiếng Việt, lọc theo danh mục, hệ điều hành, giá, nhà phát triển đã xác minh; trang chi tiết có scope dữ liệu kèm mục đích, license, chính sách quyền riêng tư, lịch sử thay đổi; đánh giá chỉ cho người đã cài, nhà phát triển trả lời, báo cáo vi phạm.
>
> - Cổng nhà phát triển (`/developer`): đăng ký cá nhân hoặc tổ chức (cần email đã xác minh và TOTP), Thỏa thuận nhà phát triển, xác minh tên miền bằng DNS TXT, nhóm và vai trò, client ID cùng địa chỉ chuyển hướng, manifest `f-app.toml`, nộp gói qua web hoặc CLI `f-dev`, kiểm tra chữ ký mã, kênh stable/beta và phát hành dần theo phần trăm, số liệu tổng hợp.
> - Kiểm duyệt: hàng đợi kiểm tra tự động (quét mã độc bằng engine chữ ký của F-Eco và ClamAV, chữ ký mã, kiến trúc, packer), quy tắc duyệt thủ công hay tự động, chính sách nội dung (`/policy/content`), từ chối kèm điều khoản và cách sửa, kháng nghị, gỡ khẩn cấp, quét lại 7 ngày (cả gói lẫn lỗ hổng thư viện trong mã nguồn đã nộp; lỗ hổng mới được báo cho nhà phát triển và hiện ở mục "Cảnh báo kiểm duyệt" của trang Hàng chờ duyệt).
> - F-SDK cho Rust, C (ABI ổn định), Python và TypeScript/Node.js: đăng nhập bằng mã khởi chạy của launcher hoặc qua trình duyệt (PKCE), tự làm mới token, `/v1/me`, kiểm tra quyền sử dụng (`/v1/entitlements`), báo lỗi khi chạy, kiểm chữ ký webhook. App dùng thư viện OIDC chuẩn không cần F-SDK. Mã lỗi thống nhất, liệt kê ở `/developer/docs`.
>
> - Đơn xây app theo yêu cầu (`/orders`): gửi yêu cầu kèm tệp (tối đa 200 MB) và chấp nhận Điều khoản dịch vụ xây app (bản nháp), báo giá theo mốc (đặt cọc ≤ 30%, hạn 7 ngày, đề nghị điều chỉnh), bàn giao từng mốc dưới dạng app riêng tư trong F-Launcher, nghiệm thu, lượt sửa miễn phí, tự nghiệm thu sau 14 ngày, tin nhắn kèm tệp, hủy đơn, tranh chấp, tải bàn giao cuối khi đã trả đủ.
>
> - Thanh toán qua ZaloPay (sandbox): chọn VietQR, thẻ ATM, ví ZaloPay hoặc thẻ quốc tế trên trang của cổng (F-Eco không nhận số thẻ), callback có chữ ký, truy vấn chủ động khi quá 15 phút, không thu hai lần, hoàn tiền (một phần, từ 10 triệu cần hai người duyệt), hóa đơn (bản nháp, chờ nhà cung cấp hóa đơn điện tử), lịch sử thanh toán, đối soát hằng ngày.
>
> - Thanh toán còn qua VNPAY (QR, thẻ ATM, thẻ quốc tế) và MoMo (ví); hóa đơn điện tử qua MISA meInvoice khi cấu hình (chưa có tài khoản thử).
> - Bảng quản trị (`/admin`) với 5 vai trò, bắt buộc passkey hoặc khóa bảo mật; thông báo qua email và trung tâm thông báo trên web và trong F-Launcher, tắt được loại tùy chọn, hủy đăng ký không cần đăng nhập.
>
> Chưa có: máy ảo chạy thử app, môi trường sandbox, tên miền email (SPF, DKIM, DMARC), tài khoản thử của VNPAY, MoMo, MISA. Chưa dùng được cho người dùng cuối.

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
  - [9. Dùng thử đơn xây app](#9-dùng-thử-đơn-xây-app)
  - [10. Dùng thử thanh toán ZaloPay](#10-dùng-thử-thanh-toán-zalopay)
  - [11. Dùng thử bảng quản trị và thông báo](#11-dùng-thử-bảng-quản-trị-và-thông-báo)
  - [12. Dùng thử ví F-Coin, vai trò và dự án](#12-dùng-thử-ví-f-coin-vai-trò-và-dự-án)
  - [13. Dừng và xóa dữ liệu](#13-dừng-và-xóa-dữ-liệu)
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

`scripts/dev-local.sh` dùng đúng cấu hình trong `infra/`, nên hành vi giống hệt cách A. Dữ liệu nằm trong `infra/.data` (không được commit): PostgreSQL trong `infra/.data/pg`, Kratos và Hydra trong `kratos.sqlite`, `hydra.sqlite`, nên tài khoản và client OAuth còn sau khi khởi động lại. Muốn chạy Kratos và Hydra trong bộ nhớ như trước (mỗi lần khởi động là sạch), đặt `FECO_DEV_MEMORY=1`.

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
# {"status":"ok","environment":"dev","version":"1.5.0"}
```

Ở môi trường `dev`, mọi địa chỉ đã có giá trị mặc định trỏ tới hạ tầng ở bước 2, và lược đồ cơ sở dữ liệu được tạo tự động khi khởi động. Xem thêm phần [Cấu hình](#cấu-hình).

### 4. Dùng thử tài khoản

Mở http://127.0.0.1:8080 trong trình duyệt. Giao diện theo ngôn ngữ của trình duyệt; đổi bằng nút **VI · EN · FR · 日本語** ở góc phải trên. Nút **Giao diện** bên cạnh mở trang tùy chỉnh: phong cách (Chân phương, Trang nhã, Giấy da, Bích họa, Trời sao, Hồ sen, Lập thể, Hoa huệ, Phố cổ, Sơn mài), chủ đề sáng/tối, màu nhấn, mật độ, cỡ chữ. Tùy chỉnh lưu trên trình duyệt (cookie `feco_ui`).

1. **Đăng ký** tại http://127.0.0.1:8080/registration. Mật khẩu phải dài từ 10 ký tự và không nằm trong các vụ rò rỉ dữ liệu, nên ví dụ `Password123!` sẽ bị từ chối.
   - Chọn **Vai trò**: Người dùng phổ thông (mặc định), Khách hàng cá nhân, Khách hàng doanh nghiệp, Lập trình viên cá nhân hoặc Công ty phát triển phần mềm.
   - Khách hàng doanh nghiệp và Công ty phát triển phần mềm phải nhập tên pháp lý và mã số thuế (10 số, ví dụ `0312345678`).
   - Vai trò khác Người dùng phổ thông chờ Quản trị hệ thống duyệt hoặc từ chối ở http://127.0.0.1:8080/admin/members. Yêu cầu ghi "Chọn khi đăng ký" kèm trạng thái xác minh email. Kết quả gửi về email; trong lúc chờ, tài khoản là Người dùng phổ thông.
2. **Xác minh email**: mở Mailpit tại http://127.0.0.1:8025. Mặc định mọi email của hệ thống dev đều được giữ ở đây, không gửi ra ngoài. Mở email "Xác minh email tài khoản F-Eco" rồi bấm liên kết. Muốn email tới hòm thư thật (ví dụ Gmail), xem [Gửi email tới hòm thư thật](#gửi-email-tới-hòm-thư-thật).
3. **Đăng nhập** tại http://127.0.0.1:8080/login. Trang chủ là **trang hồ sơ**: tên, email và trạng thái xác minh, vai trò, ngày tham gia; thẻ số liệu (ứng dụng đã kết nối, thiết bị, đơn hàng, thông báo chưa đọc); lối tắt bảo mật (kèm trạng thái xác thực hai lớp), quyền riêng tư, mua sắm và công cụ theo vai trò.
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
11. **Đăng nhập bằng Google hoặc GitHub**: nút hiện ở trang đăng nhập và đăng ký khi nhà cung cấp đã được khai báo (mục Cấu hình).
    - Tài khoản mới nhận email đã xác minh từ nhà cung cấp. Email chưa được nhà cung cấp xác minh thì không được dùng.
    - Email trùng một tài khoản có sẵn: F-Eco yêu cầu đăng nhập bằng mật khẩu cũ một lần để liên kết.
12. **Cảnh báo refresh token bị dùng lại**: khi một refresh token cũ của F-Launcher hay app con được dùng lại, phiên đó bị đăng xuất và bạn nhận email "Phiên đăng nhập … có dấu hiệu bị sao chép" kèm thiết bị và IP.

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
bộ mà script dưới đây dựng sẵn. App trong kho được thêm bằng `scripts/dev-apps.sh` (bước 9): app
mẫu Ghi chú, Bấm giờ dùng F-SDK và các app mã nguồn mở từ GitHub. Demo App không còn nằm trong kho
ứng dụng; nó chỉ dùng để thử đăng nhập một lần ở bước 5.

1. Chuẩn bị (cần Hydra đang chạy). Script tạo khóa và kho TUF trong `.dev/` (không commit), đăng ký
   client OAuth của launcher, của Demo App và của các app mẫu trong Hydra, rồi in các biến môi
   trường cần dùng. Môi trường dev từ bản 1.x còn Demo App trong kho thì gỡ bằng
   `scripts/dev-launcher.sh remove-demo`.

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
5. Thêm app vào kho (`scripts/dev-apps.sh all`, bước 9), bấm **Đồng bộ**, rồi trang **Cửa hàng**
   (thanh bên trái) → **Cài** Ghi chú. Hộp thoại xác nhận có dòng "App này chạy với quyền như mọi
   ứng dụng khác trên máy bạn". Gói được kiểm tra chữ ký và SHA-256 qua TUF rồi cài vào
   `%LOCALAPPDATA%\F-Eco\Launcher\apps` (macOS: `~/Library/Application Support/F-Eco/Launcher`).
6. Trang **Thư viện** → **Mở** Ghi chú: launcher xin mã khởi chạy dùng một lần (sống 60 giây) rồi
   chạy app. App dùng F-SDK đổi mã lấy token, không cần mật khẩu, và chào bạn bằng tên. Lần đầu,
   nếu bạn chưa đồng ý cho app đọc hồ sơ, F-SDK mở trình duyệt để hỏi.
7. Thử cập nhật, quay lại, thu hồi:

   ```bash
   scripts/dev-launcher.sh publish   # ký phiên bản Ghi chú mới → launcher hiện "Cập nhật lên ..."
   scripts/dev-launcher.sh revoke    # thu hồi bản Ghi chú mới nhất → sau lần đồng bộ kế tiếp không mở được, hiện lý do
   scripts/dev-launcher.sh refresh   # timestamp TUF hết hạn sau 1 giờ: chạy lại khi launcher báo metadata hết hạn
   ```

   Thử F-Launcher tự cập nhật: dựng bản release mang số phiên bản mới hơn bản đang chạy, rồi ký nó vào kho dev. F-Launcher bản cũ tải bản mới sau lần đồng bộ tới, hiện nút **Khởi động lại để cập nhật**, và cài bản mới khi khởi động lại.

   ```bash
   cargo build --release -p feco-launcher
   scripts/dev-launcher.sh publish-self   # gói feco-launcher-<phiên bản>-<nền tảng>.zip, mã app feco-launcher
   ```

   Bấm **Đồng bộ** để launcher đồng bộ ngay (bình thường mỗi giờ). **Quay lại** khôi phục bản trước
   mà không tải lại. **Gỡ** hỏi có xóa dữ liệu cục bộ không (mặc định giữ) và nhắc rằng quyền dữ liệu
   chỉ thu hồi ở trang Ứng dụng đã kết nối.
8. Ngắt mạng (hoặc dừng thư mục kho) rồi mở lại launcher: Thư viện vẫn mở được app đã cài, thanh
   bên báo "Đang ngoại tuyến".
9. Thêm app để thấy rõ lưới, danh sách và bộ lọc:

   ```bash
   scripts/dev-apps.sh all
   ```

   Lệnh này ký vào kho dev năm app, rồi bấm **Đồng bộ** để thấy chúng trong Cửa hàng:
   - hai app mẫu dùng F-SDK: **Ghi chú** và **Bấm giờ** (`examples/apps`);
   - ba app mã nguồn mở dựng từ GitHub: **kibi** (soạn thảo trong terminal), **bottom** (theo dõi hệ
     thống trong terminal), **eframe template** (app cửa sổ).

   Cách làm từng bước có trong [docs/github-apps.md](docs/github-apps.md). App terminal mở trong
   cửa sổ console riêng.
10. Giao diện launcher (rê chuột lên một icon để xem tooltip):
    - icon **Lưới** / **Danh sách** ở góc phải thanh công cụ; ở dạng lưới mọi ô cùng kích thước;
    - ô tìm (không phân biệt dấu, ví dụ "ghi chu"), bộ lọc nhóm và kiểu sắp xếp (tên, mở gần
      đây, cài gần đây, nhóm, dung lượng);
    - **Cài đặt** (icon bánh răng ở thanh bên) mở trong cửa sổ riêng: chủ đề sáng, tối hoặc theo hệ thống; màu nhấn; cỡ chữ 80–150%; bố cục
      mặc định; cỡ ô lưới; trang mở đầu; mô tả ngắn; chế độ gọn; khôi phục mặc định.

    Mọi lựa chọn lưu trong `%LOCALAPPDATA%\F-Eco\Launcher\settings.json` và có hiệu lực ngay.
11. Bấm vào một app để mở trang chi tiết trong cửa sổ riêng (mỗi app một cửa sổ, tối đa 6; bấm lại thì cửa sổ đó hiện lên trước): giới thiệu, tính năng, có gì mới, bảng thông tin, nút
    Cài, Mở, Gỡ, Mã nguồn, Website. Nút **‹ Trước** / **Sau ›** (hoặc phím mũi tên trái, phải)
    chuyển sang app kề bên theo đúng thứ tự đang hiển thị; **Esc** quay lại danh sách.
12. **Ngôn ngữ và phong cách**: Cài đặt → Giao diện. Chọn "English", "Français" hoặc "日本語" để
    đổi ngôn ngữ ngay. Bấm một ô xem trước để đổi phong cách: Chân phương, Trang nhã, Giấy da, Bích họa,
    Trời sao, Hồ sen, Lập thể, Hoa huệ, Phố cổ, Sơn mài. Tám phong cách sau dùng bảng màu riêng, nên
    ô chủ đề và màu nhấn bị tắt.
13. **Vai trò**: dưới tên người dùng ở thanh bên là vai trò nền tảng (Người sử dụng ứng dụng, Lập
    trình viên hoặc Quản trị viên). Lập trình viên có nút **Cổng nhà phát triển**; Quản trị viên có
    thêm **Bảng quản trị**. Đổi vai trò ở `/admin/members` (Quản trị hệ thống) hoặc bằng
    `feco-catalog role-set <email> developer "<lý do>"`.
14. **Thanh bên thu gọn**: bấm nút cạnh logo (hoặc Ctrl+B) để thanh bên chỉ còn icon. Rê chuột lên
    icon để xem tên; số thông báo chưa đọc và số bản cập nhật hiện thành huy hiệu. Launcher nhớ lựa
    chọn này; cửa sổ hẹp hơn 720 px thì thanh bên tự thu gọn.
15. **App web ThesisAI**: app web chạy trong trình duyệt và đăng nhập bằng tài khoản F-Eco.

    ```bash
    scripts/dev-apps.sh thesisai
    ```

    Bấm **Đồng bộ**. ThesisAI có nhãn "App web" trong Cửa hàng: **Thêm** rồi **Mở** để mở
    `http://localhost:3000/auth/feco` bằng trình duyệt. Muốn ThesisAI chạy thật thì làm theo
    [integrations/thesisai/README.md](integrations/thesisai/README.md) (áp bản vá "Đăng nhập bằng
    F-Eco", đặt biến `FECO_*` mà lệnh trên in ra).
16. **F-Launcher di động** (`clients/feco-mobile`, Flutter): ứng dụng đồng hành trên Android 10+ và iOS 18+.
    - Năm tab: Ứng dụng của tôi, Cửa hàng (mở app web, xem app desktop), Ví (mua app), Thông báo, Tài khoản.
    - Đăng nhập bằng tài khoản F-Eco qua trình duyệt hệ thống.
    - `scripts/dev-launcher.sh setup` tạo client OAuth di động. Chạy `feco-server` với `FECO_MOBILE_CLIENT_ID` mà script in ra, rồi chạy app trên Android emulator:

    ```bash
    cd clients/feco-mobile && flutter run --dart-define=FECO_CLIENT_ID=$(cat ../../.dev/mobile-client-id)
    ```

    Thêm `--dart-define=FECO_DEMO=true` để xem với dữ liệu mẫu, không cần máy chủ. Chi tiết trong [clients/feco-mobile/README.md](clients/feco-mobile/README.md).
17. **Mentor**: ứng dụng học tập cho học sinh và giáo viên, có cả app web và app desktop Windows, đăng nhập bằng tài khoản F-Eco.

    ```bash
    scripts/dev-apps.sh mentor
    ```

    - Lệnh trên tạo client OAuth phía máy chủ, ký target web của Mentor và ghi danh mục. Đặt `MENTOR_DESKTOP_ZIP` tới gói `mentor-<phiên bản>-windows-x64.zip` của bản phát hành để thêm app desktop.
    - Chạy gateway Mentor (cổng 8100) và app web (cổng 5173) theo [integrations/mentor/README.md](integrations/mentor/README.md), với các biến `FECO_*` mà lệnh trên in ra.
    - Trong launcher: **Đồng bộ**, Cửa hàng → Mentor → **Thêm** → **Mở**. Hoặc mở `http://localhost:5173` rồi bấm "Đăng nhập bằng F-Eco".
    - Lần đầu đăng nhập, Mentor tạo tài khoản học sinh. Giáo viên do quản trị Mentor nâng vai trò.
    - Trước khi tích hợp app bên ngoài, quét repo tại đúng commit sẽ dùng. Báo cáo nằm ở `.dev/scans/`; phát hiện đã xem xét ghi trong `integrations/<app>/scan-allow.toml`, tóm tắt trong `integrations/<app>/SCAN.md`:

      ```bash
      scripts/scan-integration.sh katD2707/agent-mentor-maximum-experience "$(cat integrations/mentor/MENTOR_COMMIT)" integrations/mentor/scan-allow.toml
      ```
18. **App web BinhAI Office**: bản thử "phòng AI" với 18 agent và văn phòng 3D, chạy hoàn toàn trong trình duyệt, không cần đăng nhập. F-Eco dựng app từ mã nguồn đã quét, rồi so với bản dựng sẵn của tác giả:

    ```bash
    integrations/binhai-office/build.sh ~/.feco-dev-ext/binhai-office/site --verify-dist
    python -m http.server 5180 --bind 127.0.0.1 --directory ~/.feco-dev-ext/binhai-office/site
    scripts/dev-apps.sh binhai
    ```

    Bấm **Đồng bộ**, rồi trong Cửa hàng chọn BinhAI Office → **Thêm** → **Mở**. Kết quả quét bảo mật nằm trong [integrations/binhai-office/SCAN.md](integrations/binhai-office/SCAN.md).
19. **App desktop VPdf** (Windows x64): đọc PDF offline, tìm tiếng Việt không dấu. F-Eco dựng gói từ mã nguồn đã quét, kèm PDFium và qpdf (kiểm SHA-256):

    ```bash
    integrations/vpdf/build.sh ~/.feco-dev-ext/vpdf/pkg
    scripts/dev-apps.sh vpdf
    ```

    Bấm **Đồng bộ**, rồi trong Cửa hàng chọn VPdf → **Cài** → **Mở**. Quét bảo mật và giấy phép: [integrations/vpdf/SCAN.md](integrations/vpdf/SCAN.md), [integrations/vpdf/NOTICE.md](integrations/vpdf/NOTICE.md).
20. **Tin nhắn**: thanh bên có mục "Tin nhắn" kèm số tin chưa đọc (cập nhật mỗi 60 giây). Trang Tin nhắn có danh sách hội thoại và luồng tin; gõ rồi bấm **Gửi** hoặc Ctrl+Enter. Khi đang mở một hội thoại, launcher hỏi tin mới mỗi 5 giây. Tệp đính kèm chỉ gửi trên web; tệp đã quét sạch mở bằng trình duyệt. F-Launcher di động có tab "Tin nhắn" (sáu tab).

Biến của launcher: `FECO_LAUNCHER_SERVER` (mặc định `http://127.0.0.1:8080/` chỉ ở bản debug; bản release bắt buộc đặt, trừ khi `FECO_LAUNCHER_DEV=1`), `FECO_LAUNCHER_HYDRA`
(`http://127.0.0.1:4444/`, cùng quy tắc), `FECO_LAUNCHER_DATA_DIR` (thư mục dữ liệu), `FECO_LAUNCHER_TRUSTED_ROOT`
(root.json tin cậy; mặc định `metadata/1.root.json` của kho cục bộ), `FECO_LAUNCHER_FONT` (phông chữ
`.ttf` nếu máy không có Segoe UI/Arial). Ngoài dev, các địa chỉ phải là HTTPS.

F-Store: `scripts/dev-apps.sh` cũng đưa các app vào danh mục, xem ở
http://127.0.0.1:8080/store. Sau khi cài một app bằng launcher (đã đăng nhập), bạn viết được nhận
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

   **Quét mã nguồn.** Phiên bản kèm mã nguồn được quét sau khi gói sạch:
   - bí mật lộ trong mã, như khóa riêng hay token, thì bị từ chối;
   - thư viện có lỗ hổng đã công bố (tra OSV.dev) hoặc mã đáng ngờ thì chuyển duyệt tay;
   - báo cáo hiện ở trang app và trang duyệt.

   Kèm mã nguồn bằng một trong ba cách: tải `source.zip` hoặc nhập repo GitHub và commit ở trang app; khai `[source]` trong `f-app.toml`; hoặc dùng `--source`:

   ```bash
   FDEV_TOKEN=fdev_... cargo run -p f-dev -- publish --manifest f-app.toml --package dist/ --source .
   ```

   `f-dev` bỏ `.git`, `target`, `node_modules` và tệp `.env` thật khi đóng gói mã nguồn.

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

Bàn giao mốc của đơn xây app (bước 9) tự cấp quyền `license` cho khách. Cho tới khi có thanh toán (nhóm 13), các quyền khác cấp bằng tay:

```bash
cargo run -p feco-server --bin feco-catalog -- grant <app hoặc client ID> <email> owner custom-build:<mã đơn>
cargo run -p feco-server --bin feco-catalog -- ungrant <app hoặc client ID> <email> owner
```

### 9. Dùng thử đơn xây app

1. Đăng nhập bằng tài khoản đã xác minh email, mở http://127.0.0.1:8080/orders (hoặc liên kết "Đơn xây app" ở trang chủ). Điền yêu cầu, đính kèm spec (tổng tối đa 200 MB), đọc và chấp nhận Điều khoản dịch vụ xây app (http://127.0.0.1:8080/policy/custom-build), bấm **Gửi yêu cầu**. Mailpit nhận email xác nhận kèm mã đơn (`DH-000001`), người nhận trong `FECO_ADMIN_EMAILS` nhận việc báo giá.
2. Đóng vai đội F-Eco bằng `feco-catalog` (tên người làm lấy từ `FECO_REVIEWER`):

   ```bash
   cargo run -p feco-server --bin feco-catalog -- orders submitted                 # đơn chờ báo giá
   cargo run -p feco-server --bin feco-catalog -- order-quote DH-000001 docs/order-quote.example.json
   ```

3. Trên trang đơn, xem báo giá (phạm vi, không bao gồm, quyền sở hữu mã nguồn, các mốc), bấm **Chấp nhận báo giá** hoặc **Đề nghị điều chỉnh**. Chấp nhận tạo yêu cầu thanh toán tiền đặt cọc; trả qua ZaloPay như bước 10, hoặc (khi chưa đặt khóa ZaloPay) đánh dấu đã trả bằng tay:

   ```bash
   cargo run -p feco-server --bin feco-catalog -- order DH-000001                  # xem mã yêu cầu thanh toán
   cargo run -p feco-server --bin feco-catalog -- order-paid <payment-request-id>
   ```

4. Bàn giao một mốc: bản build là một app trong danh mục, đặt riêng tư cho khách **trước** khi phát hành (bước 6 và 7), rồi bàn giao kèm kết quả kiểm tra. App chỉ hiện trong F-Launcher của khách hàng đó.

   ```bash
   cargo run -p feco-server --bin feco-catalog -- private <app> <email khách>
   cargo run -p feco-server --bin feco-catalog -- order-deliver DH-000001 1 <app> <phiên bản> ket-qua-kiem-tra.txt "Ghi chú thay đổi"
   cargo run -p feco-server --bin feco-catalog -- order-message DH-000001 "Chị nhập kho theo lô hay theo từng món?"
   ```

5. Khách **Nghiệm thu mốc này**, **Yêu cầu sửa** (trừ lượt sửa miễn phí) hoặc **Mở tranh chấp** trên trang đơn. Quá 7 ngày không phản hồi thì nền tảng nhắc, 14 ngày thì tự nghiệm thu (máy chủ chạy việc này mỗi giờ; `feco-catalog order-due` chạy ngay). Tranh chấp được giải quyết bằng `order-resolve DH-000001 continue|full|partial <số tiền>`.
6. Khi đơn Hoàn tất và đã trả đủ, đặt gói bàn giao cuối để khách tải về: `feco-catalog order-handover DH-000001 ban-giao.zip`.

### 10. Dùng thử thanh toán ZaloPay

1. Lấy khóa sandbox ZaloPay (`app_id`, `key1`, `key2`) ở tài liệu https://docs.zalopay.vn (bộ mẫu `app_id` 2554 dùng được ngày 11/11/2026) hoặc tài khoản sandbox riêng. Đặt trước khi chạy `feco-server`; không ghi khóa vào tệp trong repo:

   ```bash
   export FECO_ZALOPAY_APP_ID=<app_id> FECO_ZALOPAY_KEY1=<key1> FECO_ZALOPAY_KEY2=<key2>
   export FECO_ZALOPAY_ENDPOINT=https://sb-openapi.zalopay.vn
   ```

   Ngoài production chỉ nhận endpoint sandbox; trỏ tới `openapi.zalopay.vn` thì máy chủ từ chối khởi động.
2. Trên trang đơn (bước 9), sau khi chấp nhận báo giá, bấm **Thanh toán** ở dòng tiền đặt cọc: trang hiện số tiền, nội dung mốc và bốn phương thức. Chọn một phương thức để sang trang của ZaloPay (trả bằng ứng dụng ZaloPay sandbox hoặc thẻ thử trong tài liệu ZaloPay). Liên kết hết hạn sau 15 phút.
3. Ở máy dev, ZaloPay không gọi callback tới `127.0.0.1` được: trạng thái được cập nhật khi bạn quay về trang chuyển hướng (máy chủ truy vấn cổng), hoặc trong vòng một phút nhờ việc truy vấn chủ động. Đơn chuyển "Đang thực hiện", email kèm hóa đơn tới Mailpit.
4. **Lịch sử thanh toán** (http://127.0.0.1:8080/payments): giao dịch, hoàn tiền, tải lại hóa đơn, và thông tin xuất hóa đơn (cá nhân hoặc doanh nghiệp có mã số thuế).
5. Đóng vai quản trị viên:

   ```bash
   cargo run -p feco-server --bin feco-catalog -- payments DH-000001                       # giao dịch của đơn
   FECO_REVIEWER=lan cargo run -p feco-server --bin feco-catalog -- refund <transaction-id> 4000000 "Tranh chấp mốc 2"
   FECO_REVIEWER=minh cargo run -p feco-server --bin feco-catalog -- refund-approve <refund-id>   # từ 10 triệu cần người thứ hai
   cargo run -p feco-server --bin feco-catalog -- reconcile                                # đối soát ngày hôm qua
   ```

Không có khóa ZaloPay thì trang thanh toán báo chưa cấu hình cổng; ở dev có thể ghi nhận tay bằng `feco-catalog order-paid <payment-request-id>`. Thử riêng chữ ký với sandbox thật: đặt `FECO_ZALOPAY_SANDBOX_APP_ID`, `_KEY1`, `_KEY2` rồi chạy `cargo test -p feco-server --test zalopay_sandbox`.

### 11. Dùng thử bảng quản trị và thông báo

1. **Tài khoản quản trị viên mặc định** luôn có sẵn: máy chủ tạo hoặc khôi phục nó mỗi lần khởi động và mỗi 10 phút, đủ năm vai trò, không hạ vai trò hay xóa được.
   - Ở dev, email là `admin@f-eco.local`; mật khẩu ngẫu nhiên được sinh vào `.dev/admin-password` (không commit) ở lần chạy đầu.
   - Ở staging và production, phải đặt `FECO_ADMIN_EMAIL` và `FECO_ADMIN_PASSWORD_FILE` (tệp bí mật, mật khẩu tối thiểu 12 ký tự); thiếu thì máy chủ không khởi động.
   - Như mọi quản trị viên, phải thêm passkey trước khi vào bảng quản trị (bước 2).

   Muốn cấp vai trò quản trị cho tài khoản của bạn (email phải đã xác minh):

   ```bash
   cargo run -p feco-server --bin feco-catalog -- admin-set <email> project_manager,finance,sysadmin
   ```

2. Bảng quản trị cần passkey hoặc khóa bảo mật: vào Cài đặt (http://127.0.0.1:8080/settings), bấm **Thêm passkey**, rồi đăng xuất và đăng nhập lại (mật khẩu, sau đó passkey). Mở http://127.0.0.1:8080/admin: trang **Tổng quan** có thẻ số liệu theo vai trò (yêu cầu quyền Lập trình viên, người dùng, phiên bản chờ duyệt, đơn sắp trễ, hoàn tiền chờ duyệt, doanh thu 30 ngày và biểu đồ 14 ngày); thanh bên chỉ có mục của vai trò bạn có (hàng chờ duyệt, đơn xây app, hoàn tiền, hỗ trợ người dùng phải nhập lý do, phân quyền).

   Mỗi danh sách quản trị (hàng chờ duyệt, đơn xây app, hoàn tiền, phân quyền, dự án, báo cáo tin nhắn, hàng chờ rút và cấp F-Coin) có thanh công cụ:
   - tìm kiếm không phân biệt hoa thường và dấu tiếng Việt;
   - lọc trạng thái và khoảng ngày;
   - sắp xếp mới nhất, cũ nhất, người gửi A–Z hoặc Z–A;
   - "Xóa lọc" và dòng "Đang hiện N/M".

   Điều kiện nằm trên địa chỉ trang, ví dụ http://127.0.0.1:8080/admin/orders?q=an&status=all&sort=sender. Danh sách đơn mặc định chỉ hiện đơn đang mở; chọn "Tất cả" để xem cả đơn đã xong hoặc đã hủy.

   Trên F-Launcher, nút **Bảng quản trị** mở menu các mục bạn được vào. Trên điện thoại, tab **Tài khoản** có thẻ **Bảng quản trị**. Cả hai mở trang web tương ứng.

   Trình duyệt thường không cho tạo passkey trên địa chỉ `127.0.0.1`. Ở máy dev, đặt `FECO_ADMIN_PASSKEY_OPTIONAL=true` (cùng `FECO_ENV=dev`) để vào bảng quản trị chỉ bằng mật khẩu; mọi trang quản trị sẽ hiện cảnh báo. Biến này làm staging và production từ chối khởi động.
3. Trung tâm thông báo: http://127.0.0.1:8080/notifications (và tab **Thông báo** trong F-Launcher). Tắt hoặc bật thông báo cập nhật app, gợi ý app, bản tin; email bản tin có liên kết hủy đăng ký không cần đăng nhập.
4. Thêm cổng VNPAY, MoMo và hóa đơn điện tử MISA bằng biến môi trường (khóa sandbox của chính bạn, không ghi vào repo): `FECO_VNPAY_TMN_CODE`, `FECO_VNPAY_HASH_SECRET`, `FECO_VNPAY_PAYMENT_URL`; `FECO_MOMO_PARTNER_CODE`, `FECO_MOMO_ACCESS_KEY`, `FECO_MOMO_SECRET_KEY`, `FECO_MOMO_ENDPOINT`; `FECO_MISA_CLIENT_ID`, `FECO_MISA_CLIENT_SECRET`, `FECO_MISA_TAX_CODE`, `FECO_MISA_USERNAME`, `FECO_MISA_PASSWORD`, `FECO_MISA_INV_SERIES`, `FECO_MISA_VAT_RATE`. Cổng gọi callback về `FECO_PAYMENT_CALLBACK_BASE` (địa chỉ công khai, ví dụ tunnel).
5. Kiểm tên miền gửi email: `cargo run -p feco-server --bin feco-catalog -- mail-dns <tên miền> <selector DKIM>`. Production chỉ nhận `FECO_SMTP_URL` dạng `smtps://` hoặc `smtp://...?tls=starttls`.

### 12. Dùng thử ví F-Coin, vai trò và dự án

F-Coin là đơn vị thanh toán chỉ dùng trong F-Eco. Tỷ giá ban đầu là 1 F-Coin = 1.000 ₫; quản trị viên Tài chính đổi được tỷ giá.

1. **Ví**: mở http://127.0.0.1:8080/wallet.
   - Nạp: nhập số F-Coin (10–50.000), chọn cổng thanh toán (cần cấu hình ZaloPay, VNPAY hoặc MoMo sandbox ở bước 10 và 11). Cổng xác nhận đã thu thì F-Coin vào ví.
   - Ở máy dev chưa có cổng: dùng `feco-catalog` hoặc trang thanh toán thủ công để ghi nhận.
   - Rút: hoàn về một lần nạp (qua cổng) hoặc chuyển khoản ngân hàng. F-Coin bị giữ ngay và chờ Tài chính duyệt. Biến `FECO_WALLET_WITHDRAWALS=false` tắt việc tạo lệnh rút mới.
   - Xem lịch sử giao dịch (lọc theo loại) và thống kê thu chi theo tháng ở http://127.0.0.1:8080/wallet/stats.
2. **Mua app**: app có giá trên F-Store và trong F-Launcher có nút **Mua · N F-Coin**. Nhà phát triển đặt giá ở trang app trong Cổng nhà phát triển.
3. **Vai trò**: http://127.0.0.1:8080/account/role để xin vai trò Khách hàng cá nhân, Khách hàng doanh nghiệp, Lập trình viên cá nhân hoặc Công ty phát triển phần mềm. Quản trị hệ thống duyệt ở `/admin/members`, hoặc gán thẳng:

   ```bash
   cargo run -p feco-server --bin feco-catalog -- role-set <email> customer "Dùng thử dự án"
   ```

4. **Dự án**: http://127.0.0.1:8080/projects.
   - Lập trình viên hoặc công ty bật hồ sơ nhận việc.
   - Khách hàng tìm người nhận việc, gửi yêu cầu, thương lượng giá.
   - Doanh nghiệp hoặc công ty lập hợp đồng, hai bên ký.
   - Khách ký quỹ F-Coin, nhận bàn giao, chấp nhận thì F-Coin chuyển cho người nhận việc.
   - Tranh chấp do Quản lý dự án xử lý ở `/admin/projects`.
5. **Quản trị ví**: http://127.0.0.1:8080/admin/wallet (Tài chính, Quản trị hệ thống) gồm tổng F-Coin, tỷ giá (mỗi lần đổi tối đa 20%), phí, duyệt lệnh rút, khóa ví, tra giao dịch.
   - **Chuyển F-Coin** (Tài chính): nhập email người nhận (đã xác minh), số F-Coin, lý do, chọn nguồn **Ví của tôi** hoặc **Quỹ nền tảng**. Cấp từ quỹ nền tảng từ 1.000.000 ₫ trở lên chờ một quản trị viên Tài chính khác duyệt ở mục "Yêu cầu cấp F-Coin chờ duyệt".
   - Quản trị viên không duyệt hay đánh dấu đã chuyển lệnh rút của chính mình. Lệnh rút cần hai người duyệt khi tổng rút trong 24 giờ từ 10.000.000 ₫.

   Kiểm tra sổ cái:

   ```bash
   cargo run -p feco-server --bin feco-catalog -- wallet-check
   ```

6. **Ứng dụng của tôi**: http://127.0.0.1:8080/library hiện app đã cài qua F-Launcher và app đã mua, kèm nhãn "Có bản mới". F-Launcher tự gợi ý cập nhật sau mỗi lần đồng bộ.
   - "Có bản mới" chỉ tính bản phát hành trên kênh ổn định cho mọi thiết bị. Bản đang phát hành dần, bản beta và bản tạm dừng không tính.
   - F-Launcher báo máy chủ sau khi cập nhật, quay lại bản trước hoặc gỡ app. Nhờ vậy, web và điện thoại thôi hiện "Có bản mới" sau khi bạn cập nhật trên máy tính.
   - Thông báo "có bản cập nhật" gửi một lần cho mỗi phiên bản, khi bản đó tới tay mọi người, và chỉ gửi cho người đang cài bản cũ hơn. Bản phát hành bằng `feco-catalog release` được báo trong vòng 10 phút.
   - Trên điện thoại: tab "Ứng dụng của tôi" có huy hiệu số app có bản mới, kèm dòng "Mở F-Launcher trên máy tính để cập nhật". App tải lại danh sách khi bạn quay lại từ nền.

7. **Tin nhắn**: http://127.0.0.1:8080/messages.
   - Khách hàng bấm **Nhắn nhà phát triển** ở trang app trong F-Store hoặc ở danh sách nhà phát triển trong Dự án.
   - Mỗi dự án có một hội thoại giữa khách hàng và người nhận việc; tin nhắn cũ của dự án được chuyển sang.
   - Tin đến ngay không cần tải lại trang (Server-Sent Events). Đính kèm một tệp tối đa 20 MB mỗi tin; tệp được quét mã độc trước khi tải được.
   - Nhà phát triển chặn được hội thoại trực tiếp; ai cũng báo cáo được. Quản trị Hỗ trợ xem báo cáo ở `/admin/chat-reports`.
   - Thông báo và email chỉ ghi "Tin nhắn mới từ …", không chép nội dung, tối đa một lần mỗi 15 phút cho mỗi hội thoại. Tắt email ở cài đặt thông báo, nhóm "Tin nhắn".

### 13. Dừng và xóa dữ liệu

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
| `POST /oauth2/token` | Điểm nhận token: chuyển tiếp tới Hydra, ghi băm refresh token để phát hiện dùng lại. Tài liệu discovery của Hydra trỏ `token_endpoint` về đây |
| `GET /auth/after-login?next=` | Đích sau đăng nhập và đăng ký: ghi nhật ký và báo thiết bị mới cho đăng nhập Google/GitHub, rồi chuyển tới `next` (chỉ trong F-Eco) |
| `POST /v1/launch-codes` | Chỉ token của F-Launcher: `{"client_id"}` → mã khởi chạy dùng một lần, sống 60 giây |
| `/store`, `/store/{id}` | F-Store: danh mục, tìm kiếm (`?q=&category=&os=windows\|macos&price=free\|paid&verified=1`), trang chi tiết; `POST /store/{id}/reviews`, `/store/{id}/reviews/{rid}/reply`, `/store/{id}/report` |
| `GET /v1/catalog`, `GET /v1/catalog/{id}` | Danh mục dạng JSON (cùng tham số tìm kiếm); chi tiết kèm scope, mục đích, phiên bản, nhận xét |
| `POST /v1/catalog/installs` | Chỉ token của F-Launcher: `{"app_id", "version"}` ghi nhận đã cài, cập nhật hoặc quay lại một bản (điều kiện để đánh giá) |
| `DELETE /v1/catalog/installs/{app}` | Chỉ token của F-Launcher: đã gỡ app; app rời "Ứng dụng của tôi" (trừ app đã mua) và không còn nhận thông báo cập nhật |
| `GET /v1/catalog/{id}/update?platform=&device=&channel=` | Phiên bản một launcher nên dùng, theo kênh và phần trăm phát hành dần |
| `/developer`, `/developer/apps/{id}`, `/developer/docs` | Cổng nhà phát triển và tài liệu |
| `POST /v1/dev/apps/{app}/versions`, `PUT /v1/dev/versions/{id}/packages/{platform}`, `POST /v1/dev/versions/{id}/submit` | API cho CLI `f-dev` (token `fdev_...`): nộp `f-app.toml`, tải gói (header `X-Feco-Sha256`, tối đa 2 GB), gửi duyệt |
| `/policy/content` | Chính sách nội dung (các điều khoản dùng khi từ chối phiên bản) |
| `POST /v1/apps/crashes` | App báo một lần lỗi khi chạy (`{"version"}`, access token của app); chỉ dùng cho số liệu tổng hợp |
| `POST /v1/launch-codes/exchange` | App con: `{"code", "client_id", "redirect_uri", "code_challenge", "scope"}` → `authorization_code` để đổi lấy token tại Hydra bằng PKCE. Chỉ cấp lại scope người dùng đã đồng ý; chưa đồng ý thì trả `consent_required` |
| `/wallet`, `/wallet/stats`, `POST /wallet/topup`, `POST /wallet/withdraw` | Ví F-Coin: số dư, nạp, rút, lịch sử, thống kê |
| `GET /v1/library` | Chỉ token của F-Launcher (máy tính hoặc di động): "Ứng dụng của tôi" dạng JSON |
| `GET /v1/chat/unread` | Token của F-Launcher: tổng số tin chưa đọc |
| `GET`, `POST /v1/chat/conversations` | Token của F-Launcher: danh sách hội thoại; bắt đầu hội thoại với `{"developer"}` hoặc `{"app_id"}` |
| `GET`, `POST /v1/chat/conversations/{id}/messages` | Token của F-Launcher: tin sau `?after=<id>`; gửi `{"body"}` |
| `POST /v1/chat/conversations/{id}/read` | Token của F-Launcher: `{"up_to"}`, đánh dấu đã đọc |
| `PUT /v1/dev/versions/{id}/source` | Token `fdev_`: mã nguồn dạng zip (header `X-Feco-Sha256`) |
| `GET /v1/wallet`, `POST /v1/wallet/purchases` | Chỉ token của F-Launcher: số dư, app đã mua, giá; mua app `{"app_id", "expected_price"}` |
| `/account/role`, `/projects`, `/projects/{id}`, `/library` | Xin vai trò, dự án phần mềm, ứng dụng của tôi |
| `/admin/wallet`, `/admin/projects` | Quản trị ví F-Coin (Tài chính) và dự án (Quản lý dự án) |
| `GET /store/{id}/icon.png`, `GET /favicon.svg` | Logo app và logo F-Launcher |
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
| `FECO_SMTP_URL_FILE` | không đặt | Tệp bí mật chứa URL SMTP (có mật khẩu), thay cho `FECO_SMTP_URL`. `FECO_SMTP_URL` được ưu tiên nếu đặt cả hai |
| `FECO_EMAIL_FROM` | `F-Eco <no-reply@f-eco.example>` | |
| `FECO_CHALLENGE_PROVIDER` | `test` | `test` (chỉ dev/staging) hoặc `turnstile` (Cloudflare Turnstile). Production cấm `test` |
| `FECO_TURNSTILE_SITE_KEY` / `FECO_TURNSTILE_SECRET_KEY` | | Bắt buộc khi `FECO_CHALLENGE_PROVIDER=turnstile` |
| `FECO_TRUSTED_PROXIES` | rỗng | IP proxy phía trước, phân tách bằng dấu phẩy. Chỉ kết nối từ các IP này mới được tin `X-Forwarded-For` |
| `FECO_DATA_KEY` | khóa dev cố định | Khóa AES-256 (32 byte, base64) mã hóa khóa webhook và file xuất dữ liệu khi lưu. Bắt buộc ở production. Tạo: `openssl rand -base64 32` |
| `FECO_RATE_IP_PER_MIN` | `1200` | Số yêu cầu mỗi phút từ một IP; vượt thì trả 429 kèm `Retry-After` |
| `FECO_RATE_APP_PER_MIN` / `FECO_RATE_USER_PER_MIN` | `1200` / `120` | Số lần gọi `/v1/me` mỗi phút của một app / một người dùng |
| `FECO_DNS_SERVERS` | `1.1.1.1:53,8.8.8.8:53` | Máy chủ DNS dùng để tra bản ghi TXT khi xác minh tên miền của nhà phát triển |
| `FECO_PACKAGE_DIR` | `infra/.data/packages` | Thư mục lưu gói nhà phát triển tải lên. Bắt buộc ở production |
| `FECO_CLAMD_ADDR` | không đặt | ClamAV (`clamd`, ví dụ `127.0.0.1:3310`), engine quét thứ hai của khâu kiểm duyệt. Bắt buộc ở production |
| `FECO_ADMIN_EMAILS` | không đặt | Email nhận cảnh báo kiểm duyệt (mã độc, phát hiện muộn), ngăn bởi dấu phẩy. Bắt buộc ở production |
| `FECO_TUF_REPO` / `FECO_TUF_SIGN_KEYS` | không đặt | Kho TUF và các file khóa targets, snapshot, timestamp để ký gói khi phát hành. Có đặt thì máy chủ cũng tự ký lại metadata trước khi hết hạn (lúc khởi động và mỗi 5 phút). Không đặt thì bỏ qua bước ký |
| `FECO_ADMIN_PASSKEY_OPTIONAL` | `false` | `true`: bảng quản trị không bắt buộc passkey. Chỉ được đặt khi `FECO_ENV=dev`; môi trường khác từ chối khởi động |
| `FECO_LAUNCHER_CLIENT_ID` | không đặt | Client ID của F-Launcher trong Hydra. Có thì bật mã khởi chạy app con (`/v1/launch-codes`) và launcher không phải qua màn hình đồng ý |
| `FECO_VNPAY_*`, `FECO_MOMO_*` | không đặt | Dev/staging chỉ được trỏ tới sandbox của cổng thanh toán |
| `FECO_MOBILE_CLIENT_ID` | | Client OAuth của F-Launcher di động (bên thứ nhất: không hỏi đồng ý, gọi được API dành cho launcher) |
| `FECO_OSV_URL` | `https://api.osv.dev` | Dịch vụ tra lỗ hổng thư viện khi quét mã nguồn. Chỉ gửi hệ sinh thái, tên và phiên bản thư viện |
| `FECO_OSV` | bật | `off` thì không tra lỗ hổng (phiên bản có mã nguồn chuyển duyệt tay) |
| `FECO_SOURCE_FETCH_URL` | `https://codeload.github.com` | Nơi tải mã nguồn khai bằng repo GitHub và commit |
| `FECO_ADMIN_EMAIL` | dev: `admin@f-eco.local` | Email tài khoản quản trị viên mặc định; bắt buộc ngoài dev |
| `FECO_ADMIN_PASSWORD_FILE` | dev: `.dev/admin-password` (tự sinh) | Tệp bí mật chứa mật khẩu ban đầu (≥ 12 ký tự), chỉ dùng khi tạo tài khoản; bắt buộc ngoài dev |
| `FECO_WALLET_WITHDRAWALS` | `true` | `false`: tạm ngừng nhận lệnh rút F-Coin mới (lệnh đang chờ vẫn xử lý) |
| `RUST_LOG` | `info` | Mức nhật ký, ví dụ `debug` |

**Kratos và Hydra khi triển khai** (không phải biến của `feco-server`):

- `SELFSERVICE_METHODS_OIDC_CONFIG_PROVIDERS` (Kratos): mảng JSON các nhà cung cấp Google và GitHub, gồm `client_id`, `client_secret` và `mapper_url: file:///etc/config/kratos/oidc/mapper.jsonnet`. Mẫu ở `openspec/changes/oidc-login-and-token-reuse/design.md` (O2). Không đặt thì không có nút đăng nhập ngoài.
- `WEBFINGER_OIDC_DISCOVERY_TOKEN_URL` (Hydra): `https://<máy chủ F-Eco>/oauth2/token`, để app dùng thư viện OIDC chuẩn đổi token qua F-Eco.

Mọi bí mật (mật khẩu CSDL, khóa) được in ra nhật ký dưới dạng `***`. Không commit bí mật thật vào kho mã; CI có bước quét bí mật.

## Kiểm thử

```bash
cargo test --workspace                          # test đơn vị (không cần hạ tầng)
cargo clippy --workspace --all-targets -- -D warnings
cargo deny check                                # license, lỗ hổng, nguồn crate
cargo run -p feco-license-check                 # license dịch vụ và ảnh container
npx -y @fission-ai/openspec@1.16.1 validate add-f-ecosystem-platform --strict
```

**Kiểm thử đầu-cuối** chạy với Kratos, Hydra, PostgreSQL và Mailpit thật, theo từng tình huống trong spec (đăng ký, xác minh, chống dò mật khẩu, TOTP, thiết bị, xóa tài khoản, khôi phục, đồng ý theo scope, pairwise, thu hồi, webhook, xuất dữ liệu, danh mục, cổng nhà phát triển, kiểm duyệt, F-SDK, đơn xây app với thời gian mô phỏng, thanh toán với cổng ZaloPay giả cùng giao thức). Cần hạ tầng ở bước 2 đang chạy, và cổng 8080 phải trống (dừng `feco-server` trước):

```bash
FECO_E2E=1 cargo test -p feco-server --test e2e_auth
```

Mỗi bộ E2E dùng cơ sở dữ liệu riêng, tự tạo: `e2e_auth` dùng `feco_e2e` (đặt `FECO_E2E_DATABASE_URL` để đổi), chat dùng `feco_chat_e2e`, quét mã nguồn dùng `feco_scan_e2e`. Không bộ nào ghi vào cơ sở dữ liệu dev `feco`.

Các E2E đọc email trong Mailpit. Nếu Kratos đang gửi email thật (đã có `.dev/smtp-url`), hãy khởi động lại Kratos với Mailpit trước khi chạy E2E, để test không gửi thư ra ngoài:

```bash
FECO_SMTP_URL="smtp://127.0.0.1:1025/?disable_starttls=true" scripts/dev-local.sh kratos
```


```bash
FECO_E2E=1 cargo test -p feco-server --test e2e_chat --test e2e_source_scan
```

Đăng nhập Google/GitHub chạy với nhà cung cấp OIDC giả trong test (`127.0.0.1:4499`). Kratos cần thêm `infra/kratos/kratos.e2e-oidc.yml`; thiếu thì test bỏ qua, trừ khi đặt `FECO_E2E_OIDC=1` (CI):

```bash
FECO_KRATOS_E2E_OIDC=1 scripts/dev-local.sh kratos                                # Kratos chạy trực tiếp trên máy
docker compose -f infra/docker-compose.yml -f infra/docker-compose.e2e.yml up -d  # hoặc bằng docker
FECO_E2E=1 FECO_E2E_OIDC=1 cargo test -p feco-server --test e2e_auth -- external_login refresh_token_reuse
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

**Hiệu năng, sao lưu, trạng thái** (nhóm task 15):

```bash
# Kiểm thử tải /v1/me: 1.000 yêu cầu/giây trong 30 giây, đạt khi p95 < 300 ms và không lỗi
FECO_E2E=1 cargo test --release -p feco-server --test e2e_auth -- --ignored load_v1_me --nocapture
# Thời gian khởi động F-Launcher (dưới 3 giây), Windows
powershell -File scripts\measure-launcher-startup.ps1 -Exe target\release\feco-launcher.exe
# Diễn tập sao lưu liên tục và khôi phục (cụm Postgres tạm, không đụng dữ liệu đang dùng)
PG_BIN=/duong/dan/pgsql/bin scripts/backup-drill.sh docs/ops/drills/<ngày>-backup-drill.md
# Mọi tài liệu có bản tiếng Anh đủ phần
python3 scripts/check-docs-i18n.py
```

Trang trạng thái: http://127.0.0.1:8080/status. Cách vận hành sao lưu và giám sát: [docs/ops/status-and-backup.md](docs/ops/status-and-backup.md).

CI (GitHub Actions) chạy tất cả những bước trên (trừ kiểm thử tải, đo khởi động và diễn tập sao lưu, chạy tay mỗi quý), kể cả kiểm thử đầu-cuối với hạ tầng Docker và diễn tập TUF, ở mỗi lần push.

## Gửi email tới hòm thư thật

Mặc định, Kratos (email xác minh, khôi phục mật khẩu) và feco-server (thông báo, cảnh báo) gửi email tới Mailpit. Để gửi tới hòm thư thật, cả hai dùng chung một máy chủ SMTP:

1. **Gmail (cho máy dev)**:
   - bật xác minh 2 bước cho tài khoản Google;
   - vào https://myaccount.google.com/apppasswords, tạo một "mật khẩu ứng dụng" (16 ký tự).
2. Tự ghi URL SMTP vào tệp bí mật `.dev/smtp-url` (thư mục `.dev` không được commit). Không gửi mật khẩu này cho ai, kể cả vào khung chat:

   ```text
   smtps://<tên-tài-khoản>%40gmail.com:<mật-khẩu-ứng-dụng-viết-liền>@smtp.gmail.com:465
   ```

   Ghi địa chỉ gửi (trùng tài khoản Gmail) vào `.dev/smtp-from`:

   ```text
   <tên-tài-khoản>@gmail.com
   ```

3. Khởi động lại Kratos (`scripts/dev-local.sh kratos`) và feco-server.
   - Kratos đọc `FECO_SMTP_URL` hoặc `.dev/smtp-url`, và `FECO_EMAIL_FROM` hoặc `.dev/smtp-from`.
   - feco-server đọc `FECO_SMTP_URL_FILE=.dev/smtp-url` và `FECO_EMAIL_FROM`.
   - Có tệp `.dev/smtp-url` thì Kratos in "Kratos gửi email qua SMTP đã cấu hình" lúc khởi động.
4. Đăng ký một tài khoản bằng email thật: email xác minh tới hòm thư đó, không vào Mailpit.

Giới hạn:
- Gmail cho gửi khoảng 500 email mỗi ngày.
- Liên kết xác minh trỏ về địa chỉ công khai của Kratos (`http://127.0.0.1:4433` trên máy dev), nên chỉ mở được trên chính máy đó.
- Khi triển khai thật, dùng tên miền gửi riêng có SPF, DKIM, DMARC (kiểm bằng `feco-catalog mail-dns`). Đặt `COURIER_SMTP_CONNECTION_URI`, `COURIER_SMTP_FROM_ADDRESS` cho Kratos và `FECO_SMTP_URL_FILE` cho feco-server.

## Xử lý sự cố

| Triệu chứng | Cách xử lý |
|---|---|
| Windows báo `LINK : fatal error LNK1104: cannot open file ...` khi biên dịch | Đường dẫn thư mục dự án quá dài (giới hạn 260 ký tự). Đặt thư mục build ngắn hơn: `export CARGO_TARGET_DIR=C:/t/feco` (PowerShell: `$env:CARGO_TARGET_DIR="C:\t\feco"`), hoặc clone vào thư mục nông như `C:\src` |
| `feco-server` thoát ngay với "Cấu hình không hợp lệ" | Đọc thông báo: nó chỉ đúng biến bị thiếu hoặc sai. Ở dev chỉ cần `FECO_ENV=dev` |
| "Không cập nhật được lược đồ cơ sở dữ liệu" | PostgreSQL chưa chạy hoặc sai `FECO_DATABASE_URL` |
| Trang báo "Dịch vụ tài khoản đang tạm gián đoạn" | Kratos chưa chạy; kiểm tra `curl http://127.0.0.1:4433/health/ready` |
| Không nhận được email | Mặc định mọi email dev nằm ở Mailpit http://127.0.0.1:8025. Muốn gửi tới hòm thư thật, xem [Gửi email tới hòm thư thật](#gửi-email-tới-hòm-thư-thật) |
| F-Launcher báo "Dữ liệu phân phối của máy chủ đã hết hạn" | Metadata `timestamp` của kho TUF chỉ có hạn 1 giờ. Chạy máy chủ với `FECO_TUF_REPO` và `FECO_TUF_SIGN_KEYS` để máy chủ tự ký lại (lúc khởi động và mỗi 5 phút). Không có máy chủ thì chạy tay `cargo run -p feco-tuf -- refresh --repo .dev/tuf/repo --sign .dev/tuf/keys/snapshot.pk8,.dev/tuf/keys/timestamp.pk8`. Nếu giờ trên máy tính sai, chỉnh lại giờ |
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
| `crates/feco-codescan` | Quét mã nguồn (bí mật lộ, mã đáng ngờ, lỗ hổng thư viện qua OSV.dev, giấy phép); lệnh `feco-codescan` |
| `services/feco-server` | Dịch vụ HTTP (axum): trang tài khoản, OAuth login/consent, `/v1/me`, webhook, xuất dữ liệu, F-Store, đơn xây app (`/orders`); công cụ `feco-catalog`; `migrations/` là lược đồ PostgreSQL; `templates/` là giao diện |
| `clients/f-builder` | F-Builder: công cụ tự xây app bằng coding agent, phát triển từ Craft Anything 0.6.0 (Cargo workspace riêng; xem `clients/f-builder/README.md` và `NOTICE`) |
| `clients/feco-mobile` | F-Launcher di động (Flutter, Android 10+, iOS 18+) |
| `clients/feco-launcher` | F-Launcher: thư viện (đăng nhập, cài/cập nhật/gỡ qua TUF, mã khởi chạy, API cục bộ) và giao diện egui |
| `tools/feco-demo-app` | App con mẫu: đăng nhập một lần và gọi `/v1/me`; khi được launcher mở thì đăng nhập bằng mã khởi chạy |
| `sdk/` | F-SDK: `rust` (crate `fsdk`), `c` (C ABI `fsdk_c` và header), `python` (binding `ctypes`), `typescript` (gói `@f-eco/sdk`) |
| `examples/standard-oidc` | App tích hợp bằng thư viện OIDC chuẩn (Authlib), không dùng F-SDK |
| `examples/apps` | App mẫu dùng F-SDK: `f-notes` (Ghi chú), `f-timer` (Bấm giờ), phần dùng chung `sample-common` |
| `tools/f-dev` | CLI cho nhà phát triển: `f-dev validate`, `f-dev pack`, `f-dev publish` |
| `tools/feco-license-check` | Kiểm tra license dịch vụ và ảnh container |
| `infra/` | Hạ tầng dev: `docker-compose.yml`, cấu hình Kratos (schema, mẫu email) và Hydra |
| `scripts/` | `dev-local.sh`: chạy hạ tầng không cần Docker; `tuf-drill.sh`: diễn tập chuỗi tin cậy TUF; `dev-launcher.sh`: kho TUF và client OAuth để dùng thử launcher; `dev-apps.sh`: đưa app mẫu, app mã nguồn mở từ GitHub, app web ThesisAI và Mentor vào kho dev; `scan-integration.sh`: quét repo bên ngoài trước khi tích hợp |
| `integrations/` | Tích hợp app bên ngoài: `thesisai/` (bản vá "Đăng nhập bằng F-Eco" cho ThesisAI, hướng dẫn, nguồn gốc); `mentor/` (bản vá "Đăng nhập bằng F-Eco" cho Mentor, script dựng app desktop, hướng dẫn, nguồn gốc) |
| `docs/security/` | Quy trình xử lý sự cố, quản lý khóa TUF, biên bản diễn tập |
| `third_party/` | Khai báo license và ngoại lệ đã duyệt |
| `openspec/` | Yêu cầu, thiết kế, kế hoạch |

## Giấy phép

Phát hành theo lựa chọn của bạn một trong hai giấy phép: MIT ([LICENSE-MIT](LICENSE-MIT)) hoặc Apache License 2.0 ([LICENSE-APACHE](LICENSE-APACHE)).
Trừ khi bạn nói rõ khác đi, mọi đóng góp bạn gửi vào dự án được cấp phép theo cả hai giấy phép trên, không kèm điều khoản bổ sung.
