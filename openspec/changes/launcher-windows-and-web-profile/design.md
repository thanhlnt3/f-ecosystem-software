# Design

## L1. Icon vẽ bằng nét

Có ba cách làm icon:
- **Phông biểu tượng**: Segoe Fluent/MDL2 chỉ có trên Windows; đóng gói phông khác thì thêm tệp và giấy phép.
- **Ảnh SVG**: cần thêm thư viện vẽ SVG.
- **Vẽ bằng `egui::Painter`**: chọn cách này.

Module `icons.rs` vẽ 21 icon bằng đường thẳng, cung tròn và đa giác, theo màu chữ của trạng thái nút (thường, rê chuột, đang chọn, tắt). Như vậy icon đổi màu theo phong cách và chủ đề, giống hệt trên mọi hệ điều hành, không thêm tài sản hay giấy phép.

Có ba loại nút:
- `icons::button`: nút vuông chỉ có icon. Tooltip là bắt buộc trong chữ ký hàm, nên không thể quên.
- `icons::labeled`: icon và chữ, dùng cho mục thanh bên.
- `icons::filled`: icon và chữ trên nền màu nhấn, dùng cho Mở, Cài, Cập nhật, Đăng nhập.

## L2. Cửa sổ riêng

egui hỗ trợ nhiều cửa sổ hệ điều hành (viewport) với renderer wgpu đang dùng. Launcher dùng `show_viewport_immediate`: cửa sổ con vẽ trong cùng vòng lặp, dùng chung trạng thái của launcher, nên không phải đồng bộ dữ liệu giữa các cửa sổ.

- **Cài đặt**:
  - một cửa sổ (`ViewportId` cố định);
  - bấm lại khi đang mở thì gửi `ViewportCommand::Focus`.
- **Chi tiết app**:
  - mỗi cửa sổ có `key` riêng, không đổi khi chuyển sang app trước hoặc sau, để cửa sổ không bị tạo lại;
  - tối đa 6 cửa sổ; mở thêm thì đóng cửa sổ cũ nhất;
  - bấm lại app đang mở thì đưa cửa sổ của nó lên trước.
- **Hộp thoại xác nhận** (cài, gỡ) hiện ở cửa sổ nơi phát sinh thao tác (`dialog_viewport`). Đóng cửa sổ đó thì hủy hộp thoại.
- Mỗi cửa sổ chi tiết có thanh trạng thái riêng, để người dùng thấy tiến trình cài đặt ngay tại đó.

## L3. Ô lưới đều nhau

- Số cột là số lớn nhất sao cho ô không hẹp hơn cỡ ô đã chọn. Chiều rộng ô trải đều để hàng lấp đầy (`grid_columns`).
- Chiều cao ô cố định, tính từ cỡ ô, cỡ chữ và tùy chọn "hiện mô tả ngắn". Nội dung dài bị cắt.
- Ô được vẽ vào một hình chữ nhật cố định:
  - nền và viền vẽ bằng painter;
  - nội dung vẽ trong `Ui` con, không đẩy bố cục;
  - nút bên trong vẫn nhận chuột trước vùng bấm của cả ô.

## W1. Trang hồ sơ

`profile::load` hỏi song song (`tokio::join!`):
- vai trò nền tảng;
- phiên đồng ý trong Hydra (ứng dụng đã kết nối);
- phiên Kratos (thiết bị);
- số đơn hàng và thông báo chưa đọc;
- danh tính đầy đủ (để biết đã bật TOTP, khóa bảo mật hay mã khôi phục).

Mục nào lỗi thì hiện "—" và trang vẫn mở. Ảnh đại diện là chữ cái đầu trên nền màu cố định theo mã người dùng, không cần lưu ảnh.

## W2. Tiếng Pháp trên web

Cách làm giống tiếng Nhật:
- bảng `i18n_fr.rs`, tra trước bảng chính, thiếu thì dùng tiếng Anh;
- test chung `check_complete` kiểm cả bảng tiếng Nhật và tiếng Pháp: đủ khóa, cùng chỗ `{…}`, không có khóa thừa;
- schema danh tính nhận `locale = fr`.

## W3. Đếm người dùng

`admin::user_count` giữ kết quả 10 phút trong `AppState` (QUESTIONS 80): dashboard mở nhiều lần không gọi Kratos liệt kê danh tính mỗi lần. Số liệu có thể chậm tối đa 10 phút; dashboard chỉ cần con số tổng quan nên chấp nhận được.

## D1. Dữ liệu dev bền vững

`scripts/dev-local.sh` chạy Kratos và Hydra với SQLite trong `infra/.data`, và tự chạy migration trước khi khởi động. Biến `FECO_DEV_MEMORY=1` giữ cách cũ (trong bộ nhớ).
