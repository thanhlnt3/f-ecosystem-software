## MODIFIED Requirements

### Requirement: Đăng nhập trong launcher
F-Launcher SHALL đăng nhập qua trình duyệt mặc định của hệ điều hành bằng OAuth 2.0 Authorization Code + PKCE, với địa chỉ chuyển hướng loopback hoặc custom URI scheme. Launcher MUST không có ô nhập mật khẩu riêng, và MUST lưu refresh token trong kho bảo mật của hệ điều hành (Windows Credential Manager, macOS Keychain, Secret Service trên Linux). Launcher MUST không lưu refresh token ra tệp thường khi không có kho bảo mật.

#### Scenario: Đăng nhập lần đầu
- **WHEN** người dùng bấm Đăng nhập trong launcher
- **THEN** trình duyệt mở trang đăng nhập F-Eco, và sau khi đăng nhập xong launcher hiện tên người dùng

#### Scenario: Khởi động lại máy
- **GIVEN** người dùng đã đăng nhập launcher
- **WHEN** máy khởi động lại rồi người dùng mở launcher
- **THEN** launcher vẫn ở trạng thái đăng nhập mà không cần làm lại

#### Scenario: Đăng nhập trên Linux
- **GIVEN** máy Linux có Secret Service (GNOME Keyring, KWallet hoặc KeePassXC) đã mở khóa
- **WHEN** người dùng đăng nhập launcher rồi mở lại launcher
- **THEN** refresh token nằm trong Secret Service và launcher vẫn ở trạng thái đăng nhập

#### Scenario: Linux không có Secret Service
- **GIVEN** máy Linux không có dịch vụ Secret Service trên D-Bus
- **WHEN** người dùng bấm Đăng nhập
- **THEN** launcher báo cần cài và mở khóa GNOME Keyring, KWallet hoặc KeePassXC, và không ghi token ra tệp
