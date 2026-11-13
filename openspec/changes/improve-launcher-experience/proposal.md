# Proposal

## Why

Bản 1.0 của F-Launcher xếp mọi app thành một danh sách dọc, mỗi app một khung, với thanh tab đơn giản. Khi có nhiều app, người dùng khó tìm, không sắp xếp được, và không chỉnh được giao diện (sáng, tối, cỡ chữ). Các launcher của những bộ ứng dụng desktop thường cho xem app theo lưới hoặc danh sách, tìm kiếm, lọc theo nhóm, và có trang cài đặt giao diện. F-Launcher cần những chức năng đó. F-Launcher dùng thiết kế và tên riêng của F-Eco, không dùng tên, logo hay tài sản của sản phẩm khác.

Danh mục 1.0 chỉ có Demo App, nên chưa thấy được giá trị của launcher. Nhà phát triển cũng chưa có hướng dẫn đưa một app mã nguồn mở có sẵn trên GitHub vào F-Eco: dựng từ mã nguồn, giữ giấy phép gốc, và đóng gói cả app chạy trong terminal.

## What Changes

- **Bố cục lưới và danh sách** cho "Thư viện" (app đã cài) và "Cửa hàng" (danh mục): đổi qua lại bằng một nút và được nhớ lại. Có tìm kiếm không dấu, lọc theo nhóm, và sắp xếp theo tên, lần mở gần nhất, ngày cài, nhóm hoặc dung lượng.
- **Trang Cài đặt** của launcher:
  - giao diện: chủ đề sáng, tối hoặc theo hệ thống; màu nhấn; cỡ chữ; bố cục mặc định; cỡ ô lưới; trang mở đầu; hiện mô tả ngắn; chế độ gọn;
  - cập nhật: tự động cập nhật;
  - tài khoản và thông tin phiên bản;
  - nút khôi phục mặc định.

  Cài đặt lưu trên máy, không gửi lên máy chủ.
- **Thông tin hiển thị của app** trong metadata TUF đã ký: mô tả ngắn, nhóm, nhà phát triển, địa chỉ mã nguồn, cờ "chạy trong terminal". Máy chủ điền các trường này khi phát hành, từ manifest và danh mục.
- **Manifest `f-app.toml`** có thêm hai trường không bắt buộc: `source` (địa chỉ mã nguồn, HTTPS) và `terminal` (app chạy trong cửa sổ terminal).
- **Mở app chạy trong terminal**: launcher mở cửa sổ terminal riêng cho app.
- **App mới trong danh mục dev**:
  - hai app mẫu do F-Eco viết, dùng F-SDK: Ghi chú và Bấm giờ;
  - ba app mã nguồn mở dựng từ GitHub, không dùng F-SDK: kibi (soạn thảo văn bản trong terminal, MIT hoặc Apache-2.0), bottom (theo dõi hệ thống, MIT) và eframe template (app cửa sổ mẫu, MIT hoặc Apache-2.0).
- **Hướng dẫn tích hợp app từ mã nguồn trên GitHub**, kèm script dev tự tải, dựng, đóng gói (có giấy phép gốc), ký TUF và đưa vào danh mục.

## Capabilities

### New Capabilities

Không có.

### Modified Capabilities

- `launcher`: bố cục lưới và danh sách, tìm kiếm, lọc, sắp xếp; trang cài đặt giao diện; hiện thông tin app từ metadata đã ký; mở app chạy trong terminal.
- `developer-portal`: manifest có `source` và `terminal`; phát hành ghi thông tin hiển thị vào metadata TUF; gói của app mã nguồn mở phải kèm giấy phép gốc.

## Impact

- Mã:
  - `clients/feco-launcher`: giao diện (`main.rs`), các mô-đun mới `prefs.rs` và `library.rs`, thêm trường trong `catalog.rs`, `state.rs`, `launch.rs`;
  - `crates/feco-tuf` (`TargetMeta`, CLI `publish`), `crates/feco-manifest`;
  - `services/feco-server/src/developer.rs` (ký target), `tools/f-dev` (kiểm tra giấy phép trong gói).
- App mới: `examples/apps/f-notes` và `examples/apps/f-timer` (thành viên workspace); script `scripts/dev-apps.sh`.
- Tài liệu: `docs/integration-guide.md` (thêm mục), `docs/github-apps.md` (mới), README.
- Tương thích: các trường metadata mới đều không bắt buộc. Launcher 1.0 bỏ qua trường lạ, launcher mới đọc được metadata cũ. `state.json` của 1.0 vẫn đọc được.
