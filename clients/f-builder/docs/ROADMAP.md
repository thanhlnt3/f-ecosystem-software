# Kế hoạch F-Builder trong F-Eco

Kế hoạch chính là nhóm task 11 trong `openspec/changes/add-f-ecosystem-platform/tasks.md` ở gốc repo
(spec `app-builder`). Các dòng `- [ ]` dưới đây là việc còn mở của F-Builder (`cargo xtask pending`);
dòng `- [x]` là việc đã xong. Kế hoạch của Craft Anything 0.6.0 (bản gốc) không được mang theo.

## Nhóm 11

### R11.1 Fork, đổi tên, gỡ tài sản thương hiệu
- [x] Nhập Craft Anything 0.6.0 vào `clients/f-builder` (git subtree, squash), giữ LICENSE và NOTICE
- [x] Đổi tên crate, binary, thư mục dữ liệu, biến môi trường, định danh; gỡ `docs/brand` và biểu tượng
      con quạ; biểu tượng và màu giao diện của F-Eco
- [ ] Dựng lại `packaging/macos/dmg/DS_Store` bằng `generate.py` (cần resvg, ds_store, mac_alias)

### R11.2 Tạo dự án và bốn bước
- [x] Nhận thư mục OpenSpec (`openspec/changes/<tên>/`) làm tài liệu đầu vào, app dựng trong `app/`
- [x] Hỏi nền tảng và ngôn ngữ trước khi xây khi dự án tạo từ mô tả còn thiếu
- [x] Lịch sử các lượt trong thư mục dự án
- [x] Mở lại dự án bị gián đoạn thì đề xuất Tiếp tục

### R11.3 Không bị treo im lặng
- [x] Cảnh báo sau 10 phút không tiến triển (nhật ký, file, CPU) với Tiếp tục chờ, Chạy lại bước, Dừng
- [x] Kết thúc lượt khi agent đã bàn giao kết quả, kể cả khi tiến trình agent chưa thoát
- [x] Máy chủ MCP hủy các lệnh đang chờ khi stdin đóng

### R11.4 Clean room và thương hiệu
- [x] Xác nhận quyền phân tích khi bật đọc sâu, ghi vào dự án
- [ ] Chặn nộp lên F-Store khi dự án có lượt chạy ở chế độ đọc sâu
- [x] Bước Kiểm tra cảnh báo khi giao diện app mới chứa tên sản phẩm tham khảo

### R11.5 Khóa API và chạy lệnh
- [ ] Khóa API chỉ lưu trong kho bảo mật của hệ điều hành (bỏ tệp dự phòng dạng chữ)
- [ ] Hỏi trước khi cài thư viện, chạy lệnh mạng, chạy mã agent tạo; gợi ý `--ignore-scripts` cho npm
- [ ] Agent chỉ ghi file trong thư mục dự án (kể cả ở chế độ tự động)

### R11.6 Nộp app và nhờ xây hộ
- [ ] Tạo `f-app.toml` nháp và gói cài đặt từ dự án Sẵn sàng, nộp qua API cổng nhà phát triển
- [ ] Đăng nhập F-Eco bằng F-SDK
- [ ] "Nhờ F-Eco xây hộ": tạo đơn nháp đính kèm spec (chờ nhóm 12)
