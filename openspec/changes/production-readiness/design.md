# Design

## K1. Tài khoản quản trị viên mặc định

**Cấu hình** (`AdminBootstrapConfig`):

| Biến | Dev | Production |
|---|---|---|
| `FECO_ADMIN_EMAIL` | mặc định `admin@f-eco.local` | bắt buộc |
| `FECO_ADMIN_PASSWORD_FILE` | mặc định `.dev/admin-password`; chưa có thì máy chủ tự sinh 24 ký tự ngẫu nhiên và ghi vào đó | bắt buộc; tệp phải tồn tại |

Quy tắc chung:
- Mật khẩu tối thiểu 12 ký tự.
- Mật khẩu chỉ đọc khi cần tạo tài khoản. Không ghi vào nhật ký, không đặt lại mật khẩu của tài khoản đã có (người quản trị đổi mật khẩu bằng trang cài đặt hoặc khôi phục qua email).
- Không có mật khẩu mặc định nào trong mã nguồn.

**`bootstrap::ensure_admin`** chạy lúc khởi động (trước khi nhận yêu cầu) và mỗi 10 phút:
1. Tìm danh tính theo email (`credentials_identifier`). Chưa có thì tạo qua API quản trị của Kratos:
   - traits: email, tên "Quản trị viên F-Eco", ngôn ngữ `vi`;
   - mật khẩu;
   - địa chỉ email đã xác minh;
   - trạng thái `active`.
2. Danh tính đang `inactive` thì bật lại (`active`).
3. Ghi `system_admins(subject, email)` (migration 0018).
4. Chưa là Quản trị viên với đủ năm vai trò (Kiểm duyệt, Hỗ trợ, Quản lý dự án, Tài chính, Quản trị hệ thống) thì đặt lại bằng `roles::set_role`, người thực hiện "hệ thống".
5. Mỗi lần tạo hoặc khôi phục thì ghi nhật ký kiểm toán `admin.bootstrap`.

Kratos hoặc PostgreSQL chưa sẵn sàng lúc khởi động thì ghi lỗi và thử lại ở lượt 10 phút sau. Không chặn khởi động, vì `/health` cần trả lời để hạ tầng biết máy chủ đang chờ.

**Bảo vệ**:
- `roles::set_role` từ chối hạ vai trò, hoặc bỏ vai trò Quản trị hệ thống, của một subject có trong `system_admins` (`RoleError::ProtectedAccount`). Áp dụng cho bảng quản trị, `feco-catalog role-set` và mọi đường khác, vì tất cả đều đi qua `set_role`.
- Xóa tài khoản (`/account/delete`) bị từ chối với tài khoản này.
- Kratos vẫn là nơi lưu mật khẩu và xác thực hai lớp. Bảng quản trị vẫn đòi passkey như mọi quản trị viên khác.

## K2. Quản trị viên chuyển F-Coin

**Biểu mẫu** ở `/admin/wallet` (vai trò Tài chính):
- email người nhận;
- số F-Coin, từ 1 đến 1.000.000;
- nguồn: `own` (Ví của tôi) hoặc `platform` (Quỹ nền tảng);
- lý do, bắt buộc, tối đa 500 ký tự;
- khóa `idem`: UUID sinh khi hiện trang, để gửi lại cùng biểu mẫu không chuyển hai lần.

**Kiểm**:
- người nhận có danh tính trong Kratos, ví không bị khóa;
- người nhận khác người gửi;
- số F-Coin và lý do hợp lệ.

**Bút toán** (`wallet::post`, khóa `admin-transfer:{idem}`):

| Nguồn | Điều kiện | Bút toán |
|---|---|---|
| Ví của tôi | luôn ghi ngay | `admin_transfer`: ví quản trị viên −N, ví người nhận +N. `post` đã kiểm ví người gửi đủ F-Coin và không bị khóa |
| Quỹ nền tảng | N × tỷ giá < 1.000.000 ₫ | `admin_grant`: tài khoản hệ thống `issuance` −N, ví người nhận +N |
| Quỹ nền tảng | N × tỷ giá ≥ 1.000.000 ₫ | tạo `wallet_grants` (mã `CP-000001`) trạng thái `pending`; quản trị viên Tài chính khác duyệt thì ghi `admin_grant`, hoặc từ chối kèm lý do |

Người tạo yêu cầu cấp F-Coin không tự duyệt được.

**Thông báo và kiểm toán**:
- Người nhận nhận thông báo loại Thanh toán (kèm email): "Bạn nhận được N F-Coin", lý do, mã giao dịch.
- Nhật ký: `wallet.admin_transfer`, `wallet.admin_grant`, `wallet.grant_request`, `wallet.grant_reject`.

**Hiển thị**:
- Lịch sử của người dùng: "Nhận F-Coin từ F-Eco" (khoản cộng) hoặc "Chuyển F-Coin" (khoản trừ).
- Thống kê: nhóm "Chuyển khoản".
- `wallet-check` không đổi: mọi bút toán vẫn cân.

## K3. Các task còn lại của add-f-ecosystem-platform

| Task | Làm trong change này | Còn chờ |
|---|---|---|
| 1.3 | Job CI `secrets-selftest`: chạy gitleaks trên thư mục mẫu có khóa giả (chỉ tạo lúc chạy, không commit) và yêu cầu gitleaks báo lỗi | — |
| 2.4 | Xác minh: E2E `totp_recovery_codes_and_second_factor_login`, `developer_portal_end_to_end` (nhà phát triển phải bật hai lớp), `admin_console_end_to_end` (quản trị viên phải đăng nhập bằng passkey, kiểm bằng khóa mềm WebAuthn) | — |
| 2.5 | — | Cảnh báo refresh token bị dùng lại: launcher đổi refresh token trực tiếp với Hydra, F-Eco không thấy sự kiện này; cần Hydra phát sự kiện (QUESTIONS.md 95) |
| Còn lại | — | Hạ tầng đám mây, luật sư, máy Mac, chứng thư ký mã, kiểm thử xâm nhập độc lập, tên miền email, client OAuth Google/GitHub, sandbox cổng thanh toán |

## K4. Rà soát bảo mật và chức năng

Ba lượt rà soát độc lập, mỗi lượt chỉ đọc mã:
1. Xác thực, phiên, OAuth, CSRF, phân quyền, HTML.
2. Luồng tiền: ví, nạp, rút, mua app, ký quỹ, đơn hàng, cổng thanh toán.
3. Launcher và chuỗi tin cậy TUF.

Mỗi lỗi được xác nhận thì có một test chứng minh trước khi sửa. Bảng lỗi và cách sửa nằm trong tasks.md, nhóm 4.
