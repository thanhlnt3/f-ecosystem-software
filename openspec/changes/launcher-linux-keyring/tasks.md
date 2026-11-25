# Tasks

## 1. Kho bảo mật trên Linux

- [ ] 1.1 Thêm `zbus-secret-service-keyring-store` (chỉ Linux, `crypto-rust`), chọn kho Secret Service trong `auth.rs`; xác minh bằng `cargo deny check`, clippy và dựng trên Linux, scenario "Đăng nhập trên Linux"
- [ ] 1.2 Báo lỗi rõ khi không có Secret Service, không ghi token ra tệp; xác minh bằng unit test và chạy launcher trên Linux không có Secret Service, scenario "Linux không có Secret Service"

## 2. Tài liệu và phát hành

- [ ] 2.1 README hai thứ tiếng, CHANGELOG; phát hành 1.21.0
