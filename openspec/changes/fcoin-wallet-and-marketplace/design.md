# Design

## R1. Sáu vai trò

`PlatformRole` có sáu giá trị:

| Mã | Tên | Lưu ở |
|---|---|---|
| `user` | Người dùng phổ thông | mặc định, không có dòng nào |
| `customer` | Khách hàng cá nhân | `role_grants.role` |
| `business_customer` | Khách hàng doanh nghiệp | `role_grants.role` |
| `developer` | Lập trình viên cá nhân | `role_grants.role` |
| `dev_company` | Công ty phát triển phần mềm | `role_grants.role` |
| `admin` | Quản trị viên | `admin_members` (giữ 5 vai trò con) |

Quyền hỏi qua hàm, không so tên vai trò rải rác:

| Hàm | Vai trò có quyền |
|---|---|
| `can_develop` (Cổng nhà phát triển, gửi app lên kho) | developer, dev_company, admin |
| `can_take_projects` (nhận yêu cầu làm phần mềm) | developer, dev_company |
| `can_commission` (gửi yêu cầu làm phần mềm) | customer, business_customer |
| `can_sign_contracts` (lập hợp đồng) | business_customer, dev_company |
| ví (nạp, rút, mua app) | mọi tài khoản |

Lập trình viên hiện có giữ mã `developer`, nay mang tên "Lập trình viên cá nhân". Vì vậy migration không phải đổi dữ liệu.

`role_requests` có thêm các cột:
- `role`;
- `legal_name` và `tax_code`: bắt buộc với business_customer và dev_company, kiểm tra mã số thuế như thông tin xuất hóa đơn.

## R2. Sổ cái ghi kép

- `wallet_accounts`: mỗi người dùng có một tài khoản (`kind = 'user'`), tạo khi cần. Có bốn tài khoản hệ thống:

  | Mã | Ý nghĩa |
  |---|---|
  | `issuance` | đối ứng của F-Coin phát hành; âm bằng tổng F-Coin đang lưu hành |
  | `escrow` | ký quỹ dự án |
  | `withdrawals` | F-Coin đang chờ rút |
  | `fees` | phí nền tảng |

- `wallet_txns` là một bút toán, gồm loại, mã `GD-000001`, ghi chú, tham chiếu, người tạo và khóa idempotency (duy nhất).
- `wallet_entries` là các dòng của bút toán: tài khoản, số F-Coin (có dấu), số dư sau dòng.
- Mỗi bút toán có tổng các dòng bằng 0. `wallet::post` kiểm tra điều này trước khi ghi.
- Số dư tài khoản người dùng có ràng buộc `CHECK (kind <> 'user' OR balance >= 0)`. Vì vậy dù mã có lỗi, cơ sở dữ liệu vẫn không cho số dư âm.
- `post` khóa các tài khoản liên quan (`FOR UPDATE`, theo thứ tự id) trong cùng giao dịch Postgres với thay đổi nghiệp vụ, như đổi trạng thái dự án. Như vậy không có nửa thao tác.
- Đơn vị là nguyên F-Coin (`BIGINT`):
  - người dùng chọn số F-Coin cần nạp, số tiền phải trả = F-Coin × tỷ giá, không phải làm tròn;
  - phí tính theo phần vạn (bps) rồi làm tròn xuống. Phần lẻ thuộc về người nhận.
- Kiểm tra bất biến (unit test và `feco-catalog wallet-check`): tổng số dư mọi tài khoản bằng 0; số dư mỗi tài khoản bằng tổng các dòng của nó.

## R3. Tỷ giá

- `coin_rates` lưu lịch sử tỷ giá: số đồng cho 1 F-Coin, người đặt, lý do, thời điểm. Migration thêm dòng 1.000 ₫.
- Tỷ giá hiện hành là dòng mới nhất.
- Chỉ quản trị viên có vai trò con Tài chính được đổi. Lý do là bắt buộc, giá trị trong khoảng 1–1.000.000 ₫, và mỗi lần đổi ghi nhật ký kiểm toán.
- Lệnh nạp và lệnh rút lưu tỷ giá lúc tạo. Đổi tỷ giá không làm thay đổi lệnh đang chờ.
- Giá app và giá dự án tính bằng F-Coin nên không đổi theo tỷ giá.

## R4. Nạp qua cổng thanh toán

`payment_requests` hiện gắn với đơn hàng. Migration:
- bỏ `NOT NULL` của `order_id`;
- thêm `topup_id`;
- thêm ràng buộc: có đúng một trong hai cột.

