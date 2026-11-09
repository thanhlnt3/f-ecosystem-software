# Khóa và vận hành kho TUF

Gói cài đặt của F-Eco được phân phối theo mô hình The Update Framework (TUF), design D3. Mã nằm ở
`crates/feco-tuf` (thư viện và công cụ `feco-tuf`); mọi bước ký và xác minh dùng crate `tough`.

## Vai trò và nơi giữ khóa

| Vai trò | Ký cái gì | Nơi giữ khóa | Ngưỡng | Hạn metadata | Ký lại |
|---|---|---|---|---|---|
| `root` | Danh sách khóa của mọi vai trò | Ngoại tuyến, 3 người giữ, mỗi người một thiết bị riêng | 2/3 | 365 ngày | Mỗi năm, hoặc khi xoay khóa |
| `targets` | Hash, độ dài, app, phiên bản, nền tảng của từng gói | KMS/HSM, chỉ dịch vụ phát hành dùng sau khi kiểm duyệt xong | 1 | 90 ngày | Khi phát hành, thu hồi; `resign` mỗi tháng |
| `snapshot` | Phiên bản của targets | KMS, khóa trực tuyến của máy chủ phân phối | 1 | 7 ngày | `refresh` mỗi 15 phút |
| `timestamp` | Phiên bản và hash của snapshot | KMS, khóa trực tuyến của máy chủ phân phối | 1 | 1 giờ | `refresh` mỗi 15 phút |

Bản dev giữ khóa trong file Ed25519 (PKCS#8). Production thay bằng nguồn khóa KMS (cùng trait
`KeySource` của `tough`) khi đã chọn nhà cung cấp hạ tầng (`QUESTIONS.md`, task 1.4).

Không bao giờ đưa file khóa vào kho mã. CI chạy gitleaks trên mọi commit.

## Cam kết với launcher

- Bộ cài launcher mang theo `1.root.json`. Launcher tự đi theo chuỗi `2.root.json`, `3.root.json`...,
  mỗi root mới phải có đủ ngưỡng chữ ký của root trước và của chính nó.
- Launcher đồng bộ mỗi giờ (`client::SYNC_INTERVAL`) và lưu metadata đã tin để chặn rollback.
- Timestamp hết hạn sau 1 giờ, nên bản thu hồi tới launcher trong vòng 1 giờ, kể cả khi kẻ tấn
  công giữ lại metadata cũ ("đóng băng"). Đổi lại, máy chủ phải chạy `refresh` đều đặn: nếu
  `refresh` ngừng quá 1 giờ, mọi launcher sẽ không cài hay cập nhật được.
- Mỗi gói chỉ được dùng khi khớp độ dài và SHA-256 trong targets đã ký. Tên target là duy nhất,
  không thay được nội dung sau khi phát hành, kể cả sau khi bị thu hồi.

## Công việc định kỳ

```bash
# Mỗi 15 phút, trên máy chủ phân phối (khóa trực tuyến):
feco-tuf refresh --repo /srv/feco-tuf --sign <snapshot>,<timestamp>

# Mỗi tháng, bởi dịch vụ phát hành (khóa targets):
feco-tuf resign --repo /srv/feco-tuf --sign <targets>,<snapshot>,<timestamp>

# Kiểm tra kho như một launcher mới cài:
feco-tuf status --repo /srv/feco-tuf
```

Thư mục `metadata/` và `targets/` của kho được phục vụ tĩnh qua HTTPS (CDN được). Máy chủ web chỉ
cần quyền đọc; khóa không nằm trên máy chủ web.

## Phát hành và thu hồi

```bash
# Sau khi phiên bản qua kiểm duyệt (nhóm 9 sẽ gọi bước này tự động):
feco-tuf publish --repo DIR --file soundlab.zip --name soundlab-1.2.0-windows-x64.zip \
  --app soundlab --version 1.2.0 --platform windows-x64 --sign <targets>,<snapshot>,<timestamp>

# Thu hồi khẩn cấp (lý do được ký cùng metadata và hiện ở launcher):
feco-tuf revoke --repo DIR --name soundlab-1.2.0-windows-x64.zip --reason "Phát hiện mã độc" \
  --sign <targets>,<snapshot>,<timestamp>
```

## Buổi ký root

1. Chuẩn bị một máy không nối mạng, cài sẵn `feco-tuf` đã kiểm hash bản build.
2. Mỗi người giữ khóa root tự tạo khóa trên thiết bị của mình (`feco-tuf keygen`) và đọc to key ID.
   Thư ký ghi key ID vào biên bản.
3. Dựng kho (`feco-tuf init`) với 3 khóa root; ít nhất 2 người ký. Thư ký ghi key ID của khóa
   targets, snapshot, timestamp.
4. Chép `metadata/1.root.json` ra ngoài để đưa vào bộ cài launcher. Khóa root rời máy cùng người giữ.
5. Hai người cùng ký biên bản.

## Xoay khóa

| Tình huống | Lệnh | Ai phải có mặt |
|---|---|---|
| Khóa targets, snapshot hoặc timestamp bị lộ, hoặc đến hạn thay | `feco-tuf rotate --role targets --keys <khóa mới>` | 2 người giữ khóa root, cộng khóa targets, snapshot, timestamp đang hiệu lực theo root mới |
| Một khóa root bị lộ hoặc người giữ nghỉ việc | `feco-tuf rotate --role root --keys <3 khóa root mới> --threshold 2` | 2 người giữ khóa root hiện tại (không tính khóa bị lộ) và người giữ khóa mới |
| Root sắp hết hạn | `feco-tuf rotate --role timestamp --keys <khóa timestamp hiện tại>` (giữ nguyên khóa, chỉ tăng phiên bản và gia hạn) | 2 người giữ khóa root |

`rotate` từ chối phát hành root thiếu chữ ký, và ký lại targets, snapshot, timestamp theo root mới
trong cùng bước, để launcher không bao giờ thấy kho ở trạng thái lệch. Khi khóa bị lộ, ghi sự cố theo
`docs/security/incident-response.md`.

## Diễn tập

`scripts/tuf-drill.sh` chạy toàn bộ quy trình trên khóa tạm: dựng kho 2/3, phát hành, xoay khóa
targets, xoay khóa root (khóa bị loại không còn được tính), thu hồi, làm mới timestamp. Script in
biên bản Markdown và dừng với mã lỗi nếu một bước sai mong đợi. CI chạy script này trên Ubuntu và
Windows ở mỗi commit. Biên bản lưu ở `docs/security/drills/`.

Buổi diễn tập với người giữ khóa thật (máy ngoại tuyến, khóa thật, chữ ký trên biên bản) làm khi đã
chỉ định người giữ khóa root.
