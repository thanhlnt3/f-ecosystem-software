# Spec Delta

## Purpose

Bổ sung cho capability `launcher` (thay đổi `add-f-ecosystem-platform`): cách sắp xếp và xem app, trang cài đặt giao diện, thông tin hiển thị của app, và mở app chạy trong terminal.

## ADDED Requirements

### Requirement: Bố cục lưới và danh sách
F-Launcher SHALL hiện "Thư viện" (app đã cài) và "Cửa hàng" (danh mục) theo một trong hai bố cục:
- **Lưới**: mỗi app là một ô có biểu tượng, tên, nhóm và nút chính (Mở, Cài hoặc Cập nhật);
- **Danh sách**: mỗi app là một hàng có biểu tượng, tên, mô tả ngắn, nhóm, phiên bản, dung lượng và các nút.

Người dùng MUST đổi được bố cục bằng một nút trên thanh công cụ. Lựa chọn MUST được nhớ cho lần mở launcher sau.

#### Scenario: Đổi sang danh sách
- **GIVEN** Thư viện đang ở bố cục lưới
- **WHEN** người dùng bấm nút "Danh sách"
- **THEN** các app hiện thành hàng, mỗi hàng có mô tả ngắn và phiên bản

#### Scenario: Nhớ bố cục
- **GIVEN** người dùng đã chọn bố cục danh sách
- **WHEN** người dùng đóng rồi mở lại launcher
- **THEN** Thư viện vẫn ở bố cục danh sách

### Requirement: Tìm, lọc và sắp xếp app
F-Launcher SHALL cho tìm app theo tên, mô tả ngắn, nhà phát triển hoặc nhóm. Tìm kiếm MUST không phân biệt hoa thường và không phân biệt dấu tiếng Việt.

Launcher SHALL cho lọc theo nhóm, và sắp xếp theo:
- tên (A–Z);
- lần mở gần nhất (chỉ Thư viện);
- ngày cài (chỉ Thư viện);
- nhóm;
- dung lượng (chỉ Cửa hàng).

Kiểu sắp xếp MUST được nhớ cho lần mở sau.

#### Scenario: Tìm không dấu
- **GIVEN** Thư viện có app "Ghi chú"
- **WHEN** người dùng gõ "ghi chu"
- **THEN** chỉ các app khớp, trong đó có "Ghi chú", được hiện

#### Scenario: Sắp xếp theo lần mở gần nhất
- **GIVEN** người dùng mở "Bấm giờ" rồi mở "Ghi chú"; app "kibi" chưa mở lần nào
- **WHEN** người dùng chọn sắp xếp "Mở gần đây"
- **THEN** thứ tự là "Ghi chú", "Bấm giờ", rồi "kibi"

#### Scenario: Lọc theo nhóm
- **WHEN** người dùng chọn nhóm "Công cụ" trong Cửa hàng
- **THEN** chỉ các app thuộc nhóm "Công cụ" được hiện, và nhãn bộ lọc cho biết đang lọc

#### Scenario: Không có kết quả
- **WHEN** chuỗi tìm không khớp app nào
- **THEN** launcher báo "Không có app nào khớp" kèm nút xóa bộ lọc

### Requirement: Cài đặt giao diện của launcher
F-Launcher SHALL có trang Cài đặt cho người dùng chỉnh:
- chủ đề: theo hệ thống, sáng hoặc tối;
- màu nhấn;
- cỡ chữ: 80–150%;
- bố cục mặc định: lưới hoặc danh sách;
- cỡ ô lưới: nhỏ, vừa hoặc lớn;
- trang mở đầu: Thư viện hoặc Cửa hàng;
- hiện mô tả ngắn;
- chế độ gọn.

Trang cũng gom tùy chọn tự động cập nhật, tài khoản, và thông tin phiên bản. Thay đổi MUST có hiệu lực ngay, không cần khởi động lại.

#### Scenario: Đổi chủ đề tối
- **WHEN** người dùng chọn chủ đề "Tối" trong Cài đặt
- **THEN** giao diện chuyển sang nền tối ngay, và vẫn tối khi mở lại launcher

### Requirement: Lưu cài đặt giao diện trên máy
F-Launcher SHALL lưu cài đặt giao diện trên máy người dùng và MUST không gửi lên máy chủ. Trang Cài đặt MUST có nút khôi phục mặc định. File cài đặt hỏng hoặc có giá trị ngoài khoảng MUST không làm launcher lỗi: launcher dùng giá trị mặc định cho trường đó.

#### Scenario: Khôi phục mặc định
- **GIVEN** người dùng đã đổi cỡ chữ thành 140% và màu nhấn thành cam
- **WHEN** người dùng bấm "Khôi phục mặc định"
- **THEN** cỡ chữ về 100%, màu nhấn và các tùy chọn khác về mặc định

#### Scenario: File cài đặt hỏng
- **GIVEN** `settings.json` bị ghi đè bằng dữ liệu không phải JSON, hoặc cỡ chữ là 900%
- **WHEN** người dùng mở launcher
- **THEN** launcher mở bình thường với giá trị mặc định cho phần hỏng, và cỡ chữ nằm trong khoảng 80–150%

### Requirement: Thông tin hiển thị của app
F-Launcher SHALL hiện mô tả ngắn, nhóm, nhà phát triển và liên kết mã nguồn của app. Các thông tin này MUST lấy từ metadata TUF đã được nền tảng ký, không lấy từ nguồn chưa ký.

Mỗi app SHALL có một biểu tượng chữ cái đầu với màu cố định theo mã app. App thiếu thông tin thì hiện nhóm "Khác" và không có mô tả.

#### Scenario: App có thông tin đầy đủ
- **GIVEN** bản phát hành của "Ghi chú" có mô tả ngắn, nhóm "Năng suất" và nhà phát triển "F-Eco"
- **WHEN** người dùng xem Cửa hàng ở bố cục danh sách
- **THEN** hàng của "Ghi chú" hiện mô tả ngắn, nhóm "Năng suất" và "F-Eco"

#### Scenario: Biểu tượng ổn định
- **WHEN** launcher vẽ biểu tượng của cùng một app ở hai lần mở khác nhau
- **THEN** chữ cái đầu và màu nền giống hệt nhau

### Requirement: Mở app chạy trong terminal
Với app có metadata đã ký đánh dấu `terminal`, F-Launcher SHALL mở app trong một cửa sổ terminal riêng:
- Windows: một cửa sổ console mới;
- macOS: Terminal.

Launcher MUST không mở terminal cho app không có cờ này.

#### Scenario: Mở trình soạn thảo trong terminal
- **GIVEN** app "kibi" có cờ `terminal`
- **WHEN** người dùng bấm Mở trên Windows
- **THEN** kibi chạy trong một cửa sổ console mới, và launcher vẫn dùng được
