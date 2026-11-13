# Design

## Context

F-Launcher 1.0 (`clients/feco-launcher`) là app egui:
- `main.rs` chỉ vẽ giao diện;
- thư viện `feco_launcher` giữ mọi logic: TUF, cài đặt, mã khởi chạy, `state.json`.

Danh mục trong launcher dựng từ metadata TUF đã ký (`TargetMeta`), gồm `app`, `title`, `version`, `platform`, `entry`, `client_id`, `private`. Mô tả, nhóm, nhà phát triển nằm trong bảng `catalog_apps` của F-Store, nhưng launcher không đọc bảng này: nguồn tin cậy của launcher là TUF.

## Goals / Non-Goals

**Goals**
- Xem app theo lưới hoặc danh sách, tìm, lọc, sắp xếp, và nhớ lựa chọn giữa các lần mở.
- Trang cài đặt giao diện, lưu trên máy.
- Danh mục dev có thêm app mẫu dùng F-SDK và app mã nguồn mở dựng từ GitHub.
- Hướng dẫn có thể làm lại từng bước để đưa một app từ GitHub vào F-Eco.

**Non-Goals**
- Đồng bộ cài đặt giao diện giữa các máy.
- Ảnh biểu tượng (icon) do nhà phát triển tải lên. Bản này dùng ô chữ cái đầu có màu riêng cho từng app. Icon thật cần thêm một target TUF, để sau.
- Bản dịch tiếng Anh của giao diện launcher (task 15.1 của `add-f-ecosystem-platform`).
- Hộp cát cho app ngoài: vẫn theo mức cô lập đã công bố (D5).

## Decisions

### E1. Cài đặt giao diện lưu trong `settings.json`, tách khỏi `state.json`

`state.json` là trạng thái cài đặt app; một lỗi ở đó có thể làm mất dấu app đã cài. Cài đặt giao diện để trong một file riêng (`<data>/settings.json`):
- ghi nguyên tử, giống `state.json`;
- file hỏng hoặc có giá trị lạ thì dùng giá trị mặc định cho trường đó, không chặn launcher;
- cỡ chữ bị kẹp trong khoảng 80–150%.

"Tự động cập nhật" vẫn nằm trong `state.json` như 1.0, vì logic cập nhật dùng nó. Trang Cài đặt chỉ là nơi hiển thị.

### E2. Sắp xếp và lọc là hàm thuần trong thư viện

`library::arrange(items, &Query)` nhận danh sách, chuỗi tìm, nhóm và kiểu sắp xếp, rồi trả thứ tự hiển thị. Giao diện chỉ gọi hàm này. Nhờ vậy mỗi scenario sắp xếp và lọc có unit test mà không cần dựng giao diện.
- Tìm kiếm bỏ dấu tiếng Việt (giống F-Store) trên tên, mô tả ngắn, nhà phát triển và nhóm.
- "Mở gần đây" dùng `last_opened`, trường mới trong `InstalledApp`, được ghi (`Launcher::mark_opened`) khi launcher khởi chạy app thành công. App chưa mở lần nào xếp cuối, theo tên.

### E3. Thông tin hiển thị nằm trong metadata TUF đã ký

`TargetMeta` thêm các trường không bắt buộc: `summary`, `category`, `developer`, `source`, `terminal`.
- Vì nằm trong `custom` của target, các trường này được ký cùng mã băm: một máy chủ phân phối bị chiếm quyền không đổi được tên nhà phát triển hay cờ terminal.
- Máy chủ điền các trường khi ký:
  - `summary`, `category`, `source`, `terminal` từ manifest của phiên bản;
  - `developer` từ `catalog_apps.developer`.
- Trường thiếu thì launcher hiện giá trị trống: nhóm "Khác", không có mô tả.

Phương án đã loại: launcher gọi `GET /v1/catalog/{id}` để lấy mô tả. Cách này cần mạng mỗi lần vẽ, không chạy được ngoại tuyến, và đặt nội dung chưa ký lên cùng màn hình với nút Cài.

### E4. Ô chữ cái đầu thay cho icon

- Mỗi app có một ô vuông bo góc, hiện 1–2 chữ cái đầu của tên.
- Màu nền lấy từ băm FNV-1a của mã app, chọn trong một bảng màu cố định có độ tương phản đủ với chữ trắng.
- Hàm `monogram(title)` và `tile_color(app_id)` ở trong thư viện và có unit test: cùng app luôn cùng màu, chữ cái đầu đúng với tên có dấu.

