# Design

## Context

Mọi chuỗi giao diện của F-Launcher 1.1 viết thẳng bằng tiếng Việt trong `main.rs` (khoảng 120 chuỗi), cùng một số nhãn trong `library.rs` và `prefs.rs`. Cài đặt giao diện lưu trong `settings.json`: mỗi trường đọc riêng, giá trị hỏng thì về mặc định (design E1 của `improve-launcher-experience`). Giao diện vẽ bằng egui 0.37: màu và độ bo góc lấy từ `egui::Style` cho từng chủ đề sáng và tối.

## Goals / Non-Goals

**Goals**
- Đổi ngôn ngữ và phong cách ngay khi chọn, không phải khởi động lại; nhớ cho lần mở sau.
- Không còn chuỗi giao diện nào chưa dịch: có test tự động.
- Phong cách nào cũng đủ tương phản để đọc: có test tự động.

**Non-Goals**
- Dịch thông báo lỗi kỹ thuật từ các thư viện bên dưới (TUF, mạng, cài đặt). Hiện vẫn tiếng Việt, ghi trong QUESTIONS.md.
- Dịch nội dung do nhà phát triển viết.
- Ảnh nền, tranh, hay phông chữ riêng cho từng phong cách.

## Decisions

### P1. Bảng dịch theo chuỗi tiếng Việt gốc

- `i18n::t("Thư viện")` trả bản dịch theo ngôn ngữ đang chọn. Chuỗi có tham số dùng `i18n::tf("Đã cài {} {}", &[&title, &version])`; bản dịch giữ đúng số `{}`.
- Bảng dịch nằm ngay trong mã, mỗi dòng gồm `(vi, en, fr)`. Không thêm thư viện i18n.
- Khóa chính là chuỗi tiếng Việt. Vì vậy mã vẫn đọc được như trước, và chuỗi chưa có bản dịch thì tự hiện tiếng Việt.
- Ngôn ngữ đang chọn nằm trong một biến toàn cục (`AtomicU8`), được đặt khi áp cài đặt. Không phải truyền tham số qua mọi hàm vẽ.
- **Test**:
  - quét mã nguồn `main.rs`, `library.rs`, `prefs.rs`, `style.rs` tìm mọi lời gọi `t("…")` và `tf("…")`, rồi kiểm từng chuỗi có trong bảng với bản tiếng Anh và tiếng Pháp không rỗng;
  - số `{}` của bản dịch khớp bản gốc;
  - không còn chuỗi có dấu tiếng Việt nằm ngoài `t` hoặc `tf` trong mã giao diện, trừ nhật ký cho nhà phát triển (`eprintln!`).
- Tên ngôn ngữ luôn hiện bằng chính ngôn ngữ đó ("Tiếng Việt", "English", "Français"), để người không đọc được ngôn ngữ hiện tại vẫn tìm ra lựa chọn của mình.

### P2. Phong cách là dữ liệu thuần trong `style.rs`

Mỗi phong cách (`UiStyle`) có:
- **chế độ**:
  - Chân phương và Trang nhã theo chủ đề sáng, tối hoặc hệ thống và màu nhấn người dùng chọn;
  - phong cách họa sĩ có bảng màu cố định (nền sáng hoặc tối) và màu nhấn riêng, nên ô chọn chủ đề và màu nhấn bị tắt kèm lời giải thích;
- **bảng màu**: nền cửa sổ, nền thanh bên, nền thẻ, chữ, chữ mờ, viền, màu nhấn, chữ trên nút nhấn;
- **hình khối**: bo góc nút và thẻ, độ dày viền, khoảng cách;
- **dải họa tiết**: 5 màu vẽ thành dải dưới tên F-Launcher ở thanh bên và trong ô xem trước.

`main.rs` chỉ chuyển dữ liệu này thành `egui::Style`.

**Test tương phản** (WCAG 2.1) cho mọi phong cách:
- chữ trên nền thẻ và nền cửa sổ đạt ít nhất 4,5:1;
- chữ mờ đạt ít nhất 3:1;
- chữ trên nút nhấn đạt ít nhất 4,5:1.

### P3. Bảng màu của phong cách họa sĩ

Bảng màu gợi lại cách dùng màu thường gặp trong tranh của họa sĩ, do F-Eco tự chọn. Không dùng, cắt hay tái tạo bất kỳ bức tranh nào.

| Phong cách | Nền | Gợi ý màu |
|---|---|---|
| Leonardo da Vinci | sáng | giấy da, nâu sepia, đất nung (bản phác thảo) |
| Michelangelo | sáng | vữa bích họa, xanh lam đá, đỏ đất |
| Vincent van Gogh | tối | xanh đêm thẫm, vàng crôm, xanh coban |
| Claude Monet | sáng | xanh lục nhạt mặt nước, tím oải hương, hồng phấn |
| Pablo Picasso | sáng | xanh thời kỳ Xanh, góc vuông, viền đậm như tranh lập thể |
| Tô Ngọc Vân | sáng | trắng ngà, lục nhạt, vàng hoa huệ |
| Bùi Xuân Phái | sáng | vàng đất tường phố cũ, xám ngói, nâu son |
| Nguyễn Gia Trí | tối | đen sơn mài, son đỏ, vàng thếp |

Tên họa sĩ chỉ dùng để mô tả nguồn cảm hứng. Nhãn và mô tả ghi "lấy cảm hứng từ bảng màu …", không ngụ ý họa sĩ hay người thừa kế bảo trợ sản phẩm (QUESTIONS.md).

### P4. Cài đặt

`settings.json` thêm hai trường:
- `language`: `vi`, `en` hoặc `fr`;
- `style`: `plain`, `elegant`, `davinci`, `michelangelo`, `vangogh`, `monet`, `picasso`, `to_ngoc_van`, `bui_xuan_phai`, `nguyen_gia_tri`.

Giá trị lạ thì về mặc định, như các trường khác. "Khôi phục mặc định" đưa cả hai về tiếng Việt và Chân phương.

## Risks / Trade-offs

- [Bảng dịch trong mã dễ thiếu khi thêm chuỗi mới] → Test P1 làm CI báo lỗi ngay khi thiếu.
- [Phong cách tối và sáng làm một số màu trạng thái (cảnh báo, lỗi) khó đọc] → Màu cảnh báo và lỗi lấy theo nền sáng hoặc tối của phong cách, và có trong test tương phản.
- [Tên họa sĩ còn được bảo hộ quyền nhân thân hoặc liên quan tới người thừa kế (họa sĩ Việt Nam)] → Chỉ dùng tên để mô tả cảm hứng; câu hỏi pháp lý ghi trong QUESTIONS.md, có thể đổi tên hiển thị mà không đổi mã cài đặt.

## Migration Plan

`settings.json` của 1.1 không có hai trường mới, nên giữ nguyên giao diện như cũ: tiếng Việt, Chân phương.
