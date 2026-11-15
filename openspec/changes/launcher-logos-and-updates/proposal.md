# Proposal

## Why

Chủ dự án muốn bốn việc cho F-Launcher và web:

1. **Logo.** App nào cũng có logo, nhận ra ngay ở launcher và F-Store. Logo lấy từ repo của app nếu có, nếu không thì tự thiết kế. F-Launcher có logo riêng, cách điệu theo nhận diện của Tập đoàn FPT.
2. **Cửa sổ.** Cửa sổ con của launcher (Cài đặt, chi tiết app) hiện ở giữa màn hình và là cửa sổ chặn. Launcher khi mở cũng nằm giữa màn hình.
3. **Đồng bộ tính năng.** Launcher và web có cùng tính năng.
4. **Gợi ý cập nhật.** Launcher phát hiện phiên bản mới của các app con nó quản lý và gợi ý người dùng cập nhật.

Hiện trạng:
- Launcher và F-Store chỉ vẽ ô chữ cái đầu.
- Cửa sổ con mở ở vị trí hệ điều hành chọn, không chặn cửa sổ chính.
- Launcher có nút "Cập nhật" nhưng không chủ động báo khi có bản mới.
- Web không có trang tương ứng Thư viện của launcher; launcher không có lối tới ví, dự án, đơn hàng trên web.

## What Changes

- **Logo app**:
  - metadata TUF thêm trường `icon` (PNG base64, tối đa 64 KB), ký cùng target;
  - `feco-tuf publish --icon` và `feco-tuf set-icon`;
  - danh mục F-Store lưu logo (`catalog_apps.icon`) và phục vụ ở `/store/{id}/icon.png`;
  - launcher và web hiện logo, không có thì vẫn dùng chữ cái đầu.
- **Logo trong môi trường dev**:
  - bottom, kibi, eframe template: lấy từ repo gốc (đã được chủ dự án cho phép tải);
  - Ghi chú, Bấm giờ: F-Eco tự thiết kế.
- **Logo F-Launcher**: chữ F nghiêng ghép từ ba thanh cam, xanh lá, xanh dương. Logo dùng làm:
  - biểu tượng cửa sổ;
  - logo thanh bên;
  - biểu tượng trang web (`/favicon.svg`) và logo thanh trên cùng của web.
- **Cửa sổ**:
  - launcher mở ở giữa màn hình;
  - Cài đặt và chi tiết app mở ở giữa màn hình, là cửa sổ chặn: cửa sổ chính bị phủ mờ, không nhận thao tác, và bấm vào thì đưa cửa sổ con lên trước, cho tới khi cửa sổ con đóng.
- **Đồng bộ tính năng launcher và web**:
  - web thêm trang "Ứng dụng của tôi" (`/library`): app đã cài qua launcher, app đã mua, báo bản mới;
  - launcher thêm menu "F-Eco trên web" (hồ sơ và vai trò, ví, ứng dụng của tôi, dự án, đơn xây app, thông báo);
  - ví F-Coin, giá, mua app có ở cả hai nơi (thay đổi fcoin-wallet-and-marketplace).
- **Gợi ý cập nhật**:
  - sau mỗi lần đồng bộ (lúc mở, mỗi giờ, khi bấm Đồng bộ), launcher so phiên bản đã cài với danh mục đã ký;
  - có bản mới chưa gợi ý thì hiện hộp thoại "Có bản cập nhật mới": phiên bản cũ → mới, ghi chú phát hành, các nút Cập nhật, Bỏ qua bản này, Cập nhật tất cả, Để sau;
  - Thư viện có băng nhắc khi còn bản cập nhật;
  - khi đang bật "Tự động cập nhật" thì không hỏi.

## Capabilities

### New Capabilities

Không có.

### Modified Capabilities

- `launcher`: logo app và logo launcher; cửa sổ giữa màn hình và cửa sổ chặn; gợi ý cập nhật; menu web.
- `app-catalog`: logo app trên F-Store; trang "Ứng dụng của tôi" trên web.

## Impact

- `crates/feco-tuf`: `TargetMeta::icon`, `icon_from_png`, `Repo::set_icon`, lệnh `set-icon`.
- Máy chủ:
  - migration `0016_app_icons.sql`;
  - `catalog::icon`, `catalog::library`;
  - các route `/store/{id}/icon.png`, `/library`, `/favicon.svg`;
  - `feco-catalog add-app` hiểu trường `icon_file`.
- F-Launcher: logo, cửa sổ, gợi ý cập nhật, menu web; thêm crate `png` (đã có trong cây phụ thuộc của eframe).
- `assets/logos/`: logo và nguồn gốc. `scripts/dev-apps.sh icons`.
