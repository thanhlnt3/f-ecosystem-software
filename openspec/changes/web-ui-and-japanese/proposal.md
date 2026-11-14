# Proposal

## Why

Chủ dự án cần ba thay đổi:
- **Tiếng Nhật** cho cả F-Launcher và trang web.
- **Tùy chỉnh giao diện trên web** như trên F-Launcher: ngôn ngữ, chủ đề, màu nhấn, phong cách. Trang web hiện chỉ có tiếng Việt, tiếng Anh và một kiểu giao diện cố định.
- **Bảng quản trị chuyên nghiệp hơn.** Hiện bảng quản trị là một trang chữ với danh sách liên kết, không có trang tổng quan để người quản trị thấy ngay việc cần làm.

Ở môi trường dev, bảng quản trị bắt buộc passkey, nhưng trình duyệt không cho tạo passkey trên địa chỉ IP `127.0.0.1` (QUESTIONS.md mục 5). Vì vậy người vận hành không thử được bảng quản trị trên máy mình.

## What Changes

- **Tiếng Nhật**:
  - **F-Launcher**: thêm 日本語 vào danh sách ngôn ngữ (Tiếng Việt, English, Français, 日本語).
  - **Web**: thêm 日本語 bên cạnh tiếng Việt và tiếng Anh. Mọi chuỗi giao diện web có bản tiếng Nhật, có test bảo đảm.
  - Thông báo do hệ thống tạo mà chưa có bản tiếng Nhật (email, thông báo nghiệp vụ) hiện bằng tiếng Anh.
- **Tùy chỉnh giao diện web** ở trang mới `/preferences` (Giao diện):
  - ngôn ngữ; chủ đề sáng, tối hoặc theo hệ thống; màu nhấn;
  - phong cách: 10 phong cách giống F-Launcher;
  - mật độ: thường hoặc gọn; cỡ chữ.

  Tùy chỉnh lưu trong cookie trên trình duyệt, không gửi lên tài khoản. Máy chủ áp tùy chỉnh khi trả trang, nên trang hiện đúng giao diện ngay, không cần JavaScript.
- **Bố cục web mới**:
  - thanh trên cùng có thương hiệu, liên kết chính (Trang chủ, Cửa hàng, Đơn hàng, Thanh toán, Thông báo, Nhà phát triển, Quản trị), chọn ngôn ngữ nhanh và liên kết Giao diện;
  - thẻ, nút, bảng theo hệ biến CSS của phong cách đang chọn.
- **Dashboard quản trị**:
  - **thanh bên** theo vai trò con;
  - **trang tổng quan** có thẻ số liệu, mỗi thẻ dẫn tới mục tương ứng:
    - yêu cầu quyền Lập trình viên chờ duyệt;
    - phiên bản chờ duyệt thủ công;
    - đơn sắp trễ;
    - lệnh hoàn chờ phê duyệt;
    - số người dùng, Lập trình viên, Quản trị viên;
    - doanh thu 30 ngày;
  - **biểu đồ cột** doanh thu 14 ngày, vẽ bằng HTML và CSS.

  Mỗi thẻ chỉ hiện với người có vai trò con tương ứng.
- **Bỏ yêu cầu passkey chỉ ở dev**:
  - cấu hình `FECO_ADMIN_PASSKEY_OPTIONAL=true` chỉ được nhận khi `FECO_ENV=dev`; staging và production từ chối khởi động;
  - khi bật, bảng quản trị hiện cảnh báo "Chế độ dev: không bắt buộc passkey".
- Bảng màu phong cách chuyển vào `feco-core` (`ui_palette`), để F-Launcher và web dùng chung một nguồn.

## Capabilities

### New Capabilities

Không có.

### Modified Capabilities

- `platform-support`: ngôn ngữ web có thêm tiếng Nhật; tùy chỉnh giao diện web.
- `admin-console`: dashboard tổng quan; ngoại lệ passkey chỉ ở dev.
- `launcher`: tiếng Nhật.

## Impact

- `crates/feco-core`: mô-đun mới `ui_palette` (bảng màu, độ tương phản), có test.
- `crates/feco-config`: `FECO_ADMIN_PASSKEY_OPTIONAL`.
- `services/feco-server`:
  - `i18n.rs` (`Lang::Ja`), `i18n_ja.rs` (bảng tiếng Nhật);
  - `ui_prefs.rs` (cookie, middleware gắn thuộc tính giao diện, trang `/preferences`);
  - `templates/base.html` (bố cục và CSS mới), `templates/preferences.html`, `templates/admin.html` (dashboard);
  - `admin.rs` (số liệu tổng quan).
- `clients/feco-launcher`: `i18n.rs` (`Language::Ja` và cột tiếng Nhật), `style.rs` (dùng `feco_core::ui_palette`).
- Tương thích:
  - cookie `feco_lang` cũ vẫn đọc được;
  - `settings.json` của launcher không đổi định dạng (thêm giá trị `ja`).
