# Proposal

## Why

Chủ dự án muốn người dùng chọn vai trò ngay khi đăng ký tài khoản F-Eco, gồm năm vai trò:
- Người dùng phổ thông;
- Khách hàng cá nhân;
- Khách hàng doanh nghiệp;
- Lập trình viên cá nhân;
- Công ty phát triển phần mềm.

Vai trò khác Người dùng phổ thông phải được quản trị viên xác nhận (duyệt hoặc từ chối).

Hiện trạng (bản 1.6.0, thay đổi fcoin-wallet-and-marketplace):
- Trang đăng ký chỉ hỏi email, tên, ngôn ngữ, mật khẩu. Mọi tài khoản mới là Người dùng phổ thông.
- Muốn vai trò khác thì sau khi đăng nhập phải vào trang Vai trò (`/account/role`) gửi yêu cầu. Quản trị hệ thống duyệt hoặc từ chối ở trang Phân quyền (`/admin/members`).

## What Changes

- **Trang đăng ký** có thêm ô "Vai trò" với năm lựa chọn, mặc định Người dùng phổ thông. Có thêm hai ô "Tên pháp lý" và "Mã số thuế", bắt buộc với Khách hàng doanh nghiệp và Công ty phát triển phần mềm.
  - Thiếu hoặc sai mã số thuế: F-Eco báo lỗi ngay trên trang đăng ký và không tạo tài khoản.
  - Các ô này là trường của F-Eco (`feco_role`, `feco_legal_name`, `feco_tax_code`), không gửi tới Kratos và không lưu vào hồ sơ Kratos.
- **Đăng ký thành công với vai trò khác Người dùng phổ thông**:
  - F-Eco tạo yêu cầu vai trò đang chờ, ghi chú "Chọn khi đăng ký";
  - quản trị viên nhận cảnh báo như yêu cầu gửi từ trang Vai trò;
  - trong lúc chờ, tài khoản là Người dùng phổ thông.
- **Trang "Đã gửi email xác minh"**: khi chọn vai trò cần duyệt thì trang này nói thêm vai trò đó cần quản trị viên duyệt. Email đã có tài khoản cũng thấy đúng trang đó, để không lộ email nào đã đăng ký; trường hợp này không tạo yêu cầu.
- **Trang Phân quyền**: mỗi yêu cầu ghi thêm hai thông tin để quản trị viên quyết định:
  - nguồn: "Chọn khi đăng ký" hoặc "Trang Vai trò";
  - email đã xác minh hay chưa.
- **Kết quả**: duyệt hay từ chối (kèm lý do) giữ như hiện nay. Người đăng ký nhận thông báo và email; trang Vai trò hiện kết quả.

## Capabilities

### New Capabilities

Không có.

### Modified Capabilities

- `account-identity`: chọn vai trò khi đăng ký.
- `admin-console`: yêu cầu vai trò hiện nguồn và trạng thái xác minh email.

## Impact

- Máy chủ:
  - `auth.rs`: đọc và kiểm `feco_role` khi gửi biểu mẫu đăng ký; tạo yêu cầu vai trò sau khi Kratos tạo tài khoản;
  - `views.rs`: thêm ô vai trò vào biểu mẫu đăng ký;
  - `roles.rs`: hằng `REGISTRATION_NOTE`, danh sách vai trò chọn được khi đăng ký;
  - `admin.rs` và `admin.html`: nguồn và trạng thái xác minh email.
- Không đổi schema của Kratos, không thêm migration (dùng bảng `role_requests` có sẵn).
- Chuỗi giao diện bốn thứ tiếng.
