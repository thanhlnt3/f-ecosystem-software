# Spec Delta

## Purpose

F-Builder là app con có sẵn của F-Eco, giúp người dùng tự xây một app mới từ mô tả hoặc bộ spec (ví dụ OpenSpec) bằng coding agent, kiểm tra kết quả, rồi chạy thử hoặc nộp lên F-Eco.

## ADDED Requirements

### Requirement: Tạo dự án
F-Builder SHALL tạo dự án từ một trong các nguồn: mô tả bằng lời, thư mục chứa spec (gồm cấu trúc OpenSpec), hoặc một địa chỉ web hay ứng dụng để tham khảo. Mỗi dự án MUST có thư mục riêng chứa: yêu cầu ban đầu, dữ liệu đã thu thập, mã nguồn app, kết quả kiểm tra, câu hỏi còn mở và lịch sử các lượt.

#### Scenario: Tạo từ OpenSpec
- **WHEN** người dùng chọn một thư mục có `openspec/changes/<tên>/` và yêu cầu "Xây theo spec này"
- **THEN** dự án được tạo, spec được nhận làm tài liệu đầu vào, và app được dựng trong thư mục `app/` bên cạnh

#### Scenario: Tạo từ mô tả
- **WHEN** người dùng nhập "Ứng dụng quản lý chi tiêu gia đình, có biểu đồ theo tháng"
- **THEN** dự án được tạo và F-Builder hỏi các điểm còn thiếu (nền tảng, ngôn ngữ) trước khi xây

### Requirement: Quy trình Tìm hiểu, Xây, Kiểm tra, Sẵn sàng
Mỗi lượt xây SHALL đi qua bốn bước có hiển thị tiến độ: Tìm hiểu (đọc spec hoặc nguồn tham khảo), Xây (coding agent viết mã), Kiểm tra (build, chạy test, so với spec hoặc nguồn tham khảo), Sẵn sàng (báo kết quả, câu hỏi mở). Người dùng MUST dừng, tiếp tục hoặc yêu cầu sửa ở bất kỳ lúc nào.

#### Scenario: Yêu cầu sửa
- **GIVEN** dự án đang ở bước Sẵn sàng
- **WHEN** người dùng nhập "Thêm nút xuất Excel"
- **THEN** một lượt mới bắt đầu từ bước Xây trên chính mã nguồn hiện có

#### Scenario: Tiếp tục sau khi gián đoạn
- **GIVEN** máy tắt đột ngột trong bước Xây
- **WHEN** người dùng mở lại dự án và chọn Tiếp tục
- **THEN** lượt xây chạy tiếp từ trạng thái đã lưu gần nhất, không mất phần mã đã viết

### Requirement: Không bị treo im lặng
Khi một bước không có tiến triển trong 10 phút (không ghi nhật ký, không đổi file, tiến trình con không dùng CPU), F-Builder SHALL báo cho người dùng và đề xuất Tiếp tục chờ, Chạy lại bước hoặc Dừng. Khi coding agent đã bàn giao kết quả, F-Builder MUST kết thúc lượt và hiện kết quả, không chờ thêm.

#### Scenario: Bước kiểm tra bị kẹt
- **WHEN** bước Kiểm tra không có tiến triển trong 10 phút
- **THEN** F-Builder hiện cảnh báo "Bước Kiểm tra không có tiến triển trong 10 phút" kèm ba lựa chọn

### Requirement: Chế độ clean room mặc định
Khi có nguồn tham khảo, F-Builder SHALL mặc định chỉ dùng những gì người dùng bình thường quan sát được (giao diện, chữ hiển thị, hành vi). Chế độ đọc sâu (đọc mã, giải nén, dịch ngược) MUST yêu cầu người dùng xác nhận là phần mềm của họ hoặc họ có quyền phân tích, và kết quả của chế độ này MUST bị chặn khi nộp lên danh mục công khai.

#### Scenario: Bật chế độ đọc sâu
- **WHEN** người dùng bật chế độ đọc sâu cho một ứng dụng tham khảo
- **THEN** F-Builder yêu cầu xác nhận quyền phân tích và ghi lại xác nhận đó vào dự án

#### Scenario: Nộp kết quả từ chế độ đọc sâu
- **GIVEN** dự án có lượt chạy ở chế độ đọc sâu
- **WHEN** người dùng chọn Nộp lên F-Store
- **THEN** F-Builder chặn và giải thích rằng kết quả từ chế độ đọc sâu chỉ dùng riêng

### Requirement: Không sao chép thương hiệu và tài sản
F-Builder SHALL dặn coding agent không dùng tên, logo, hình ảnh, văn bản giao diện của sản phẩm tham khảo, trừ khi người dùng xác nhận có quyền. Bước Kiểm tra MUST cảnh báo khi phát hiện tên sản phẩm tham khảo trong giao diện app mới.

#### Scenario: Tên gốc trong giao diện
- **WHEN** app mới hiện tên của sản phẩm tham khảo trên thanh tiêu đề
- **THEN** bước Kiểm tra cảnh báo "Giao diện chứa tên của sản phẩm tham khảo"

### Requirement: Chọn coding agent và nhà cung cấp AI
F-Builder SHALL cho người dùng chọn coding agent đã cài trên máy (ví dụ Claude Code) bằng tài khoản của chính họ, hoặc nhà cung cấp AI bằng khóa API của họ. Khóa API MUST lưu trong kho bảo mật của hệ điều hành, và MUST không gửi về máy chủ F-Eco.

#### Scenario: Dùng khóa API riêng
- **WHEN** người dùng nhập khóa API của nhà cung cấp AI trong Cài đặt
- **THEN** khóa được lưu vào kho bảo mật của hệ điều hành và không xuất hiện trong nhật ký hay dữ liệu đồng bộ

### Requirement: Chạy lệnh có kiểm soát
Coding agent SHALL chỉ ghi file trong thư mục dự án. Việc cài thư viện, chạy lệnh mạng hay chạy mã do agent tạo MUST cần người dùng cho phép, trừ khi người dùng đã bật "Cho phép chạy không cần hỏi" cho dự án đó.

#### Scenario: Agent muốn cài thư viện
- **WHEN** agent muốn chạy `npm install` lần đầu trong dự án
- **THEN** F-Builder hỏi người dùng, và gợi ý `--ignore-scripts` để không chạy script cài đặt

### Requirement: Chạy thử và nộp app
Khi dự án ở trạng thái Sẵn sàng, người dùng SHALL chạy thử app ngay trong F-Builder, và tạo được `f-app.toml` cùng gói cài đặt để nộp qua cổng nhà phát triển. Nộp lên danh mục MUST đi qua đủ quy trình của capability `app-review`.

#### Scenario: Nộp app tự xây
- **WHEN** người dùng là nhà phát triển chọn Nộp lên F-Store cho một dự án đã Sẵn sàng
- **THEN** F-Builder tạo manifest nháp để người dùng xem lại, rồi nộp phiên bản ở trạng thái "Chờ duyệt"

### Requirement: Chuyển thành đơn xây app theo yêu cầu
Từ một dự án F-Builder, người dùng SHALL chọn "Nhờ F-Eco xây hộ" để tạo đơn theo capability `custom-build-orders`. Spec, mô tả và (nếu người dùng cho phép) mã nguồn hiện có được đính kèm vào đơn.

#### Scenario: Nhờ xây hộ
- **WHEN** người dùng bấm "Nhờ F-Eco xây hộ" trong một dự án có spec OpenSpec
- **THEN** một đơn nháp được tạo, đã đính kèm spec, và chờ người dùng gửi
