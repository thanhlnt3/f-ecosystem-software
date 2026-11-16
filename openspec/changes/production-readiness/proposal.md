# Proposal

## Why

Chủ dự án chuẩn bị phát hành bản production và yêu cầu bốn việc:

1. Hoàn thành các nhóm chức năng còn lại trong spec.
2. Luôn có một tài khoản quản trị viên mặc định đang hoạt động.
3. Tài khoản quản trị viên chuyển được F-Coin cho một tài khoản người dùng khác.
4. Kiểm tra, quét các lỗi tiềm ẩn (lỗi bảo mật, lỗi chức năng) trước khi phát hành.

Hiện trạng:
- Change `add-f-ecosystem-platform` còn 21 task mở. Đa số chờ yếu tố bên ngoài mã nguồn: nhà cung cấp đám mây, luật sư, máy Mac, chứng thư ký mã, đơn vị kiểm thử xâm nhập, tên miền email. Một số task làm được ngay.
- Quản trị viên đầu tiên phải cấp bằng tay (`feco-catalog role-set`). Mất tài khoản đó (xóa, hạ vai trò, khóa) thì không còn ai vào bảng quản trị.
- Quản trị viên chưa có cách chuyển F-Coin cho người dùng (hỗ trợ, khuyến mại, bồi hoàn).

## What Changes

- **Tài khoản quản trị viên mặc định**:
  - mỗi lần khởi động và mỗi 10 phút, máy chủ bảo đảm tài khoản `FECO_ADMIN_EMAIL` tồn tại, đang hoạt động, email đã xác minh, và có đủ năm vai trò quản trị;
  - tài khoản này không bị hạ vai trò, bỏ vai trò Quản trị hệ thống, hay xóa qua bảng quản trị hoặc `feco-catalog`;
  - mật khẩu ban đầu đọc từ tệp bí mật `FECO_ADMIN_PASSWORD_FILE`, tối thiểu 12 ký tự. Production thiếu biến hoặc tệp thì không khởi động. Dev tự sinh mật khẩu ngẫu nhiên vào `.dev/admin-password` (không commit);
  - mật khẩu chỉ dùng khi tạo tài khoản; không bao giờ ghi cứng trong mã nguồn hay ghi vào nhật ký;
  - như mọi quản trị viên, phải bật xác thực hai lớp mới vào được bảng quản trị.
- **Quản trị viên chuyển F-Coin** (vai trò Tài chính), ở trang Ví F-Coin của bảng quản trị:
  - nguồn **Ví của tôi**: trừ ví của chính quản trị viên, ghi có ví người nhận ngay;
  - nguồn **Quỹ nền tảng**: phát hành thêm F-Coin (khuyến mại, bồi hoàn). Dưới 1.000.000 ₫ (theo tỷ giá hiện hành) thì ghi ngay. Từ 1.000.000 ₫ trở lên cần một quản trị viên Tài chính khác duyệt;
  - bắt buộc lý do; không chuyển cho chính mình; người nhận phải có tài khoản và ví không bị khóa;
  - chống gửi trùng bằng khóa idempotency trong biểu mẫu;
  - người nhận nhận thông báo và email; ghi nhật ký kiểm toán;
  - lịch sử, thống kê và kiểm sổ cái hiểu hai loại bút toán mới `admin_transfer`, `admin_grant`.
- **Các task còn lại làm được ngay** (change `add-f-ecosystem-platform`):
  - 1.3: CI tự kiểm gitleaks bắt được khóa giả;
  - 2.4: xác minh bắt buộc hai lớp cho nhà phát triển và quản trị viên;
  - 2.5: cảnh báo khi refresh token bị dùng lại.
- **Sửa lỗi tìm được khi rà soát** bảo mật và chức năng trước khi phát hành (liệt kê ở design.md, mục K4).

## Capabilities

### New Capabilities

Không có.

### Modified Capabilities

- `admin-console`: tài khoản quản trị viên mặc định; chuyển F-Coin.
- `fcoin-wallet`: bút toán chuyển và cấp F-Coin của quản trị viên.
- `platform-security`: các sửa lỗi bảo mật.

## Impact

- `crates/feco-config`: `AdminBootstrapConfig` (`FECO_ADMIN_EMAIL`, `FECO_ADMIN_PASSWORD_FILE`).
- Máy chủ:
  - module mới `bootstrap.rs`;
  - migration `0018_admin_bootstrap_and_transfers.sql` (bảng `system_admins`, `wallet_grants`, loại bút toán mới);
  - `roles.rs` (bảo vệ tài khoản mặc định), `account.rs` (không xóa được);
  - `wallet.rs`, `wallet_web.rs`, `admin_wallet.html` (chuyển F-Coin);
  - `kratos.rs` (`find_identity`, `create_identity`, `activate_identity`).
- CI: bước tự kiểm gitleaks.
