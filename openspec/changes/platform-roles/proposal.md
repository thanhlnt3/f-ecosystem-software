# Proposal

## Why

Hiện nay ai có tài khoản F-Eco cũng tự mở được tài khoản nhà phát triển và nộp app. Quản trị viên lại được quản lý ở một chỗ riêng (`admin_members`). Chủ dự án cần ba vai trò rõ ràng trên toàn nền tảng:
- **Quản trị viên**;
- **Lập trình viên**: được phát hành app;
- **Người sử dụng ứng dụng**: chỉ cài và dùng app.

Ngoài ra còn ba việc dọn dẹp:
- Demo App là app thử nghiệm của bản 1.0, không nên nằm trong kho ứng dụng nữa.
- Tên các phong cách giao diện dùng tên họa sĩ, gây vướng mắc về quyền nhân thân (QUESTIONS.md 75). Nên đổi sang tên phổ thông.

## What Changes

- **Ba vai trò nền tảng** cho mỗi tài khoản:
  - **Người sử dụng ứng dụng** (mặc định): đăng nhập, cài và dùng app, đặt đơn xây app.
  - **Lập trình viên**: thêm quyền vào Cổng nhà phát triển, tạo app, nộp phiên bản, dùng `f-dev`.
  - **Quản trị viên**: thêm quyền vào bảng quản trị (vẫn bắt buộc passkey, vẫn chia 5 vai trò con như cũ) và quyền phân quyền cho người khác. Quản trị viên cũng có mọi quyền của Lập trình viên.
- **Phân quyền** trong bảng quản trị (vai trò con Quản trị hệ thống):
  - tìm người dùng theo email, đổi vai trò, phải nhập lý do;
  - ghi vào nhật ký kiểm toán;
  - không tự hạ quyền của chính mình, và luôn còn ít nhất một Quản trị hệ thống.
- **Yêu cầu quyền Lập trình viên**: người dùng mở Cổng nhà phát triển mà chưa có quyền thì thấy nút gửi yêu cầu. Quản trị viên duyệt hoặc từ chối, người dùng nhận thông báo.
- **Cổng nhà phát triển và API `f-dev` kiểm vai trò**:
  - token `f-dev` của người bị thu hồi quyền Lập trình viên bị từ chối ngay;
  - thành viên nhóm nhà phát triển có từ trước được cấp sẵn vai trò Lập trình viên (migration).
- **F-Launcher hiện vai trò** của tài khoản ở thanh bên. Lập trình viên có lối tắt tới Cổng nhà phát triển; Quản trị viên có lối tắt tới bảng quản trị.
- **Công cụ dòng lệnh**: `feco-catalog role-set` và `role-requests`.
- **Gỡ Demo App khỏi kho ứng dụng**:
  - danh mục F-Store dev và kho TUF dev không còn Demo App;
  - `scripts/dev-launcher.sh setup` không đưa Demo App vào kho nữa, nhưng vẫn tạo client OAuth để thử đăng nhập một lần (README bước 5).
- **Đổi tên phong cách** sang tên phổ thông. Mã lưu trong `settings.json` đổi theo, nhưng vẫn đọc được mã cũ:

  | Tên cũ | Tên mới |
  |---|---|
  | Leonardo da Vinci | Giấy da |
  | Michelangelo | Bích họa |
  | Vincent van Gogh | Trời sao |
  | Claude Monet | Hồ sen |
  | Pablo Picasso | Lập thể |
  | Tô Ngọc Vân | Hoa huệ |
  | Bùi Xuân Phái | Phố cổ |
  | Nguyễn Gia Trí | Sơn mài |

## Capabilities

### New Capabilities

Không có.

### Modified Capabilities

- `account-identity`: vai trò nền tảng của tài khoản; phân quyền; yêu cầu quyền Lập trình viên.
- `launcher`: hiện vai trò và lối tắt theo vai trò.

## Impact

- Máy chủ:
  - migration `0013_platform_roles.sql` (bảng `role_grants`, `role_requests`);
  - mô-đun mới `roles.rs`;
  - `dev_web.rs` (chặn theo vai trò, trang yêu cầu);
  - `admin.rs` (trang Phân quyền);
  - endpoint `GET /v1/account/role` cho F-Launcher;
  - `feco-catalog`.
- F-Launcher: `auth.rs` (đọc vai trò), `main.rs` (thanh bên), `style.rs` và `i18n.rs` (tên phong cách).
- Dữ liệu dev: danh mục và kho TUF dev không còn Demo App.
- Tương thích: nhà phát triển hiện có giữ quyền nhờ migration; `settings.json` dùng mã phong cách cũ vẫn đọc được.
