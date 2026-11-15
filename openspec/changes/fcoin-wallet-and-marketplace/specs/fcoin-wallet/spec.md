# Spec Delta

## Purpose

Capability mới `fcoin-wallet`: đồng F-Coin chỉ dùng trong F-Eco, ví của mỗi tài khoản, tỷ giá, nạp, rút, mua app, lịch sử giao dịch và thống kê.

## ADDED Requirements

### Requirement: Ví và sổ cái F-Coin
Mỗi tài khoản SHALL có một ví F-Coin. Đơn vị là nguyên F-Coin. Mọi thay đổi số dư MUST đi qua một bút toán ghi kép có tổng các dòng bằng 0, kèm loại, mã giao dịch và khóa idempotency duy nhất. Số dư ví người dùng MUST không bao giờ âm, kể cả khi hai thao tác chạy song song. F-Coin MUST không chuyển tự do được giữa người dùng: F-Coin chỉ đi qua nạp, rút, mua app, ký quỹ và giải ngân dự án.

#### Scenario: Hai lần trừ song song
- **GIVEN** ví có 100 F-Coin
- **WHEN** hai lệnh trừ 80 F-Coin chạy cùng lúc
- **THEN** đúng một lệnh thành công, lệnh kia báo không đủ số dư, và số dư còn 20 F-Coin

#### Scenario: Bất biến sổ cái
- **WHEN** chạy kiểm tra sổ cái sau một loạt nạp, mua, ký quỹ, rút
- **THEN** tổng số dư mọi tài khoản bằng 0 và số dư mỗi tài khoản bằng tổng các dòng của nó

### Requirement: Tỷ giá do quản trị viên đặt
Tỷ giá (số đồng cho 1 F-Coin) SHALL do quản trị viên có vai trò con Tài chính đặt. Mỗi lần đổi MUST có lý do, giá trị từ 1 đến 1.000.000, và được ghi vào lịch sử tỷ giá và nhật ký kiểm toán. Tỷ giá ban đầu SHALL là 1 F-Coin = 1.000 ₫. Lệnh nạp và lệnh rút MUST dùng tỷ giá lúc tạo lệnh.

#### Scenario: Tỷ giá ban đầu
- **WHEN** máy chủ mới cài đặt
- **THEN** trang ví ghi "1 F-Coin = 1.000 ₫"

#### Scenario: Đổi tỷ giá
- **GIVEN** quản trị viên Tài chính
- **WHEN** đặt tỷ giá 1.200 ₫ với lý do "Điều chỉnh quý I"
- **THEN** lệnh nạp 10 F-Coin tạo sau đó phải trả 12.000 ₫, và lịch sử tỷ giá có dòng 1.200 ₫ kèm lý do

#### Scenario: Không phải Tài chính
- **WHEN** quản trị viên chỉ có vai trò con Kiểm duyệt gửi biểu mẫu đổi tỷ giá
- **THEN** thao tác bị từ chối và tỷ giá không đổi

### Requirement: Nạp F-Coin qua cổng thanh toán
Người dùng đã đăng nhập SHALL nạp được từ 10 đến 50.000 F-Coin mỗi lần qua các cổng đã kết nối. Số tiền phải trả MUST bằng số F-Coin nhân tỷ giá lúc tạo lệnh. F-Coin MUST chỉ vào ví khi cổng xác nhận đã thu (callback có chữ ký, truy vấn chủ động hoặc đối soát), và MUST chỉ vào một lần dù xác nhận đến nhiều lần.

#### Scenario: Nạp thành công
- **GIVEN** ví có 0 F-Coin, tỷ giá 1.000 ₫
- **WHEN** người dùng nạp 50 F-Coin, chọn ZaloPay và cổng gửi callback thành công 50.000 ₫
- **THEN** ví có 50 F-Coin, lịch sử có dòng "Nạp" +50, và lệnh nạp ở trạng thái thành công

#### Scenario: Callback lặp lại
- **WHEN** cổng gửi lại cùng callback thành công lần thứ hai
- **THEN** số dư không đổi

#### Scenario: Ngoài giới hạn
- **WHEN** người dùng nạp 5 F-Coin
- **THEN** yêu cầu bị từ chối với thông báo số F-Coin nạp phải từ 10 đến 50.000

### Requirement: Tạo lệnh rút
Người dùng SHALL tạo được lệnh rút từ 10 F-Coin trở lên, không quá số dư. F-Coin của lệnh MUST bị giữ ngay khi tạo. Lệnh bị từ chối, bị người dùng hủy (khi chưa đủ duyệt) hoặc hoàn thất bại MUST trả lại F-Coin vào ví. Khi tắt `FECO_WALLET_WITHDRAWALS`, người dùng MUST không tạo được lệnh rút mới.

#### Scenario: Rút quá số dư
- **GIVEN** ví có 30 F-Coin
- **WHEN** người dùng rút 40 F-Coin
- **THEN** yêu cầu bị từ chối và số dư không đổi

#### Scenario: Bị từ chối
- **WHEN** quản trị viên từ chối lệnh rút 60 F-Coin với lý do
- **THEN** 60 F-Coin về lại ví và người dùng nhận thông báo kèm lý do

