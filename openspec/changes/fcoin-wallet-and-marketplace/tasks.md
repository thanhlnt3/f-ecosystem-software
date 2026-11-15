# Tasks

## 1. Vai trò

- [x] 1.1 Sáu vai trò trong `roles.rs` (`customer`, `business_customer`, `dev_company`; hàm `can_develop`, `can_take_projects`, `can_commission`, `can_sign_contracts`), migration `role_grants`/`role_requests`; xác minh bằng unit test vai trò và E2E scenario "Lập trình viên có từ trước", "Công ty vào Cổng nhà phát triển"
  - Xác minh: unit test `roles_parse_and_permissions` (ma trận quyền sáu vai trò); E2E `six_roles_and_role_requests` (Lập trình viên có từ trước mang tên "Lập trình viên cá nhân" và vẫn thấy biểu mẫu Cổng nhà phát triển; Công ty phát triển phần mềm vào được Cổng nhà phát triển).
- [x] 1.2 Trang Vai trò `/account/role` (xin vai trò, tên pháp lý, mã số thuế), duyệt trong bảng quản trị; xác minh bằng E2E scenario "Xin vai trò Khách hàng doanh nghiệp", "Thiếu mã số thuế", "Duyệt yêu cầu Công ty"
  - Xác minh: E2E `six_roles_and_role_requests` (thiếu mã số thuế bị từ chối; Khách hàng doanh nghiệp được duyệt ở trang Phân quyền, có email "Đã được cấp vai trò Khách hàng doanh nghiệp"; duyệt Công ty ghi `role.set`).

## 2. Ví F-Coin

- [x] 2.1 Migration `0015_fcoin.sql` (tài khoản, bút toán, dòng, tỷ giá 1.000 ₫, cài đặt phí), `wallet::post` ghi kép có khóa và idempotency, kiểm tra sổ cái (`feco-catalog wallet-check`); xác minh bằng E2E scenario "Hai lần trừ song song", "Bất biến sổ cái"
  - Xác minh: E2E `fcoin_wallet_end_to_end` (hai lệnh trừ 80 F-Coin song song từ ví 100: đúng một lệnh thành công, còn 20; `check_ledger` đạt sau toàn bộ kiểm thử); lệnh `feco-catalog wallet-check`.
- [x] 2.2 Tỷ giá và phí (Tài chính, lý do, lịch sử, kiểm toán); xác minh bằng E2E scenario "Tỷ giá ban đầu", "Đổi tỷ giá", "Không phải Tài chính", "Phí bán app"
  - Xác minh: E2E `fcoin_wallet_end_to_end` (trang ví ghi "1 F-Coin = 1.000 ₫"; Tài chính đặt 1.200 ₫ kèm lý do thì lệnh nạp 10 F-Coin phải trả 12.000 ₫; Kiểm duyệt viên bị 403; phí app 10% thì nhà phát triển nhận 45, phí 4).
- [x] 2.3 Nạp qua cổng (`wallet_topups`, `payment_requests.topup_id`, `payments::settle` gọi `wallet::topup_settled`); xác minh bằng E2E scenario "Nạp thành công", "Callback lặp lại", "Ngoài giới hạn", "Trang thanh toán của lệnh nạp"
  - Xác minh: E2E `fcoin_wallet_end_to_end` (trang thanh toán ghi "Nạp 50 F-Coin", 50.000 ₫; callback ZaloPay có chữ ký cộng 50 F-Coin; callback lặp lại không cộng thêm; nạp 5 F-Coin bị từ chối).
- [x] 2.4 Rút tiền (giữ F-Coin, hai cách nhận, duyệt một hoặc hai người, mã hóa số tài khoản, hoàn qua cổng, từ chối, hủy, công tắc `FECO_WALLET_WITHDRAWALS`); xác minh bằng E2E scenario "Rút hoàn về giao dịch nạp", "Rút chuyển khoản ngân hàng", "Bị từ chối", "Rút quá số dư", "Lệnh lớn cần hai người duyệt", "Cổng từ chối hoàn"
  - Xác minh: E2E `fcoin_wallet_end_to_end` (rút 20 F-Coin hoàn về giao dịch nạp: cổng giả nhận lệnh hoàn 20.000 ₫, lệnh đã trả; rút ngân hàng 60: số tài khoản mã hóa, trang quản trị chỉ hiện ···6789, "Đã chuyển" kèm mã FT2611150001; từ chối trả lại F-Coin và gửi email; rút quá số dư bị từ chối; 12.000 F-Coin cần hai người duyệt; cổng từ chối hoàn thì lệnh thất bại, F-Coin về ví, có cảnh báo quản trị); unit test `large_withdrawals_need_two_approvals`.
- [x] 2.5 Mua app (giá F-Coin, `price_changed`, quyền `license`, người bán, phí), giá ở Cổng nhà phát triển, trang F-Store; xác minh bằng E2E scenario "Mua app", "Không đủ F-Coin", "Mua lại", "Trang app có giá", "Nhà phát triển đặt giá"
  - Xác minh: E2E `fcoin_wallet_end_to_end` (trang app ghi "49 F-Coin (≈ 49.000 ₫)", nút "Mua · 49 F-Coin"; mua xong còn 1 F-Coin, nhà phát triển nhận 49, quyền `license` nguồn `fcoin:GD-…`; mua lại trả 409 `already_owned`; không đủ trả 402 `insufficient_funds`); E2E `developer_sets_app_price` (đặt 99 F-Coin ở Cổng nhà phát triển, bộ lọc Miễn phí không còn app).
