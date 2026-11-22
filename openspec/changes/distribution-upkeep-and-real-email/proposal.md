# Proposal

## Why

Sau khi chạy vài ngày, F-Launcher 1.15 hiện hai thông báo:

- "Đang ngoại tuyến. Danh mục sẽ cập nhật khi có mạng; các app đã cài vẫn mở được."
- "Metadata phân phối không hợp lệ, đã từ chối (Tuf(ExpiredMetadata { role: Timestamp …"

Cả hai có chung một nguyên nhân (22/11/2026):

1. **Metadata `timestamp` của kho phân phối TUF chỉ có hạn 1 giờ** (`Expiry::default`), `snapshot` 7 ngày, `targets` 90 ngày. Thời hạn ngắn là có chủ đích: client từ chối metadata quá hạn để chống tấn công "đóng băng" (giữ client ở bản cũ có lỗ hổng).
2. **Không có gì ký lại metadata định kỳ.** `Repo::refresh` có ghi chú "máy chủ chạy việc này định kỳ", nhưng feco-server chưa có việc đó. Metadata chỉ được ký lại khi phát hành một phiên bản, hoặc khi chạy tay `feco-tuf refresh`. Trên máy dev, `timestamp.json` hết hạn từ 19/11/2026 11:24 UTC. Máy chủ dev cũng chưa được cấp `FECO_TUF_REPO` và `FECO_TUF_SIGN_KEYS`.
3. **F-Launcher coi mọi lần đồng bộ lỗi là "ngoại tuyến".** `Launcher::sync` đặt `online = false` cả khi mạng tốt nhưng metadata bị từ chối. Vì vậy người dùng thấy cùng lúc "Đang ngoại tuyến" và một chuỗi lỗi kỹ thuật (`Tuf(ExpiredMetadata …)`).

Web không đọc kho TUF nên không bị ảnh hưởng. App di động không dùng TUF, nhưng gặp lỗi cùng loại: mọi lỗi khi tải dữ liệu đều hiện "Không kết nối được máy chủ F-Eco", kể cả khi máy chủ có trả lời (lỗi 5xx). App cũng không tự thử lại khi người dùng quay lại app.

Ngoài ra, email xác minh khi đăng ký luôn vào Mailpit, hòm thư giả của máy dev. Lý do là `scripts/dev-local.sh kratos` gán cứng `COURIER_SMTP_CONNECTION_URI` về `127.0.0.1:1025`. feco-server dùng được SMTP thật (`FECO_SMTP_URL`), nhưng chỉ nhận mật khẩu SMTP qua biến môi trường, không qua tệp bí mật.

## What Changes

- **`feco-tuf`**: `Repo::maintain(signers, expiry, now)` ký lại trước khi hết hạn:
  - `targets` còn dưới 1/3 thời hạn: `resign`;
  - `snapshot` hoặc `timestamp` còn dưới một nửa thời hạn: `refresh`;
  - còn lại: không làm gì.
- **feco-server**: khi có `FECO_TUF_REPO`, chạy `maintain` lúc khởi động và mỗi 5 phút, dùng chung khóa `tuf_lock` với phát hành. Lỗi được ghi log, không làm dừng máy chủ.
- **F-Launcher máy tính**:
  - metadata hết hạn hoặc không hợp lệ không còn bị coi là ngoại tuyến;
  - thay chuỗi lỗi kỹ thuật bằng lời dễ hiểu: dữ liệu phân phối của máy chủ đã hết hạn, các app đã cài vẫn mở được, F-Launcher sẽ tự thử lại, và nếu giờ của máy tính sai thì cần chỉnh lại;
  - chi tiết kỹ thuật chỉ ghi vào nhật ký;
  - lần đồng bộ thành công sau đó xóa thông báo.
- **App di động**:
  - phân biệt "không kết nối được máy chủ" với "máy chủ đang gặp lỗi";
  - khi quay lại app mà lần tải trước lỗi, app tải lại.
- **Email thật**:
  - `scripts/dev-local.sh kratos` lấy SMTP từ `FECO_SMTP_URL`, hoặc từ tệp `.dev/smtp-url` (không commit); địa chỉ gửi từ `FECO_EMAIL_FROM` hoặc `.dev/smtp-from`. Không có thì vẫn dùng Mailpit.
  - feco-server nhận thêm `FECO_SMTP_URL_FILE`.
  - README hướng dẫn dùng Gmail (mật khẩu ứng dụng) cho máy dev, và tên miền gửi riêng cho production.

## Capabilities

### Modified Capabilities
- `launcher`: phân phối an toàn (tự gia hạn metadata; thông báo khi metadata bị từ chối).
- `mobile-launcher`: thông báo lỗi tải dữ liệu.
- `notifications`: gửi email qua SMTP thật.

## Impact

- `crates/feco-tuf`: `repo.rs` và test `chain_of_trust`.
- `services/feco-server`: `main.rs`, `developer.rs` (hàm `maintain_tuf`).
- `crates/feco-config`: `FECO_SMTP_URL_FILE`.
- `clients/feco-launcher`: `install.rs`, `main.rs`, `i18n*.rs`, test.
- `clients/feco-mobile`: `app_state.dart`, `ui.dart`, `i18n.dart`, test.
- `scripts/dev-local.sh`, README, QUESTIONS.md.
- Không đổi cơ sở dữ liệu.
