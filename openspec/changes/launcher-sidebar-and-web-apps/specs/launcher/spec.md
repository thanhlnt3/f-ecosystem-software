# Spec Delta

## Purpose

Bổ sung cho capability `launcher`: thanh bên thu gọn được, chỉ còn icon; app web.

## ADDED Requirements

### Requirement: Thanh bên thu gọn
Thanh bên của launcher SHALL có hai chế độ: đầy đủ (icon và chữ, rộng 210 px) và thu gọn (chỉ icon, rộng 56 px). Nút ẩn/hiện ở đầu thanh bên và phím tắt Ctrl+B (Cmd+B trên macOS) SHALL chuyển giữa hai chế độ. Launcher MUST nhớ chế độ trong `settings.json`.

#### Scenario: Thu gọn thanh bên
- **GIVEN** thanh bên đang đầy đủ
- **WHEN** người dùng bấm nút ẩn/hiện
- **THEN** thanh bên rộng 56 px, chỉ còn icon, và `settings.json` có `"sidebar_collapsed": true`

#### Scenario: Mở lại launcher
- **GIVEN** người dùng đã thu gọn thanh bên rồi đóng launcher
- **WHEN** người dùng mở lại launcher
- **THEN** thanh bên vẫn thu gọn

#### Scenario: Cài đặt cũ
- **GIVEN** `settings.json` của bản trước, không có trường `sidebar_collapsed`
- **WHEN** launcher đọc cài đặt
- **THEN** thanh bên đầy đủ và các cài đặt khác giữ nguyên

### Requirement: Icon ở thanh bên thu gọn
Ở chế độ thu gọn, mỗi icon SHALL có tooltip ghi tên mục. Số thông báo chưa đọc và số bản cập nhật SHALL hiện thành huy hiệu ở góc icon (trên 9 thì ghi "9+"). Mọi chức năng của chế độ đầy đủ MUST vẫn dùng được: Thư viện, Cửa hàng, Thông báo, Cài đặt, đồng bộ, F-Eco trên web, bảng quản trị, cổng nhà phát triển, ví F-Coin, đăng nhập, đăng xuất.

#### Scenario: Huy hiệu thông báo
- **GIVEN** thanh bên thu gọn và có 12 thông báo chưa đọc
- **THEN** icon Thông báo có huy hiệu "9+" và tooltip "Thông báo (12)"

### Requirement: Thanh bên khi cửa sổ hẹp
Cửa sổ hẹp hơn 720 px thì thanh bên SHALL hiện thu gọn mà không đổi chế độ đã lưu.

#### Scenario: Cửa sổ hẹp
- **GIVEN** thanh bên đang đầy đủ
- **WHEN** người dùng thu cửa sổ còn 700 px
- **THEN** thanh bên hiện thu gọn; kéo cửa sổ rộng lại 1000 px thì thanh bên đầy đủ trở lại

### Requirement: App web trong launcher
Launcher SHALL hiện app web (target TUF nền tảng `web` có `web_url` trong metadata đã ký) trên mọi hệ điều hành, gắn nhãn "App web". App có cả gói desktop cho máy đang dùng và bản web thì launcher SHALL dùng gói desktop.

#### Scenario: App web trên Windows
- **GIVEN** danh mục đã ký có ThesisAI 0.1.0 nền tảng `web`, `web_url` là `http://localhost:3000/auth/feco`
- **WHEN** launcher trên Windows đồng bộ
- **THEN** Cửa hàng có ô ThesisAI với nhãn "App web" và nút "Thêm"

### Requirement: Mở app web
"Thêm" SHALL tải và kiểm gói của app web như app desktop rồi ghi vào Thư viện. "Mở" SHALL mở `web_url` bằng trình duyệt mặc định, không tạo tiến trình con hay mã khởi chạy.

#### Scenario: Mở app web
- **GIVEN** ThesisAI đã có trong Thư viện
- **WHEN** người dùng bấm "Mở"
- **THEN** trình duyệt mặc định mở `http://localhost:3000/auth/feco` và Thư viện ghi lần mở

### Requirement: Địa chỉ app web an toàn
`web_url` MUST chỉ lấy từ metadata đã xác minh và MUST là địa chỉ HTTPS, hoặc `http://localhost`, `http://127.0.0.1`, không chứa thông tin đăng nhập. Địa chỉ khác thì `feco-tuf` MUST từ chối khi phát hành và launcher MUST không mở mà báo lỗi.

#### Scenario: Địa chỉ không an toàn
- **WHEN** nhà vận hành phát hành app web với `web_url` là `http://example.com/app`
- **THEN** `feco-tuf publish` từ chối và kho TUF không đổi
