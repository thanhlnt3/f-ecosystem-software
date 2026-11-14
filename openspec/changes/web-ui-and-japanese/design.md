# Design

## Context

- **Web**: dùng askama. Mọi template nhận `i18n: Lang` (tiếng Việt, tiếng Anh) và kế thừa `base.html`, một khung `main` hẹp với CSS nội tuyến. Ngôn ngữ lưu trong cookie `feco_lang`.
- **Bảng quản trị**: cũng chỉ là một trang con của khung đó.
- **F-Launcher**: có `i18n.rs` (Việt, Anh, Pháp) và `style.rs` (10 phong cách, kiểm độ tương phản WCAG).

## Goals / Non-Goals

**Goals**
- Đổi ngôn ngữ và giao diện web ngay, không cần đăng nhập, không phụ thuộc JavaScript.
- Một nguồn bảng màu cho F-Launcher và web.
- Dashboard cho người quản trị thấy ngay việc cần làm theo vai trò.

**Non-Goals**
- Tiếng Pháp trên web. Ghi trong QUESTIONS.md; có thể làm theo cùng cách với tiếng Nhật.
- Dịch email và thông báo nghiệp vụ sang tiếng Nhật. Hiện dùng bản tiếng Anh.
- Lưu tùy chỉnh giao diện vào tài khoản để đồng bộ giữa các máy.

## Decisions

### W1. Tiếng Nhật trên web: bảng riêng, rơi về tiếng Anh

- `Lang::Ja`, mã `ja`.
- Bảng chính giữ dạng `"khóa" => ("vi", "en")`. Bảng tiếng Nhật nằm trong `i18n_ja.rs` (`"khóa" => "ja"`), để không phải viết lại 490 dòng của bảng chính.
- `t(Ja, key)` trả bản tiếng Nhật, không có thì trả tiếng Anh.
- **Test**: quét `i18n.rs` lấy mọi khóa, kiểm khóa nào cũng có bản tiếng Nhật không rỗng.
- Chỗ trong mã đang khớp hết các trường hợp `Lang::Vi` và `Lang::En` (chuỗi song ngữ viết thẳng trong mã, ví dụ thông báo lỗi của Cổng nhà phát triển) cho `Ja` dùng bản tiếng Anh.
- **Tự nhận ngôn ngữ**: `Accept-Language` bắt đầu bằng `ja` thì chọn tiếng Nhật.
- **Đổi ngôn ngữ**: `/lang/ja` nhận như `/lang/vi` và `/lang/en`.

### W2. Tùy chỉnh giao diện web bằng cookie và middleware

- **Cookie** `feco_ui`, có hạn 1 năm, dạng `theme=dark&accent=teal&style=parchment&density=compact&size=110`.
  - Mỗi trường đọc riêng; giá trị lạ về mặc định, giống `settings.json` của launcher.
  - Cỡ chữ kẹp 90–125%.
- **Middleware** `ui_prefs::apply`: với phản hồi `text/html`, gắn `data-theme`, `data-style`, `data-accent`, `data-density` và `style="--scale:…"` vào thẻ `<html>` đầu tiên. Không phải thêm trường vào hàng chục struct template, và trang hiện đúng giao diện từ lần vẽ đầu (không nhấp nháy).
- **CSS** trong `base.html` dùng biến:
  - `--bg`, `--card`, `--fg`, `--muted`, `--line`, `--accent`, `--on-accent`, `--err`, `--warn`, `--ok`, `--radius`, `--radius-card`, `--gap`;
  - mỗi phong cách là một khối `[data-style=…]`, sinh từ `feco_core::ui_palette` khi biên dịch; test bảo đảm CSS khớp bảng màu;
  - Chân phương và Trang nhã theo `data-theme` (hoặc `prefers-color-scheme` khi chọn theo hệ thống) và `data-accent`.
- **Trang `/preferences`**: biểu mẫu GET/POST có CSRF, ghi cookie rồi quay lại trang trước (chỉ chấp nhận đường dẫn nội bộ).

