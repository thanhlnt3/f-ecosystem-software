# Nhật ký thay đổi

## 1.4.0 (14/11/2026)

Thay đổi OpenSpec `web-ui-and-japanese`.

### Tiếng Nhật

- F-Launcher có tiếng Nhật (日本語) trong Cài đặt → Giao diện, cùng tiếng Việt, tiếng Anh, tiếng
  Pháp. Chữ Nhật dùng phông có sẵn của hệ điều hành (Yu Gothic, Meiryo, MS Gothic trên Windows;
  Hiragino trên macOS; Noto CJK hoặc IPA trên Linux), không đóng gói phông. Có thể chỉ định bằng
  `FECO_LAUNCHER_JA_FONT`.
- Trang web có tiếng Nhật: nút VI · EN · 日本語 ở thanh trên cùng, `/lang/ja`, tự nhận trình duyệt
  gửi `Accept-Language: ja`. Toàn bộ chuỗi giao diện web có bản tiếng Nhật; nội dung hệ thống tạo
  chưa có bản tiếng Nhật (thông báo của Kratos, email) hiện bằng tiếng Anh.
- Hồ sơ người dùng nhận ngôn ngữ `ja` (schema danh tính Kratos).

### Giao diện web

- Thiết kế mới: thanh trên cùng (Tài khoản, Cửa hàng, Đơn hàng, Thông báo, Nhà phát triển), chọn
  ngôn ngữ, lối tới trang **Giao diện**; dải màu trang trí theo phong cách.
- Trang **Giao diện** (`/preferences`): ngôn ngữ, phong cách (10 phong cách như F-Launcher), chủ đề
  (theo hệ thống, sáng, tối), màu nhấn, mật độ (thường, gọn), cỡ chữ 90–125%. Lưu trong cookie
  `feco_ui`, áp dụng từ lần vẽ đầu của mọi trang, không cần JavaScript; giá trị hỏng về mặc định.
- Bảng màu dùng chung: `feco_core::ui_palette` là nguồn duy nhất cho F-Launcher và CSS của web
  (`/ui.css`), có kiểm thử độ tương phản WCAG 2.1.
- Đổi ngôn ngữ giờ quay lại đúng trang đang xem.

### Bảng quản trị

- Bố cục mới với thanh bên và trang **Tổng quan**: thẻ số liệu theo vai trò con (yêu cầu quyền
  Lập trình viên và số người dùng cho Quản trị hệ thống; phiên bản chờ duyệt thủ công cho Người
  duyệt app; đơn sắp trễ cho Quản lý dự án; lệnh hoàn chờ duyệt, doanh thu 30 ngày và biểu đồ
  doanh thu 14 ngày cho Tài chính). Bấm thẻ để tới mục tương ứng.
- `FECO_ADMIN_PASSKEY_OPTIONAL=true` bỏ yêu cầu passkey của bảng quản trị, **chỉ khi
  `FECO_ENV=dev`** (WebAuthn không chạy trên địa chỉ 127.0.0.1). Mọi trang quản trị hiện cảnh báo;
  staging và production từ chối khởi động nếu đặt biến này.

## 1.3.0 (14/11/2026)

Thay đổi OpenSpec `platform-roles`, cùng các thay đổi về thanh toán và thông báo.

### Phân quyền người dùng

- Ba vai trò nền tảng:
  - **Người sử dụng ứng dụng** (mặc định);
  - **Lập trình viên**: vào được Cổng nhà phát triển và dùng `f-dev`;
  - **Quản trị viên**: bảng quản trị với 5 vai trò con như trước, và mọi quyền của Lập trình viên.
- Trang **Phân quyền người dùng** (`/admin/members`, Quản trị hệ thống):
  - đổi vai trò theo email, bắt buộc lý do, ghi nhật ký kiểm toán;
  - không tự đổi vai trò của mình, luôn còn ít nhất một Quản trị hệ thống;
  - duyệt hoặc từ chối yêu cầu quyền Lập trình viên.
- Cổng nhà phát triển:
  - người chưa có quyền thấy trang "Cần quyền Lập trình viên" và gửi được yêu cầu;
  - token `f-dev` của người bị thu hồi quyền bị từ chối ngay (`developer_role_required`);
  - thành viên nhà phát triển có từ trước được cấp sẵn quyền.
