# Tasks

## 1. Tài khoản quản trị viên mặc định

- [ ] 1.1 `AdminBootstrapConfig`, `bootstrap::ensure_admin` (tạo, bật lại, đủ vai trò, chạy lúc khởi động và mỗi 10 phút), bảo vệ trong `set_role` và xóa tài khoản; xác minh bằng unit test cấu hình và E2E scenario "Lần khởi động đầu", "Tài khoản bị tắt", "Production thiếu mật khẩu", "Hạ vai trò tài khoản mặc định"

## 2. Chuyển F-Coin

- [ ] 2.1 Migration 0018, `wallet::admin_transfer`, yêu cầu cấp F-Coin và duyệt, biểu mẫu ở `/admin/wallet`; xác minh bằng E2E scenario "Chuyển từ ví của quản trị viên", "Ví của quản trị viên không đủ", "Cấp từ quỹ nền tảng", "Cấp lớn cần người thứ hai", "Gửi lại biểu mẫu", "Chuyển cho chính mình"

## 3. Các task còn lại

- [ ] 3.1 CI tự kiểm gitleaks (task 1.3 của add-f-ecosystem-platform); đánh dấu task 2.4 với bằng chứng E2E; ghi rõ task còn chờ yếu tố bên ngoài

## 4. Rà soát bảo mật và chức năng

- [ ] 4.1 Ba lượt rà soát (xác thực và web, luồng tiền, launcher và TUF); sửa mọi lỗi được xác nhận, mỗi lỗi có test

## 5. Tài liệu và phát hành

- [ ] 5.1 Chuỗi giao diện bốn thứ tiếng, README (biến `FECO_ADMIN_*`), CHANGELOG, QUESTIONS.md; phát hành 1.9.0
