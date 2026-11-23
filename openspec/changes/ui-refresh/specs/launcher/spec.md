## MODIFIED Requirements

### Requirement: Phong cách giao diện
F-Launcher SHALL cho người dùng chọn phong cách giao diện trong Cài đặt, mỗi phong cách có ô xem trước. Danh sách gồm:
- **Chân phương**: góc gần vuông, phẳng;
- **Trang nhã** (mặc định từ 1.20.0): nút và thẻ bo tròn;
- các phong cách có bảng màu riêng: Giấy da, Bích họa, Trời sao, Hồ sen, Lập thể, Hoa huệ, Phố cổ, Sơn mài. Tên phổ thông theo thay đổi `platform-roles`; bản 1.2 dùng tên họa sĩ.

Cài đặt mới SHALL dùng chủ đề sáng và ô lưới lớn. Cài đặt đã lưu MUST được giữ nguyên. Đổi phong cách MUST có hiệu lực ngay và MUST được nhớ cho lần mở sau.

#### Scenario: Chọn phong cách trang nhã
- **WHEN** người dùng chọn "Trang nhã"
- **THEN** các nút và thẻ app hiện bo tròn ngay

#### Scenario: Chọn phong cách Trời sao
- **WHEN** người dùng chọn "Trời sao"
- **THEN** launcher dùng nền xanh đêm thẫm với màu nhấn vàng, và dải họa tiết màu ở thanh bên. Ô chọn chủ đề và màu nhấn bị tắt, kèm lời giải thích rằng phong cách này dùng bảng màu riêng

#### Scenario: Khôi phục mặc định
- **GIVEN** người dùng đang dùng tiếng Pháp và phong cách "Phố cổ"
- **WHEN** người dùng bấm "Khôi phục mặc định"
- **THEN** launcher về tiếng Việt và phong cách Trang nhã

#### Scenario: Cài đặt cũ thiếu phong cách
- **GIVEN** tệp `settings.json` của bản 1.1 không có trường phong cách
- **WHEN** launcher đọc tệp
- **THEN** launcher dùng phong cách Trang nhã và giữ các lựa chọn khác trong tệp
