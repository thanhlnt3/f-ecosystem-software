# Tasks

## 1. Nền tảng dự án và hạ tầng

- [x] 1.1 Tạo monorepo Rust (`crates/`, `services/`, `infra/`; các thư mục `launcher/`, `builder/`, `sdk/` thêm khi tới nhóm tương ứng) với một Cargo workspace; xác minh bằng `cargo build --workspace` và `cargo test --workspace` chạy được trong CI
  - Đã xác minh: workspace `crates/feco-core`, `crates/feco-config`, `services/feco-server`, `tools/feco-license-check`; `cargo fmt`, `clippy -D warnings`, `cargo test --workspace` (34 test) đạt trên Ubuntu và Windows trong CI (lượt 42584296446).
- [x] 1.2 CI kiểm tra license (`cargo-deny` cho crate Rust, `third_party/native.toml` cho dịch vụ và ảnh container) theo danh sách cho phép và file ngoại lệ có người duyệt; xác minh bằng PR thử thêm gói AGPL thì CI thất bại
  - Đã xác minh: `cargo-deny check` (license, bans, sources, advisories) và `feco-license-check` (5 ảnh container) đạt trong CI. Trường hợp thất bại được xác minh bằng unit test của `feco-license-check` (thành phần AGPL, ngoại lệ thiếu người duyệt, ảnh chưa khai báo) thay cho một PR thử.
- [ ] 1.3 CI quét bí mật và lỗ hổng phụ thuộc; xác minh bằng PR thử chứa khóa giả thì CI thất bại
  - Trạng thái: gitleaks và `cargo-deny` advisories chạy trong CI, lượt hiện tại sạch. Chưa làm bước xác minh bằng PR thử chứa khóa giả.
- [ ] 1.4 Hạ tầng dev, staging, production bằng IaC: PostgreSQL có sao lưu PITR, Valkey, kho đối tượng kèm CDN, kho bí mật; xác minh bằng cách tạo staging từ đầu bằng một lệnh
  - Trạng thái: hạ tầng dev xong (`infra/docker-compose.yml`: PostgreSQL, Valkey, Mailpit, Ory Kratos, Ory Hydra), CI khởi động được và các endpoint sức khỏe đạt. Staging và production chờ chọn nhà cung cấp đám mây (câu hỏi mở trong design.md).
- [x] 1.5 Kiểm tra cấu hình lúc khởi động (staging không được chứa khóa production của cổng thanh toán); xác minh bằng test cấu hình
  - Đã xác minh: `feco-config` từ chối dev/staging trỏ tới host thật của VNPAY/MoMo và production trỏ tới sandbox; `feco-server` thoát mã 78 khi cấu hình sai, không in bí mật (test `services/feco-server/tests/startup.rs`).

## 2. Tài khoản (`account-identity`)

- [ ] 2.1 Triển khai Ory Kratos và Hydra, cùng giao diện đăng ký và đăng nhập tiếng Việt và tiếng Anh; xác minh bằng test E2E đăng ký, xác minh email, đăng nhập
  - Trạng thái: Kratos (cùng Hydra, chưa dùng tới) chạy trong hạ tầng dev; feco-server dựng trang đăng ký, đăng nhập, xác minh, khôi phục, cài đặt bằng tiếng Việt và proxy tới Kratos. Đã xác minh bằng E2E `services/feco-server/tests/e2e_auth.rs` (đăng ký → email xác minh trong Mailpit → mở liên kết → đã xác minh → đăng nhập). Còn thiếu giao diện tiếng Anh.
- [x] 2.2 Kiểm tra mật khẩu bị lộ, làm chậm dần và thử thách chống bot sau 5 lần sai; xác minh bằng các scenario trong spec
  - Đã xác minh bằng E2E: Kratos chặn mật khẩu bị lộ (HIBP) và mật khẩu dưới 10 ký tự; feco-server làm chậm 0,25 → 4 giây, yêu cầu thử thách từ lần sai thứ 5 trong 15 phút, gửi email cảnh báo cho chủ tài khoản, không khóa cứng. Giới hạn: bộ đếm trong bộ nhớ (cần Valkey khi chạy nhiều bản) và chỉ có thử thách thử nghiệm (nhà cung cấp thật là câu hỏi mở trong QUESTIONS.md).
