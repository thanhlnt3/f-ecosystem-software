# Design

## Context

Dự án mới (động lực: xem `proposal.md`, mục Why). Có ba ràng buộc chi phối thiết kế:

- **Hệ sinh thái mở cho bên thứ ba**: khác với ArtCraft Launcher (danh mục cố định trong mã, chỉ maintainer thêm được app cộng đồng), F-Eco phải tiếp nhận app từ nhiều nhà phát triển không quen biết. Vì vậy chuỗi tin cậy (ký, xác minh, thu hồi) và kiểm duyệt là phần cốt lõi, không phải phần phụ.
- **Dữ liệu cá nhân và tiền**: tài khoản dùng chung, chia sẻ dữ liệu cho bên thứ ba và thanh toán khiến nền tảng chịu nghĩa vụ theo pháp luật Việt Nam về bảo vệ dữ liệu cá nhân, thanh toán và hóa đơn điện tử.
- **Đội nhỏ**: ưu tiên dùng lại thành phần mã nguồn mở đã trưởng thành, có license cho phép (`specs/dependency-licensing`), thay vì tự viết phần nhạy cảm như OAuth/OIDC.

## Goals / Non-Goals

**Goals:**
- Một nhà cung cấp danh tính duy nhất, chuẩn OIDC, dùng cho web, launcher, F-Builder và mọi app con.
- Chuỗi tin cậy cho gói cài đặt, chống được giả mạo, quay về bản cũ có lỗ hổng (rollback) và đóng băng cập nhật (freeze).
- Luồng thanh toán không bao giờ chạm vào dữ liệu thẻ, idempotent, và đối soát được.
- F-Builder chạy cục bộ trên máy người dùng, dùng coding agent và tài khoản AI của chính họ.

**Non-Goals:**
- Sandbox cấp hệ điều hành cho app con ở phiên bản đầu (xem Risks).
- Bán app trả phí của bên thứ ba và chia doanh thu.
- Tự xây cổng thanh toán hay tự lưu thẻ.

## Decisions

### D1. Kiến trúc tổng thể
- **Backend**: các dịch vụ Rust (`axum`, MIT) dùng chung PostgreSQL (PostgreSQL License) và Redis (dùng bản fork Valkey, BSD-3-Clause, để tránh license SSPL của các bản Redis mới). Các dịch vụ gồm: `identity` (bọc Ory), `consent`, `catalog`, `dev-portal`, `review`, `orders`, `payments`, `notify`, `admin`.
- **Giao tiếp**: REST + JSON có phiên bản (`/v1`). Sự kiện nội bộ đi qua bảng outbox trong PostgreSQL, để webhook và email không bị mất khi một dịch vụ khởi động lại.
- **Web**: cũng viết bằng Rust (quyết định của chủ dự án, 2026-11-09). Trang được dựng phía máy chủ bằng `axum` + template `askama` (MIT/Apache-2.0), kèm một ít JavaScript thuần cho tương tác nhỏ. Cách này không cần build step riêng cho frontend. Nếu sau này cần giao diện tương tác nhiều, có thể chuyển sang `leptos` (MIT).
- **Lý do chọn Rust cho toàn bộ hệ thống**: một ngôn ngữ, một bộ công cụ CI; dùng chung kiểu dữ liệu (manifest, scope, tiền VND) giữa backend, web, launcher, F-Builder và F-SDK; cùng hệ sinh thái với Crafting Apps.
- **Phương án đã cân nhắc**: TypeScript + React cho web (hệ sinh thái giao diện lớn hơn, nhưng thêm một ngôn ngữ và chuỗi build); Node.js cho backend (phải giữ hai bộ kiểu dữ liệu).
- **Ngoại lệ**: Ory Kratos và Hydra viết bằng Go, chạy dưới dạng dịch vụ (container) và không được nhúng vào mã Rust.

### D2. Danh tính: Ory Kratos + Ory Hydra
- **Chọn**: Ory Kratos (Apache-2.0) lo đăng ký, đăng nhập, xác thực hai lớp (TOTP, WebAuthn/passkey), khôi phục, đăng nhập Google/GitHub. Ory Hydra (Apache-2.0) là máy chủ OAuth 2.0/OIDC đã được chứng nhận OpenID, hỗ trợ PKCE, refresh token xoay vòng, định danh `pairwise`, và luồng đồng ý do ứng dụng của mình tự vẽ.
- **Lý do**: phần dễ sai nhất về bảo mật nằm trong mã đã được kiểm chứng. Màn hình đồng ý theo scope (`specs/user-data-consent`) do dịch vụ `consent` tự làm qua consent API của Hydra.
- **Phương án đã cân nhắc**: Keycloak (Apache-2.0, đầy đủ nhưng nặng, khó tùy biến giao diện tiếng Việt); Zitadel (đổi sang AGPL từ v3, không hợp `dependency-licensing`); tự viết (rủi ro cao).

