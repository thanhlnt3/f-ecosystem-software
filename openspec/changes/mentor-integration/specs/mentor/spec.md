# mentor

## Purpose

Mentor (ứng dụng học tập cho học sinh và giáo viên) trong F-Eco: đăng nhập bằng tài khoản F-Eco, app web trong F-Store và F-Launcher, app desktop Windows.

## ADDED Requirements

### Requirement: Đăng nhập Mentor bằng F-Eco
Gateway Mentor đã áp bản vá SHALL cho đăng nhập bằng tài khoản F-Eco theo OIDC authorization code + PKCE, với client phía máy chủ có secret, qua thư viện Authlib. Tính năng MUST tắt khi thiếu `FECO_CLIENT_ID` hoặc `FECO_CLIENT_SECRET`; khi đó màn hình đăng nhập không có nút F-Eco. Đăng nhập bằng mật khẩu MUST vẫn dùng được.

#### Scenario: Lần đầu đăng nhập bằng F-Eco
- **GIVEN** người dùng chưa có tài khoản Mentor
- **WHEN** người dùng bấm "Đăng nhập bằng F-Eco" và đăng nhập F-Eco
- **THEN** Mentor tạo tài khoản học sinh liên kết với `sub` của F-Eco, tên hiển thị là tên trên F-Eco, và mở app đã đăng nhập

#### Scenario: Lần sau
- **GIVEN** tài khoản Mentor đã liên kết với tài khoản F-Eco
- **WHEN** người dùng đăng nhập bằng F-Eco lần nữa
- **THEN** Mentor dùng đúng tài khoản đó, không tạo tài khoản mới

#### Scenario: Chưa bật
- **GIVEN** gateway không có `FECO_CLIENT_ID`
- **WHEN** người dùng mở màn hình đăng nhập
- **THEN** không có nút "Đăng nhập bằng F-Eco"

### Requirement: Mã dùng một lần và địa chỉ quay về
Gateway SHALL chỉ quay về origin nằm trong danh sách origin được phép của Mentor (app web, webview Tauri, `CORS_ORIGINS`). Địa chỉ khác MUST bị từ chối với 400 `bad_return`. Gateway SHALL trả về app một mã ngẫu nhiên dùng một lần, sống 120 giây, lưu dạng băm. Token phiên MUST không nằm trong địa chỉ. App SHALL đổi mã lấy phiên rồi xóa mã khỏi thanh địa chỉ.

#### Scenario: Địa chỉ quay về lạ
- **WHEN** ai đó mở `/api/auth/feco/login?return=https://evil.test`
- **THEN** gateway trả 400 `bad_return` và không chuyển tới F-Eco

#### Scenario: Mã dùng lại
- **GIVEN** app đã đổi mã lấy phiên
- **WHEN** mã đó được gửi lại tới `/api/auth/feco/exchange`
- **THEN** gateway trả 401 `invalid_code`, và màn hình đăng nhập ghi "Mã đăng nhập F-Eco đã hết hạn. Thử lại nhé."

#### Scenario: Người dùng hủy
- **WHEN** người dùng từ chối trên F-Eco
- **THEN** app quay về màn hình đăng nhập và ghi "Bạn đã hủy đăng nhập bằng F-Eco."

### Requirement: Vai trò giáo viên
Đăng nhập bằng F-Eco MUST không bao giờ tạo tài khoản giáo viên. Quản trị Mentor SHALL nâng vai trò bằng `python -m mentor_gateway.cli set-role`; lệnh này kết thúc các phiên đang mở của tài khoản. Tài khoản giáo viên đã liên kết F-Eco SHALL đăng nhập vào giao diện giáo viên.

#### Scenario: Nâng vai trò
- **GIVEN** học sinh đăng nhập bằng F-Eco lần đầu
- **WHEN** quản trị chạy `set-role --role teacher` cho tài khoản đó
- **THEN** phiên cũ hết hiệu lực, và lần đăng nhập F-Eco sau vào giao diện giáo viên

### Requirement: Mentor trong F-Store và F-Launcher
`scripts/dev-apps.sh mentor` SHALL:
- tạo client OAuth phía máy chủ cho Mentor (scope `openid profile`, chuyển hướng về `/api/auth/feco/callback` của gateway);
- ký target `web` có `web_url` dẫn tới đăng nhập F-Eco của gateway;
- ký thêm target `windows-x64` khi có gói desktop;
- ghi mục F-Store kèm logo Mentor và nguồn.

#### Scenario: Mở bản web từ launcher
- **GIVEN** Mentor có trong kho TUF dev với target `web`
- **WHEN** người dùng bấm "Mở" ở Mentor
- **THEN** trình duyệt mở gateway Mentor, qua F-Eco, rồi vào app Mentor đã đăng nhập

#### Scenario: App desktop
- **GIVEN** Mentor có target `windows-x64`
- **WHEN** người dùng xem Mentor trong launcher trên Windows
- **THEN** launcher dùng gói desktop: "Cài" rồi "Mở" chạy `Mentor.exe`

### Requirement: Dựng app desktop Mentor
CI SHALL tải Mentor đúng commit đã ghi, áp bản vá, chạy test của Mentor, rồi dựng app desktop Windows bằng Tauri với trình tự cập nhật của Mentor đã tắt. Bản phát hành F-Eco SHALL đính kèm `mentor-<phiên bản>-windows-x64.zip`, gồm `Mentor.exe`, NOTICE và nguồn gốc.

#### Scenario: Bản vá còn áp được
- **WHEN** CI chạy job `mentor-desktop`
- **THEN** bản vá áp sạch lên commit đã ghi, pytest và vitest của Mentor qua, và có `Mentor.exe`