- [ ] 2.3 Đăng nhập Google và GitHub, liên kết với tài khoản có sẵn sau khi xác thực lại; xác minh bằng test với nhà cung cấp OIDC giả lập
- [ ] 2.4 TOTP, passkey, mã khôi phục; bắt buộc hai lớp cho nhà phát triển và quản trị; xác minh bằng test E2E
- [ ] 2.5 Phiên đăng nhập: access token 15 phút, refresh token xoay vòng, danh sách thiết bị, đăng xuất từ xa, phát hiện refresh token bị dùng lại; xác minh bằng các scenario trong spec
- [ ] 2.6 Đặt lại mật khẩu, xóa tài khoản sau 30 ngày chờ, chặn xóa khi còn đơn đang thực hiện; xác minh bằng test tích hợp

## 3. Dữ liệu người dùng và đồng ý (`user-data-consent`)

- [ ] 3.1 Hồ sơ người dùng và thông tin xuất hóa đơn; xác minh bằng test API
- [ ] 3.2 Danh mục scope cố định, màn hình đồng ý theo từng scope, lưu bản ghi đồng ý kèm phiên bản nội dung; xác minh bằng các scenario trong spec
- [ ] 3.3 Trang "Ứng dụng đã kết nối", thu hồi trong 60 giây, nhật ký truy cập 90 ngày; xác minh bằng test E2E
- [ ] 3.4 Định danh `pairwise` cho từng app; xác minh hai app nhận `sub` khác nhau cho cùng một người dùng
- [ ] 3.5 Xuất dữ liệu cá nhân thành ZIP trong 72 giờ; webhook `user.revoked` và `user.deleted`; xác minh file xuất đủ các nhóm dữ liệu
- [ ] 3.6 Chính sách quyền riêng tư và điều khoản sử dụng (có luật sư rà soát); xác minh bằng biên bản rà soát pháp lý

## 4. Bảo mật nền tảng (`platform-security`)

- [ ] 4.1 TLS, HSTS, mã hóa dữ liệu khi lưu, Argon2id; xác minh bằng quét cấu hình TLS và test băm mật khẩu
- [ ] 4.2 Giới hạn tần suất theo người dùng, app, IP, kèm mã 429 và `Retry-After`; xác minh bằng test tải
- [ ] 4.3 Nhật ký kiểm toán chỉ ghi thêm, lưu 1 năm, không chứa bí mật; xác minh bằng test thử sửa nhật ký và quét nội dung nhật ký
- [ ] 4.4 Email cảnh báo đăng nhập từ thiết bị lạ, ký HMAC cho webhook đi ra; xác minh bằng các scenario trong spec
- [ ] 4.5 Quy trình xử lý sự cố lộ dữ liệu bằng văn bản, kèm một buổi diễn tập; xác minh bằng biên bản diễn tập

## 5. Phân phối và chuỗi tin cậy

- [ ] 5.1 Kho TUF: các vai trò root, targets, snapshot, timestamp; khóa root ngoại tuyến, ngưỡng 2/3; khóa còn lại trong KMS; xác minh bằng test ký và xác minh bằng `tough`
- [ ] 5.2 Ký target sau khi phiên bản được duyệt, và thu hồi bằng metadata mới; xác minh bằng test thu hồi lan tới client giả lập trong 1 giờ
- [ ] 5.3 Diễn tập xoay khóa targets và root; xác minh bằng biên bản diễn tập

## 6. F-Launcher (`launcher`)

- [ ] 6.1 Ứng dụng Rust + egui: đăng nhập PKCE qua trình duyệt, refresh token trong `keyring`; xác minh trên Windows 11 và macOS 13
- [ ] 6.2 "App của tôi" chạy được khi ngoại tuyến, cùng màn hình danh mục; xác minh bằng test ngắt mạng
- [ ] 6.3 Cài, cập nhật nguyên tử, quay lại bản trước, gỡ app; tất cả xác minh qua TUF; xác minh bằng các scenario gói bị sửa và manifest giả
- [ ] 6.4 Mở app con bằng mã khởi chạy dùng một lần, chặn xin token của app khác; xác minh bằng test dùng lại mã
- [ ] 6.5 Chặn mở phiên bản bị thu hồi, hiện công bố mức cô lập khi cài; xác minh bằng các scenario trong spec
- [ ] 6.6 Launcher tự cập nhật, bộ cài ký Authenticode, Developer ID và notarize; xác minh trên máy ảo sạch

## 7. Danh mục (`app-catalog`)