- [x] 2.6 Trang ví, lịch sử có lọc và phân trang, thống kê theo tháng và loại; khóa ví; xác minh bằng E2E scenario "Thống kê chi tiêu", "Ví bị khóa"
  - Xác minh: E2E `fcoin_wallet_end_to_end` (thống kê tháng: Thu 100, Chi 60, nhóm Mua app; ví bị khóa thì mua trả 423 `wallet_frozen`, số dư không đổi); unit test `monthly_statistics`, `statistics_use_vietnam_time`.

## 3. Dự án phần mềm

- [x] 3.1 Hồ sơ nhận việc và danh sách người nhận việc; xác minh bằng E2E scenario "Khách hàng tìm người nhận việc"
  - Xác minh: E2E `software_projects_end_to_end` (tìm "flutter" thấy hồ sơ của Lan, trang không có email của Lan).
- [x] 3.2 Yêu cầu, thương lượng, ký quỹ, bàn giao, chấp nhận, sửa, rút lui, hủy, tin nhắn, thông báo; xác minh bằng E2E scenario "Người dùng phổ thông gửi yêu cầu", "Thương lượng hai vòng", "Tự chấp nhận đề nghị của mình", "Trả tiền sau khi nhận bàn giao", "Không đủ F-Coin để ký quỹ", "Yêu cầu sửa"
  - Xác minh: E2E `software_projects_end_to_end` (Người dùng phổ thông thấy thông báo cần vai trò Khách hàng; thương lượng 800 → 650 F-Coin, tự chấp nhận đề nghị của mình bị 403; ký quỹ thiếu báo "còn thiếu 550 F-Coin"; yêu cầu sửa quay lại Đang làm; chấp nhận bàn giao: Lan nhận 650, ký quỹ về 0).
- [x] 3.3 Tranh chấp và trang Dự án trong bảng quản trị; xác minh bằng E2E scenario "Chia ký quỹ", "Xử lý tranh chấp"
  - Xác minh: E2E `software_projects_end_to_end` (chia 400/250 F-Coin; Quản lý dự án hoàn toàn bộ ở `/admin/projects`, ghi `project.resolve`, hai bên nhận thông báo).
- [x] 3.4 Hợp đồng (mẫu, SHA-256, ký hai bên, chặn ký quỹ); xác minh bằng E2E scenario "Doanh nghiệp lập và hai bên ký", "Khách hàng cá nhân lập hợp đồng", "Ký quỹ khi hợp đồng chưa ký đủ"
  - Xác minh: E2E `software_projects_end_to_end` (doanh nghiệp lập hợp đồng, ký quỹ khi mới một bên ký trả 409, Lan ký xong thì hợp đồng "Đã ký" với SHA-256 đúng nội dung; Khách hàng cá nhân lập hợp đồng bị 403); unit test `contract_template_fills_parties_and_price`, `signature_name_ignores_case_and_spacing`.

## 4. Bảng quản trị

- [x] 4.1 Trang Ví F-Coin (số liệu, biểu đồ, tỷ giá, phí, hàng đợi rút, tra giao dịch, khóa ví); xác minh bằng E2E scenario "Tài chính duyệt lệnh rút", "Kiểm duyệt viên mở trang Ví"
  - Xác minh: E2E `fcoin_wallet_end_to_end` (Tài chính duyệt, hiện số tài khoản, đánh dấu đã chuyển; tổng "Đang chờ rút" giảm 60; nhật ký có `wallet.withdrawal_approve`, `wallet.withdrawal_paid`, `wallet.withdrawal_reveal`; Kiểm duyệt viên mở trang Ví bị 403).

## 5. F-Launcher

- [x] 5.1 API `GET /v1/wallet`, `POST /v1/wallet/purchases` (chỉ token launcher); xác minh bằng E2E scenario "Token của app khác"
  - Xác minh: E2E `fcoin_wallet_end_to_end` (`GET /v1/wallet` bằng token launcher trả số dư, app đã mua, giá; token của app khác trả 403 `launcher_only`).
- [x] 5.2 Số dư ở thanh bên, giá ở ô và trang chi tiết, nút "Mua · N F-Coin", hộp thoại mua, mua xong thì cài; xác minh bằng unit test scenario "Mua rồi cài", "Không đủ F-Coin trong launcher" và chạy thử scenario "Xem số dư"
  - Xác minh: unit test `paid_app_needs_purchase_before_install` (app có giá chưa mua: nút Mua; đã mua: Cài; chưa đăng nhập: Đăng nhập để mua), `shortfall_when_balance_is_low` ("Còn thiếu 39 F-Coin"); chạy thử launcher 1.6.0 (số dư ở thanh bên).

## 6. Tài liệu và phát hành

- [x] 6.1 Chuỗi giao diện bốn thứ tiếng (web) và bốn thứ tiếng (launcher), README, CHANGELOG, QUESTIONS.md (pháp lý F-Coin, chữ ký hợp đồng, cổng chi hộ); phát hành 1.6.0
  - Xác minh: test `every_key_has_japanese`, `every_key_has_french`, `every_ui_string_has_translations`; README, CHANGELOG 1.6.0, QUESTIONS.md 85–90; CI và release.yml của tag `v1.6.0`.
