# Design

## J1. Trường vai trò nằm ở F-Eco, không ở Kratos

Có hai cách:
- **Thêm trait vào schema danh tính của Kratos**: vai trò xin sẽ nằm mãi trong hồ sơ, người dùng sửa được ở trang cài đặt, và phải sửa schema ở cả Docker lẫn môi trường dev.
- **Trường của F-Eco trong biểu mẫu**: chọn cách này. feco-server vốn nhận POST của biểu mẫu đăng ký rồi chuyển tiếp tới Kratos, và đã bỏ mọi trường `feco_*` trước khi chuyển. Sau khi Kratos trả 200 kèm `identity.id`, F-Eco tạo yêu cầu vai trò bằng `roles::request_role` có sẵn.

Các trường:

| Trường | Giá trị | Ghi chú |
|---|---|---|
| `feco_role` | `user`, `customer`, `business_customer`, `developer`, `dev_company` | Giá trị lạ thì báo lỗi, không tạo tài khoản |
| `feco_legal_name` | tên pháp lý, tối đa 200 ký tự | Bắt buộc với `business_customer`, `dev_company` |
| `feco_tax_code` | 10 số, hoặc 10 số và `-` 3 số | Bắt buộc với `business_customer`, `dev_company`; kiểm bằng `payments::valid_tax_code` |

## J2. Kiểm trước khi tạo tài khoản

`submit_flow` (luồng đăng ký) đọc `RoleChoice` từ biểu mẫu trước khi bỏ các trường `feco_*`.
- `RoleChoice::validate` sai thì không gửi tới Kratos. F-Eco lấy lại luồng và hiện lại trang đăng ký:
  - giữ email, tên, vai trò, tên pháp lý, mã số thuế đã nhập (mật khẩu thì không, như mọi lỗi khác);
  - thông báo lỗi nằm dưới ô sai.
- Kratos trả lỗi (ví dụ mật khẩu yếu): trang hiện lại vẫn giữ lựa chọn vai trò.

## J3. Sau khi tạo tài khoản

- **Kratos trả 200**: nếu vai trò khác `user`, F-Eco gọi `request_role(subject = identity.id, email, role, note = REGISTRATION_NOTE, org)`.
  - Lỗi khi tạo yêu cầu chỉ ghi nhật ký; tài khoản vẫn được tạo, người dùng gửi lại được ở trang Vai trò.
  - `request_role` có sẵn đã lo phần còn lại: ghi `role.request` vào nhật ký kiểm toán, gửi cảnh báo `role_request` cho quản trị viên, mỗi người chỉ một yêu cầu đang chờ.
- **Email đã có tài khoản**: Kratos trả lỗi `DUPLICATE_IDENTIFIER`. F-Eco hiện cùng trang "Đã gửi email" như đăng ký mới và không tạo yêu cầu.
- **Trang "Đã gửi email"**: với vai trò cần duyệt thì thêm dòng "Vai trò X cần quản trị viên duyệt..." ở cả hai trường hợp trên. Nhờ vậy trang không cho biết email đã đăng ký hay chưa (spec account-identity, "Email đã được dùng").

## J4. Trang Phân quyền

Mỗi yêu cầu đang chờ có thêm:
- nhãn nguồn: "Chọn khi đăng ký" khi `note` là `REGISTRATION_NOTE`, ngược lại "Trang Vai trò";
- trạng thái email: "Email đã xác minh" hoặc "Email chưa xác minh". Đọc từ `verifiable_addresses` của danh tính qua API quản trị của Kratos; Kratos không trả lời được thì không hiện dòng này.

Duyệt và từ chối dùng `roles::decide` như cũ:
- duyệt: gán vai trò, ghi `role.set`;
- từ chối: bắt buộc lý do, ghi `role.request_rejected`;
- cả hai trường hợp đều gửi thông báo bắt buộc kèm email.

## J5. Kiểm thử

- Unit test: `RoleChoice::from_form`, `RoleChoice::validate` (thiếu tên pháp lý, mã số thuế sai, vai trò lạ, `user` không cần gì); biểu mẫu đăng ký có ô vai trò với năm lựa chọn.
- E2E:
  - đăng ký với Khách hàng doanh nghiệp: yêu cầu đang chờ có tên pháp lý và mã số thuế; trang Phân quyền ghi "Chọn khi đăng ký" và "Email chưa xác minh"; quản trị viên duyệt thì vai trò đổi và có email;
  - đăng ký Công ty thiếu mã số thuế: báo lỗi, không có tài khoản;
  - đăng ký Lập trình viên cá nhân rồi bị từ chối: có lý do trong email, vai trò vẫn là Người dùng phổ thông;
  - đăng ký Người dùng phổ thông: không có yêu cầu;
  - email đã có tài khoản: cùng trang, không có yêu cầu mới.
