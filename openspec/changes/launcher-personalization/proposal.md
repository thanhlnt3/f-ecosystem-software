# Proposal

## Why

F-Launcher 1.1 chỉ có giao diện tiếng Việt. Người dùng chỉ đổi được chủ đề sáng hoặc tối và màu nhấn. Người dùng nói tiếng Anh hoặc tiếng Pháp không đọc được launcher. Người dùng cũng muốn launcher mang dáng vẻ riêng hơn: nút bo tròn, hoặc bảng màu gợi nhớ tranh của các họa sĩ họ yêu thích.

## What Changes

- **Ngôn ngữ giao diện của launcher**: tiếng Việt (mặc định), tiếng Anh, tiếng Pháp.
  - Người dùng chọn trong Cài đặt; đổi ngay, và được nhớ cho lần mở sau.
  - Mọi chữ trên giao diện launcher đều có bản dịch, kể cả tên nhóm, kiểu sắp xếp và tên phong cách.
  - Nội dung do nhà phát triển viết (mô tả, tính năng, ghi chú phát hành) giữ nguyên ngôn ngữ gốc.
- **Phong cách giao diện**, chọn trong Cài đặt kèm ô xem trước:
  - **Chân phương** (mặc định): góc gần vuông, phẳng, theo chủ đề sáng, tối hoặc hệ thống và màu nhấn người dùng chọn, như 1.1;
  - **Trang nhã**: nút và thẻ bo tròn, khoảng cách rộng hơn, vẫn theo chủ đề và màu nhấn;
  - **Phong cách họa sĩ**: Leonardo da Vinci, Michelangelo, Vincent van Gogh, Claude Monet, Pablo Picasso, Tô Ngọc Vân, Bùi Xuân Phái, Nguyễn Gia Trí. Mỗi phong cách có bảng màu, độ bo góc và dải họa tiết màu riêng, lấy cảm hứng từ cách dùng màu thường thấy trong tranh của họa sĩ.
  - Phong cách chỉ dùng màu và hình khối do F-Eco tự vẽ, không dùng hình ảnh hay bản sao tác phẩm nào.

## Capabilities

### New Capabilities

Không có.

### Modified Capabilities

- `launcher`: ngôn ngữ giao diện (vi, en, fr); phong cách giao diện; cả hai lưu trong cài đặt trên máy.

## Impact

- Mã trong `clients/feco-launcher`:
  - `i18n.rs` (mới): bảng dịch, `t`, `tf`;
  - `style.rs` (mới): các phong cách, bảng màu, độ tương phản;
  - `prefs.rs`: thêm `language`, `style`;
  - `main.rs`: mọi chuỗi giao diện đi qua bảng dịch, áp phong cách, trang Cài đặt mới;
  - `library.rs`: nhãn nhóm và nhãn sắp xếp được dịch.
- Không đổi máy chủ, metadata TUF hay F-SDK.
- `settings.json` cũ vẫn đọc được: thiếu `language` thì dùng tiếng Việt, thiếu `style` thì dùng Chân phương.
