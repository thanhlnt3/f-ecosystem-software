# Proposal

## Why

Bản phát hành có gói Linux từ lâu, nhưng F-Launcher trên Linux không đăng nhập được. Launcher chỉ lưu refresh token vào kho bảo mật của hệ điều hành, và mới có Windows Credential Manager, macOS Keychain. Trên Linux, launcher báo "F-Launcher chỉ hỗ trợ Windows và macOS".

Chủ dự án yêu cầu (25/11/2026) làm hỗ trợ Linux, và cho phép thêm thư viện `zbus-secret-service-keyring-store`.

## What Changes

- **Kho bảo mật trên Linux**: F-Launcher lưu refresh token qua Secret Service (D-Bus). Đây là giao diện chung của GNOME Keyring, KWallet (KDE 5.97 trở lên), KeePassXC.
  - Thư viện `zbus-secret-service-keyring-store` 1.0.1 (MIT OR Apache-2.0), cùng họ `keyring-core` launcher đang dùng. Thuần Rust: không cần `libdbus` khi dựng hay khi chạy.
  - Mã hóa phiên D-Bus bằng RustCrypto (tính năng `crypto-rust`), không dùng OpenSSL.
  - Chỉ biên dịch cho Linux.
- **Máy không có Secret Service** (máy chủ không giao diện, trình quản lý cửa sổ tối giản): launcher báo rõ cần cài và mở khóa GNOME Keyring, KWallet hoặc KeePassXC. Launcher không lưu token ra tệp thường.
- **Hệ điều hành khác** (BSD…): vẫn báo chưa hỗ trợ, như trước.
- README hai thứ tiếng: mục Linux trong "Cài đặt từ bản phát hành".

## Capabilities

### Modified Capabilities
- `launcher`: đăng nhập và lưu refresh token trên Linux.

## Impact

- `clients/feco-launcher`: `auth.rs` (chọn kho theo hệ điều hành), `Cargo.toml` (phụ thuộc chỉ cho Linux).
- `Cargo.lock` thêm `zbus`, `secret-service` và các crate RustCrypto; `cargo deny` kiểm giấy phép.
- Windows và macOS không đổi.
