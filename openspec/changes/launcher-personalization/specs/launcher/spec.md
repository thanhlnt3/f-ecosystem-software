# Spec Delta

## Purpose

Bổ sung cho capability `launcher`: chọn ngôn ngữ giao diện và phong cách giao diện của F-Launcher.

## ADDED Requirements

### Requirement: Ngôn ngữ giao diện của launcher
F-Launcher SHALL hiện giao diện bằng tiếng Việt, tiếng Anh hoặc tiếng Pháp, theo lựa chọn của người dùng trong Cài đặt. Mặc định là tiếng Việt. Đổi ngôn ngữ MUST có hiệu lực ngay, không cần khởi động lại, và MUST được nhớ cho lần mở sau. Tên mỗi ngôn ngữ trong danh sách chọn MUST viết bằng chính ngôn ngữ đó.

#### Scenario: Đổi sang tiếng Anh
- **GIVEN** launcher đang hiện tiếng Việt
- **WHEN** người dùng chọn "English" trong Cài đặt
- **THEN** thanh bên hiện "Library", "Store", "Notifications", "Settings" ngay, và vẫn hiện tiếng Anh khi mở lại launcher

#### Scenario: Đổi sang tiếng Pháp
- **WHEN** người dùng chọn "Français"
- **THEN** nút cài hiện "Installer", bộ lọc nhóm hiện "Toutes les catégories", và tên nhóm "Năng suất" hiện "Productivité"

### Requirement: Không còn chuỗi giao diện chưa dịch
Mọi chuỗi giao diện của launcher SHALL có bản tiếng Anh và tiếng Pháp, kể cả tên nhóm, kiểu sắp xếp, nhãn nền tảng và tên phong cách. Bản dịch MUST giữ đúng các chỗ điền giá trị của chuỗi gốc. Nội dung do nhà phát triển viết (tên app, mô tả, tính năng, ghi chú phát hành) SHALL giữ nguyên ngôn ngữ gốc.

#### Scenario: Thiếu bản dịch
- **WHEN** một lập trình viên thêm chuỗi giao diện mới mà không thêm bản tiếng Anh và tiếng Pháp
- **THEN** kiểm thử tự động báo lỗi và chỉ ra chuỗi còn thiếu

### Requirement: Phong cách giao diện
F-Launcher SHALL cho người dùng chọn phong cách giao diện trong Cài đặt, mỗi phong cách có ô xem trước. Danh sách gồm:
- **Chân phương** (mặc định): góc gần vuông, phẳng;
- **Trang nhã**: nút và thẻ bo tròn;
- các phong cách có bảng màu riêng: Giấy da, Bích họa, Trời sao, Hồ sen, Lập thể, Hoa huệ, Phố cổ, Sơn mài. Tên phổ thông theo thay đổi `platform-roles`; bản 1.2 dùng tên họa sĩ.

Đổi phong cách MUST có hiệu lực ngay và MUST được nhớ cho lần mở sau.

#### Scenario: Chọn phong cách trang nhã
- **WHEN** người dùng chọn "Trang nhã"
- **THEN** các nút và thẻ app hiện bo tròn ngay, chủ đề sáng hoặc tối và màu nhấn đang chọn vẫn giữ nguyên

#### Scenario: Chọn phong cách Trời sao
- **WHEN** người dùng chọn "Trời sao"
- **THEN** launcher dùng nền xanh đêm thẫm với màu nhấn vàng, và dải họa tiết màu ở thanh bên. Ô chọn chủ đề và màu nhấn bị tắt, kèm lời giải thích rằng phong cách này dùng bảng màu riêng

#### Scenario: Khôi phục mặc định
- **GIVEN** người dùng đang dùng tiếng Pháp và phong cách "Phố cổ"
- **WHEN** người dùng bấm "Khôi phục mặc định"
- **THEN** launcher về tiếng Việt và phong cách Chân phương

### Requirement: Phong cách đọc được và không sao chép tác phẩm
Mọi phong cách SHALL đủ tương phản theo WCAG 2.1:
- chữ thường trên nền: ít nhất 4,5:1;
- chữ mờ: ít nhất 3:1;
- chữ trên nút nhấn: ít nhất 4,5:1.

Phong cách họa sĩ MUST chỉ dùng màu và hình khối do F-Eco tự vẽ, MUST không dùng hình ảnh hay bản sao tác phẩm nào, và nhãn MUST nói rõ đây là cảm hứng từ bảng màu.

#### Scenario: Kiểm tra tương phản
- **WHEN** kiểm thử tự động tính tỉ lệ tương phản của mọi phong cách
- **THEN** mọi cặp màu chữ và nền đạt ngưỡng trên