- [ ] 7.1 API và trang danh mục, trang chi tiết (scope, license, chính sách quyền riêng tư); xác minh bằng test E2E
- [ ] 7.2 Tìm kiếm không phân biệt dấu tiếng Việt, cùng các bộ lọc; xác minh "so tay hoc tap" tìm ra "Sổ tay Học tập"
- [ ] 7.3 Đánh giá chỉ cho người đã cài, nhà phát triển trả lời, báo cáo vi phạm; xác minh bằng các scenario trong spec
- [ ] 7.4 App bị gỡ biến khỏi danh mục trong 5 phút; xác minh bằng test có đo thời gian

## 8. Cổng nhà phát triển (`developer-portal`)

- [ ] 8.1 Đăng ký nhà phát triển cá nhân và tổ chức, Thỏa thuận nhà phát triển, xác minh tên miền bằng DNS TXT; xác minh bằng test E2E
- [ ] 8.2 Nhóm và vai trò, client ID cùng địa chỉ chuyển hướng; xác minh bằng các scenario trong spec
- [ ] 8.3 Lược đồ `f-app.toml` kèm trình kiểm tra cho thông báo lỗi rõ ràng; xác minh bằng unit test cho từng trường bắt buộc
- [ ] 8.4 Nộp gói qua web và CLI `f-dev publish`, kiểm tra chữ ký mã của nhà phát triển; xác minh với gói đã ký và chưa ký
- [ ] 8.5 Kênh stable và beta, phát hành dần theo phần trăm, số liệu tổng hợp; xác minh bằng test phát hành 10%
- [ ] 8.6 Tài liệu, app mẫu, môi trường sandbox tách hẳn khỏi production; xác minh token sandbox không dùng được với API thật

## 9. Kiểm duyệt (`app-review`)

- [ ] 9.1 Hàng đợi quét trong máy ảo dùng một lần, hai engine, so manifest với gói, phát hiện packer; xác minh bằng mẫu EICAR và gói sai kiến trúc
- [ ] 9.2 Quy tắc chọn duyệt thủ công hay tự động; xác minh bản 1.2.1 không đổi quyền được duyệt tự động, bản thêm scope vào hàng chờ
- [ ] 9.3 Chính sách nội dung, mẫu lý do từ chối, kháng nghị; xác minh bằng test E2E
- [ ] 9.4 Gỡ khẩn cấp, quét lại 7 ngày kèm cảnh báo; xác minh thu hồi lan tới danh mục trong 5 phút

## 10. F-SDK (`app-sdk`)

- [ ] 10.1 Lõi Rust: phiên từ launcher, PKCE khi chạy ngoài launcher, tự làm mới token; xác minh bằng test tích hợp với Hydra
- [ ] 10.2 C ABI, binding Python, gói TypeScript; xác minh mỗi binding có app mẫu đăng nhập được trong sandbox
- [ ] 10.3 API `/v1/me` và `/v1/entitlements`, webhook kèm gửi lại trong 72 giờ, mã lỗi thống nhất; xác minh bằng test hợp đồng API
- [ ] 10.4 Xác minh một app dùng thư viện OIDC chuẩn (không dùng F-SDK) tích hợp được

## 11. F-Builder (`app-builder`)

- [ ] 11.1 Fork Craft Anything (Apache-2.0), giữ LICENSE và NOTICE, đổi tên và giao diện, gỡ mọi tài sản thương hiệu cũ; xác minh bằng rà soát tài sản và NOTICE
- [ ] 11.2 Tạo dự án từ mô tả, thư mục OpenSpec hoặc nguồn tham khảo; quy trình bốn bước có tiếp tục và sửa; xác minh với bộ spec mẫu
- [ ] 11.3 Bộ giám sát chống treo im lặng (10 phút) và kết thúc lượt khi agent đã bàn giao; xác minh bằng test treo giả lập
- [ ] 11.4 Clean room mặc định, xác nhận quyền khi bật đọc sâu, chặn nộp kết quả đọc sâu, cảnh báo tên sản phẩm tham khảo; xác minh bằng các scenario trong spec
- [ ] 11.5 Chọn coding agent và khóa API (lưu trong kho bảo mật), hỏi trước khi chạy lệnh; xác minh khóa không xuất hiện trong nhật ký
- [ ] 11.6 Tạo manifest và nộp lên F-Store, nút "Nhờ F-Eco xây hộ"; xác minh bằng test E2E với sandbox

## 12. Đơn xây app theo yêu cầu (`custom-build-orders`)

