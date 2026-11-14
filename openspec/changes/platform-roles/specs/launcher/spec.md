# Spec Delta

## Purpose

Bổ sung cho capability `launcher`: hiện vai trò nền tảng, và đổi tên phong cách giao diện sang tên phổ thông.

## ADDED Requirements

### Requirement: Hiện vai trò trong launcher
F-Launcher SHALL hiện vai trò nền tảng của tài khoản đang đăng nhập (Người sử dụng ứng dụng, Lập trình viên hoặc Quản trị viên) ở thanh bên:
- Lập trình viên và Quản trị viên SHALL có lối tắt "Cổng nhà phát triển";
- Quản trị viên SHALL có thêm lối tắt "Bảng quản trị".

Launcher MUST lấy vai trò từ máy chủ bằng chính access token của launcher. Không hỏi được thì giữ vai trò lần trước. Mọi kiểm soát quyền vẫn nằm ở máy chủ.

#### Scenario: Lập trình viên mở launcher
- **GIVEN** tài khoản có vai trò Lập trình viên
- **WHEN** launcher đồng bộ
- **THEN** thanh bên hiện "Lập trình viên" và nút "Cổng nhà phát triển"; bấm nút mở `/developer` trong trình duyệt

#### Scenario: Token của app khác
- **WHEN** một app con gọi `GET /v1/account/role` bằng access token của chính app đó
- **THEN** máy chủ trả `403`, vì chỉ F-Launcher được hỏi vai trò

### Requirement: Tên phong cách phổ thông
Tên các phong cách giao diện SHALL là tên phổ thông, không dùng tên người:
- Chân phương, Trang nhã;
- Giấy da, Bích họa, Trời sao, Hồ sen, Lập thể, Hoa huệ, Phố cổ, Sơn mài.

Cài đặt đã lưu bằng mã phong cách của bản 1.2 MUST vẫn mở đúng phong cách tương ứng.

#### Scenario: Cài đặt cũ
- **GIVEN** `settings.json` của bản 1.2 có `"style": "vangogh"`
- **WHEN** người dùng mở launcher bản mới
- **THEN** launcher dùng phong cách "Trời sao" với cùng bảng màu