- F-Launcher hiện vai trò ở thanh bên, có lối tắt "Cổng nhà phát triển" và "Bảng quản trị".
- `feco-catalog role-set`, `role-requests`, `role-decide`.

### F-Launcher

- Tên phong cách đổi sang tên phổ thông: Giấy da, Bích họa, Trời sao, Hồ sen, Lập thể, Hoa huệ,
  Phố cổ, Sơn mài. Cài đặt của bản 1.2 vẫn đọc đúng.
- Demo App không còn trong kho ứng dụng: `feco-tuf remove` gỡ app khỏi danh mục mà không thu hồi,
  `scripts/dev-launcher.sh remove-demo` dọn môi trường dev cũ.

### Thanh toán và thông báo

- Thông báo và email theo ngôn ngữ người dùng:
  - sửa lỗi đọc nhầm trait `language` thay vì `locale`, khiến mọi người dùng chỉ nhận tiếng Việt;
  - thông báo đơn hàng, thanh toán, hoàn tiền và email hóa đơn có bản tiếng Anh;
  - email kiểm duyệt gửi nhà phát triển có đủ phần tiếng Anh.
- Thông báo thanh toán và hoàn tiền giờ cũng hiện trong trung tâm thông báo (web và F-Launcher).
- `feco-catalog payments-check` kiểm khóa ZaloPay, VNPAY, MoMo và tài khoản MISA bằng truy vấn vô hại.
- Lỗi khi cổng thanh toán trả trang HTML (ví dụ 403 do chặn IP) được báo rõ mã HTTP.
- Hướng dẫn đăng ký và cấu hình cổng thanh toán, hóa đơn điện tử, tên miền email:
  `docs/payments-setup.md`.

## 1.2.0 (13/11/2026)

Thay đổi OpenSpec `launcher-personalization`: tùy biến giao diện F-Launcher.

- **Ngôn ngữ**: tiếng Việt (mặc định), tiếng Anh, tiếng Pháp, chọn ở Cài đặt → Giao diện → Ngôn
  ngữ.
  - Đổi ngay, và được nhớ cho lần mở sau.
  - Mọi chữ trên giao diện launcher đều được dịch, kể cả tên nhóm, kiểu sắp xếp và tên phong cách.
  - Nội dung do nhà phát triển viết giữ nguyên ngôn ngữ gốc.
  - Một test tự động báo lỗi khi có chuỗi giao diện chưa dịch.
- **Phong cách**, chọn ở Cài đặt → Giao diện → Phong cách, có ô xem trước:
  - **Chân phương** (mặc định);
  - **Trang nhã**: nút và thẻ bo tròn;
  - tám phong cách lấy cảm hứng từ bảng màu của họa sĩ: Leonardo da Vinci, Michelangelo, Vincent
    van Gogh, Claude Monet, Pablo Picasso, Tô Ngọc Vân, Bùi Xuân Phái, Nguyễn Gia Trí.

  Phong cách họa sĩ có bảng màu, bo góc và dải họa tiết riêng. Chúng chỉ dùng màu và hình khối do
  F-Eco tự vẽ, không dùng hình ảnh tác phẩm. Mọi phong cách đạt độ tương phản WCAG 2.1 và được kiểm
  bằng test tự động.
- Màu cảnh báo và màu lỗi đổi theo nền sáng hoặc tối, nên dễ đọc hơn.
- CI kiểm `Cargo.lock` của cả hai workspace khớp `Cargo.toml` (workflow release dựng bằng
  `--locked`).

## 1.1.0 (13/11/2026)

Thay đổi OpenSpec `improve-launcher-experience`. Gồm cả các bản sửa của 1.0.1.

### F-Launcher

- Bố cục mới:
  - thanh bên với Thư viện, Cửa hàng, Thông báo, Cài đặt và tài khoản;
  - Thư viện và Cửa hàng xem theo lưới hoặc danh sách, nhớ lựa chọn;
  - tìm không dấu, lọc theo nhóm;
  - sắp xếp theo tên, lần mở gần nhất, ngày cài, nhóm hoặc dung lượng.
