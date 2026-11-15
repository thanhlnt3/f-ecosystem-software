# Spec Delta

## Purpose

Bổ sung cho capability `launcher`: icon kèm tooltip, cửa sổ riêng cho Cài đặt và chi tiết app, ô lưới đều nhau.

## ADDED Requirements

### Requirement: Nút icon kèm tooltip
F-Launcher SHALL dùng icon thay cho chữ ở các nút công cụ: Cài đặt, Lưới, Danh sách, Xóa bộ lọc, Đồng bộ, Đăng xuất, Bảng quản trị, Cổng nhà phát triển, menu thao tác của app, Mã nguồn, Website, Quay lại bản trước, Gỡ, app trước và app sau. Mỗi nút chỉ có icon MUST có tooltip (đã dịch) nói nút làm gì. Icon MUST theo màu của phong cách và chủ đề đang chọn, và không cần phông chữ hay tệp ảnh riêng.

#### Scenario: Chuyển lưới bằng icon
- **WHEN** người dùng rê chuột lên icon lưới ở thanh công cụ
- **THEN** tooltip hiện "Lưới" (theo ngôn ngữ đang chọn), và bấm vào thì danh sách chuyển sang dạng lưới

#### Scenario: Icon vẽ trong ô của nó
- **WHEN** kiểm thử tự động vẽ từng icon vào một ô
- **THEN** mọi icon đều vẽ ra hình, nằm trong ô và dùng đúng màu được truyền vào

### Requirement: Cửa sổ riêng cho Cài đặt và chi tiết app
Bấm Cài đặt hoặc bấm vào một app SHALL mở một cửa sổ riêng của hệ điều hành, cửa sổ chính vẫn giữ trang đang xem. Mỗi app MUST mở trong một cửa sổ chi tiết riêng, tối đa 6 cửa sổ. Bấm lại một app hoặc Cài đặt đang mở thì cửa sổ đó MUST được đưa lên trước, không mở thêm. Esc MUST đóng cửa sổ (trừ khi đang gõ chữ). Hộp thoại xác nhận cài, gỡ MUST hiện ở cửa sổ nơi người dùng bấm.

#### Scenario: Mở chi tiết trong cửa sổ mới
- **WHEN** người dùng bấm vào ô của app "F-Notes" trong Cửa hàng
- **THEN** một cửa sổ "F-Notes · F-Launcher" mở ra với trang chi tiết, và cửa sổ chính vẫn hiện Cửa hàng

#### Scenario: Bấm lại app đang mở
- **GIVEN** cửa sổ chi tiết của F-Notes đang mở
- **WHEN** người dùng bấm lại F-Notes
- **THEN** cửa sổ đó được đưa lên trước, không có cửa sổ thứ hai

### Requirement: Ô lưới đều nhau
Ở dạng lưới (Thư viện và Cửa hàng), mọi ô SHALL có cùng chiều rộng và chiều cao, xếp liền nhau theo hàng và trải đều chiều rộng cửa sổ. Ô MUST không hẹp hơn cỡ ô người dùng chọn. Nội dung dài hơn ô MUST bị cắt, không làm ô cao lên.

#### Scenario: Ô lưới đều nhau
- **WHEN** cửa sổ rộng 900 điểm, cỡ ô 180 điểm, khoảng cách 12 điểm
- **THEN** lưới có 4 cột cùng chiều rộng, tổng chiều rộng không vượt quá cửa sổ, và mọi ô cùng chiều cao dù mô tả app dài ngắn khác nhau
