# Nhật ký thay đổi

## 1.14.0 (21/11/2026)

Thay đổi OpenSpec `app-update-notices`: rà soát việc phát hiện app con có bản mới và báo người dùng trên F-Launcher máy tính, web và điện thoại. Kèm bản sửa lỗ hổng Next.js của ThesisAI (QUESTIONS.md 109).

### Thông báo cập nhật app

- **Đúng lúc**: thông báo gửi khi một phiên bản được phát hành trên kênh ổn định cho mọi thiết bị (100%). Phát hành 10%, kênh beta, tạm dừng và đổi phần trăm không còn gửi thông báo.
- **Một lần**: mỗi phiên bản chỉ được báo một lần (bảng `app_update_notices`, migration `0022`). Phát hành lại cùng phiên bản không gửi lần nữa. App lên nhiều bản liền thì chỉ báo bản mới nhất.
- **Đúng người**: chỉ người đang cài bản cũ hơn. Người đã nhận bản mới qua phát hành dần và người đã gỡ app không nhận.
- **Phát hành bằng `feco-catalog release`**: việc định kỳ 10 phút gửi thông báo, trước đây không có.

### "Ứng dụng của tôi" trên web và điện thoại

- "Có bản mới" chỉ tính bản phát hành trên kênh ổn định cho mọi thiết bị. Trước đây bản beta, bản 10% và bản tạm dừng cũng làm hiện nhãn này.
- `DELETE /v1/catalog/installs/{app}`: F-Launcher báo đã gỡ app. App rời danh sách (trừ app đã mua) và không còn nhận thông báo cập nhật.

### F-Launcher máy tính

- Báo máy chủ phiên bản đang cài sau khi cập nhật một app, cập nhật tất cả, tự cập nhật và quay lại bản trước. Trước đây chỉ báo khi cài hoặc mua, nên web và điện thoại vẫn hiện "Có bản mới" sau khi đã cập nhật.
- Báo máy chủ khi gỡ app.
- Huy hiệu Thư viện, nút "Cập nhật tất cả (N)" và tự cập nhật không tính bản đã "Bỏ qua bản này", khớp với hộp thoại và dải thông báo. Mục "Cập nhật lên" của từng app vẫn cài được bản đó.

### Điện thoại

- Quay lại app từ nền thì tải lại "Ứng dụng của tôi" và thông báo, trước đây chỉ tải lại tin nhắn.
- Tab "Ứng dụng của tôi" có huy hiệu số app có bản mới, kèm dòng "Số app có bản mới: N. Mở F-Launcher trên máy tính để cập nhật." (bốn thứ tiếng).
- App web không còn nhãn "Có bản mới", vì app web luôn mở bản mới nhất.

### ThesisAI: Next.js 16.4.6

- `integrations/thesisai/thesisai-fe.patch` nâng Next.js từ 16.4.0 lên 16.4.6. Bản này vá hai lỗ hổng CRITICAL chạy mã từ xa không cần đăng nhập (GHSA-2464-fhr4-w9xw, GHSA-673q-mjm6-r5xr) và GHSA-p3m5-59mp-jv5r.
- Lockfile cũng nâng `undici`, `body-parser`, `systeminformation`, `hono`, `fast-uri`, `qs` trong khoảng phiên bản cho phép.
- Quét lại: high từ 10 xuống 1, medium từ 19 xuống 1. Còn `zod` 3 của công cụ `shadcn`, xem `integrations/thesisai/SCAN.md`.
- Job CI mới `thesisai-fe`: áp bản vá lên commit đã ghi, `npm ci`, `next build`.

### Kiểm thử

- **E2E**:
  - `app_update_notices`: kênh beta, người đã có bản mới, phát hành bằng công cụ vận hành, gỡ app, app đã mua;
  - `developer_portal_end_to_end`: phát hành 10%, tạm dừng, 100%, phát hành lại, qua cổng nhà phát triển thật.
- **F-Launcher**: `update_all_leaves_skipped_versions`, `reports_installed_version_and_uninstall`.
- **Điện thoại**: widget test `scenario_bao_ban_moi_tren_dien_thoai`.

## 1.13.0 (20/11/2026)

Thay đổi OpenSpec `oidc-login-and-token-reuse`: xong phần lập trình của task 2.3 và 2.5 trong `add-f-ecosystem-platform`.

### Đăng nhập bằng Google và GitHub

- Kratos bật phương thức `oidc`. Nhà cung cấp khai báo khi triển khai bằng `SELFSERVICE_METHODS_OIDC_CONFIG_PROVIDERS`; client ID và secret không nằm trong repo.
- Nút "Đăng nhập bằng Google/GitHub" và "Đăng ký bằng …" bốn thứ tiếng.
- **Email**:
  - chỉ nhận email nhà cung cấp đã xác minh, và tính là đã xác minh trên F-Eco (`infra/kratos/oidc/mapper.jsonnet`);
  - email chưa xác minh thì phải nhập email và xác minh như thường.
- **Liên kết tài khoản**: email trùng tài khoản có sẵn thì phải đăng nhập bằng mật khẩu cũ một lần, không tự gộp.
- **Sau khi đăng nhập**: `/auth/after-login` ghi nhật ký kiểm toán `auth.login` (phương thức `oidc`), báo thiết bị mới, hủy yêu cầu xóa tài khoản đang chờ. Mỗi phiên một lần; chỉ chuyển tới đường dẫn trong F-Eco.

### Cảnh báo refresh token bị dùng lại

- **`POST /oauth2/token` của F-Eco**:
  - chuyển nguyên yêu cầu tới Hydra;
  - tài liệu discovery của Hydra trỏ `token_endpoint` về đây (`webfinger.oidc_discovery.token_url`);
  - F-Launcher máy tính đổi token qua đây.
- **F-Eco chỉ lưu băm SHA-256** của refresh token (`0021_refresh_tokens.sql`), xóa sau 31 ngày.
- **Khi Hydra từ chối một refresh token đã biết, còn hạn**:
  - người dùng nhận một thông báo và email "Bảo mật" cho mỗi chuỗi phiên, ghi app, thời điểm, thiết bị và IP;
  - có nhật ký kiểm toán `auth.refresh_token_reused`;
  - Hydra đã thu hồi cả chuỗi; các thiết bị khác không bị đăng xuất.