- Trang Cài đặt:
  - chủ đề sáng, tối hoặc theo hệ thống; màu nhấn; cỡ chữ 80–150%;
  - bố cục mặc định, cỡ ô lưới, trang mở đầu, mô tả ngắn, chế độ gọn;
  - tự động cập nhật, tài khoản, thông tin phiên bản;
  - khôi phục mặc định.

  Cài đặt lưu trong `settings.json` trên máy. File hỏng thì dùng giá trị mặc định.
- Hiện mô tả ngắn, nhóm, nhà phát triển và liên kết mã nguồn từ metadata TUF đã ký. Biểu tượng là
  chữ cái đầu, màu cố định theo app.
- App có cờ `terminal` mở trong cửa sổ terminal riêng: Windows dùng console mới, macOS dùng
  Terminal.
- Trang chi tiết app: bấm vào app trong Thư viện hoặc Cửa hàng để xem biểu tượng, tên, giới thiệu,
  tính năng, liên kết GitHub và website, phiên bản mới nhất và đã cài, ngày phát hành, dung lượng,
  các nền tảng chạy được, có gì mới trong bản mới nhất; nút Cài, Cập nhật, Mở, Gỡ, Quay lại bản
  trước. Nút **‹ Trước** / **Sau ›** và phím mũi tên chuyển sang app kề bên theo thứ tự đang
  hiển thị; Esc quay lại.
- Renderer wgpu và giờ địa phương (như 1.0.1).

### Nhà phát triển

- `f-app.toml` có thêm `source` (địa chỉ mã nguồn, HTTPS), `terminal`, `website`, `features`
  (tối đa 12 dòng) và `release_notes`.
- Máy chủ ký vào target TUF các trường mô tả ngắn, giới thiệu, tính năng, nhóm, nhà phát triển,
  mã nguồn, website, ghi chú phát hành, giấy phép, thời điểm phát hành, terminal. `release_notes`
  cũng là lịch sử thay đổi của phiên bản trên F-Store. App không khai scope thì target không có
  `client_id`.
- `f-dev pack` đóng gói và kiểm tra trước khi nộp. Gói của app có `source` phải kèm giấy phép gốc.
- App mẫu dùng F-SDK: Ghi chú (`examples/apps/f-notes`) và Bấm giờ (`examples/apps/f-timer`).
- `scripts/dev-apps.sh` đưa vào môi trường dev hai app mẫu và các app mã nguồn mở dựng từ GitHub
  (kibi, bottom, eframe template). Cách làm có trong `docs/github-apps.md`.

## 1.0.1 (13/11/2026)

Bản vá cho F-Launcher.

- **F-Launcher hiện cửa sổ trắng trên một số máy Windows.** Trên Windows 10 với card NVIDIA,
  renderer glow (OpenGL) của eframe 0.37 không vẽ được gì. Launcher chuyển sang renderer wgpu
  (DirectX 12 trên Windows, Metal trên macOS, Vulkan trên Linux). Tệp chạy lớn hơn một chút; các
  thư viện mới đều MIT hoặc Apache-2.0 và có trong `THIRD_PARTY_NOTICES` và SBOM của gói.
- Thời điểm đồng bộ và thời điểm của thông báo trong launcher hiện theo giờ của máy, không còn
  hiện giờ UTC.
- F-SDK cho Python và TypeScript có số phiên bản 1.0.1, mã không đổi.

## 1.0.0 (13/11/2026)

Bản 1.0 đầu tiên của F-Eco: nền tảng tài khoản, đăng nhập một lần, F-Store và F-Launcher để
cài và mở các app con, cổng nhà phát triển kèm kiểm duyệt, F-SDK, F-Builder, đơn xây app theo yêu
cầu, thanh toán, bảng quản trị và thông báo.

### Tích hợp app con

- **F-Launcher** (Windows, macOS, Linux): đăng nhập PKCE, đồng bộ danh mục qua TUF, cài và cập
  nhật nguyên tử, quay lại bản trước, gỡ app, chặn bản bị thu hồi, mở app bằng mã khởi chạy
  dùng một lần, API cục bộ để app xin mã mới, app riêng tư, trung tâm thông báo.