### W3. Bảng màu dùng chung

- `feco_core::ui_palette` chứa:
  - `StyleId` (10 phong cách);
  - `Palette` (màu nền, thẻ, chữ, chữ mờ, viền, màu nhấn, chữ trên nút, cảnh báo, lỗi);
  - `Shape` (bo góc, khoảng cách);
  - `motif`;
  - hàm độ tương phản WCAG cùng test.
- F-Launcher `style.rs` giữ tên hiển thị (qua `t()`) và chuyển dữ liệu từ `feco_core`.
- Web dùng cùng dữ liệu để sinh CSS. Không còn hai bảng màu phải giữ khớp tay.

### W4. Dashboard quản trị

- `admin::home` tính số liệu bằng các truy vấn tổng hợp nhỏ:
  - `role_requests` đang chờ;
  - phiên bản chờ duyệt thủ công;
  - đơn có mốc đến hạn trong 2 ngày chưa bàn giao (cùng điều kiện cảnh báo trễ hạn ở 14.2);
  - lệnh hoàn chờ phê duyệt;
  - doanh thu 30 ngày và theo ngày trong 14 ngày (giao dịch thành công, trừ phần đã hoàn);
  - số người dùng (Kratos), Lập trình viên, Quản trị viên.
- **Quyền xem thẻ**:

  | Thẻ | Vai trò con |
  |---|---|
  | Yêu cầu quyền, số người dùng | Quản trị hệ thống |
  | Phiên bản chờ duyệt | Người duyệt app |
  | Đơn sắp trễ | Quản lý dự án |
  | Hoàn tiền chờ duyệt, doanh thu | Tài chính |

- **Biểu đồ** là các thanh `div` có chiều cao tính sẵn theo phần trăm của ngày cao nhất, có `title` ghi số tiền. Không dùng thư viện JavaScript.
- **Bố cục**: `admin.html` có thanh bên (các mục theo vai trò, mục đang mở được tô) và vùng nội dung. Trên màn hẹp, thanh bên chuyển thành hàng ngang.

### W5. Passkey ở dev

- `feco-config` đọc `FECO_ADMIN_PASSKEY_OPTIONAL` và chỉ nhận `true` khi môi trường là `dev`. Staging hay production đặt biến này thì báo lỗi cấu hình và không khởi động.
- `admin::gate` bỏ kiểm passkey khi cờ bật; mọi kiểm tra khác giữ nguyên (đăng nhập, là thành viên quản trị, vai trò con).
- Trang quản trị hiện dải cảnh báo khi cờ bật.

### W6. F-Launcher tiếng Nhật

- `Language::Ja`, tên `日本語`. Bản tiếng Nhật nằm ở bảng riêng `i18n_ja.rs` (`JA`: khóa tiếng Việt, tiếng Nhật), giống cách làm ở web, để không sửa lại từng dòng của `TABLE`; test kiểm mọi khóa của `TABLE` có bản tiếng Nhật với cùng số `{}` và `JA` không có khóa thừa.
- **Phông chữ**: Segoe UI không có chữ Nhật, nên launcher nạp thêm phông dự phòng của hệ điều hành:
  - Windows: Yu Gothic (`YuGothM.ttc`) hoặc Meiryo (`meiryo.ttc`);
  - macOS: Hiragino;
  - Linux: Noto Sans CJK.

  Không đóng gói phông chữ. Không tìm thấy phông nào thì vẫn chạy, nhưng chữ Nhật không hiện được; ghi cảnh báo vào nhật ký.

## Risks / Trade-offs

- [Middleware sửa HTML] → Chỉ đụng thẻ `<html` đầu tiên của phản hồi `text/html` nhỏ hơn 2 MB; có unit test.
- [Bản dịch tiếng Nhật do người không bản ngữ viết] → Ghi vào QUESTIONS.md để người bản ngữ rà soát trước khi phát hành thương mại.
- [Bỏ passkey ở dev] → Chỉ nhận ở `FECO_ENV=dev`, có test cấu hình; trang hiện cảnh báo rõ.