- Gỡ quyền app ở "Ứng dụng đã kết nối" không gây cảnh báo.

### Kiểm thử và CI

- **E2E**:
  - `external_login_with_fake_oidc_provider`: nhà cung cấp OIDC giả chạy trong test, ID token ES256;
  - `refresh_token_reuse_alert`.
- CI chạy Kratos với `infra/docker-compose.e2e.yml` (mạng của máy, `kratos.e2e-oidc.yml`) và bắt buộc test OIDC (`FECO_E2E_OIDC=1`).
- `scripts/dev-local.sh kratos` nhận `FECO_KRATOS_E2E_OIDC=1`.

## 1.12.0 (19/11/2026)

Thay đổi OpenSpec `chat-and-source-scan`: chat giữa khách hàng và nhà phát triển, quét mã nguồn trước khi duyệt và trước khi tích hợp.

### Chat

- **Hội thoại trực tiếp**:
  - nút **Nhắn nhà phát triển** ở trang app trong F-Store và ở danh sách nhà phát triển trong Dự án;
  - mỗi cặp khách hàng và nhà phát triển có một hội thoại;
  - nhà phát triển chặn được; ai cũng báo cáo được (Hỗ trợ xử lý ở `/admin/chat-reports`).
- **Hội thoại dự án** thay luồng tin nhắn cũ; tin cũ được chuyển sang và tính là đã đọc. Quản lý dự án gửi tin trong hội thoại khi xử lý tranh chấp.
- **Tin nhắn**:
  - tối đa 5.000 ký tự;
  - một tệp đính kèm tối đa 20 MB, quét mã độc trước khi tải được; tệp có mã độc bị chặn và quản trị được báo;
  - đánh dấu đã đọc, số chưa đọc ở thanh trên cùng.
- **Giới hạn**: 20 hội thoại mới mỗi 24 giờ, 30 tin mỗi phút.
- **Tin đến ngay** trên web (Server-Sent Events, PostgreSQL `LISTEN/NOTIFY` giữa các máy chủ).
- **F-Launcher máy tính**: mục "Tin nhắn" ở thanh bên (có huy hiệu), danh sách, luồng tin, gửi tin, hỏi tin mới mỗi 5 giây.
- **F-Launcher di động**: tab "Tin nhắn" (sáu tab); phiên bản app lấy từ một hằng số, test so với `pubspec.yaml`.
- **API** `/v1/chat/*` cho launcher.
- **Thông báo**:
  - nhóm "Tin nhắn" (bật mặc định, tắt email được);
  - chỉ ghi người gửi, không chép nội dung, tối đa một lần mỗi 15 phút cho mỗi hội thoại;
  - thông báo tin nhắn dự án cũng không còn trích nội dung.

### Quét mã nguồn

- **Crate và lệnh mới `feco-codescan`** tìm:
  - bí mật lộ (khóa riêng, khóa đám mây, token);
  - mã đáng ngờ (đào tiền ảo, đọc kho mật khẩu trình duyệt, móc bàn phím, tắt Defender, tải rồi chạy);
  - thư viện có lỗ hổng: đọc lockfile Rust, npm, pnpm, yarn, Python, Go và tra OSV.dev; chỉ gửi tên và phiên bản thư viện;
  - giấy phép không khớp manifest.

  Bí mật trong báo cáo được che.
- **Duyệt app**:
  - phiên bản kèm mã nguồn qua trang app, `[source]` trong `f-app.toml`, `PUT /v1/dev/versions/{id}/source` hoặc `f-dev publish --source`;
  - quét mã nguồn chạy sau khi gói sạch:
    - phát hiện nghiêm trọng thì từ chối (`source_scan`);
    - phát hiện mức cao thì chuyển duyệt tay;
  - báo cáo hiện cho nhà phát triển và người duyệt.
- **Trước khi tích hợp app bên ngoài**:
  - `scripts/scan-integration.sh`;
  - `integrations/<app>/SCAN.md` và `scan-allow.toml` cho Mentor và ThesisAI;
  - job CI `integration-scan`;
  - `build-desktop.sh` quét Mentor trước khi dựng.
- **Cấu hình mới**: `FECO_OSV_URL`, `FECO_OSV`, `FECO_SOURCE_FETCH_URL`.
- **Kiểm thử**: E2E `e2e_chat`, `e2e_source_scan` (cơ sở dữ liệu riêng); `FECO_E2E_DATABASE_URL` cho `e2e_auth`;
  máy chủ E2E nâng hạn mức theo IP; integration test iOS có hạn 20 phút mỗi lần thử để Simulator treo không giữ CI 6 giờ.

## 1.11.0 (19/11/2026)

Thay đổi OpenSpec `mentor-integration`: Mentor trong F-Eco.

