# Proposal

## Why

Task 6.6 của `add-f-ecosystem-platform` ("Launcher tự cập nhật, bộ cài ký Authenticode, Developer ID và notarize") chưa làm phần nào. Phần ký mã cần chứng thư Authenticode và tài khoản Apple Developer (QUESTIONS.md câu 17). Phần tự cập nhật thì làm được ngay: F-Launcher đã có kho phân phối ký số (TUF), và đã tải, kiểm tra, giải nén gói app con qua kho đó.

Hiện mỗi bản F-Launcher mới phải được tải tay từ GitHub Releases.

## What Changes

- **Gói F-Launcher trong kho phân phối.** Mã app dành riêng `feco-launcher`, mỗi nền tảng một gói (`feco-launcher-<phiên bản>-<nền tảng>.zip`), ký như app con. Gói này không hiện trong Cửa hàng hay Thư viện.
- **Tự phát hiện và tải về.** Sau mỗi lần đồng bộ, nếu kho có bản F-Launcher mới hơn bản đang chạy, F-Launcher tự tải về nền. Gói được kiểm tra bằng chuỗi tin cậy TUF (chữ ký của F-Eco, hash, độ dài), giải nén vào `self-update/<phiên bản>/`, và ghi `pending.json`. Gói sai hash, sai chữ ký hoặc thiếu tệp chạy bị từ chối và ghi sự kiện bảo mật.
- **Cài khi khởi động lại.**
  - Khi bản mới đã tải xong, F-Launcher hiện "F-Launcher X đã tải xong. Khởi động lại để cập nhật." cùng nút "Khởi động lại".
  - Lần khởi động sau (do người dùng bấm hoặc tự mở lại), F-Launcher đổi tên tệp đang chạy thành `.old`, chép bản mới vào chỗ cũ, rồi chạy bản mới.
  - Chép lỗi thì đặt lại bản cũ.
  - Tệp `.old` được xóa ở lần chạy sau.
- **Công cụ**: `scripts/dev-launcher.sh publish-self` đóng gói và ký F-Launcher vừa dựng vào kho dev, để thử tự cập nhật.
- **Chưa làm (vẫn chờ chứng thư)**: kiểm chữ ký Authenticode và Developer ID của bản mới, notarize. Task 6.6 vẫn để mở.

## Capabilities

### Modified Capabilities
- `launcher`: tự cập nhật qua kho phân phối.

## Impact

- `clients/feco-launcher`: `install.rs`, `paths.rs`, `selfupdate.rs` (mới), `main.rs`, `catalog.rs`, `i18n*.rs`, test.
- `scripts/dev-launcher.sh`, README.
- Máy chủ không đổi: gói được ký bằng `feco-tuf publish` như app con.
