# Nhật ký thay đổi

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
