# Proposal

## Why

Các danh sách của bảng quản trị chưa lọc, sắp xếp hay tìm kiếm được:

- hàng chờ duyệt app;
- đơn xây app;
- lệnh hoàn tiền;
- phân quyền (yêu cầu vai trò và người có quyền);
- dự án;
- báo cáo tin nhắn;
- hàng chờ rút F-Coin và yêu cầu cấp F-Coin.

Mỗi danh sách chỉ có một thứ tự cố định. Khi danh sách dài, quản trị viên không tìm được theo người gửi, theo thời gian hay theo trạng thái.

Bảng quản trị chỉ có trên web. F-Launcher máy tính có một nút "Bảng quản trị" mở trang chủ quản trị. App di động chưa có lối vào nào. Chủ dự án chọn **dùng chung trang web** cho cả ba phiên bản (21/11/2026): F-Launcher và app di động mở thẳng từng mục quản trị trên web, nơi có lọc, sắp xếp và tìm kiếm. Thao tác quản trị vẫn đi qua web với xác thực hai lớp như hiện nay.

## What Changes

- **Thanh công cụ chung** cho mọi danh sách quản trị:
  - ô tìm kiếm: không phân biệt hoa thường và dấu tiếng Việt; nhiều từ thì phải khớp tất cả;
  - lọc trạng thái;
  - lọc khoảng ngày (từ, đến);
  - sắp xếp: mặc định (thứ tự ưu tiên hiện nay), mới nhất, cũ nhất, người gửi A–Z, người gửi Z–A;
  - nút "Xóa lọc" và dòng "Đang hiện N/M".
- Điều kiện lọc nằm trên URL (`GET`), nên chia sẻ hay tải lại trang vẫn giữ nguyên.
- **Người gửi** của từng danh sách:
  - hàng chờ duyệt: nhà phát triển;
  - đơn: khách hàng;
  - hoàn tiền: người tạo lệnh;
  - yêu cầu vai trò: người xin;
  - người có quyền: email;
  - dự án: khách hàng;
  - báo cáo tin nhắn: người báo cáo;
  - rút F-Coin và cấp F-Coin: email ví.
- Đơn xây app lọc được cả đơn đã xong hoặc đã hủy. Mặc định vẫn chỉ hiện đơn đang mở, như trước. Danh sách dự án vẫn hiện mọi trạng thái, giờ lọc được theo từng trạng thái.
- Bảng quản trị vừa màn hình điện thoại: bảng cuộn ngang, thanh công cụ xuống dòng.
- `GET /v1/account/role` trả thêm `admin_sections`: các mục quản trị mà tài khoản được vào, gồm khóa và địa chỉ.
- **F-Launcher máy tính**: nút "Bảng quản trị" mở menu các mục quản trị; chọn một mục thì mở trang đó trong trình duyệt.
- **App di động**: tab Tài khoản có thẻ "Bảng quản trị" cho quản trị viên, liệt kê các mục và mở trang web tương ứng.

Trang "Người dùng" (Hỗ trợ) giữ cách tìm theo đúng email hoặc mã đơn và che email, để không lộ danh sách người dùng (spec admin-console, "Hỗ trợ khách hàng").

## Capabilities

### Modified Capabilities
- `admin-console`: lọc, sắp xếp, tìm kiếm danh sách; giao diện trên điện thoại; mục quản trị cho ứng dụng.
- `launcher`: menu mục quản trị.
- `mobile-launcher`: lối vào bảng quản trị.

## Impact

- `services/feco-server`:
  - module mới `listing.rs` và template `admin_toolbar.html`;
  - `admin.rs`, `projects_web.rs`, `chat_web.rs`, `chat.rs`, `wallet_web.rs`, `launch.rs`, `i18n*.rs`, `templates/admin*.html`, `base.html`;
  - E2E `admin_list_tools`.
- `clients/feco-launcher`: `auth.rs` (`AccountRole.admin_sections`), `main.rs`.
- `clients/feco-mobile`: `models.dart`, `api.dart`, `ui.dart`, `i18n.dart`, `demo.dart`, kèm test.
- Không đổi cơ sở dữ liệu.