- **F-SDK** cho Rust, C (ABI ổn định, phiên bản 1), Python và TypeScript/Node.js: phiên từ
  launcher hoặc đăng nhập trình duyệt, tự làm mới token, `/v1/me`, `/v1/entitlements`, báo lỗi khi
  chạy, kiểm chữ ký webhook. App dùng thư viện OIDC chuẩn cũng tích hợp được.
- **Cổng nhà phát triển** và CLI `f-dev`: manifest `f-app.toml` có trình kiểm tra, nộp gói, kiểm
  tra chữ ký mã, kênh stable và beta, phát hành dần theo phần trăm, số liệu tổng hợp, webhook.
- **Kiểm duyệt**: quét tự động bằng hai engine, duyệt thủ công kèm khác biệt quyền, chính sách
  nội dung, kháng nghị, gỡ khẩn cấp, quét lại 7 ngày.
- Hướng dẫn: [`docs/integration-guide.md`](docs/integration-guide.md).

### Tài khoản và dữ liệu

- Ory Kratos và Hydra: đăng ký, xác minh email, TOTP, passkey, mã khôi phục, thiết bị và đăng
  xuất từ xa, đặt lại mật khẩu, xóa tài khoản sau 30 ngày chờ.
- Đồng ý theo từng scope, định danh pairwise cho từng app, trang "Ứng dụng đã kết nối", thu hồi,
  xuất dữ liệu cá nhân.
- Giới hạn tần suất, nhật ký kiểm toán chỉ ghi thêm, cảnh báo đăng nhập lạ, mã hóa dữ liệu nhạy
  cảm khi lưu.

### Sản phẩm và vận hành

- F-Store: tìm kiếm không dấu, bộ lọc, đánh giá, báo cáo vi phạm.
- F-Builder: tạo app từ mô tả hoặc OpenSpec, bộ giám sát chống treo, clean room, nộp lên F-Store.
- Đơn xây app theo yêu cầu: báo giá theo mốc, bàn giao dưới dạng app riêng tư, nghiệm thu, tranh
  chấp, hủy đơn.
- Thanh toán qua ZaloPay, VNPAY, MoMo (sandbox), hoàn tiền có hai người duyệt, đối soát, hóa đơn
  (bản nháp, hoặc MISA meInvoice khi cấu hình).
- Bảng quản trị 5 vai trò, bắt buộc passkey; thông báo qua email, web và launcher.
- Gói phát hành kèm `THIRD_PARTY_NOTICES` và SBOM CycloneDX cho từng sản phẩm.

### Sửa lỗi trong đợt kiểm tra 1.0

- Mở app từ launcher không còn đòi đồng ý lại mỗi lần khi người dùng đã bỏ qua một scope tùy chọn.
- feco-server gọi được các dịch vụ HTTPS bên ngoài (cổng thanh toán, hóa đơn) khi chạy thật.
- Số hóa đơn trùng được báo lỗi thay vì bị bỏ qua.

### Giới hạn đã biết

Bản 1.0 chưa dùng cho người dùng cuối thật. Còn mở (chi tiết trong
`openspec/changes/add-f-ecosystem-platform/tasks.md` và `QUESTIONS.md`):

- Hạ tầng staging, production và IaC (1.4); máy ảo cô lập để quét và chạy thử app (9.1); môi trường
  sandbox cho nhà phát triển (8.6).
- Đăng nhập Google và GitHub (2.3), một số chính sách phiên (2.4, 2.5).
- Khóa root TUF trong KMS và lễ xoay khóa thật (5.1, 5.3).
- Launcher tự cập nhật và bộ cài đã ký mã (6.6): bản 1.0 phát hành chương trình chưa ký.
- Thanh toán thật: thử callback và hoàn tiền với sandbox thật, khóa VNPAY và MoMo (13.2); tài khoản
  MISA (13.4); tên miền email với SPF, DKIM, DMARC (14.3).
- Rà soát pháp lý: chính sách quyền riêng tư, điều khoản (3.6, 12.6), quy trình sự cố (4.5), lưu trữ
  dữ liệu (15.4); kiểm thử tải (15.2), trang trạng thái (15.3), kiểm thử xâm nhập (15.6); dịch đủ
  tiếng Anh (15.1).
