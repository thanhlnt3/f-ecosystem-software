# Design

## S1. Gói và phát hiện

`install::SELF_APP = "feco-launcher"`.

- `Launcher::self_update(current)` trả bản `feco-launcher` mới nhất cho đúng nền tảng, khi bản đó mới hơn `current` (`CARGO_PKG_VERSION`).
- `catalog_latest` loại `SELF_APP`, nên gói không hiện trong Cửa hàng.
- Thu hồi một bản F-Launcher dùng chung cơ chế thu hồi của app con: bản đã thu hồi không được tải.

## S2. Tải và chờ cài (`stage_self_update`)

1. Kiểm tên phiên bản và tệp chạy an toàn (`safe_component`, `safe_relative`) như khi cài app.
2. `download_and_extract` vào `self-update/.staging-<v>-<ngẫu nhiên>`. Bước này đi qua TUF: kiểm hash, độ dài, chữ ký.
3. Đổi tên thành `self-update/<v>/`, kiểm tệp chạy có thật.
4. Ghi `self-update/pending.json`: `{"version", "entry"}`.
5. Ghi sự kiện `self_update_staged`. Lỗi gói thì ghi sự kiện `invalid_package`, như khi cài app.

`Launcher::pending_self_update()` trả phiên bản đang chờ cài (đọc `pending.json`).

## S3. Cài (`selfupdate::apply_pending`)

Hàm được gọi đầu `main`, trước khi mở cửa sổ:

1. Còn `<exe>.old` từ lần trước thì xóa (lỗi thì bỏ qua).
2. Không có `pending.json`, hoặc phiên bản chờ không mới hơn bản đang chạy, hoặc thiếu tệp → xóa `pending.json`, trả `None`.
3. Đổi tên tệp đang chạy thành `<exe>.old`. Trên Windows được phép đổi tên tệp đang chạy, nhưng không được ghi đè.
4. Chép tệp mới vào đường dẫn cũ. Lỗi thì đổi `.old` về như cũ và trả lỗi.
5. Xóa `pending.json`, trả `Some(version)`.

`main` nhận `Some` thì chạy lại chính đường dẫn đó (giờ là bản mới) với cùng tham số, rồi thoát.

## S4. Giao diện

- Lần đồng bộ thành công, có bản mới và bản đó chưa được tải → tải nền. Lỗi chỉ ghi log.
- Nút trên thanh bên: "Khởi động lại để cập nhật" khi `pending_self_update` có giá trị.
- Bấm nút: chạy lại `current_exe` rồi đóng cửa sổ hiện tại. Bản mới được cài ở bước S3 của tiến trình mới.

## S5. Chữ ký mã (chưa làm)

Khi có chứng thư:

- trước bước 3 của S3, kiểm Authenticode (Windows, `WinVerifyTrust`) hoặc chữ ký Developer ID và notarize (macOS) của tệp mới;
- không đạt thì từ chối và ghi sự kiện bảo mật.

Hiện bước này thay bằng chuỗi tin cậy TUF: chỉ gói được F-Eco ký mới tải về được.
