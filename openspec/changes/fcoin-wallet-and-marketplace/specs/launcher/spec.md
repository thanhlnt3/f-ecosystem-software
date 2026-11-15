# Spec Delta

## Purpose

Bổ sung cho capability `launcher`: số dư F-Coin, giá app và mua app trong F-Launcher.

## ADDED Requirements

### Requirement: Số dư F-Coin trong launcher
Khi người dùng đã đăng nhập, thanh bên của F-Launcher SHALL hiện số dư F-Coin với icon ví và tooltip. Bấm vào số dư thì mở trang ví trên trình duyệt. Số dư MUST được đọc lại mỗi lần đồng bộ và sau mỗi lần mua. `GET /v1/wallet` MUST chỉ nhận token của F-Launcher.

#### Scenario: Xem số dư
- **GIVEN** ví có 120 F-Coin
- **WHEN** người dùng đăng nhập launcher
- **THEN** thanh bên ghi "120 F-Coin"

#### Scenario: Token của app khác
- **WHEN** một app khác gọi `GET /v1/wallet` bằng access token của nó
- **THEN** trả `403 launcher_only`

### Requirement: Mua app trước khi cài
Ô app và trang chi tiết SHALL hiện giá. Với app có giá mà người dùng chưa mua, nút chính MUST là "Mua · N F-Coin" thay cho "Cài đặt".
- Bấm nút thì hiện hộp thoại xác nhận, gồm giá và số dư sau khi mua.
- Mua thành công thì cài ngay.
- Không đủ F-Coin thì hộp thoại báo số còn thiếu và có nút mở trang nạp.
- Chưa đăng nhập thì nút là "Đăng nhập để mua".

#### Scenario: Mua rồi cài
- **GIVEN** app giá 49 F-Coin, ví có 120 F-Coin
- **WHEN** người dùng bấm "Mua · 49 F-Coin" rồi xác nhận
- **THEN** số dư còn 71 F-Coin và app bắt đầu cài đặt

#### Scenario: Không đủ F-Coin trong launcher
- **GIVEN** ví có 10 F-Coin
- **WHEN** người dùng xác nhận mua app giá 49 F-Coin
- **THEN** hộp thoại báo "Còn thiếu 39 F-Coin" và có nút "Nạp F-Coin"