Luồng nạp:
1. Người dùng chọn số F-Coin (10–50.000).
2. `wallet_topups` (mã `NAP-000001`, trạng thái `pending`) và một `payment_request` loại `charge` được tạo trong cùng giao dịch.
3. Người dùng được chuyển tới trang thanh toán sẵn có (`/payments/{id}/pay`). Trang này dùng chung chọn cổng, liên kết 15 phút, truy vấn chủ động, đối soát và hóa đơn.
4. `payments::settle` gặp yêu cầu nạp thì gọi `wallet::topup_settled`. Hàm này ghi bút toán `issuance → user` với khóa idempotency `topup:<id>`, nên callback lặp lại không cộng hai lần.

Các phần khác của module thanh toán (`charge`, `owner`, thư, trang kết quả) đọc thông tin chung qua `LEFT JOIN` cả đơn hàng lẫn lệnh nạp.

## R5. Rút tiền

1. Người dùng tạo lệnh rút (mã `RUT-000001`, tối thiểu 10 F-Coin). Bút toán `user → withdrawals` giữ F-Coin ngay, nên không tiêu được hai lần.
2. Chọn cách nhận tiền:
   - **Hoàn về giao dịch nạp**: chọn một lần nạp thành công của chính mình, qua cổng thật, còn hoàn được đủ số tiền. Phần còn hoàn được = số đã thu − tổng lệnh hoàn chưa thất bại. Khi được duyệt, máy chủ tạo lệnh hoàn sẵn có của module thanh toán (`refunds.kind = 'withdrawal'`, `withdrawal_id`). Lệnh hoàn này coi như đã được duyệt, nên cổng nhận lệnh ngay.
   - **Chuyển khoản ngân hàng**: tên ngân hàng, số tài khoản, tên chủ tài khoản. Số tài khoản được mã hóa bằng `crypto::DataKey` (AAD `withdrawal-account:<id>`); trang quản trị chỉ hiện 4 số cuối, trừ khi người Tài chính bấm "Hiện" (ghi kiểm toán). Khi được duyệt, Tài chính chuyển khoản bên ngoài rồi bấm "Đã chuyển" kèm mã chuyển khoản.
3. Phê duyệt:
   - dưới 10.000.000 ₫ cần một người Tài chính, từ 10.000.000 ₫ cần hai người khác nhau (như lệnh hoàn);
   - người dùng hủy được lệnh khi chưa đủ duyệt.
4. Kết thúc:

   | Kết quả | Bút toán | Trạng thái |
   |---|---|---|
   | Thành công | `withdrawals → issuance` (F-Coin bị hủy) | `paid` |
   | Bị từ chối, bị hủy hoặc hoàn thất bại | `withdrawals → user` | `rejected`, `cancelled` hoặc `failed` |

5. Công tắc `FECO_WALLET_WITHDRAWALS` (mặc định bật) cho phép tắt việc tạo lệnh rút mới khi pháp lý chưa rõ. Lệnh đang chờ vẫn xử lý được.

## R6. Mua app

- Thêm `catalog_apps.price_coin`; migration điền bằng `ceil(price_vnd / 1000)`. Bộ lọc "Miễn phí" dùng `price_coin = 0`.
- Nhà phát triển đặt giá ở trang app trong Cổng nhà phát triển. Giá lưu ở `developer_apps.price_coin` và được chép sang danh mục.
- Mua một app:
  1. khóa ví;
  2. kiểm tra chưa có quyền sử dụng;
  3. ghi bút toán `người mua → người bán` (trừ phí) và `→ fees`;
  4. cấp `entitlements` loại `license`, nguồn `fcoin:<mã giao dịch>`.

  Khóa idempotency `purchase:<người mua>:<app>`.
- Người bán là chủ (`owner`) tài khoản nhà phát triển của app. App của F-Eco (không có nhà phát triển) ghi vào `fees`.
- App giá 0 không cần mua.
- Giá thay đổi giữa lúc xem và lúc bấm: yêu cầu mua mang `expected_price`, khác thì trả `409 price_changed`.

## R7. Dự án phần mềm

- `provider_profiles`: tên hiển thị, giới thiệu, kỹ năng, `accepting`. Chỉ người có `can_take_projects` tạo được. Danh sách người nhận việc chỉ hiện hồ sơ đang nhận việc, không lộ email.
- `projects`: mã `DA-000001`, khách hàng, người nhận việc, tiêu đề, mô tả, ngân sách đề xuất, giá và thời hạn đã chốt, trạng thái.
- `project_offers`: đề nghị giá và thời hạn của từng bên. Đề nghị mới thay đề nghị cũ (`superseded`).
- `project_events`: lịch sử trạng thái.
- `project_messages`: tin nhắn.

Trạng thái:

