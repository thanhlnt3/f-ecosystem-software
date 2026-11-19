# Design

## O1. Bật OIDC trong Kratos

`infra/kratos/kratos.yml`:

```yaml
selfservice:
  methods:
    oidc:
      enabled: true
      config:
        providers: []      # khai báo khi triển khai, xem O2
  flows:
    registration:
      after:
        oidc:
          hooks:
            - hook: session   # đăng ký bằng Google/GitHub thì đăng nhập luôn
```

- Đăng ký bằng mật khẩu vẫn không tự đăng nhập, để không lộ email đã tồn tại.
- Với OIDC thì không có rủi ro đó: email đến từ nhà cung cấp, không do người dùng gõ.

Jsonnet `infra/kratos/oidc/mapper.jsonnet` dùng chung cho Google và GitHub (Kratos chuẩn hóa claim của cả hai):

- chỉ đặt `traits.email` khi `email_verified` đúng;
- thêm email đó vào `verified_addresses`, để tài khoản mới có email đã xác minh;
- đặt `traits.name` nếu có.

Email chưa xác minh thì không được đặt. Khi đó Kratos hiện ô email của bước đăng ký, và email người dùng gõ phải được xác minh như bình thường.

## O2. Nhà cung cấp khai báo khi triển khai

Client ID và secret không nằm trong repo. Khi triển khai, đặt `SELFSERVICE_METHODS_OIDC_CONFIG_PROVIDERS` là mảng JSON:

```json
[{"id": "google", "provider": "google", "label": "Google", "client_id": "…", "client_secret": "…",
  "mapper_url": "file:///etc/config/kratos/oidc/mapper.jsonnet", "scope": ["openid", "email", "profile"]},
 {"id": "github", "provider": "github", "label": "GitHub", "client_id": "…", "client_secret": "…",
  "mapper_url": "file:///etc/config/kratos/oidc/mapper.jsonnet", "scope": ["user:email"]}]
```

Không khai báo thì trang đăng nhập không có nút, như hiện nay.

## O3. Giao diện và liên kết tài khoản

- **Nút đăng nhập.** `views::flow_view` đã dựng nhóm `oidc` từ node của Kratos. Biểu mẫu gửi về feco-server, feco-server chuyển tới Kratos. Kratos trả 422 `redirect_browser_to`; feco-server đã xử lý trường hợp này (chuyển trình duyệt tới nhà cung cấp và giữ cookie liên tục của Kratos).
- **Bản dịch.** Thêm nhãn và thông báo bốn thứ tiếng theo id của Kratos:
  - 1010002 "Sign in with {provider}", 1040002 "Sign up with {provider}";
  - 1010016: email đã có tài khoản, đăng nhập bằng cách cũ để liên kết;
  - 4000027: tài khoản đã tồn tại.
- **Liên kết tài khoản** (cơ chế của Kratos từ v1.1). Đăng nhập GitHub với email trùng một tài khoản có sẵn:
  - Kratos không tạo tài khoản mới, đưa trình duyệt về trang đăng nhập kèm thông báo 1010016;
  - người dùng nhập mật khẩu tài khoản cũ;
  - Kratos thêm thông tin OIDC vào tài khoản đó. Lần sau GitHub đăng nhập thẳng.

### Việc của F-Eco sau khi đăng nhập bằng OIDC

Callback của OIDC đi thẳng tới Kratos, không qua bước gửi biểu mẫu của feco-server. Để vẫn ghi nhật ký kiểm toán `auth.login`, báo thiết bị mới và hủy yêu cầu xóa tài khoản đang chờ:

- feco-server đặt `return_to` của mọi luồng đăng nhập và đăng ký thành `/auth/after-login?next=<đích cũ>`;
- `GET /auth/after-login`:
  - chỉ xử lý khi phiên vừa xác thực trong 5 phút và có phương thức `oidc`;
  - mỗi phiên xử lý một lần (tra `audit_log` theo id phiên);
  - xong thì chuyển tới `next`, chỉ nhận đường dẫn trong F-Eco.
- Đăng nhập bằng mật khẩu đã làm các việc này lúc gửi biểu mẫu, nên ở đây chỉ chuyển tiếp.

## T1. Điểm nhận token của F-Eco

`POST /oauth2/token` trên feco-server (`src/token_proxy.rs`):

- chuyển nguyên thân yêu cầu (tối đa 64 KB), `Authorization` và `Content-Type` tới `{FECO_HYDRA_PUBLIC_URL}oauth2/token`;
- trả về đúng mã, thân, `Content-Type`, `Cache-Control`, `Pragma` của Hydra; Hydra không trả lời thì 503 `temporarily_unavailable`;
- không ghi thân yêu cầu hay phản hồi vào log, vì trong đó có client secret và token;
- không cần CSRF (không dùng cookie); vẫn qua giới hạn yêu cầu theo IP.

Hydra trỏ `token_endpoint` của tài liệu discovery về đây: `webfinger.oidc_discovery.token_url`, triển khai thật đặt bằng `WEBFINGER_OIDC_DISCOVERY_TOKEN_URL`. Ai dùng thư viện OIDC chuẩn (F-Launcher di động qua AppAuth, F-SDK, Mentor, app bên thứ ba) tự đi qua F-Eco. F-Launcher máy tính tự ghép đường dẫn, nên đổi sang `{máy chủ F-Eco}/oauth2/token`. Gọi thẳng Hydra vẫn chạy; xem T3 cho trường hợp này.