### E5. App chạy trong terminal

App như kibi và bottom cần một cửa sổ terminal. `TargetMeta.terminal = true` thì launcher mở như sau:
- **Windows**: chạy `cmd /C start "" <file>`; bản thân `cmd` không hiện cửa sổ (`CREATE_NO_WINDOW`). `start` mở một cửa sổ console mới với stdin/stdout riêng. Chỉ đặt cờ `CREATE_NEW_CONSOLE` thì chưa đủ: app vẫn nhận stdio mà launcher thừa hưởng (ví dụ một file nhật ký), và app terminal báo "handle is invalid".
- **macOS**: `open -a Terminal <file>`. Biến môi trường không qua được `open`, nên app terminal không nhận được mã khởi chạy. Vì vậy `terminal = true` kèm scope đăng nhập bị f-dev cảnh báo trên macOS.
- **Linux** (chỉ dev): `x-terminal-emulator -e <file>`, hoặc `$TERMINAL`.

Lệnh mở được dựng bằng hàm thuần `launch::command`, có unit test theo từng hệ điều hành.

### E6. App mã nguồn mở bên ngoài

App mã nguồn mở được đưa vào giống mọi app con khác: có manifest, được kiểm duyệt, ký TUF. App không dùng F-SDK nên không khai báo scope, và launcher không xin mã khởi chạy cho app không có `client_id`, như 1.0.

Thêm hai yêu cầu:
1. Gói phải chứa file giấy phép gốc (`LICENSE*`, `COPYING*` hoặc `NOTICE*`) khi manifest có `source`. `f-dev pack` từ chối gói thiếu file này.
2. `license` trong manifest phải khớp giấy phép của dự án gốc. Người kiểm duyệt đối chiếu qua liên kết `source`.

Ở dev, `scripts/dev-apps.sh github <owner/repo> <ref> ...` làm các bước:
- tải tệp nén mã nguồn của đúng tag hoặc commit từ `codeload.github.com` vào thư mục riêng ngoài repo;
- dựng bằng `cargo build --release --locked`;
- đóng gói file chạy kèm giấy phép gốc;
- ký TUF và ghi vào danh mục.

Mã nguồn tải về không được commit. Ba app trong danh mục dev được chọn vì giấy phép nằm trong danh sách cho phép (MIT, Apache-2.0), là dự án Rust dựng được bằng toolchain sẵn có, và gồm cả app terminal lẫn app cửa sổ.

### E7. App mẫu dùng F-SDK

`examples/apps/f-notes` (Ghi chú) và `examples/apps/f-timer` (Bấm giờ) là app egui nhỏ dùng crate `fsdk`:
- có phiên từ launcher (mã khởi chạy) thì hiện tên người dùng qua `/v1/me`;
- không có thì vẫn chạy được ở chế độ khách;
- dữ liệu lưu trong thư mục app mà launcher cấp (`FECO_APP_DATA_DIR`).

Hai app là thành viên workspace, nên được kiểm tra fmt, clippy, test và giấy phép như mã còn lại. Chúng là mẫu để nhà phát triển sao chép.

## Risks / Trade-offs

- [Mã nguồn bên thứ ba khi dựng chạy build script] → Chỉ dựng tag hoặc commit cố định, đã xem build script (kibi và bottom chỉ gọi `git describe`), và dựng trong thư mục riêng ngoài repo. Hướng dẫn nhắc nhà phát triển làm như vậy.
- [App terminal trên macOS không nhận được mã khởi chạy] → Ghi rõ trong hướng dẫn; f-dev cảnh báo.
- [Ô chữ cái đầu trùng màu giữa hai app] → Bảng 12 màu, kèm chữ cái đầu và tên, nên vẫn phân biệt được.

## Migration Plan

- Metadata cũ thiếu trường mới: launcher hiện giá trị mặc định.
- Launcher 1.0 gặp metadata mới: serde bỏ qua trường lạ, nên vẫn cài được. Với app terminal:
  - Windows tự cấp console cho chương trình console được mở từ một app không có console, nên vẫn chạy;
  - macOS mở app mà không có cửa sổ terminal.

  Chấp nhận được, vì launcher sẽ tự cập nhật (task 6.6).
