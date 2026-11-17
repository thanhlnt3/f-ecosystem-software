# mobile-launcher

## Purpose

F-Launcher trên điện thoại (Android, iOS): ứng dụng đồng hành với tài khoản F-Eco, F-Store, app web, ví F-Coin, "Ứng dụng của tôi" và thông báo.

## ADDED Requirements

### Requirement: Nền tảng hỗ trợ
F-Launcher di động SHALL chạy trên Android 10 (API 29) trở lên và iOS 18 trở lên, nhắm Android 16 (API 36). Gói và bundle id SHALL là `vn.feco.launcher`, biểu tượng app là logo F-Launcher.

#### Scenario: Android thấp nhất và mới nhất
- **WHEN** CI cài APK lên Android emulator API 29 và API 36
- **THEN** app mở được và hiện năm tab trên cả hai

#### Scenario: iOS
- **WHEN** CI dựng app với mục tiêu iOS 18 và chạy trên iOS Simulator
- **THEN** app mở được và hiện năm tab

### Requirement: Đăng nhập bằng tài khoản F-Eco trên điện thoại
App SHALL đăng nhập bằng OIDC (authorization code + PKCE) qua trình duyệt hệ thống bằng thư viện AppAuth, và MUST không có ô nhập mật khẩu trong app. Refresh token MUST lưu trong Keychain (iOS) hoặc Keystore (Android); access token chỉ giữ trong bộ nhớ. API trả 401 thì app SHALL làm mới token một lần rồi gọi lại; làm mới thất bại thì app SHALL về trạng thái chưa đăng nhập.

#### Scenario: Đăng nhập lần đầu
- **WHEN** người dùng bấm "Đăng nhập" và đăng nhập F-Eco trong trình duyệt
- **THEN** app hiện tên người dùng và số dư F-Coin, và không hiện màn hình đồng ý chia sẻ dữ liệu

#### Scenario: Token hết hạn
- **GIVEN** access token đã hết hạn
- **WHEN** app tải ví
- **THEN** app làm mới token bằng refresh token rồi hiện số dư mà người dùng không phải đăng nhập lại

### Requirement: Năm tab
App SHALL có năm tab: Ứng dụng của tôi, Cửa hàng, Ví, Thông báo, Tài khoản. Chưa đăng nhập thì Cửa hàng vẫn xem được; các tab khác hiện nút "Đăng nhập".

#### Scenario: Chưa đăng nhập
- **WHEN** người dùng mở app lần đầu
- **THEN** tab Cửa hàng hiện danh mục, tab Ví hiện nút "Đăng nhập"

### Requirement: Cửa hàng trên điện thoại
Tab Cửa hàng SHALL hiện danh mục F-Store: logo, tên, nhà phát triển, mô tả ngắn, giá F-Coin. Người dùng SHALL tìm được theo tên và lọc được theo nhóm.
- App web SHALL có nút "Thêm" (ghi vào "Ứng dụng của tôi") và "Mở" (mở `web_url` bằng trình duyệt). `web_url` MUST là HTTPS, hoặc `localhost`, `127.0.0.1`, `10.0.2.2` trong môi trường dev.
- App desktop SHALL ghi "Dùng trên máy tính qua F-Launcher" và không có nút cài.

#### Scenario: Mở app web
- **WHEN** người dùng bấm "Mở" ở ThesisAI
- **THEN** trình duyệt mở `web_url` của ThesisAI

#### Scenario: App desktop
- **WHEN** người dùng xem kibi trong Cửa hàng
- **THEN** ô kibi ghi "Dùng trên máy tính qua F-Launcher" và không có nút cài

### Requirement: Ví và mua app trên điện thoại
Tab Ví SHALL hiện số dư F-Coin và tỷ giá, kèm nút mở trang nạp và rút trên web. App có giá SHALL có nút "Mua · N F-Coin" với hộp thoại xác nhận ghi số dư sau khi mua. Không đủ F-Coin thì hộp thoại SHALL ghi số còn thiếu kèm nút "Nạp F-Coin" mở trang ví trên web.

#### Scenario: Không đủ F-Coin
- **GIVEN** số dư 10 F-Coin
- **WHEN** người dùng bấm "Mua · 49 F-Coin"
- **THEN** hộp thoại ghi "Còn thiếu 39 F-Coin" và có nút "Nạp F-Coin"

### Requirement: Ứng dụng của tôi và thông báo trên điện thoại
Tab Ứng dụng của tôi SHALL hiện app đã thêm hoặc đã mua của tài khoản (dữ liệu từ máy chủ, chung với web và desktop), nhãn "Có bản mới" khi có, và nút "Mở" với app web. Tab Thông báo SHALL hiện thông báo của tài khoản, mới nhất trước.

#### Scenario: Thêm app web
- **WHEN** người dùng bấm "Thêm" ở ThesisAI
- **THEN** ThesisAI có trong tab Ứng dụng của tôi và trong trang "Ứng dụng của tôi" trên web

### Requirement: Ngôn ngữ trên điện thoại
App SHALL có tiếng Việt, English, Français, 日本語. Ngôn ngữ mặc định theo ngôn ngữ của máy; người dùng đổi được ở tab Tài khoản. Mọi chuỗi giao diện MUST có đủ bốn thứ tiếng.

#### Scenario: Đổi ngôn ngữ
- **WHEN** người dùng chọn "日本語" ở tab Tài khoản
- **THEN** các tab hiện bằng tiếng Nhật ngay