### Requirement: Duyệt lệnh rút
Lệnh rút MUST được quản trị viên Tài chính duyệt. Lệnh từ 10.000.000 ₫ MUST có hai người duyệt khác nhau.

#### Scenario: Lệnh lớn cần hai người duyệt
- **GIVEN** tỷ giá 1.000 ₫
- **WHEN** một người Tài chính duyệt lệnh rút 12.000 F-Coin
- **THEN** lệnh vẫn chờ duyệt, và chính người đó không duyệt lần hai được

### Requirement: Cách nhận tiền khi rút
Lệnh rút SHALL chọn một trong hai cách nhận tiền:
- **Hoàn về một giao dịch nạp** của chính người dùng, qua cổng thanh toán đó. Giao dịch nạp phải còn hoàn được đủ số tiền.
- **Chuyển khoản ngân hàng**. Số tài khoản MUST được mã hóa khi lưu.

#### Scenario: Rút hoàn về giao dịch nạp
- **GIVEN** người dùng đã nạp 50 F-Coin qua ZaloPay và còn 50 F-Coin
- **WHEN** người dùng rút 20 F-Coin về giao dịch nạp đó, rồi quản trị viên Tài chính duyệt
- **THEN** cổng nhận lệnh hoàn 20.000 ₫, lệnh rút ở trạng thái đã trả, và ví còn 30 F-Coin

#### Scenario: Rút chuyển khoản ngân hàng
- **GIVEN** ví có 100 F-Coin
- **WHEN** người dùng rút 60 F-Coin về tài khoản ngân hàng
- **THEN** ví còn 40 F-Coin ngay; trang quản trị chỉ hiện 4 số cuối của số tài khoản; khi Tài chính bấm "Đã chuyển" kèm mã chuyển khoản, lệnh ở trạng thái đã trả

### Requirement: Mua app bằng F-Coin
App có giá SHALL được mua bằng F-Coin theo giá niêm yết. Giá 0 F-Coin nghĩa là miễn phí, không cần mua. Mua thành công MUST trừ ví người mua, ghi F-Coin cho nhà phát triển (trừ phí nền tảng) và cấp quyền sử dụng `license`. Người dùng MUST không mua cùng app hai lần. Nếu giá đã đổi so với giá người dùng thấy thì yêu cầu MUST bị từ chối.

#### Scenario: Mua app
- **GIVEN** app "So Tay Pro" giá 49 F-Coin, ví có 50 F-Coin, phí app 0
- **WHEN** người dùng bấm "Mua · 49 F-Coin"
- **THEN** ví còn 1 F-Coin, nhà phát triển nhận 49 F-Coin, và `/v1/entitlements` của app có quyền `license` cho người dùng

#### Scenario: Không đủ F-Coin
- **GIVEN** ví có 10 F-Coin
- **WHEN** người dùng mua app giá 49 F-Coin
- **THEN** trả `402 insufficient_funds` kèm số F-Coin còn thiếu và lối tắt nạp

#### Scenario: Mua lại
- **WHEN** người dùng mua lại app đã mua
- **THEN** trả `409 already_owned` và không trừ F-Coin

### Requirement: Lịch sử giao dịch và thống kê
Người dùng SHALL xem được lịch sử giao dịch của mình: mã, thời điểm, loại, số F-Coin (+/−), số dư sau, ghi chú. Lịch sử có lọc theo loại và phân trang. Trang thống kê SHALL hiện:
- tổng chi và tổng thu theo loại;
- biểu đồ chi và thu theo tháng trong 12 tháng gần nhất.

Người dùng MUST chỉ thấy giao dịch của chính mình.

#### Scenario: Thống kê chi tiêu
- **GIVEN** trong tháng người dùng đã nạp 100 F-Coin, mua hai app tổng 60 F-Coin
- **WHEN** người dùng mở trang thống kê
- **THEN** tháng này có Thu 100 (Nạp) và Chi 60 (Mua app)

### Requirement: Phí nền tảng và khóa ví
Quản trị viên Tài chính SHALL đặt được phí bán app và phí dự án (phần vạn, 0–3.000, mặc định 0). Mỗi lần đổi ghi nhật ký kiểm toán. Phí tính trên mỗi lần bán hoặc giải ngân, làm tròn xuống.

Quản trị viên Tài chính hoặc Quản trị hệ thống SHALL khóa và mở được ví của một tài khoản, kèm lý do. Ví bị khóa MUST không nạp, rút, mua hay ký quỹ được (`423 wallet_frozen`), nhưng vẫn nhận được F-Coin.

#### Scenario: Phí bán app
- **GIVEN** phí app 1.000 phần vạn (10%)
- **WHEN** người dùng mua app giá 49 F-Coin
- **THEN** nhà phát triển nhận 45 F-Coin và tài khoản phí nhận 4 F-Coin

#### Scenario: Ví bị khóa
- **GIVEN** quản trị viên đã khóa ví của Minh
- **WHEN** Minh mua app
- **THEN** trả `423 wallet_frozen` và số dư không đổi