Ghi chú: đây chỉ là chuyển tiếp. Mọi xử lý OAuth (kiểm client, PKCE, xoay vòng, thu hồi chuỗi) vẫn do Hydra làm.

## T2. Dữ liệu (`0021_refresh_tokens.sql`)

```sql
CREATE TABLE oauth_token_families (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  subject UUID NOT NULL,
  client_id TEXT NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  revoked_at TIMESTAMPTZ,           -- người dùng gỡ quyền app: không cảnh báo
  reuse_detected_at TIMESTAMPTZ     -- đã cảnh báo: không cảnh báo lần hai
);
CREATE TABLE oauth_refresh_tokens (
  token_hash TEXT PRIMARY KEY,      -- SHA-256 hex, không lưu token
  family UUID NOT NULL REFERENCES oauth_token_families(id) ON DELETE CASCADE,
  issued_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  rotated_at TIMESTAMPTZ
);
```

Khi Hydra trả 200:

- **`authorization_code` có refresh token:** introspect access token mới (API quản trị của Hydra) lấy `sub` và `client_id`, tạo chuỗi mới và ghi token.
- **`refresh_token`:** token đã gửi có `rotated_at`; token mới vào cùng chuỗi. Token gửi lên chưa có trong bảng (cấp trước khi có điểm nhận này) thì tạo chuỗi mới như trên.

## T3. Phát hiện dùng lại

Hydra trả 400 `invalid_grant` cho `grant_type=refresh_token`, và token gửi lên có trong bảng. F-Eco coi là dùng lại khi đủ ba điều:

- token được cấp chưa quá 720 giờ (hạn refresh token của Hydra);
- chuỗi chưa bị thu hồi chủ động;
- chuỗi chưa từng bị phát hiện dùng lại.

Có hai trường hợp, cả hai đều cảnh báo:

- **Token đã xoay vòng (`rotated_at` có giá trị)** bị gửi lại.
- **Token chưa xoay vòng mà Hydra từ chối.** Nghĩa là ai đó đã dùng nó thẳng với Hydra (bỏ qua F-Eco) rồi chủ token mới gửi lên.

Khi đó F-Eco:

- đặt `reuse_detected_at`;
- gửi thông báo loại **Bảo mật** (bắt buộc, có email), tiếng Việt và tiếng Anh (tiếng Nhật và tiếng Pháp dùng bản tiếng Anh, như các thông báo khác):
  - nội dung: tên app (`client_name` của Hydra), thời điểm, thiết bị (User-Agent) và IP của yêu cầu;
  - "F-Eco đã đăng xuất phiên đó";
  - liên kết tới `/sessions` và lời khuyên đổi mật khẩu;
- ghi `audit_log` `auth.refresh_token_reused`.

Hydra đã thu hồi cả chuỗi (`rotation_grace_period` mặc định 0). F-Eco không thu hồi thêm, để các thiết bị khác của người dùng không bị đăng xuất.

**Thu hồi chủ động:**

- `connected.rs`: gỡ quyền app thì đặt `revoked_at` cho các chuỗi của người dùng với client đó;
- xóa tài khoản: xóa các chuỗi (`store::delete_subject_data`).

Worker hằng ngày xóa chuỗi tạo quá 31 ngày.

**Giới hạn:** app tự gọi `/oauth2/revoke` của Hydra rồi lại dùng token đó sẽ gây cảnh báo nhầm. App chuẩn không làm vậy. Ghi ở QUESTIONS.md.

## T4. Kiểm thử

- **E2E `refresh_token_reuse_alert`:**
  - discovery trỏ về F-Eco;
  - xoay vòng hai lần qua F-Eco;
  - gửi lại token cũ: 400, một email và một thông báo; token mới nhất cũng hết hiệu lực;
  - gửi lại lần nữa: không có email thứ hai;
  - chuỗi thứ hai bị dùng thẳng với Hydra rồi chủ token gửi lên: có cảnh báo;
  - chuỗi thứ ba bị gỡ quyền: không cảnh báo.
- **E2E `external_login_with_fake_oidc_provider`.** Nhà cung cấp OIDC giả chạy trong test ở `127.0.0.1:4499`: discovery, authorize tự đồng ý, token, JWKS; ID token ký ES256 bằng `ring`, khóa sinh lúc chạy. Kratos dùng thêm `infra/kratos/kratos.e2e-oidc.yml` với hai nhà cung cấp `generic` tên `google` và `github` trỏ tới đó.
  - **Đăng nhập Google lần đầu:** tài khoản mới, email đã xác minh, có `auth.login` phương thức `oidc`.
  - **Liên kết với tài khoản có sẵn:** trang đăng nhập yêu cầu mật khẩu; sau khi nhập, lần sau GitHub vào thẳng đúng tài khoản cũ.
  - **Email chưa xác minh:** không tạo tài khoản với email đó.
- **CI:** Kratos trong docker chạy `network_mode: host` (`infra/docker-compose.e2e.yml`) để gọi được nhà cung cấp giả. Trên máy: `FECO_KRATOS_E2E_OIDC=1 scripts/dev-local.sh kratos`.