### D3. Chuỗi tin cậy cho gói cài đặt: mô hình TUF
- **Chọn**: metadata phân phối theo mô hình The Update Framework (TUF), dùng crate `tough` (MIT/Apache-2.0) ở launcher.
  - Các vai trò `root`, `targets`, `snapshot`, `timestamp`; khóa `root` để ngoại tuyến, các khóa còn lại nằm trong KMS hoặc HSM.
  - Mỗi phiên bản app là một target kèm SHA-256 và độ dài, được ký sau khi qua kiểm duyệt.
  - Gói của nhà phát triển vẫn giữ chữ ký Authenticode hoặc Developer ID riêng của họ.
- **Lý do**: chặn được giả mạo, rollback và freeze, những loại tấn công mà "manifest + SHA-256" đơn thuần không chặn được. Thu hồi khẩn cấp chỉ cần phát hành metadata mới.
- **Phương án đã cân nhắc**: tự ký manifest bằng Ed25519, giống `SHA256SUMS.txt` của ArtCraft Launcher (đơn giản, nhưng không chống được rollback hay freeze).

### D4. F-Launcher
- **Chọn**: Rust + egui (MIT/Apache-2.0); đăng nhập qua trình duyệt hệ thống bằng PKCE với địa chỉ loopback; refresh token lưu bằng crate `keyring`.
- **Mở app con**: launcher khởi chạy app kèm biến môi trường `FECO_LAUNCH_CODE` (mã dùng một lần, sống 60 giây). App đổi mã lấy token tại máy chủ, theo một grant tùy biến gắn với client ID.
- **Cập nhật**: tải vào thư mục tạm, xác minh TUF, rồi đổi tên nguyên tử; giữ một bản trước để quay lại.

### D5. Cô lập app con
- **Phiên bản đầu**: app con chạy như ứng dụng desktop thông thường dưới quyền người dùng. Spec không hứa hẹn cô lập ở cấp hệ điều hành (`specs/launcher`, "Công bố mức cô lập"). An toàn dựa vào kiểm duyệt, ký mã, quét định kỳ và thu hồi.
- **Lộ trình**: bắt buộc macOS App Sandbox cho app mới và thử MSIX/AppContainer trên Windows; app đã sandbox được gắn nhãn "Đã cô lập" trong danh mục.

### D6. Kiểm duyệt tự động
- **Chọn**: hàng đợi job quét chạy trong máy ảo dùng một lần.
  - Hai engine quét: một dịch vụ quét thương mại có hợp đồng cho phép dùng thương mại, và ClamAV chạy thành dịch vụ riêng. ClamAV là GPL nhưng không được phân phối cho người dùng; nó vẫn phải được ghi vào ngoại lệ đã duyệt, vì `dependency-licensing` kiểm cả ảnh container của backend.
  - Kiểm tra chữ ký mã, so manifest với gói, phát hiện packer.
  - Quét lại định kỳ 7 ngày.
- **Duyệt thủ công**: bảng quản trị hiện khác biệt về scope và quyền so với phiên bản trước, kèm máy ảo cô lập để chạy thử.

### D7. F-SDK
- **Chọn**: lõi Rust, xuất C ABI qua `cbindgen` (MPL-2.0, chỉ là công cụ dựng, không đi kèm sản phẩm); binding Python bằng PyO3 (MIT/Apache-2.0); bản TypeScript viết thuần trên chuẩn OIDC (không dùng lõi Rust), để chạy được ở cả Node.js và trình duyệt. License MIT OR Apache-2.0.

### D8. F-Builder: phát triển tiếp từ Craft Anything
- **Chọn**: phát triển F-Builder từ mã nguồn của `storytold/craft-anything` (Apache-2.0).
  - Giữ `LICENSE` và `NOTICE`, ghi rõ nguồn gốc, đổi tên và giao diện.
  - Không dùng tên, logo hay tài sản của ArtCraft hay Craft Anything (`specs/app-review`, chính sách thương hiệu).
  - Thêm: đăng nhập F-Eco qua F-SDK, tạo manifest và nộp lên F-Store, nút "Nhờ F-Eco xây hộ", và bộ giám sát chống treo im lặng.
- **Bài học từ lần dùng Craft Anything với dự án VPdf**:
  - CLI hai lần bị kẹt sau khi coding agent đã bàn giao xong, nên spec `app-builder` có yêu cầu "Không bị treo im lặng".
  - Kết quả cần được ghi vào chính thư mục dự án, không ghi vào một bản sao.
- **Phương án đã cân nhắc**: viết mới từ đầu (mất nhiều tháng cho phần điều phối agent mà Craft Anything đã có).

