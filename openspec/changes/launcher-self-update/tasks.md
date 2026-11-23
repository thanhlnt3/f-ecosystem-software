# Tasks

## 1. F-Launcher

- [x] 1.1 `SELF_APP`, `self_update`, `stage_self_update`, `pending_self_update`, loại khỏi Cửa hàng; xác minh bằng test scenario "Có bản launcher mới", "Gói bị sửa", "Không hiện trong Cửa hàng"
  - Xác minh: `tests/launcher.rs::self_update_downloads_verifies_and_installs_on_restart` (kho TUF thật): gói `feco-launcher` 99.0.0 không có trong `catalog_latest`; `self_update("1.17.0")` trả 99.0.0, `self_update("99.0.0")` không có gì; gói bị sửa sau khi ký thì `InvalidPackage`, không có bản chờ, sự kiện `invalid_package`; gói đúng thì tải, giải nén, `pending_self_update = 99.0.0`, sự kiện `self_update_staged`; bản bị thu hồi không được đề nghị.
- [x] 1.2 `selfupdate::apply_pending` lúc khởi động, nút "Khởi động lại để cập nhật"; xác minh bằng test scenario "Có bản launcher mới" (cài khi khởi động), "Cài không thành công", và thử trên máy dev
  - Xác minh: `selfupdate::tests` (thay tệp đang chạy, giữ `.old` rồi xóa ở lần sau; bỏ bản chờ cũ hơn hoặc `pending.json` có đường dẫn `../`; chép lỗi thì đặt lại bản cũ, giữ bản chờ) và bước cài trong test tích hợp trên. `main` gọi `apply_pending` trước khi mở cửa sổ và chạy lại bản mới; nút "Khởi động lại để cập nhật" ở thanh bên khi có bản chờ. Chưa thử cả vòng trên máy dev: F-Launcher 1.16 đang chạy chưa có tính năng này, nên lần lên 1.17 vẫn cài tay.
- [x] 1.3 `scripts/dev-launcher.sh publish-self`; xác minh bằng thử trên máy dev
  - Xác minh: `scripts/dev-launcher.sh publish-self` trên máy dev (23/11/2026) ký `feco-launcher-1.17.0-windows-x64.zip` vào kho `.dev/tuf/repo`; targets.json có đúng target này.

## 2. Tài liệu

- [x] 2.1 README hai thứ tiếng, CHANGELOG; ghi chú vào task 6.6 của `add-f-ecosystem-platform` (phần chữ ký mã vẫn chờ)
  - Xác minh: README hai thứ tiếng (bước 7 của F-Launcher: `publish-self`), `scripts/check-docs-i18n.py`; CHANGELOG 1.17.0; ghi chú task 6.6 (phần chữ ký mã vẫn chờ); QUESTIONS.md 132.