- **Mentor** (https://github.com/katD2707/agent-mentor-maximum-experience), ứng dụng học tập cho học sinh từ lớp 1 tới đại học và cho giáo viên. Mentor vào F-Store và F-Launcher dưới hai dạng:
  - app web (mọi máy, kể cả F-Launcher di động);
  - app desktop Windows.
- **Đăng nhập Mentor bằng tài khoản F-Eco** (`integrations/mentor/`, bản vá cho gateway và app Mentor):
  - OIDC authorization code + PKCE, client phía máy chủ, thư viện Authlib;
  - chỉ quay về địa chỉ app được phép, trả mã dùng một lần sống 120 giây; token phiên không nằm trong địa chỉ;
  - lần đầu đăng nhập tạo tài khoản học sinh; giáo viên do quản trị Mentor nâng vai trò (`set-role`);
  - đăng nhập bằng mật khẩu của Mentor vẫn giữ.
- **`scripts/dev-apps.sh mentor`**: client OAuth, target web (mở thẳng đăng nhập F-Eco), target desktop khi có `MENTOR_DESKTOP_ZIP`, logo, danh mục F-Store.
- **CI và bản phát hành**:
  - job `mentor-desktop` lấy Mentor đúng commit, áp bản vá, chạy pytest và vitest của Mentor, rồi dựng app desktop bằng Tauri;
  - bản phát hành kèm `mentor-<phiên bản>-windows-x64.zip`.
- **Thư viện của máy chủ** (cảnh báo RustSec mới):
  - `lettre` 0.11.23 lên 0.11.24 (RUSTSEC-2026-0560 tới 0565);
  - bỏ `humantime` (ngừng bảo trì, RUSTSEC-2026-0542), đọc và ghi thời điểm RFC 3339 bằng thư viện `time` sẵn có.
- **F-Builder**: `wasmtime` 49.0.5 lên 49.0.6 (RUSTSEC-2026-0566 tới 0570).

## 1.10.0 (17/11/2026)

Thay đổi OpenSpec `mobile-launcher`: F-Launcher trên điện thoại.

- **Ứng dụng mới `clients/feco-mobile`** (Flutter 3.50), gói `vn.feco.launcher`, biểu tượng là logo F-Launcher.
  - Android 10 (API 29) trở lên, nhắm Android 16 (API 36); iOS 18 trở lên. Theo số liệu tháng 10–11/2026, các phiên bản này chiếm khoảng 95% máy Android và 98% iPhone đang dùng.
  - Đăng nhập bằng tài khoản F-Eco qua trình duyệt hệ thống (OIDC, PKCE, AppAuth). Refresh token trong Keychain hoặc Keystore; tự làm mới khi API trả 401.
  - Năm tab: Ứng dụng của tôi, Cửa hàng (tìm, lọc nhóm, mở và thêm app web, app desktop ghi "Dùng trên máy tính qua F-Launcher"), Ví (số dư, mua app, báo số còn thiếu, mở trang nạp và rút), Thông báo, Tài khoản.
  - Bốn thứ tiếng: tiếng Việt, English, Français, 日本語.
  - Chế độ dữ liệu mẫu `--dart-define=FECO_DEMO=true`.
- **Máy chủ**:
  - `FECO_MOBILE_CLIENT_ID`: client di động là app bên thứ nhất, không hỏi đồng ý, gọi được API dành cho launcher (trừ mã khởi chạy);
  - API mới `GET /v1/library`;
  - `scripts/dev-launcher.sh setup` tạo client di động.
- **CI**: `flutter analyze`, unit test và widget test; integration test trên Android emulator API 29 và 36 và trên iOS Simulator.
- **Bản phát hành** kèm `feco-mobile-<phiên bản>.apk` (ký bằng khóa debug, chỉ để cài thử) và `feco-mobile-<phiên bản>-ios-simulator.zip`.

## 1.9.0 (16/11/2026)

Thay đổi OpenSpec `production-readiness`: chuẩn bị phát hành production.

### Tài khoản quản trị viên mặc định

- Máy chủ luôn bảo đảm tài khoản `FECO_ADMIN_EMAIL` tồn tại, hoạt động, email đã xác minh, đủ năm vai trò quản trị. Việc kiểm chạy lúc khởi động và mỗi 10 phút.
- Tài khoản này không bị hạ vai trò, bỏ vai trò Quản trị hệ thống, hay xóa.
- Mật khẩu ban đầu lấy từ tệp `FECO_ADMIN_PASSWORD_FILE` (tối thiểu 12 ký tự). Dev tự sinh vào `.dev/admin-password`. Production thiếu biến thì không khởi động.

### Quản trị viên chuyển F-Coin

- Ở trang Ví F-Coin của bảng quản trị, vai trò Tài chính chuyển F-Coin cho một người dùng đã xác minh email. Có hai nguồn:
  - **Ví của tôi**;
  - **Quỹ nền tảng**: phát hành thêm F-Coin; từ 1.000.000 ₫ cần một quản trị viên Tài chính khác duyệt.
- Bắt buộc lý do; không chuyển cho chính mình; gửi lại biểu mẫu không chuyển hai lần.
- Người nhận có thông báo và email; nhật ký kiểm toán ghi mọi lần chuyển. Hai loại bút toán mới `admin_transfer`, `admin_grant` (migration 0018).

### Sửa lỗi bảo mật và chức năng (rà soát trước production)

Rà soát ba vùng: xác thực và web, luồng tiền, launcher và chuỗi tin cậy TUF.

- **Xác thực và web**:
  - liên kết bàn giao dự án chỉ nhận http/https (chặn XSS `javascript:`);
  - thao tác tài khoản nhà phát triển kiểm vai trò nền tảng;
  - chỉ tin `X-Forwarded-For` từ proxy khai báo ở `FECO_TRUSTED_PROXIES`;
  - chặn chuyển hướng mở qua ký tự điều khiển;
  - chặn xóa tài khoản khi còn đơn, dự án, ký quỹ, số dư hoặc lệnh rút;
  - OAuth tôn trọng `prompt=login`, `max_age` và gửi `acr`/`amr`;
  - chỉ cấp hoặc duyệt vai trò cho email đã xác minh;
  - cookie CSRF dùng tiền tố `__Host-` trên https;
  - kiểm duyệt viên không tự duyệt app của tài khoản mình là thành viên;
  - API chỉ nhận access token, không nhận refresh token;
  - thêm thử thách chống bot Cloudflare Turnstile (`FECO_CHALLENGE_PROVIDER=turnstile`), nhờ đó production khởi động được.
- **Luồng tiền**:
  - lệnh hoàn chưa rõ kết quả (lỗi đường truyền, chữ ký phản hồi sai) không bị coi là thất bại;
  - chuyển trạng thái hoàn tiền có điều kiện, không gửi hay kết thúc hai lần;
  - quản trị viên không tự duyệt lệnh rút của mình, và lệnh của ví đã khóa không được duyệt;
  - ngưỡng hai người duyệt tính theo tổng rút 24 giờ;
  - mỗi lần đổi tỷ giá tối đa 20%;
  - lệnh nạp hết hạn sau 30 phút hoặc khi tỷ giá đổi;
  - callback lặp lại làm nốt việc cộng F-Coin hoặc bắt đầu đơn nếu lần trước dừng giữa chừng;
  - giao dịch nạp chỉ hoàn qua lệnh rút;
  - chấp nhận báo giá một lần;
  - bên của dự án không xử lý tranh chấp;
  - không xuất hóa đơn dịch vụ xây app cho lệnh nạp.
- **Launcher và TUF**:
  - thu hồi khẩn cấp được cả target đã gỡ; ký TUF trước khi đổi cơ sở dữ liệu;
  - khóa tệp khi ghi kho TUF;
  - logo PNG tối đa 1024 px;
  - chặn gói zip có hai mục trùng tên trên NTFS;
  - tệp chạy chỉ gồm `[A-Za-z0-9._/-]`, đổi tệp chạy hay chế độ terminal thì phải duyệt thủ công;
  - mã khởi chạy chỉ cấp cho app người dùng được dùng (đã mua, app riêng của mình);
  - quét gói mở zip một lần, tối đa 10.000 mục và 256 MB mỗi tệp;
  - chưa đồng bộ quá 7 ngày thì không mở app;
  - kiểm trùng tên target;
  - so địa chỉ bằng `Url`;
  - callback OAuth bỏ qua `state` sai;
  - launcher bản release bắt buộc `FECO_LAUNCHER_SERVER` và `FECO_LAUNCHER_HYDRA`.

### Khác

- CI tự kiểm gitleaks bắt được khóa giả (task 1.3); task 2.4 đã xác minh.

### Ghi chú nâng cấp

- Đặt `FECO_ADMIN_EMAIL`, `FECO_ADMIN_PASSWORD_FILE` ở staging và production.
- Production cần `FECO_CHALLENGE_PROVIDER=turnstile` cùng `FECO_TURNSTILE_SITE_KEY`, `FECO_TURNSTILE_SECRET_KEY`. Nếu chạy sau reverse proxy, đặt `FECO_TRUSTED_PROXIES`.
- Launcher bản release cần `FECO_LAUNCHER_SERVER` và `FECO_LAUNCHER_HYDRA`.
- Migration 0018 chạy tự động.

## 1.8.0 (16/11/2026)

Thay đổi OpenSpec `role-at-registration`.

- **Chọn vai trò khi đăng ký**: trang đăng ký có ô Vai trò với năm lựa chọn:
  - Người dùng phổ thông (mặc định);
  - Khách hàng cá nhân;
  - Khách hàng doanh nghiệp;
  - Lập trình viên cá nhân;
  - Công ty phát triển phần mềm.
- **Thông tin tổ chức**: Khách hàng doanh nghiệp và Công ty phát triển phần mềm phải nhập tên pháp lý và mã số thuế. Thiếu hoặc sai thì không tạo tài khoản; trang đăng ký hiện lỗi dưới ô sai và giữ giá trị đã nhập (trừ mật khẩu). Vai trò lạ (ví dụ `admin`) bị từ chối.
- **Yêu cầu chờ duyệt**: vai trò khác Người dùng phổ thông tạo yêu cầu đang chờ (ghi "Chọn khi đăng ký"), và quản trị viên nhận cảnh báo. Trong lúc chờ, tài khoản là Người dùng phổ thông.
- **Trang "Đã gửi email"**: nói vai trò đã chọn cần quản trị viên duyệt. Trang giống hệt nhau dù email mới hay đã có tài khoản; email đã có tài khoản không tạo yêu cầu.
- **Trang Phân quyền**:
  - mỗi yêu cầu ghi nguồn ("Chọn khi đăng ký" hoặc "Trang Vai trò") và trạng thái xác minh email;
  - duyệt, hoặc từ chối kèm lý do; người dùng nhận thông báo và email.
- Các trường vai trò là trường của F-Eco: không gửi tới Kratos, không đổi schema danh tính, không có migration mới.

## 1.7.0 (16/11/2026)

Thay đổi OpenSpec `launcher-sidebar-and-web-apps`.

### Thanh bên của F-Launcher

- Nút ẩn/hiện cạnh logo và phím tắt Ctrl+B: thanh bên thu gọn còn 56 px, chỉ có icon.
- Ở chế độ thu gọn:
  - tooltip ghi tên từng mục;
  - số thông báo chưa đọc và số bản cập nhật hiện thành huy hiệu ("9+" khi trên 9);
  - mọi chức năng vẫn còn: đồng bộ, F-Eco trên web, bảng quản trị, cổng nhà phát triển, ví, đăng nhập, đăng xuất.
- Launcher nhớ lựa chọn trong `settings.json` (`sidebar_collapsed`); cửa sổ hẹp hơn 720 px thì thanh bên tự thu gọn.

### App web

- **Metadata TUF**: thêm trường `web_url`, ký cùng target nền tảng `web`.
  - `feco-tuf publish --web-url` chỉ nhận `https://`, hoặc `http://localhost` và `http://127.0.0.1`.
  - Địa chỉ có tên đăng nhập hay mật khẩu bị từ chối.
- **F-Launcher**:
  - hiện app web trên mọi hệ điều hành, nhãn "App web";
  - "Thêm" ghi app vào Thư viện; "Mở" mở địa chỉ bằng trình duyệt, không cấp mã khởi chạy;
  - app có gói cho máy đang dùng thì dùng gói đó.
- **F-Store**:
  - cột `catalog_apps.web_url` (migration `0017_web_apps.sql`);
  - trang app có nút "Mở ứng dụng web", nền tảng ghi "Web";
  - bộ lọc Windows, macOS, Linux vẫn hiện app web;
  - trang "Ứng dụng của tôi" có nút "Mở" cho app web.
- **Đăng nhập**: app web dùng client OAuth phía máy chủ (client secret, PKCE, `sub` pairwise). E2E kiểm cả trường hợp sai secret và địa chỉ chuyển hướng lạ.

### ThesisAI

- ThesisAI (luyện bảo vệ đồ án tốt nghiệp với hội đồng AI) có trong F-Store và F-Launcher dưới dạng app web, kèm logo của ThesisAI.
- `integrations/thesisai/`: bản vá "Đăng nhập bằng F-Eco" cho hai repo ThesisAI.
  - Backend: Authlib, cột `feco_subject`.
  - Frontend: trang `/auth/feco` và nút đăng nhập.
  - Kèm hướng dẫn chạy cùng F-Eco và ghi nguồn gốc.
- `scripts/dev-apps.sh thesisai`: client OAuth, gói app web, danh mục; in biến môi trường cho backend ThesisAI.

### Sửa lỗi

- `scripts/dev-apps.sh`: trường `icon_file` trong tệp danh mục mất dấu nháy (JSON hỏng) khi app có logo.

### Ghi chú nâng cấp

- Migration `0017_web_apps.sql` chạy tự động khi máy chủ khởi động.
- Cổng nhà phát triển chưa nhận app web; app web được đưa vào bằng `feco-tuf` và `feco-catalog` (QUESTIONS.md 93).

## 1.6.0 (15/11/2026)

Gồm hai thay đổi OpenSpec: `fcoin-wallet-and-marketplace` và `launcher-logos-and-updates`.

### F-Coin và chợ phần mềm

- **Sáu vai trò nền tảng**:
  - Người dùng phổ thông;
  - Khách hàng cá nhân;
  - Khách hàng doanh nghiệp;
  - Lập trình viên cá nhân (lập trình viên có từ trước giữ nguyên quyền);
  - Công ty phát triển phần mềm;
  - Quản trị viên.

  Trang Vai trò (`/account/role`) để xin vai trò. Vai trò doanh nghiệp và công ty khai tên pháp lý và mã số thuế. Quản trị hệ thống duyệt ở trang Phân quyền.
- **Ví F-Coin** cho mọi tài khoản:
  - sổ cái ghi kép, số dư không âm, idempotent; lệnh `feco-catalog wallet-check` kiểm sổ cái;
  - tỷ giá do Tài chính đặt, kèm lý do và lịch sử; ban đầu 1 F-Coin = 1.000 ₫;
  - nạp qua ZaloPay, VNPAY, MoMo;
  - rút về giao dịch nạp (qua cổng) hoặc chuyển khoản ngân hàng: F-Coin bị giữ ngay, lệnh từ 10.000.000 ₫ cần hai người duyệt, số tài khoản được mã hóa;
  - lịch sử có lọc và phân trang; thống kê thu chi 12 tháng.
- **Mua app bằng F-Coin**:
  - nhà phát triển đặt giá ở Cổng nhà phát triển;
  - F-Store hiện giá F-Coin kèm số tiền quy đổi;
  - người mua nhận quyền sử dụng `license`; người bán nhận F-Coin trừ phí nền tảng (mặc định 0).
- **Dự án phần mềm**:
  - hồ sơ nhận việc, khách hàng gửi yêu cầu, thương lượng giá và thời hạn;
  - ký quỹ F-Coin, bàn giao, yêu cầu sửa, chấp nhận thì chuyển F-Coin cho người nhận việc;
  - tranh chấp do Quản lý dự án giải ngân, hoàn hoặc chia;
  - hợp đồng: doanh nghiệp hoặc công ty lập từ mẫu, hai bên ký bằng họ tên, lưu SHA-256; dự án có hợp đồng thì phải ký đủ mới ký quỹ được.
- **Bảng quản trị**:
  - trang Ví F-Coin: số liệu, biểu đồ 30 ngày, tỷ giá, phí, hàng đợi lệnh rút, khóa ví, tra giao dịch;
  - trang Dự án;
  - yêu cầu vai trò hiện vai trò xin và thông tin doanh nghiệp.
- **F-Launcher**:
  - số dư F-Coin ở thanh bên (bấm vào thì mở trang ví);
  - nút "Mua · N F-Coin" cho app có giá, hộp thoại xác nhận (số dư sau khi mua, hoặc số còn thiếu kèm nút Nạp F-Coin), mua xong cài ngay;
  - tên sáu vai trò.
- **API mới**: `GET /v1/wallet`, `POST /v1/wallet/purchases` (chỉ token của launcher); mã lỗi `insufficient_funds`, `wallet_frozen`, `price_changed`, `already_owned`...

### Logo, cửa sổ, đồng bộ tính năng, gợi ý cập nhật

- **Logo**:
  - logo F-Launcher mới: chữ F nghiêng ba màu, dùng cho biểu tượng cửa sổ, thanh bên và biểu tượng trang web;
  - logo app được ký trong metadata TUF (`feco-tuf publish --icon`, `feco-tuf set-icon`) và hiện trong launcher và F-Store (`/store/{id}/icon.png`);
  - logo bottom, kibi, eframe template lấy từ repo gốc; logo Ghi chú và Bấm giờ do F-Eco thiết kế.
- **Cửa sổ**: launcher mở giữa màn hình. Cài đặt và chi tiết app mở giữa màn hình và là cửa sổ chặn: cửa sổ chính bị phủ mờ, bấm vào thì đưa cửa sổ con lên trước.
- **Gợi ý cập nhật**:
  - sau mỗi lần đồng bộ, launcher báo bản mới của app đã cài: phiên bản cũ → mới, ghi chú phát hành;
  - các nút Cập nhật, Bỏ qua bản này, Cập nhật tất cả, Để sau;
  - Thư viện có băng nhắc.
- **Đồng bộ launcher và web**:
  - web có trang "Ứng dụng của tôi" (`/library`, báo bản mới);
  - launcher có menu "F-Eco trên web" (hồ sơ, ví, ứng dụng của tôi, dự án, đơn xây app, thông báo).

### Ghi chú nâng cấp

- Migration `0015_fcoin.sql` và `0016_app_icons.sql` chạy tự động khi máy chủ khởi động.
- Gắn logo cho app dev: `scripts/dev-apps.sh icons`.
- Rút F-Coin đổi được ra tiền thật, cần ý kiến pháp lý trước khi mở cho người dùng thật (QUESTIONS.md 85). `FECO_WALLET_WITHDRAWALS=false` tắt việc tạo lệnh rút mới.

## 1.5.0 (15/11/2026)

Thay đổi OpenSpec `launcher-windows-and-web-profile`, cùng nhóm task 15 (yêu cầu chung và ra mắt).

### F-Launcher

- Nút công cụ dùng icon kèm tooltip thay cho chữ: Cài đặt, Lưới, Danh sách, Xóa bộ lọc, Đồng bộ,
  Đăng xuất, Bảng quản trị, Cổng nhà phát triển, menu thao tác, Mã nguồn, Website, Quay lại, Gỡ, app
  trước và sau. Nút chính (Mở, Cài, Cập nhật, Đăng nhập) và mục thanh bên có icon đi kèm chữ. Icon
  vẽ bằng nét, đổi màu theo phong cách; không thêm phông hay ảnh.
- Cài đặt và trang chi tiết app mở trong cửa sổ riêng của hệ điều hành. Mỗi app một cửa sổ (tối đa
  6), bấm lại thì cửa sổ đó hiện lên trước; Esc đóng cửa sổ; hộp thoại cài, gỡ hiện ở cửa sổ nơi
  bấm.
- Ô lưới trong Thư viện và Cửa hàng cùng kích thước, xếp liền nhau và trải đều chiều rộng.
- Đo thời gian khởi động: `FECO_LAUNCHER_STARTUP_PROBE`, `scripts/measure-launcher-startup.ps1`.

### Web

- Trang hồ sơ cho người đã đăng nhập: ảnh đại diện chữ cái, vai trò, ngày tham gia, thẻ số liệu
  (ứng dụng đã kết nối, thiết bị, đơn hàng, thông báo chưa đọc), lối tắt bảo mật (kèm trạng thái
  xác thực hai lớp), quyền riêng tư, mua sắm, công cụ theo vai trò.
- Tiếng Pháp (QUESTIONS 79): nút FR ở thanh trên cùng, tự nhận trình duyệt tiếng Pháp, đủ 585 chuỗi giao diện web.
- Trang trạng thái công khai `/status` và `/status.json`: tự kiểm tra tài khoản, đăng nhập cho app
  và cơ sở dữ liệu mỗi phút; hai lần lỗi liên tiếp mở sự cố; tỉ lệ sẵn sàng 24 giờ và 30 ngày.
- Dashboard quản trị đếm người dùng tối đa mỗi 10 phút (QUESTIONS 80).

### Vận hành và ra mắt (nhóm task 15)

- Kiểm thử tải `/v1/me` (`load_v1_me`): 1.000 yêu cầu/giây trong 30 giây, 0 lỗi, p95 2,9 ms trên
  máy dev ([biên bản](docs/reports/2026-11-15-performance.md)).
- Diễn tập sao lưu liên tục và khôi phục: `scripts/backup-drill.sh`
  ([biên bản](docs/ops/drills/2026-11-15-backup-drill.md), [cách vận hành](docs/ops/status-and-backup.md)).
- Tài liệu có bản tiếng Anh (`README.en.md`, `docs/en/`); CI kiểm bằng
  `scripts/check-docs-i18n.py`. Email bảo mật có test bảo đảm luôn song ngữ.
- Bản nháp hồ sơ dữ liệu cá nhân và chuyển dữ liệu ra nước ngoài: `docs/legal/data-inventory.md`
  (chờ luật sư, QUESTIONS 82).

### Môi trường dev

- Kratos và Hydra lưu dữ liệu trong SQLite (`infra/.data`): tài khoản và client OAuth còn sau khi
  khởi động lại. `FECO_DEV_MEMORY=1` trở về chạy trong bộ nhớ.

## 1.4.0 (14/11/2026)

Thay đổi OpenSpec `web-ui-and-japanese`.

### Tiếng Nhật

- F-Launcher có tiếng Nhật (日本語) trong Cài đặt → Giao diện, cùng tiếng Việt, tiếng Anh, tiếng
  Pháp. Chữ Nhật dùng phông có sẵn của hệ điều hành (Yu Gothic, Meiryo, MS Gothic trên Windows;
  Hiragino trên macOS; Noto CJK hoặc IPA trên Linux), không đóng gói phông. Có thể chỉ định bằng
  `FECO_LAUNCHER_JA_FONT`.
- Trang web có tiếng Nhật: nút VI · EN · 日本語 ở thanh trên cùng, `/lang/ja`, tự nhận trình duyệt
  gửi `Accept-Language: ja`. Toàn bộ chuỗi giao diện web có bản tiếng Nhật; nội dung hệ thống tạo
  chưa có bản tiếng Nhật (thông báo của Kratos, email) hiện bằng tiếng Anh.
- Hồ sơ người dùng nhận ngôn ngữ `ja` (schema danh tính Kratos).

### Giao diện web

- Thiết kế mới: thanh trên cùng (Tài khoản, Cửa hàng, Đơn hàng, Thông báo, Nhà phát triển), chọn
  ngôn ngữ, lối tới trang **Giao diện**; dải màu trang trí theo phong cách.
- Trang **Giao diện** (`/preferences`): ngôn ngữ, phong cách (10 phong cách như F-Launcher), chủ đề
  (theo hệ thống, sáng, tối), màu nhấn, mật độ (thường, gọn), cỡ chữ 90–125%. Lưu trong cookie
  `feco_ui`, áp dụng từ lần vẽ đầu của mọi trang, không cần JavaScript; giá trị hỏng về mặc định.
- Bảng màu dùng chung: `feco_core::ui_palette` là nguồn duy nhất cho F-Launcher và CSS của web
  (`/ui.css`), có kiểm thử độ tương phản WCAG 2.1.
- Đổi ngôn ngữ giờ quay lại đúng trang đang xem.

### Bảng quản trị

- Bố cục mới với thanh bên và trang **Tổng quan**: thẻ số liệu theo vai trò con (yêu cầu quyền
  Lập trình viên và số người dùng cho Quản trị hệ thống; phiên bản chờ duyệt thủ công cho Người
  duyệt app; đơn sắp trễ cho Quản lý dự án; lệnh hoàn chờ duyệt, doanh thu 30 ngày và biểu đồ
  doanh thu 14 ngày cho Tài chính). Bấm thẻ để tới mục tương ứng.
- `FECO_ADMIN_PASSKEY_OPTIONAL=true` bỏ yêu cầu passkey của bảng quản trị, **chỉ khi
  `FECO_ENV=dev`** (WebAuthn không chạy trên địa chỉ 127.0.0.1). Mọi trang quản trị hiện cảnh báo;
  staging và production từ chối khởi động nếu đặt biến này.

## 1.3.0 (14/11/2026)

Thay đổi OpenSpec `platform-roles`, cùng các thay đổi về thanh toán và thông báo.

### Phân quyền người dùng

- Ba vai trò nền tảng:
  - **Người sử dụng ứng dụng** (mặc định);
  - **Lập trình viên**: vào được Cổng nhà phát triển và dùng `f-dev`;
  - **Quản trị viên**: bảng quản trị với 5 vai trò con như trước, và mọi quyền của Lập trình viên.
- Trang **Phân quyền người dùng** (`/admin/members`, Quản trị hệ thống):
  - đổi vai trò theo email, bắt buộc lý do, ghi nhật ký kiểm toán;
  - không tự đổi vai trò của mình, luôn còn ít nhất một Quản trị hệ thống;
  - duyệt hoặc từ chối yêu cầu quyền Lập trình viên.
- Cổng nhà phát triển:
  - người chưa có quyền thấy trang "Cần quyền Lập trình viên" và gửi được yêu cầu;
  - token `f-dev` của người bị thu hồi quyền bị từ chối ngay (`developer_role_required`);
  - thành viên nhà phát triển có từ trước được cấp sẵn quyền.
- F-Launcher hiện vai trò ở thanh bên, có lối tắt "Cổng nhà phát triển" và "Bảng quản trị".
- `feco-catalog role-set`, `role-requests`, `role-decide`.

### F-Launcher

- Tên phong cách đổi sang tên phổ thông: Giấy da, Bích họa, Trời sao, Hồ sen, Lập thể, Hoa huệ,
  Phố cổ, Sơn mài. Cài đặt của bản 1.2 vẫn đọc đúng.
- Demo App không còn trong kho ứng dụng: `feco-tuf remove` gỡ app khỏi danh mục mà không thu hồi,
  `scripts/dev-launcher.sh remove-demo` dọn môi trường dev cũ.

### Thanh toán và thông báo

- Thông báo và email theo ngôn ngữ người dùng:
  - sửa lỗi đọc nhầm trait `language` thay vì `locale`, khiến mọi người dùng chỉ nhận tiếng Việt;
  - thông báo đơn hàng, thanh toán, hoàn tiền và email hóa đơn có bản tiếng Anh;
  - email kiểm duyệt gửi nhà phát triển có đủ phần tiếng Anh.
- Thông báo thanh toán và hoàn tiền giờ cũng hiện trong trung tâm thông báo (web và F-Launcher).
- `feco-catalog payments-check` kiểm khóa ZaloPay, VNPAY, MoMo và tài khoản MISA bằng truy vấn vô hại.
- Lỗi khi cổng thanh toán trả trang HTML (ví dụ 403 do chặn IP) được báo rõ mã HTTP.
- Hướng dẫn đăng ký và cấu hình cổng thanh toán, hóa đơn điện tử, tên miền email:
  `docs/payments-setup.md`.

## 1.2.0 (13/11/2026)

Thay đổi OpenSpec `launcher-personalization`: tùy biến giao diện F-Launcher.

- **Ngôn ngữ**: tiếng Việt (mặc định), tiếng Anh, tiếng Pháp, chọn ở Cài đặt → Giao diện → Ngôn
  ngữ.
  - Đổi ngay, và được nhớ cho lần mở sau.
  - Mọi chữ trên giao diện launcher đều được dịch, kể cả tên nhóm, kiểu sắp xếp và tên phong cách.
  - Nội dung do nhà phát triển viết giữ nguyên ngôn ngữ gốc.
  - Một test tự động báo lỗi khi có chuỗi giao diện chưa dịch.
- **Phong cách**, chọn ở Cài đặt → Giao diện → Phong cách, có ô xem trước:
  - **Chân phương** (mặc định);
  - **Trang nhã**: nút và thẻ bo tròn;
  - tám phong cách lấy cảm hứng từ bảng màu của họa sĩ: Leonardo da Vinci, Michelangelo, Vincent
    van Gogh, Claude Monet, Pablo Picasso, Tô Ngọc Vân, Bùi Xuân Phái, Nguyễn Gia Trí.

  Phong cách họa sĩ có bảng màu, bo góc và dải họa tiết riêng. Chúng chỉ dùng màu và hình khối do
  F-Eco tự vẽ, không dùng hình ảnh tác phẩm. Mọi phong cách đạt độ tương phản WCAG 2.1 và được kiểm
  bằng test tự động.
- Màu cảnh báo và màu lỗi đổi theo nền sáng hoặc tối, nên dễ đọc hơn.
- CI kiểm `Cargo.lock` của cả hai workspace khớp `Cargo.toml` (workflow release dựng bằng
  `--locked`).

## 1.1.0 (13/11/2026)

Thay đổi OpenSpec `improve-launcher-experience`. Gồm cả các bản sửa của 1.0.1.

### F-Launcher

- Bố cục mới:
  - thanh bên với Thư viện, Cửa hàng, Thông báo, Cài đặt và tài khoản;
  - Thư viện và Cửa hàng xem theo lưới hoặc danh sách, nhớ lựa chọn;
  - tìm không dấu, lọc theo nhóm;
  - sắp xếp theo tên, lần mở gần nhất, ngày cài, nhóm hoặc dung lượng.
- Trang Cài đặt:
  - chủ đề sáng, tối hoặc theo hệ thống; màu nhấn; cỡ chữ 80–150%;
  - bố cục mặc định, cỡ ô lưới, trang mở đầu, mô tả ngắn, chế độ gọn;
  - tự động cập nhật, tài khoản, thông tin phiên bản;
  - khôi phục mặc định.

  Cài đặt lưu trong `settings.json` trên máy. File hỏng thì dùng giá trị mặc định.
- Hiện mô tả ngắn, nhóm, nhà phát triển và liên kết mã nguồn từ metadata TUF đã ký. Biểu tượng là
  chữ cái đầu, màu cố định theo app.
- App có cờ `terminal` mở trong cửa sổ terminal riêng: Windows dùng console mới, macOS dùng
  Terminal.
- Trang chi tiết app: bấm vào app trong Thư viện hoặc Cửa hàng để xem biểu tượng, tên, giới thiệu,
  tính năng, liên kết GitHub và website, phiên bản mới nhất và đã cài, ngày phát hành, dung lượng,
  các nền tảng chạy được, có gì mới trong bản mới nhất; nút Cài, Cập nhật, Mở, Gỡ, Quay lại bản
  trước. Nút **‹ Trước** / **Sau ›** và phím mũi tên chuyển sang app kề bên theo thứ tự đang
  hiển thị; Esc quay lại.
- Renderer wgpu và giờ địa phương (như 1.0.1).

### Nhà phát triển

- `f-app.toml` có thêm `source` (địa chỉ mã nguồn, HTTPS), `terminal`, `website`, `features`
  (tối đa 12 dòng) và `release_notes`.
- Máy chủ ký vào target TUF các trường mô tả ngắn, giới thiệu, tính năng, nhóm, nhà phát triển,
  mã nguồn, website, ghi chú phát hành, giấy phép, thời điểm phát hành, terminal. `release_notes`
  cũng là lịch sử thay đổi của phiên bản trên F-Store. App không khai scope thì target không có
  `client_id`.
- `f-dev pack` đóng gói và kiểm tra trước khi nộp. Gói của app có `source` phải kèm giấy phép gốc.
- App mẫu dùng F-SDK: Ghi chú (`examples/apps/f-notes`) và Bấm giờ (`examples/apps/f-timer`).
- `scripts/dev-apps.sh` đưa vào môi trường dev hai app mẫu và các app mã nguồn mở dựng từ GitHub
  (kibi, bottom, eframe template). Cách làm có trong `docs/github-apps.md`.

## 1.0.1 (13/11/2026)

Bản vá cho F-Launcher.

- **F-Launcher hiện cửa sổ trắng trên một số máy Windows.** Trên Windows 10 với card NVIDIA,
  renderer glow (OpenGL) của eframe 0.37 không vẽ được gì. Launcher chuyển sang renderer wgpu
  (DirectX 12 trên Windows, Metal trên macOS, Vulkan trên Linux). Tệp chạy lớn hơn một chút; các
  thư viện mới đều MIT hoặc Apache-2.0 và có trong `THIRD_PARTY_NOTICES` và SBOM của gói.
- Thời điểm đồng bộ và thời điểm của thông báo trong launcher hiện theo giờ của máy, không còn
  hiện giờ UTC.
- F-SDK cho Python và TypeScript có số phiên bản 1.0.1, mã không đổi.

## 1.0.0 (13/11/2026)

Bản 1.0 đầu tiên của F-Eco: nền tảng tài khoản, đăng nhập một lần, F-Store và F-Launcher để
cài và mở các app con, cổng nhà phát triển kèm kiểm duyệt, F-SDK, F-Builder, đơn xây app theo yêu
cầu, thanh toán, bảng quản trị và thông báo.

### Tích hợp app con

- **F-Launcher** (Windows, macOS, Linux): đăng nhập PKCE, đồng bộ danh mục qua TUF, cài và cập
  nhật nguyên tử, quay lại bản trước, gỡ app, chặn bản bị thu hồi, mở app bằng mã khởi chạy
  dùng một lần, API cục bộ để app xin mã mới, app riêng tư, trung tâm thông báo.
- **F-SDK** cho Rust, C (ABI ổn định, phiên bản 1), Python và TypeScript/Node.js: phiên từ
  launcher hoặc đăng nhập trình duyệt, tự làm mới token, `/v1/me`, `/v1/entitlements`, báo lỗi khi
  chạy, kiểm chữ ký webhook. App dùng thư viện OIDC chuẩn cũng tích hợp được.
- **Cổng nhà phát triển** và CLI `f-dev`: manifest `f-app.toml` có trình kiểm tra, nộp gói, kiểm
  tra chữ ký mã, kênh stable và beta, phát hành dần theo phần trăm, số liệu tổng hợp, webhook.
- **Kiểm duyệt**: quét tự động bằng hai engine, duyệt thủ công kèm khác biệt quyền, chính sách
  nội dung, kháng nghị, gỡ khẩn cấp, quét lại 7 ngày.
- Hướng dẫn: [`docs/integration-guide.md`](docs/integration-guide.md).

### Tài khoản và dữ liệu

- Ory Kratos và Hydra: đăng ký, xác minh email, TOTP, passkey, mã khôi phục, thiết bị và đăng
  xuất từ xa, đặt lại mật khẩu, xóa tài khoản sau 30 ngày chờ.
- Đồng ý theo từng scope, định danh pairwise cho từng app, trang "Ứng dụng đã kết nối", thu hồi,
  xuất dữ liệu cá nhân.
- Giới hạn tần suất, nhật ký kiểm toán chỉ ghi thêm, cảnh báo đăng nhập lạ, mã hóa dữ liệu nhạy
  cảm khi lưu.

### Sản phẩm và vận hành

- F-Store: tìm kiếm không dấu, bộ lọc, đánh giá, báo cáo vi phạm.
- F-Builder: tạo app từ mô tả hoặc OpenSpec, bộ giám sát chống treo, clean room, nộp lên F-Store.
- Đơn xây app theo yêu cầu: báo giá theo mốc, bàn giao dưới dạng app riêng tư, nghiệm thu, tranh
  chấp, hủy đơn.
- Thanh toán qua ZaloPay, VNPAY, MoMo (sandbox), hoàn tiền có hai người duyệt, đối soát, hóa đơn
  (bản nháp, hoặc MISA meInvoice khi cấu hình).
- Bảng quản trị 5 vai trò, bắt buộc passkey; thông báo qua email, web và launcher.
- Gói phát hành kèm `THIRD_PARTY_NOTICES` và SBOM CycloneDX cho từng sản phẩm.

### Sửa lỗi trong đợt kiểm tra 1.0

- Mở app từ launcher không còn đòi đồng ý lại mỗi lần khi người dùng đã bỏ qua một scope tùy chọn.
- feco-server gọi được các dịch vụ HTTPS bên ngoài (cổng thanh toán, hóa đơn) khi chạy thật.
- Số hóa đơn trùng được báo lỗi thay vì bị bỏ qua.

### Giới hạn đã biết

Bản 1.0 chưa dùng cho người dùng cuối thật. Còn mở (chi tiết trong
`openspec/changes/add-f-ecosystem-platform/tasks.md` và `QUESTIONS.md`):

- Hạ tầng staging, production và IaC (1.4); máy ảo cô lập để quét và chạy thử app (9.1); môi trường
  sandbox cho nhà phát triển (8.6).
- Đăng nhập Google và GitHub (2.3), một số chính sách phiên (2.4, 2.5).
- Khóa root TUF trong KMS và lễ xoay khóa thật (5.1, 5.3).
- Launcher tự cập nhật và bộ cài đã ký mã (6.6): bản 1.0 phát hành chương trình chưa ký.
- Thanh toán thật: thử callback và hoàn tiền với sandbox thật, khóa VNPAY và MoMo (13.2); tài khoản
  MISA (13.4); tên miền email với SPF, DKIM, DMARC (14.3).
- Rà soát pháp lý: chính sách quyền riêng tư, điều khoản (3.6, 12.6), quy trình sự cố (4.5), lưu trữ
  dữ liệu (15.4); kiểm thử tải (15.2), trang trạng thái (15.3), kiểm thử xâm nhập (15.6); dịch đủ
  tiếng Anh (15.1).
