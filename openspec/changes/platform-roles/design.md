# Design

## Context

- Bảng quản trị (`admin.rs`, migration 0012) cho phép thành viên trong `admin_members` vào, với 5 vai trò con: Người duyệt app, Hỗ trợ, Quản lý dự án, Tài chính, Quản trị hệ thống. Phiên phải có passkey.
- Cổng nhà phát triển (`dev_web.rs`, migration 0005) cho mọi người dùng đã xác minh email và bật hai lớp tự đăng ký tài khoản nhà phát triển. Token `f-dev` gắn với thành viên của tài khoản đó.
- F-Launcher đăng nhập bằng client OAuth của nền tảng. Máy chủ đã nhận diện token của launcher (dùng cho mã khởi chạy).

## Goals / Non-Goals

**Goals**
- Mỗi tài khoản có đúng một vai trò nền tảng, tính được ở mọi nơi bằng một hàm.
- Thu hồi quyền có hiệu lực ngay, cả với token `f-dev` đã cấp.
- Không làm mất quyền của nhà phát triển và quản trị viên hiện có.

**Non-Goals**
- Đổi 5 vai trò con của quản trị viên.
- Phân quyền chi tiết trong nhóm nhà phát triển (Chủ sở hữu, Quản trị, Phát triển, Chỉ xem giữ nguyên).
- Lưu vai trò trong Kratos (traits hay metadata). Vai trò là dữ liệu phân quyền phía máy chủ, không cho người dùng tự sửa.

## Decisions

### R1. Vai trò nền tảng tính từ hai bảng

`roles::role_of(subject)` tính vai trò theo thứ tự:
1. có dòng trong `admin_members` → **Quản trị viên** (giữ nguyên nơi lưu quản trị viên và vai trò con);
2. có dòng trong `role_grants` với `role = 'developer'` → **Lập trình viên**;
3. còn lại → **Người sử dụng ứng dụng**.

Quản trị viên có mọi quyền của Lập trình viên (`can_develop`). Phương án đã loại: một cột `role` duy nhất. Cách này buộc chép quản trị viên ra hai nơi, và dễ lệch với `admin_members`.

### R2. Đổi vai trò

Chỉ thực hiện qua `roles::set_role`. Hàm này được gọi từ trang Phân quyền (vai trò con Quản trị hệ thống) và từ `feco-catalog role-set`.
- **Lý do** là bắt buộc. Mỗi lần đổi ghi `audit` với hành động `role.set`, kèm vai trò cũ, vai trò mới, lý do.
- **Đặt Quản trị viên** cần ít nhất một vai trò con; ghi vào `admin_members`.
- **Đặt Lập trình viên** xóa dòng `admin_members` (nếu có) và ghi `role_grants`.
- **Đặt Người sử dụng** xóa cả hai.
- **Chặn khóa mình ra ngoài**:
  - không tự đổi vai trò của chính mình;
  - không được bỏ Quản trị hệ thống cuối cùng.

### R3. Yêu cầu quyền Lập trình viên

- Bảng `role_requests`: mỗi người dùng chỉ có một yêu cầu đang chờ (chỉ mục duy nhất có điều kiện).
- Khi có yêu cầu mới, quản trị viên nhận cảnh báo như các cảnh báo kiểm duyệt khác.
- Duyệt thì gọi `set_role(Developer)`. Duyệt hay từ chối đều gửi thông báo cho người yêu cầu, loại bắt buộc `Security`, có bản tiếng Anh.

### R4. Chặn theo vai trò ở Cổng nhà phát triển và API

- **`/developer`**: người chưa có quyền thấy trang "Cần quyền Lập trình viên", kèm trạng thái yêu cầu hoặc nút gửi yêu cầu. Trang không hiện biểu mẫu đăng ký tài khoản nhà phát triển.
- **Đăng ký, thao tác trên tài khoản, app, phiên bản**: kiểm `can_develop` ở `member()` và `register`.
- **API `f-dev`** (`token_member`): kiểm `can_develop` của chủ token ở mỗi lần gọi. Token của người bị thu hồi quyền trả `403 developer_role_required`.
- **Migration** cấp `developer` cho mọi `subject` đang là thành viên của một tài khoản nhà phát triển.

### R5. F-Launcher

- **`GET /v1/account/role`**: chỉ nhận access token của chính F-Launcher (cùng cách kiểm như `POST /v1/launch-codes`). Trả `{"role": "user"|"developer"|"admin", "admin_roles": […]}`.
- Launcher hỏi vai trò khi đồng bộ, rồi hiện nhãn vai trò dưới tên người dùng ở thanh bên:
  - Lập trình viên có nút "Cổng nhà phát triển";
  - Quản trị viên có thêm nút "Bảng quản trị".

  Cả hai nút mở trình duyệt; mọi kiểm soát quyền vẫn nằm ở máy chủ.
- Không hỏi được (ngoại tuyến) thì giữ vai trò lần trước.

### R6. Tên phong cách

- Đổi tên hiển thị sang tên phổ thông (proposal).
- Biến thể enum và mã lưu đổi theo: `parchment`, `fresco`, `starry_night`, `lotus_pond`, `cubist`, `lily`, `old_quarter`, `lacquer`.
- serde nhận thêm mã cũ (`davinci`, `michelangelo`, `vangogh`, `monet`, `picasso`, `to_ngoc_van`, `bui_xuan_phai`, `nguyen_gia_tri`), nên `settings.json` của 1.2 vẫn đọc đúng.
- Bảng màu giữ nguyên. Mô tả không còn nhắc tên họa sĩ.

### R7. Gỡ Demo App

- **F-Store**: `feco-catalog remove feco-demo-app` (đã có, ẩn app khỏi danh mục).
- **Kho TUF**: lệnh mới `feco-tuf remove --repo … --app …` gỡ mọi target của app khỏi `targets.json`, rồi ký lại targets, snapshot, timestamp.
  - Khác với thu hồi: app gỡ không còn trong danh mục launcher.
  - Máy đã cài vẫn giữ app (launcher không tự xóa app đã cài).
  - Bản phát hành thật vẫn dùng thu hồi khi có lý do an toàn. `remove` dùng để dọn app thử nghiệm.
- **`dev-launcher.sh setup`**: thôi ký và đưa Demo App vào danh mục; `publish` và `revoke` giờ thao tác trên app mẫu Ghi chú (`f-notes`).

## Risks / Trade-offs

- [Nhà phát triển mới phải chờ duyệt] → Cảnh báo tới quản trị viên ngay khi có yêu cầu. Spec cho quản trị viên 2 ngày làm việc.
- [Thu hồi quyền làm hỏng CI của nhà phát triển] → Lỗi `developer_role_required` nói rõ lý do; quản trị viên phải ghi lý do khi thu hồi.
- [Gỡ target khỏi TUF] → Chỉ dùng ở dev và cho app thử nghiệm. Thao tác được ký đầy đủ như mọi thay đổi khác của `targets.json`.