```
requested ──(bên kia đề nghị)──▶ negotiating ──(chấp nhận đề nghị của bên kia)──▶ agreed
agreed ──(khách ký quỹ)──▶ in_progress ──(bàn giao)──▶ delivered
delivered ──(khách chấp nhận)──▶ completed
delivered ──(khách yêu cầu sửa)──▶ in_progress
in_progress | delivered ──(khách mở tranh chấp)──▶ disputed ──(quản trị xử lý)──▶ completed | refunded
requested | negotiating | agreed ──(một bên hủy, hoặc người nhận việc từ chối)──▶ cancelled
in_progress ──(người nhận việc rút lui)──▶ refunded
```

Bút toán:

| Bước | Bút toán |
|---|---|
| Ký quỹ | `khách → escrow` |
| Chấp nhận bàn giao | `escrow → người nhận việc` và `escrow → fees` (phí dự án) |
| Hoàn | `escrow → khách` |
| Chia khi tranh chấp | hai dòng từ `escrow`, tổng bằng giá đã chốt |

Mỗi bước nằm trong cùng giao dịch Postgres với việc đổi trạng thái (`SELECT … FOR UPDATE` dự án).

Không ai tự chấp nhận đề nghị của chính mình. Người nhận việc phải còn quyền `can_take_projects` lúc nhận yêu cầu.

## R8. Hợp đồng

`project_contracts` gồm:
- số `HD-000001`;
- nội dung (mẫu điền sẵn các bên, phạm vi, giá F-Coin và quy ra đồng theo tỷ giá lúc lập, thời hạn, bàn giao, nghiệm thu, sở hữu mã nguồn, tranh chấp, cộng điều khoản bổ sung);
- `sha256` của nội dung;
- trạng thái `draft`, `signed` hoặc `void`;
- người ký và thời điểm ký của từng bên.

Quy tắc:
- Chỉ một bên có `can_sign_contracts` mới lập được hợp đồng, và chỉ khi dự án đã `agreed`.
- Bên kia (vai trò nào cũng được) ký để chấp nhận. Ký nghĩa là đánh dấu "Tôi đã đọc và đồng ý" và nhập đúng họ tên đã khai ở hợp đồng.
- Đủ hai chữ ký thì hợp đồng thành `signed` và không sửa được nữa.
- Lập hợp đồng mới thì bản cũ chưa ký thành `void`.
- Khi dự án có hợp đồng chưa ký đủ, ký quỹ bị chặn (`409 contract_unsigned`).

Đây là thỏa thuận điện tử có lưu vết, không phải chữ ký số có chứng thư (QUESTIONS.md).

## R9. Phí, khóa ví, thống kê

- **Phí**: `wallet_settings` lưu `app_fee_bps` và `project_fee_bps`, mặc định 0. Tài chính sửa ở trang Ví F-Coin, mỗi lần sửa ghi kiểm toán.
- **Khóa ví** (`wallet_accounts.frozen_at`, lý do): Tài chính hoặc Quản trị hệ thống khóa.
  - Ví bị khóa không nạp, rút, mua hay ký quỹ được (`423 wallet_frozen`).
  - Ví vẫn nhận được F-Coin từ dự án và bán app, để không làm hỏng việc của người khác.
- **Thống kê**:
  - Người dùng: tổng theo loại và theo tháng (12 tháng gần nhất), tính từ `wallet_entries` của chính mình.
    - Chi: mua app, ký quỹ, rút.
    - Thu: nạp, bán app, nhận từ dự án, hoàn ký quỹ.
  - Quản trị: số dư các tài khoản hệ thống, số lệnh rút chờ duyệt, nạp và rút 30 ngày (bằng F-Coin và đồng), biểu đồ nạp và rút theo ngày.

## R10. F-Launcher

- `GET /v1/wallet` (chỉ token của launcher) trả `{balance, rate_vnd, frozen, owned: [app], prices: {app: coin}}`.
- `POST /v1/wallet/purchases {app_id, expected_price}` trả `201 {balance}`. Các mã lỗi:
  - `402 insufficient_funds`;
  - `409 already_owned` hoặc `409 price_changed`;
  - `423 wallet_frozen`.
- Launcher đọc ví mỗi lần đồng bộ:
  - thanh bên hiện số dư (icon Ví, tooltip "Số dư F-Coin · bấm để nạp hoặc rút"), bấm vào thì mở `/wallet` trên trình duyệt;
  - ô và trang chi tiết hiện giá;
  - app có giá mà chưa mua thì nút chính là "Mua · N F-Coin", mở hộp thoại xác nhận (giá, số dư sau khi mua). Mua xong thì cài ngay.
- Chưa đăng nhập: không hiện số dư; nút của app có giá là "Đăng nhập để mua".
