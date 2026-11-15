# Proposal

## Why

Chủ dự án yêu cầu tinh chỉnh tiếp giao diện:
- **F-Launcher**:
  - nhiều nút còn là chữ, chiếm chỗ và khó nhận ra (nút Cài đặt, nút chuyển Lưới/Danh sách, menu "...");
  - Cài đặt và trang chi tiết app chiếm vùng chính, nên người dùng mất danh sách đang xem;
  - ô trong lưới cao thấp khác nhau theo nội dung, hàng trông lởm chởm.
- **Trang web**: trang chủ của người đã đăng nhập chỉ là lời chào và một dãy liên kết, chưa ra dáng trang hồ sơ.

Ngoài ra còn hai câu hỏi đã có hướng xử lý:
- **QUESTIONS 79**: thêm tiếng Pháp cho web để đồng nhất với F-Launcher.
- **QUESTIONS 80**: dashboard quản trị không nên đếm lại toàn bộ danh tính mỗi lần mở.

Môi trường dev cũng mất mọi tài khoản mỗi lần khởi động lại, vì Kratos và Hydra chạy trong bộ nhớ.

## What Changes

- **F-Launcher**:
  - Nút công cụ dùng **icon kèm tooltip** thay cho chữ: Cài đặt, Lưới, Danh sách, Xóa bộ lọc, Đồng bộ, Đăng xuất, Bảng quản trị, Cổng nhà phát triển, menu thao tác, Mã nguồn, Website, Quay lại bản trước, Gỡ, app trước và sau.
  - Mục thanh bên và nút hành động chính (Mở, Cài, Cập nhật, Đăng nhập) có icon đi kèm chữ.
  - Icon vẽ bằng nét (không phông, không ảnh), theo màu của phong cách đang chọn.
  - **Cài đặt** và **chi tiết app** mở trong **cửa sổ riêng** của hệ điều hành. Mỗi app một cửa sổ, tối đa 6 cửa sổ chi tiết; bấm lại app đang mở thì đưa cửa sổ đó lên trước. Esc đóng cửa sổ. Hộp thoại xác nhận cài/gỡ hiện ở cửa sổ nơi người dùng bấm.
  - **Ô lưới đều nhau**: mọi ô cùng chiều rộng và chiều cao, xếp liền nhau và trải đều theo chiều rộng cửa sổ.
- **Web**:
  - **Trang hồ sơ** cho người đã đăng nhập: phần đầu (ảnh đại diện chữ cái, tên, email và trạng thái xác minh, vai trò, ngày tham gia, nút Chỉnh sửa hồ sơ và Đăng xuất); thẻ số liệu (ứng dụng đã kết nối, thiết bị, đơn hàng, thông báo chưa đọc); nhóm lối tắt Bảo mật (kèm trạng thái xác thực hai lớp), Dữ liệu và quyền riêng tư, Mua sắm, Công cụ theo vai trò.
  - **Tiếng Pháp** (QUESTIONS 79): thêm FR vào thanh trên cùng và trang Giao diện; mọi chuỗi giao diện web có bản tiếng Pháp.
- **Bảng quản trị**: số người dùng được đếm lại tối đa mỗi 10 phút (QUESTIONS 80).
- **Môi trường dev**: Kratos và Hydra lưu dữ liệu trong SQLite (`infra/.data`), nên tài khoản và client OAuth còn sau khi khởi động lại; `FECO_DEV_MEMORY=1` trở về chạy trong bộ nhớ.

## Capabilities

### New Capabilities

Không có.

### Modified Capabilities

- `launcher`: icon và tooltip, cửa sổ riêng cho Cài đặt và chi tiết app, ô lưới đều nhau.
- `account-identity`: trang hồ sơ.
- `platform-support`: tiếng Pháp trên web.
- `admin-console`: đếm người dùng có bộ nhớ đệm.

## Impact

- **Mã**:
  - `clients/feco-launcher/src/icons.rs` mới; `main.rs` (cửa sổ, lưới, nút); bảng dịch launcher;
  - `services/feco-server/src/profile.rs` mới, `i18n_fr.rs` mới; `i18n.rs`, `auth.rs`, `admin.rs`, `lib.rs`; `templates/home.html`, `base.html`;
  - `infra/kratos/identity.schema.json` (ngôn ngữ `fr`); `scripts/dev-local.sh`.
- **Phụ thuộc**: không thêm thư viện.
- **Tương thích**:
  - không đổi API;
  - cài đặt launcher cũ vẫn đọc được;
  - trang chủ giữ các mã phần tử mà kiểm thử và tài liệu dùng (`email`, `verify-status`).
