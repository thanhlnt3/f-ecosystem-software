# Proposal

## Why

Change `add-f-ecosystem-platform` còn 19 task mở. Phần lớn chờ việc bên ngoài: luật sư, nhà cung cấp đám mây, chứng thư ký mã, đơn vị kiểm thử xâm nhập, khóa thật của cổng thanh toán. Hai task còn thiếu phần lập trình và kiểm được ngay trên máy phát triển:

- **2.3 Đăng nhập Google và GitHub.** Task được xác minh "bằng test với nhà cung cấp OIDC giả lập", nên không cần client ID thật để hoàn thành. Hiện Kratos chưa bật phương thức `oidc`.
- **2.5 Phiên đăng nhập.** Còn thiếu scenario "Refresh token bị dùng lại": Hydra đã thu hồi cả chuỗi token khi một refresh token đã xoay vòng bị dùng lại, nhưng người dùng chưa nhận email cảnh báo. Hydra không báo sự kiện này ra ngoài.

## What Changes

- **Đăng nhập bằng tài khoản bên ngoài** (task 2.3):
  - bật phương thức `oidc` của Kratos;
  - nhà cung cấp Google và GitHub khai báo bằng biến môi trường khi triển khai, không đặt client ID hay secret trong repo;
  - Jsonnet ánh xạ claim sang trait: chỉ nhận email đã được nhà cung cấp xác minh, và đánh dấu email đó là đã xác minh;
  - trang đăng nhập và đăng ký của F-Eco hiện nút "Đăng nhập bằng Google" và "Đăng nhập bằng GitHub" từ flow của Kratos, bốn thứ tiếng;
  - email trùng tài khoản có sẵn thì Kratos yêu cầu đăng nhập bằng mật khẩu cũ để liên kết, không tự gộp;
  - E2E với nhà cung cấp OIDC giả chạy trong test.
- **Cảnh báo refresh token bị dùng lại** (task 2.5):
  - `POST /oauth2/token` của F-Eco chuyển nguyên yêu cầu tới Hydra; tài liệu discovery của Hydra trỏ `token_endpoint` về đây (`webfinger.oidc_discovery.token_url`);
  - F-Eco ghi băm SHA-256 của refresh token Hydra cấp, cùng người dùng, client, chuỗi và thời điểm; không lưu token gốc;
  - khi một refresh token đã biết, chưa hết hạn và chưa bị thu hồi chủ động lại bị Hydra từ chối (`invalid_grant`), F-Eco coi là dùng lại:
    - email và thông báo bắt buộc (loại "Bảo mật") gửi một lần cho mỗi chuỗi;
    - ghi nhật ký kiểm toán.
  - F-Launcher máy tính đổi token qua F-Eco; F-Launcher di động và app bên thứ ba đi theo tài liệu discovery.
  - Gỡ quyền app ở "Ứng dụng đã kết nối" đánh dấu token đã thu hồi chủ động, nên không gây cảnh báo nhầm.
- **Dọn dữ liệu**: bản ghi token quá 31 ngày bị xóa; xóa tài khoản xóa bản ghi.

## Capabilities

### New Capabilities
- (không có)

### Modified Capabilities
- `account-identity`: chi tiết cho "Đăng nhập bằng tài khoản bên ngoài" và "Refresh token bị dùng lại".

## Impact

- **Kratos** (`infra/kratos`): `methods.oidc`, Jsonnet `oidc/*.jsonnet`, `scripts/inline-kratos-config.py` nhúng Jsonnet; docker compose thêm `host.docker.internal` để Kratos gọi được nhà cung cấp giả của E2E.
- **Hydra** (`infra/hydra/hydra.yml`): `webfinger.oidc_discovery.token_url`.
- **feco-server**: migration `0021_refresh_tokens.sql`, `src/token_proxy.rs`, i18n, `connected.rs`, `store::delete_subject_data`.
- **F-Launcher máy tính**: `auth.rs` đổi token qua máy chủ F-Eco.
- **Test**: E2E `e2e_auth` mới `external_login_with_fake_oidc_provider` và `refresh_token_reuse_alert`; unit test cho proxy và Jsonnet.
- **Tài liệu**: README (đăng nhập ngoài, cấu hình), CHANGELOG, QUESTIONS.md.
- **Không đổi**: chưa có client ID Google và GitHub thật. Khi chủ dự án tạo xong (QUESTIONS.md), chỉ cần đặt biến môi trường.
