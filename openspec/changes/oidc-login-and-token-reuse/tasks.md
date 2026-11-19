# Tasks

## 1. Đăng nhập bằng tài khoản bên ngoài

- [x] 1.1 Kratos: `methods.oidc`, `mapper.jsonnet`, hook `session` sau đăng ký OIDC, `kratos.e2e-oidc.yml`, `docker-compose.e2e.yml`, `dev-local.sh` (`FECO_KRATOS_E2E_OIDC`); xác minh bằng E2E scenario "Nút đăng nhập Google", "Đăng nhập Google lần đầu", "Email chưa xác minh ở nhà cung cấp"
  - Xác minh: E2E `external_login_with_fake_oidc_provider` (Kratos thật với `kratos.e2e-oidc.yml`, nhà cung cấp OIDC giả `tests/support/fake_oidc.rs`, ID token ES256): nút Google/GitHub ở `/login`, "Google で登録" ở `/registration` tiếng Nhật; tài khoản mới có `verifiable_addresses` đã xác minh và tên từ nhà cung cấp; email chưa xác minh thì trang đăng ký hỏi email, không có tài khoản nào với email đó.
- [x] 1.2 feco-server: bản dịch nhãn và thông báo OIDC bốn thứ tiếng, `return_to` qua `/auth/after-login`, nhật ký và báo thiết bị mới cho đăng nhập OIDC; xác minh bằng E2E scenario "Liên kết với tài khoản có sẵn", "Nhật ký đăng nhập bằng Google" và unit test bản dịch
  - Xác minh: cùng E2E: Liên kết với tài khoản có sẵn (thông báo 1010016 tiếng Việt, chưa có phiên trước khi nhập mật khẩu, cùng id danh tính, lần sau GitHub vào thẳng); Nhật ký (`auth.login` có `methods: ["oidc"]`, `provider: google`, một lần cho mỗi phiên; `next` ngoài F-Eco về `/`). Unit test `views::tests::oidc_labels_in_four_languages`. Toàn bộ `e2e_auth` (42 qua, 1 bỏ qua theo thiết kế), `e2e_chat` (5), `e2e_source_scan` (1) qua với `return_to` mới.

## 2. Refresh token bị dùng lại

- [x] 2.1 Migration `0021_refresh_tokens.sql`, `POST /oauth2/token` chuyển tiếp tới Hydra, ghi băm token, phát hiện dùng lại, cảnh báo, thu hồi chủ động, dọn dữ liệu; Hydra `webfinger.oidc_discovery.token_url`; xác minh bằng E2E scenario "Dùng lại token đã xoay vòng", "Gửi lại lần nữa", "Gỡ quyền app không gây cảnh báo"
  - Xác minh: E2E `refresh_token_reuse_alert` với Hydra thật: discovery trỏ `token_endpoint` về F-Eco; A→B→C rồi gửi lại A bị từ chối, C cũng hết hiệu lực, một email "có dấu hiệu bị sao chép" (thiết bị, `/sessions`, không chứa token) và một thông báo `security`, nhật ký `auth.refresh_token_reused`; bảng chỉ có băm; gửi lại lần nữa không có email thứ hai; token dùng thẳng với Hydra rồi gửi lên F-Eco vẫn cảnh báo; gỡ quyền app (`connected::revoke_app`) không cảnh báo. Unit test `token_proxy::tests`.
- [x] 2.2 F-Launcher máy tính đổi token qua F-Eco; xác minh bằng test `tests/auth.rs`
  - Xác minh: `clients/feco-launcher/tests/auth.rs::token_requests_go_through_feco_server` (Hydra không nghe, làm mới vẫn được qua máy chủ F-Eco); `token_inactive` cũng được coi là hết phiên.

## 3. Tài liệu và phát hành

- [x] 3.1 Đánh dấu task 2.3 và 2.5 của `add-f-ecosystem-platform`; README, CHANGELOG, QUESTIONS.md; phát hành 1.13.0
  - Xác minh: task 2.3 và 2.5 của `add-f-ecosystem-platform` đã đánh dấu; README hai thứ tiếng (mục 4, bảng API, Kratos và Hydra khi triển khai, kiểm thử), `scripts/check-docs-i18n.py`; CHANGELOG 1.13.0; QUESTIONS.md 4 (cập nhật), 117–120; clippy sạch, `cargo test --workspace` qua, `flutter test` 27 qua; CI và release.yml của tag `v1.13.0`.