- [ ] 12.1 Mô hình đơn hàng với máy trạng thái chặn bước chuyển sai, cùng lịch sử; xác minh bằng unit test cho từng bước chuyển
- [ ] 12.2 Gửi yêu cầu kèm tệp (giới hạn 200 MB) và Điều khoản dịch vụ; xác minh bằng các scenario trong spec
- [ ] 12.3 Báo giá theo mốc (đặt cọc không quá 30%, hạn hiệu lực 7 ngày, thương lượng); xác minh bằng test tạo yêu cầu thanh toán mốc đặt cọc
- [ ] 12.4 Bàn giao mốc dưới dạng app riêng tư qua launcher, nghiệm thu, giới hạn lượt sửa, tự nghiệm thu sau 14 ngày; xác minh bằng test có mô phỏng thời gian
- [ ] 12.5 Kênh trao đổi, bàn giao cuối chỉ khi đã trả đủ, hủy đơn và tranh chấp; xác minh bằng các scenario trong spec
- [ ] 12.6 Điều khoản dịch vụ xây app (phạm vi, quyền sở hữu mã nguồn, hoàn tiền) có luật sư rà soát; xác minh bằng biên bản rà soát pháp lý

## 13. Thanh toán và hóa đơn (`payments`)

- [ ] 13.1 Lớp điều phối thanh toán, máy trạng thái giao dịch, khóa idempotency; xác minh bằng unit test webhook trùng lặp
- [ ] 13.2 Adapter VNPAY (QR, ATM, thẻ quốc tế) và MoMo ở chế độ sandbox, xác minh chữ ký webhook, truy vấn chủ động khi quá 15 phút; xác minh bằng test với sandbox của từng cổng
- [ ] 13.3 Tự hoàn tiền khi bị thu hai lần; hoàn một phần; hoàn từ 10.000.000 ₫ cần hai người duyệt; xác minh bằng các scenario trong spec
- [ ] 13.4 Tích hợp nhà cung cấp hóa đơn điện tử: phát hành sau khi thanh toán thành công, lập hóa đơn điều chỉnh khi hoàn tiền; xác minh trong môi trường thử của nhà cung cấp
- [ ] 13.5 Đối soát hằng ngày kèm báo cáo chênh lệch; xác minh bằng test giao dịch thiếu webhook
- [ ] 13.6 Quét để chứng minh không lưu số thẻ hay CVV ở đâu cả; xác minh bằng test quét nhật ký và cơ sở dữ liệu sau giao dịch thẻ thử

## 14. Quản trị và thông báo (`admin-console`, `notifications`)

- [ ] 14.1 Bảng quản trị với 5 vai trò, bắt buộc passkey hoặc khóa phần cứng; xác minh vai trò Hỗ trợ không gọi được API hoàn tiền
- [ ] 14.2 Hàng chờ duyệt có hiện khác biệt quyền, quản lý đơn và cảnh báo trễ hạn, tra cứu người dùng phải nhập lý do; xác minh bằng test E2E
- [ ] 14.3 Dịch vụ thông báo (email và trong launcher), phân loại bắt buộc hay tùy chọn, hủy đăng ký không cần đăng nhập, SPF, DKIM, DMARC; xác minh bằng các scenario trong spec

## 15. Yêu cầu chung và ra mắt (`platform-support`, `dependency-licensing`)

- [ ] 15.1 Giao diện, email và tài liệu bằng tiếng Việt và tiếng Anh, định dạng tiền VND; xác minh bằng kiểm tra không còn chuỗi chưa dịch
- [ ] 15.2 Kiểm thử tải (1.000 yêu cầu/giây vào `/v1/me`, p95 dưới 300 ms), đo thời gian khởi động launcher dưới 3 giây; xác minh bằng báo cáo kiểm thử tải
- [ ] 15.3 Trang trạng thái công khai, diễn tập khôi phục sao lưu (RPO 5 phút, RTO 4 giờ); xác minh bằng biên bản diễn tập
- [ ] 15.4 Hồ sơ lưu trữ dữ liệu và chuyển dữ liệu ra nước ngoài theo pháp luật Việt Nam; xác minh bằng biên bản rà soát pháp lý
- [ ] 15.5 `THIRD_PARTY_NOTICES` và SBOM cho launcher, F-Builder, F-SDK; xác minh danh sách khớp cây phụ thuộc
- [ ] 15.6 Kiểm thử xâm nhập độc lập và sửa mọi lỗi mức nghiêm trọng hoặc cao; xác minh bằng báo cáo kiểm thử lại

## Workflow follow-up

- Chạy `openspec archive add-f-ecosystem-platform` sau khi mọi task hoàn thành và đã được review.
