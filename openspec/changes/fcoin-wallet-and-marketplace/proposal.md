# Proposal

## Why

Hiện mỗi khoản tiền trên F-Eco đi thẳng qua cổng thanh toán và chỉ có một luồng: khách đặt đơn xây app với đội F-Eco. Chủ dự án muốn:

- một **đơn vị thanh toán chung** chỉ dùng trong hệ sinh thái, tên **F-Coin**, để mua app, trả công lập trình viên và nhận tiền bán app;
- một **chợ phần mềm**: khách hàng gửi yêu cầu làm phần mềm cho lập trình viên cá nhân hoặc công ty phần mềm, thương lượng giá, và chỉ trả tiền khi đã nhận bàn giao;
- **sáu vai trò** rõ ràng thay cho ba vai trò hiện nay.

## What Changes

- **Sáu vai trò nền tảng** (thay Người sử dụng ứng dụng, Lập trình viên, Quản trị viên):

  | Vai trò | Quyền chính |
  |---|---|
  | Người dùng phổ thông (mặc định) | ví F-Coin (nạp, rút), mua app theo giá niêm yết (có thể 0 F-Coin) |
  | Khách hàng cá nhân | như trên; gửi yêu cầu làm phần mềm, thương lượng giá, ký quỹ, nhận bàn giao rồi trả tiền |
  | Khách hàng doanh nghiệp | như Khách hàng cá nhân; thêm quyền lập và ký hợp đồng |
  | Lập trình viên cá nhân | ví F-Coin; nhận yêu cầu làm phần mềm; gửi app mới lên kho (Cổng nhà phát triển) |
  | Công ty phát triển phần mềm | như Lập trình viên cá nhân; thêm quyền lập và ký hợp đồng |
  | Quản trị viên | đặt tỷ giá, phí; quản lý tài khoản (vai trò, khóa ví); duyệt app mới; duyệt rút tiền; xử lý tranh chấp |

  Lập trình viên hiện có thành Lập trình viên cá nhân. Người dùng xin vai trò Khách hàng, Lập trình viên hay Công ty ở trang Vai trò; quản trị viên duyệt. Vai trò doanh nghiệp và công ty phải khai tên pháp lý và mã số thuế.
- **Ví F-Coin** cho mọi tài khoản:
  - sổ cái ghi kép, đơn vị nguyên F-Coin;
  - không chuyển tự do giữa người dùng: F-Coin chỉ đi qua mua app, ký quỹ và giải ngân dự án, nạp, rút.
- **Tỷ giá** do quản trị viên (Tài chính) đặt, kèm lý do và lịch sử. Tỷ giá ban đầu: 1 F-Coin = 1.000 ₫. Mỗi lần nạp hay rút lưu tỷ giá lúc tạo lệnh.
- **Nạp F-Coin** qua các cổng đã kết nối (ZaloPay, VNPAY, MoMo): người dùng chọn số F-Coin, trả số tiền tương ứng. Cổng xác nhận thì F-Coin vào ví.
- **Rút F-Coin**: F-Coin bị giữ lại ngay, quản trị viên Tài chính duyệt (từ 10.000.000 ₫ cần hai người). Có hai cách nhận tiền:
  - hoàn về giao dịch nạp qua chính cổng thanh toán đó;
  - chuyển khoản ngân hàng: Tài chính chuyển rồi ghi mã chuyển khoản.

  Bị từ chối hay lỗi thì F-Coin về lại ví.
- **Mua app bằng F-Coin**:
  - giá niêm yết bằng F-Coin, nhà phát triển đặt ở Cổng nhà phát triển;
  - mua trên web hoặc trong F-Launcher, nhận quyền sử dụng (`license`);
  - nhà phát triển nhận F-Coin trừ phí nền tảng.
- **Dự án phần mềm** (chợ): mỗi dự án đi qua các bước sau.
  - Hồ sơ nhận việc: lập trình viên và công ty bật hồ sơ để khách tìm thấy.
  - Gửi yêu cầu: khách hàng gửi cho một người nhận việc.
  - Thương lượng: hai bên đưa giá và thời hạn qua lại, một bên chấp nhận đề nghị của bên kia.
  - Ký quỹ: khách hàng chuyển F-Coin vào ký quỹ.
  - Bàn giao: lập trình viên bàn giao; khách hàng chấp nhận (F-Coin được chuyển cho lập trình viên) hoặc yêu cầu sửa.
  - Tranh chấp: Quản lý dự án của F-Eco xử lý (giải ngân, hoàn, chia).
- **Hợp đồng**: Khách hàng doanh nghiệp hoặc Công ty phát triển phần mềm lập hợp đồng cho dự án từ mẫu. Hai bên ký bằng cách nhập họ tên và đồng ý. F-Eco lưu nội dung, mã băm SHA-256, người ký và thời điểm. Dự án có hợp đồng thì chỉ ký quỹ được khi hai bên đã ký.
- **Giao dịch và thống kê**:
  - người dùng xem lịch sử, lọc theo loại; xem thống kê chi tiêu và thu nhập theo tháng, theo loại;
  - quản trị viên xem tổng F-Coin đang lưu hành, đang ký quỹ, chờ rút, nạp và rút 30 ngày, phí thu được.
- **F-Launcher**:
  - hiện số dư F-Coin ở thanh bên, bấm vào thì mở trang ví;
  - hiện giá app;
  - app có giá mà chưa mua thì nút chính là "Mua · N F-Coin", mua xong mới cài.

## Capabilities

### New Capabilities

- `fcoin-wallet`: ví, sổ cái, tỷ giá, nạp, rút, mua app, giao dịch, thống kê, phí, khóa ví.
- `software-projects`: hồ sơ nhận việc, yêu cầu làm phần mềm, thương lượng, ký quỹ, bàn giao, tranh chấp, hợp đồng.

### Modified Capabilities

- `account-identity`: sáu vai trò nền tảng; yêu cầu vai trò.
- `admin-console`: trang Ví F-Coin (tỷ giá, phí, rút tiền, khóa ví, thống kê), trang Dự án (tranh chấp).
- `payments`: nạp F-Coin qua cổng; rút bằng hoàn về giao dịch nạp.
- `app-catalog`: giá app bằng F-Coin.
- `launcher`: số dư F-Coin, giá app, mua app trước khi cài.

## Impact

- Máy chủ:
  - migration `0015_fcoin.sql`;
  - module mới `wallet.rs`, `wallet_web.rs`, `projects.rs`, `projects_web.rs`;
  - sửa `roles.rs`, `payments.rs`, `payments_web.rs`, `admin.rs`, `store_web.rs`, `catalog.rs`, `dev_web.rs`;
  - template mới;
  - chuỗi giao diện bốn thứ tiếng.
- API mới cho launcher: `GET /v1/wallet`, `POST /v1/wallet/purchases`.
- F-Launcher: số dư, giá, mua app.
- Pháp lý: F-Coin có thể đổi ngược ra tiền nên cần luật sư xác nhận trước khi ra mắt (QUESTIONS.md). Ví dụ có cần giấy phép trung gian thanh toán hay ví điện tử không, và cách xuất hóa đơn khi nạp. Có công tắc `FECO_WALLET_WITHDRAWALS` để tắt rút tiền khi cần.
