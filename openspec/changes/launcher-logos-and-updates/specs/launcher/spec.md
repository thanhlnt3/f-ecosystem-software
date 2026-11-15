# Spec Delta

## Purpose

Bổ sung cho capability `launcher`: logo, cửa sổ giữa màn hình và cửa sổ chặn, gợi ý cập nhật, lối tới các tính năng trên web.

Requirement "Cửa sổ riêng cho Cài đặt và chi tiết app" của thay đổi launcher-windows-and-web-profile (tối đa 6 cửa sổ chi tiết cùng lúc) được thay bằng "Cửa sổ con là cửa sổ chặn" dưới đây.

## ADDED Requirements

### Requirement: Logo app trong launcher
Launcher SHALL hiện logo của app trong Thư viện, Cửa hàng, cửa sổ chi tiết và các hộp thoại. Logo MUST chỉ lấy từ metadata TUF đã xác minh (trường `icon`, ảnh PNG). App không có logo, hoặc logo không giải mã được, MUST hiện ô chữ cái đầu như trước.

#### Scenario: App có logo
- **GIVEN** target của kibi có logo trong metadata đã ký
- **WHEN** launcher đồng bộ
- **THEN** ô kibi trong Cửa hàng hiện logo đó

#### Scenario: Logo không phải PNG
- **WHEN** nhà vận hành đặt logo là tệp GIF bằng `feco-tuf set-icon`
- **THEN** lệnh bị từ chối và metadata không đổi

### Requirement: Logo F-Launcher
F-Launcher SHALL có logo riêng, dùng làm biểu tượng cửa sổ, logo trên thanh bên và biểu tượng trang web.

#### Scenario: Biểu tượng cửa sổ
- **WHEN** người dùng mở F-Launcher
- **THEN** thanh tác vụ hiện logo F-Launcher

### Requirement: Cửa sổ giữa màn hình
Cửa sổ chính của launcher SHALL mở ở giữa màn hình. Cửa sổ Cài đặt và cửa sổ chi tiết app SHALL mở ở giữa màn hình. Nếu màn hình nhỏ hơn cửa sổ thì cửa sổ đặt ở góc trên bên trái.

#### Scenario: Cửa sổ ở giữa màn hình
- **GIVEN** màn hình 1920 × 1080
- **WHEN** người dùng mở Cài đặt (cỡ 760 × 700)
- **THEN** cửa sổ Cài đặt mở tại vị trí (580, 190)

### Requirement: Cửa sổ con là cửa sổ chặn
Khi cửa sổ Cài đặt hoặc cửa sổ chi tiết app đang mở, cửa sổ chính MUST không nhận thao tác: nó bị phủ mờ kèm dòng "Đang mở cửa sổ khác. Đóng cửa sổ đó để tiếp tục.". Bấm vào cửa sổ chính, hoặc chọn nó, SHALL đưa cửa sổ con lên trước. Đóng cửa sổ con thì cửa sổ chính dùng được lại.

#### Scenario: Bấm vào cửa sổ chính khi đang mở chi tiết
- **GIVEN** cửa sổ chi tiết của kibi đang mở
- **WHEN** người dùng bấm vào một app khác trong cửa sổ chính
- **THEN** không có cửa sổ mới nào mở, và cửa sổ chi tiết của kibi được đưa lên trước

### Requirement: Gợi ý cập nhật app
Sau mỗi lần đồng bộ (lúc mở, mỗi giờ, khi bấm Đồng bộ), launcher SHALL so phiên bản đã cài của các app nó quản lý với danh mục đã ký. Có bản mới chưa gợi ý trong phiên chạy này thì hiện hộp thoại "Có bản cập nhật mới": logo, tên app, phiên bản cũ → mới, ghi chú phát hành, các nút Cập nhật, Bỏ qua bản này, Cập nhật tất cả, Để sau. Khi bật Tự động cập nhật thì không hiện hộp thoại.

#### Scenario: Phát hiện bản mới
- **GIVEN** đã cài kibi 0.3.2 và Ghi chú 1.5.0; danh mục có kibi 0.3.3 và Ghi chú 1.6.0
- **WHEN** launcher đồng bộ xong
- **THEN** hộp thoại gợi ý Ghi chú 1.5.0 → 1.6.0 và kibi 0.3.2 → 0.3.3, kèm ghi chú phát hành

### Requirement: Bỏ qua và nhắc lại bản cập nhật
Phiên bản người dùng bỏ qua MUST không được gợi ý lại; bản mới hơn bản đó thì vẫn được gợi ý. Khi còn bản cập nhật, Thư viện SHALL có băng nhắc kèm nút "Xem" để mở lại hộp thoại.

#### Scenario: Bỏ qua phiên bản
- **WHEN** người dùng bấm "Bỏ qua bản này" ở kibi 0.3.3
- **THEN** kibi 0.3.3 không được gợi ý nữa; khi danh mục có kibi 0.3.4 thì kibi được gợi ý lại

### Requirement: Lối tới tính năng trên web
Thanh bên của launcher SHALL có menu "F-Eco trên web": Hồ sơ và vai trò, Ví F-Coin, Ứng dụng của tôi, Dự án phần mềm, Đơn xây app, Thông báo. Mỗi mục mở trang tương ứng trên trình duyệt.

#### Scenario: Mở dự án từ launcher
- **WHEN** người dùng chọn "Dự án phần mềm" trong menu "F-Eco trên web"
- **THEN** trình duyệt mở `/projects` của máy chủ F-Eco
