# Tasks

## 1. Kho bảo mật trên Linux

- [x] 1.1 Thêm `zbus-secret-service-keyring-store` (chỉ Linux, `crypto-rust`), chọn kho Secret Service trong `auth.rs`; xác minh bằng `cargo deny check`, clippy và dựng trên Linux, scenario "Đăng nhập trên Linux"
  - Xác minh: chủ dự án cho phép thêm thư viện (25/11/2026). 35 crate mới, giấy phép MIT hoặc Apache-2.0; ba cảnh báo ngừng bảo trì RUSTSEC-2026-0370/0371/0372 ghi ngoại lệ có hạn trong `deny.toml` (chủ dự án duyệt). CI `9d99ebd`: `licenses` (cargo deny) xanh; `rust (ubuntu-latest)` clippy và test xanh; bước "F-Launcher lưu đăng nhập qua Secret Service" chạy GNOME Keyring không giao diện: `secret_service_round_trip` lưu, đọc, xóa refresh token thành công, mục của client khác tách riêng. Chưa thử trọn luồng đăng nhập trên máy Linux có màn hình (QUESTIONS.md 138).
- [x] 1.2 Báo lỗi rõ khi không có Secret Service, không ghi token ra tệp; xác minh bằng unit test và chạy launcher trên Linux không có Secret Service, scenario "Linux không có Secret Service"
  - Xác minh: unit test `failed_store_init_is_retried` (lỗi khởi tạo không bị ghi nhớ, lần sau thử lại), `locked_store_error_explains_what_to_do`. CI Linux với `DBUS_SESSION_BUS_ADDRESS` trỏ tới chỗ không có: lỗi "không kết nối được Secret Service: cần GNOME Keyring, KWallet hoặc KeePassXC … launcher không lưu phiên đăng nhập ra tệp". `KeyringStore` không có đường lưu ra tệp.

## 2. Tài liệu và phát hành

- [x] 2.1 README hai thứ tiếng, CHANGELOG; phát hành 1.21.0
  - Xác minh: README (mục Linux trong "Cài đặt từ bản phát hành", tóm tắt tính năng), `scripts/check-docs-i18n.py`; CHANGELOG 1.21.0; CI và release.yml của tag `v1.21.0`.