### D9. Thanh toán
- **Chọn**: một lớp điều phối thanh toán có adapter cho từng cổng; phiên bản đầu tích hợp **VNPAY** (VietQR, thẻ ATM nội địa, thẻ quốc tế) và **MoMo** (ví điện tử).
  - Trạng thái giao dịch là một máy trạng thái lưu trong PostgreSQL. Mọi lệnh tạo giao dịch có khóa idempotency (mã mốc, lần thử).
  - Webhook được xác minh chữ ký theo tài liệu của từng cổng, rồi đưa vào outbox. Có thêm job truy vấn chủ động các giao dịch "Chờ thanh toán" quá 15 phút.
  - Đối soát hằng ngày bằng file hoặc API báo cáo của cổng.
- **Lý do**: thanh toán qua trang hoặc thành phần nhúng của cổng giúp F-Eco không bao giờ chạm vào dữ liệu thẻ, nên phạm vi PCI DSS thu nhỏ còn mức tự đánh giá đơn giản nhất.
- **Phương án đã cân nhắc**: Stripe (theo hiểu biết của mình, chưa nhận doanh nghiệp đăng ký tại Việt Nam; cần kiểm tra lại); OnePay, ZaloPay (thêm sau qua adapter).

### D10. Hóa đơn điện tử
- **Chọn**: tích hợp API của một nhà cung cấp hóa đơn điện tử được cấp phép tại Việt Nam (ví dụ VNPT Invoice, Viettel S-Invoice, MISA meInvoice). Hóa đơn được phát hành sau khi giao dịch Thành công; hoàn tiền thì lập hóa đơn điều chỉnh. Nhà cung cấp cụ thể là câu hỏi mở.

### D11. Hạ tầng và dữ liệu
- **Chọn**: chạy trên nhà cung cấp đám mây có trung tâm dữ liệu tại Việt Nam, để đáp ứng yêu cầu lưu trữ dữ liệu (`specs/platform-support`).
  - PostgreSQL có sao lưu liên tục (point-in-time recovery 14 ngày).
  - Kho đối tượng kèm CDN cho gói cài đặt.
  - Theo dõi hệ thống bằng OpenTelemetry (Apache-2.0).
  - Ba môi trường: dev, staging, production; chỉ production có khóa thật của cổng thanh toán.

## Risks / Trade-offs

- [App con chưa được sandbox nên có thể đọc dữ liệu người dùng trên máy] → Kiểm duyệt nhiều lớp, ký mã bắt buộc, quét lại 7 ngày, thu hồi qua TUF; công bố rõ mức cô lập; lộ trình sandbox (D5).
- [Nhà phát triển độc hại vượt qua kiểm duyệt rồi đổi hành vi qua máy chủ của họ] → Duyệt thủ công khi thêm quyền; báo cáo vi phạm; thỏa thuận nhà phát triển có chế tài; theo dõi tên miền mà app gọi tới.
- [Thất lạc khóa `root` của TUF] → Khóa ngoại tuyến chia nhiều phần (ngưỡng 2/3), cất ở hai nơi; quy trình xoay khóa được diễn tập.
- [Tranh chấp về chất lượng đơn xây app] → Báo giá ghi rõ phạm vi và tiêu chí nghiệm thu; bàn giao mốc kèm kết quả kiểm tra theo spec; mọi trao đổi lưu trong đơn.
- [Chi phí AI của F-Builder] → Phiên bản đầu dùng agent và tài khoản AI của người dùng; nền tảng không gánh chi phí suy luận.
- [Thay đổi pháp luật về dữ liệu cá nhân, thanh toán, hóa đơn] → Có luật sư rà soát điều khoản trước khi ra mắt; các mục pháp lý trong spec được viết theo dạng "theo quy định hiện hành", và có checklist cập nhật mỗi quý.
- [Phụ thuộc vào Ory] → Hai sản phẩm đều Apache-2.0 và tự triển khai được; dữ liệu nằm trong PostgreSQL của mình.

## Migration Plan

Không áp dụng, vì đây là dự án mới. Kế hoạch ra mắt:
1. **Alpha nội bộ**: tài khoản, launcher, danh mục với app của F-Eco (F-Builder, và VPdf làm app mẫu).
2. **Beta kín cho nhà phát triển**: cổng nhà phát triển, kiểm duyệt, F-SDK, sandbox.
3. **Beta công khai**: đơn xây app theo yêu cầu cùng thanh toán thật, giới hạn số đơn.
4. **Ra mắt**: sau kiểm thử xâm nhập độc lập và khi các điều khoản pháp lý đã được duyệt.

## Open Questions

- Nhà cung cấp hóa đơn điện tử và cổng thanh toán cụ thể (cần so sánh phí và hợp đồng).
- Nhà cung cấp đám mây tại Việt Nam.
- Quy định mặc định về quyền sở hữu mã nguồn của đơn xây app: chuyển giao toàn bộ hay cấp license. Câu trả lời đi vào Điều khoản dịch vụ, không làm đổi spec.
- Kiểm tra nhãn hiệu cho tên "F-EcoSystem-Software" và các tên F-Launcher, F-Store, F-Builder trước khi công bố.
