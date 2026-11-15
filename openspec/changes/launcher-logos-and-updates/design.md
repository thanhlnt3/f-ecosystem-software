# Design

## G1. Logo đi cùng metadata đã ký

Có hai cách đưa logo tới launcher:
- **Target TUF riêng cho logo**: thêm một tệp phải tải và kiểm cho mỗi app.
- **Nhúng vào trường `custom` của target**: chọn cách này. Logo được ký cùng hash của gói, launcher nhận ngay khi đồng bộ, không phải tải thêm và không tin nguồn nào khác.

Quy ước logo:
- PNG, tối đa 64 KB (`MAX_ICON_BYTES`); dùng 128 px thì chỉ vài KB.
- `icon_from_png` kiểm chữ ký PNG và kích thước trước khi ký.
- `set-icon` ghi `icon` vào mọi target của app rồi ký lại targets, snapshot, timestamp. Nhờ vậy gắn được logo cho phiên bản đã phát hành mà không phải phát hành lại.

Danh mục F-Store:
- Lưu cùng logo ở cột `icon` (base64).
- Cập nhật app không kèm logo thì giữ logo cũ (`coalesce`).
- Khi máy chủ ký phát hành từ Cổng nhà phát triển, logo trong danh mục được ký cùng target.

Web:
- `/store/{id}/icon.png` trả PNG, `Cache-Control: public, max-age=3600`.
- Danh sách chỉ trả cờ `has_icon` để JSON gọn.

Launcher:
- Giải mã PNG bằng crate `png` (RGBA 8 bit) thành texture egui, mỗi app một lần.
- Logo F-Launcher nhúng trong tệp chạy.

## G2. Logo F-Launcher

Chữ F nghiêng ghép từ ba hình bình hành màu cam (`#f37021`), xanh lá (`#2fb34b`), xanh dương (`#1e88e5`) trên nền xanh đậm bo góc. Thêm một tia nhỏ gợi việc "khởi chạy".

Logo chỉ lấy cảm hứng từ bộ ba màu, không chép hình khối hay chữ của logo FPT. Việc FPT đồng ý dùng yếu tố nhận diện ghi ở QUESTIONS.md.

## G3. Cửa sổ giữa màn hình và cửa sổ chặn

- **Cửa sổ chính**: `NativeOptions::centered = true`.
- **Cửa sổ con**: vị trí = (cỡ màn hình − cỡ cửa sổ) / 2, lấy `monitor_size` của cửa sổ chính. Vị trí tính một lần khi cửa sổ mở, sau đó người dùng kéo đi đâu thì để đó.
- **Cửa sổ chặn**: egui chưa có cửa sổ modal của hệ điều hành, nên khi có cửa sổ con:
  - cửa sổ chính được phủ một lớp mờ ở tầng trên cùng, lớp này nhận mọi cú bấm, nên không bấm được gì phía dưới;
  - bấm vào lớp phủ, hoặc cửa sổ chính được chọn (focus), thì gửi `ViewportCommand::Focus` tới cửa sổ con;
  - vì cửa sổ chính bị chặn, mỗi lúc chỉ có một cửa sổ con; trong cửa sổ chi tiết vẫn chuyển được sang app trước hoặc sau.

## G4. Gợi ý cập nhật

- `update_suggestions(my_apps, skipped)` lấy các app đã cài có `update` (bản mới hơn, đúng nền tảng, đúng đợt phát hành dần), trừ phiên bản trong `skipped_updates` của `settings.json`. Có bản mới hơn bản đã bỏ qua thì gợi ý lại.
- Sau mỗi lần đồng bộ (lúc mở, mỗi giờ, khi bấm Đồng bộ):
  - nếu có bản chưa gợi ý trong phiên chạy này (`app@phiên bản`) thì mở hộp thoại;
  - "Để sau" chỉ đóng hộp thoại, Thư viện vẫn có băng nhắc và nút "Xem";
  - khi bật "Tự động cập nhật", launcher tự cập nhật nên không hỏi.

## G5. Đồng bộ tính năng launcher và web

| Tính năng | Launcher | Web |
|---|---|---|
| Thư viện (app đã cài, bản mới) | Thư viện | `/library` (từ lượt cài launcher báo về và app đã mua) |
| Cửa hàng, giá F-Coin, mua app | Cửa hàng | `/store` |
| Ví F-Coin | số dư ở thanh bên, mua trong launcher | `/wallet` (nạp, rút, lịch sử, thống kê) |
| Thông báo | trang Thông báo | `/notifications` |
| Giao diện, ngôn ngữ (vi, en, fr, ja) | Cài đặt | `/preferences` |
| Hồ sơ, vai trò, dự án, đơn xây app | menu "F-Eco trên web" | trang tương ứng |
| Cổng nhà phát triển, bảng quản trị | lối tắt theo vai trò | trang tương ứng |

Cài, mở, gỡ app chỉ làm được trên máy có launcher. Web báo "Mở F-Launcher để cập nhật".
