# Spec Delta

## Purpose

Bổ sung cho capability `app-catalog`: app web trên F-Store và đăng nhập một lần bằng tài khoản F-Eco cho app web.

## ADDED Requirements

### Requirement: App web trên F-Store
Danh mục SHALL lưu địa chỉ của app web (`web_url`). Trang app web SHALL có nút "Mở ứng dụng web" tới địa chỉ đó và ghi nền tảng "Web". Bộ lọc nền tảng (Windows, macOS, Linux) SHALL vẫn hiện app web, vì app web chạy trên mọi hệ điều hành. Trang "Ứng dụng của tôi" SHALL có nút "Mở" cho app web đã thêm qua launcher. API danh mục SHALL trả `web_url` (rỗng với app desktop).

#### Scenario: Trang app web
- **GIVEN** ThesisAI có trong danh mục với `web_url` là `http://localhost:3000/auth/feco` và phiên bản nền tảng `web`
- **WHEN** người dùng mở `/store/thesisai`
- **THEN** trang có nút "Mở ứng dụng web" tới `http://localhost:3000/auth/feco` và dòng nền tảng ghi "Web"

#### Scenario: Lọc theo Windows
- **WHEN** người dùng lọc F-Store theo Windows
- **THEN** danh sách có ThesisAI

#### Scenario: Ứng dụng của tôi
- **GIVEN** người dùng đã thêm ThesisAI qua launcher
- **WHEN** người dùng mở `/library`
- **THEN** dòng ThesisAI có nút "Mở" tới `web_url`

### Requirement: Đăng nhập bằng F-Eco cho app web
App web SHALL đăng nhập người dùng bằng tài khoản F-Eco theo chuẩn OIDC, với client OAuth phía máy chủ: authorization code + PKCE, client secret (`client_secret_basic`), địa chỉ chuyển hướng đã đăng ký, `sub` pairwise. Token endpoint MUST từ chối khi sai secret. Khi người dùng đồng ý scope `email`, `GET /v1/me` với access token SHALL trả `email` và `email_verified`.

#### Scenario: ThesisAI đăng nhập bằng F-Eco
- **GIVEN** client OAuth của ThesisAI có scope `openid profile email` và chuyển hướng `http://localhost:8000/api/auth/feco/callback`
- **WHEN** người dùng đã đăng nhập F-Eco đi qua luồng authorization code và đồng ý chia sẻ email
- **THEN** backend đổi mã bằng client secret, nhận ID token có `sub` pairwise, và `GET /v1/me` trả email của người dùng

#### Scenario: Sai client secret
- **WHEN** backend đổi mã với client secret sai
- **THEN** token endpoint trả `invalid_client` và không cấp token

#### Scenario: Chuyển hướng lạ
- **WHEN** yêu cầu đăng nhập dùng `redirect_uri` chưa đăng ký
- **THEN** F-Eco không chuyển mã tới địa chỉ đó
