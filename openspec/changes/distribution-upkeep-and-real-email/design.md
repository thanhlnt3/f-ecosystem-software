# Design

## D1. `Repo::maintain`

```rust
pub enum Maintenance { Nothing, Refreshed, Resigned }

pub async fn maintain(&self, signers, expiry: Expiry, now: Timestamp) -> Result<Maintenance>
```

Hàm đọc kho (`load`, không chặn hạn), rồi so thời gian còn lại của từng vai trò:

| Vai trò | Ngưỡng (với `Expiry::default`) | Việc làm |
|---|---|---|
| `targets` | còn dưới 1/3 thời hạn (30 ngày) | `resign`: ký lại targets, snapshot, timestamp |
| `snapshot` | còn dưới 1/2 (3,5 ngày) | `refresh`: ký lại snapshot, timestamp |
| `timestamp` | còn dưới 1/2 (30 phút) | `refresh` |

- Metadata đã hết hạn thì thời gian còn lại là số âm, nên vẫn rơi vào nhánh ký lại. Máy chủ tắt nhiều ngày rồi bật lại sẽ tự sửa.
- `resign` cần khóa targets. Máy chủ đã có khóa này trong `FECO_TUF_SIGN_KEYS` để phát hành.

## D2. Việc định kỳ trên máy chủ

`developer::maintain_tuf(state)` trả `Ok(None)` khi chưa cấu hình TUF. Khi có cấu hình, hàm:

1. lấy `state.tuf_lock`, dùng chung với phát hành để không ghi metadata cùng lúc;
2. gọi `maintain` với khóa của `FECO_TUF_SIGN_KEYS`.

`main.rs` gọi hàm này lúc khởi động và mỗi 5 phút. Với timestamp 1 giờ, việc ký lại xảy ra khoảng mỗi 30 phút. Kết quả `Refreshed` hoặc `Resigned` được ghi log mức info, lỗi ghi mức error.

## D3. F-Launcher

Thêm biến thể `LauncherError::ExpiredMetadata(String)`. `classify` chọn biến thể này khi chi tiết lỗi chứa `ExpiredMetadata`. Nội dung hiển thị:

> Dữ liệu phân phối của máy chủ đã hết hạn nên danh mục chưa cập nhật được. Các app đã cài vẫn mở được; F-Launcher sẽ tự thử lại. Nếu giờ trên máy tính đang sai, hãy chỉnh lại giờ.

`InvalidMetadata` chỉ hiện "Metadata phân phối không hợp lệ, đã từ chối". Chi tiết lỗi chỉ có trong `detail()`, dùng cho nhật ký và sự kiện bảo mật.

`Launcher::sync`:

- `online = false` chỉ khi lỗi là `Offline`;
- khi metadata bị từ chối, `online` vẫn đúng và `distribution_error` giữ thông báo;
- lần đồng bộ thành công đặt `distribution_error = None`.

Trang Cửa hàng hiện thông báo này thay cho "Đang ngoại tuyến". Sự kiện bảo mật vẫn được ghi như trước: metadata hết hạn có thể là dấu hiệu tấn công đóng băng.

## D4. App di động

`AppState.reload`:

- `ApiException` (máy chủ có trả lời) → `error_server`: "Máy chủ F-Eco đang gặp lỗi. Hãy thử lại sau ít phút.";
- lỗi khác → `error_network` như cũ.

`onResume`: nếu `error != null` thì gọi `reload()`; nếu không thì gọi `refreshLibrary()` như hiện nay.

## D5. Email thật

**Kratos** (`scripts/dev-local.sh kratos`):

- địa chỉ SMTP lấy theo thứ tự `FECO_SMTP_URL` → `.dev/smtp-url` → Mailpit;
- địa chỉ gửi lấy theo thứ tự `FECO_EMAIL_FROM` → `.dev/smtp-from`;
- có địa chỉ gửi thì đặt `COURIER_SMTP_FROM_ADDRESS` (Gmail yêu cầu địa chỉ gửi trùng tài khoản) và `COURIER_SMTP_FROM_NAME=F-Eco`;
- không in URL SMTP ra log, vì URL chứa mật khẩu.

**feco-server**:

- `FECO_SMTP_URL_FILE`: đọc URL SMTP từ tệp và cắt khoảng trắng, như `FECO_ADMIN_PASSWORD_FILE`;
- `FECO_SMTP_URL` được ưu tiên nếu đặt cả hai;
- `.dev/run-server.ps1` trên máy dev đặt biến này khi có `.dev/smtp-url`.

**Gmail** cho máy dev, ghi trong README:

- bật xác minh 2 bước, tạo "mật khẩu ứng dụng" 16 ký tự;
- URL: `smtps://<tài khoản>%40gmail.com:<mật khẩu ứng dụng>@smtp.gmail.com:465`;
- người dùng tự ghi URL vào `.dev/smtp-url`, không đưa vào chat hay repo.

Giới hạn: Gmail cho gửi khoảng 500 email mỗi ngày. Liên kết xác minh trỏ về địa chỉ công khai của Kratos (`127.0.0.1` trên máy dev), nên chỉ mở được trên chính máy đó. Production dùng tên miền gửi riêng (task 14.3).
