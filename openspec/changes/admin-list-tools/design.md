# Design

## L1. Module `listing`

`ListQuery::parse(&HashMap<String, String>)` đọc các tham số sau:

- `q`: cắt khoảng trắng hai đầu, tối đa 200 ký tự;
- `sort`: `""` (mặc định), `new`, `old`, `sender`, `sender_desc`; giá trị khác coi như mặc định;
- `status`: `""` (mặc định), `all`, hoặc một mã trạng thái;
- `from`, `to`: dạng `YYYY-MM-DD`; sai định dạng thì bỏ qua.

`listing::apply(rows, &query, hidden_by_default, |row| Item { at, sender, text, status })`:

1. **Trạng thái**:
   - `""`: bỏ các dòng có trạng thái trong `hidden_by_default`, ví dụ đơn đã xong hoặc đã hủy;
   - `all`: giữ hết;
   - mã khác: chỉ giữ dòng có đúng trạng thái đó.
2. **Ngày**: `from <= at.date() <= to`, theo ngày UTC như thời gian đang hiển thị. Dòng không có thời gian thì bỏ qua lọc ngày.
3. **Tìm kiếm**: so `fold(text + " " + sender)` với từng từ của `fold(q)`; mọi từ phải có mặt. `fold` chuyển chữ thường và bỏ dấu tiếng Việt (bảng ký tự trong module, gồm `đ` → `d`).
4. **Sắp xếp** ổn định:
   - mặc định giữ thứ tự sẵn có, tức thứ tự ưu tiên hiện nay;
   - `new` / `old` theo `at`;
   - `sender` / `sender_desc` theo `fold(sender)`, cùng người gửi thì mới nhất trước.

`Toolbar` là dữ liệu cho template `admin_toolbar.html`. Template được nhúng vào đầu mỗi danh sách. Thanh gồm:

- `<form method="get">` với `q`, `status` (khi danh sách có trạng thái), `sort`, `from`, `to`;
- nút "Lọc", liên kết "Xóa lọc" khi có điều kiện;
- `#list-count` "Đang hiện N/M".

Không cần JavaScript.

## L2. Áp dụng cho từng danh sách

| Trang | Danh sách | Thời gian | Người gửi | Trạng thái (ẩn mặc định) |
|---|---|---|---|---|
| `/admin/reviews` | hàng chờ duyệt thủ công | lúc nộp | tên tài khoản nhà phát triển | không có |
| `/admin/orders` | đơn | lúc tạo | email khách hàng | trạng thái đơn (ẩn `completed`, `cancelled`) |
| `/admin/refunds` | lệnh hoàn đang mở | lúc tạo | người tạo lệnh | `pending_approval`, `processing` |
| `/admin/members` | yêu cầu vai trò, người có quyền | lúc gửi yêu cầu (người có quyền: không có) | email | vai trò nền tảng |
| `/admin/projects` | dự án | cập nhật lần cuối | email khách hàng | trạng thái dự án (không ẩn, như trước) |
| `/admin/chat-reports` | báo cáo | lúc báo cáo | tên người báo cáo | `open`, `handled` |
| `/admin/wallet` | hàng chờ rút, yêu cầu cấp F-Coin | lúc tạo | email ví | trạng thái lệnh rút |

Ghi chú:

- **Đơn xây app**: câu truy vấn chỉ còn bỏ đơn nháp, giới hạn 500 đơn mới nhất. Lọc mặc định (đang mở) giữ hành vi cũ.
- **Dự án**: `projects::list_all` vẫn trả tối đa 200 dự án, mọi trạng thái, tranh chấp trước như trước.
- **Báo cáo tin nhắn**: `chat::reports` nối `conversations` để lấy tên người báo cáo.
- **Ví**: ô tìm sổ cái (`q`) đã có. Thanh công cụ dùng chung `q` cho hàng chờ rút, yêu cầu cấp và sổ cái; sổ cái vẫn tìm như trước.

## L3. Giao diện điện thoại

`base.html` thêm hai quy tắc dưới 800px:

- `.admin-main table { display:block; overflow-x:auto; }`;
- `.list-tools` xuống dòng, ô tìm kiếm rộng hết dòng.

## L4. `admin_sections`

`admin::sections(roles) -> Vec<(key, href)>` là nguồn chung cho `admin::nav` và `GET /v1/account/role`. Thứ tự các mục:

1. `reviews`
2. `users`
3. `orders`
4. `refunds`
5. `members`
6. `wallet`
7. `projects`
8. `chat-reports`

Mỗi mục chỉ có khi tài khoản có vai trò tương ứng. API trả `admin_sections: [{"key", "url"}]`, trong đó `url` là địa chỉ đầy đủ theo `public_url`.

## L5. F-Launcher

- `AccountRole.admin_sections: Vec<(String, String)>`: chỉ nhận URL `https` hoặc `localhost`, như `admin_console`.
- Nút "Bảng quản trị" thành menu:
  - "Trang chủ quản trị";
  - một dòng cho mỗi mục, nhãn dịch theo khóa (bốn thứ tiếng, khóa lạ thì hiện nguyên khóa).
- Chọn một dòng thì gọi `open::that(url)`.

## L6. App di động

- `Account.adminSections`: danh sách `(key, url)`, chỉ nhận `https` hoặc `localhost`.
- Tab Tài khoản, khi có mục quản trị, hiện thẻ "Bảng quản trị": mỗi mục là một `ListTile` mở URL bằng `openUrl`.
- Trên điện thoại thật, URL localhost được đổi qua `forDevice` như trang web khác.
