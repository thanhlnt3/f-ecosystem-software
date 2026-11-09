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

- [x] 2.1 Triển khai Ory Kratos và Hydra, cùng giao diện đăng ký và đăng nhập tiếng Việt và tiếng Anh; xác minh bằng test E2E đăng ký, xác minh email, đăng nhập
  - Đã xác minh: giao diện tiếng Việt và tiếng Anh (theo `Accept-Language`, đổi bằng `/lang/vi|en`), 6 test E2E trong `services/feco-server/tests/e2e_auth.rs` với Kratos và Mailpit thật: đăng ký → email xác minh → mở liên kết → đã xác minh → đăng nhập; kiểm tra giao diện tiếng Anh. Hydra đã chạy trong hạ tầng dev, dùng tới ở nhóm 3.
- [x] 2.2 Kiểm tra mật khẩu bị lộ, làm chậm dần và thử thách chống bot sau 5 lần sai; xác minh bằng các scenario trong spec
  - Đã xác minh bằng E2E: Kratos chặn mật khẩu bị lộ (HIBP) và mật khẩu dưới 10 ký tự; feco-server làm chậm 0,25 → 4 giây, yêu cầu thử thách từ lần sai thứ 5 trong 15 phút, gửi email cảnh báo cho chủ tài khoản, không khóa cứng. Giới hạn: bộ đếm trong bộ nhớ (cần Valkey khi chạy nhiều bản) và chỉ có thử thách thử nghiệm (nhà cung cấp thật là câu hỏi mở trong QUESTIONS.md).
- [ ] 2.3 Đăng nhập Google và GitHub, liên kết với tài khoản có sẵn sau khi xác thực lại; xác minh bằng test với nhà cung cấp OIDC giả lập
  - Chưa làm: cần client ID và secret của ứng dụng OAuth Google và GitHub do chủ dự án tạo (QUESTIONS.md). Kratos đã có sẵn cơ chế liên kết tài khoản trùng email sau khi xác thực lại.
- [ ] 2.4 TOTP, passkey, mã khôi phục; bắt buộc hai lớp cho nhà phát triển và quản trị; xác minh bằng test E2E
  - Trạng thái: TOTP, mã khôi phục (10+ mã, dùng một lần) và đăng nhập bắt buộc bước hai (`required_aal: highest_available`) đã xác minh bằng E2E `totp_recovery_codes_and_second_factor_login`. Passkey: nút và script WebAuthn của Kratos đã hiển thị nhưng chưa kiểm thử (cần trình duyệt thật và tên miền `localhost`). Bắt buộc hai lớp cho nhà phát triển và quản trị áp dụng khi có vai trò (nhóm 8, 14).
- [ ] 2.5 Phiên đăng nhập: access token 15 phút, refresh token xoay vòng, danh sách thiết bị, đăng xuất từ xa, phát hiện refresh token bị dùng lại; xác minh bằng các scenario trong spec
  - Trạng thái: danh sách thiết bị (trình duyệt, hệ điều hành, IP, vị trí), đăng xuất từng thiết bị và mọi thiết bị khác đã xác minh bằng E2E `device_list_and_remote_logout`. Access token 15 phút và refresh token xoay vòng 30 ngày đã cấu hình trong Hydra, kiểm chứng khi có luồng OAuth ở nhóm 3. Chưa làm: email cảnh báo khi refresh token bị dùng lại.
- [x] 2.6 Đặt lại mật khẩu, xóa tài khoản sau 30 ngày chờ, chặn xóa khi còn đơn đang thực hiện; xác minh bằng test tích hợp
  - Đã xác minh bằng E2E: khôi phục bằng mã 30 phút qua email, đặt mật khẩu mới thì mọi phiên khác bị đăng xuất (`password_recovery_signs_out_other_sessions`); xóa tài khoản cần phiên xác thực trong 15 phút, chờ 30 ngày, đăng nhập lại thì hủy, sau 30 ngày bị xóa hẳn (`account_deletion_cancel_and_purge`). Chặn xóa khi còn đơn hàng sẽ nối vào nhóm 12 (`account::has_open_orders`).

## 3. Dữ liệu người dùng và đồng ý (`user-data-consent`)

- [x] 3.1 Hồ sơ người dùng và thông tin xuất hóa đơn; xác minh bằng test API
  - Đã xác minh: hồ sơ (tên, ảnh, email, ngôn ngữ, múi giờ, số điện thoại, thông tin xuất hóa đơn gồm tên, mã số thuế, địa chỉ) nằm trong schema danh tính Kratos, sửa được ở trang cài đặt; E2E `consent_scopes_pairwise_and_revocation` đổi tên rồi `GET /v1/me` của app có scope `profile` trả tên mới ở lần gọi tiếp theo.
- [x] 3.2 Danh mục scope cố định, màn hình đồng ý theo từng scope, lưu bản ghi đồng ý kèm phiên bản nội dung; xác minh bằng các scenario trong spec
  - Đã xác minh bằng E2E: danh mục 6 scope cố định (`feco-core::Scope`); feco-server là login/consent provider của Hydra; màn hình đồng ý hiện app, nhà phát triển, từng scope kèm mục đích và chính sách; scope không bắt buộc mặc định không chọn; đồng ý một phần, từ chối (app nhận `access_denied`), xin thêm scope chỉ hỏi scope mới; `consent_records` lưu hành động, scope và phiên bản nội dung (`v1-…`, băm của nội dung đã hiện).
- [x] 3.3 Trang "Ứng dụng đã kết nối", thu hồi trong 60 giây, nhật ký truy cập 90 ngày; xác minh bằng test E2E
  - Đã xác minh bằng E2E: `/apps` liệt kê app đã cấp quyền, scope, lần truy cập gần nhất; `/apps/{client}` hiện nhật ký 90 ngày; thu hồi làm token cũ trả `invalid_token` ngay (dưới 60 giây), app khác không ảnh hưởng, lần mở sau hiện lại màn hình đồng ý. Nhật ký quá 90 ngày được xóa mỗi giờ.
- [x] 3.4 Định danh `pairwise` cho từng app; xác minh hai app nhận `sub` khác nhau cho cùng một người dùng
  - Đã xác minh bằng E2E: `sub` của hai app cho cùng người dùng khác nhau, khớp giữa ID token và `/v1/me`, không phải id danh tính thật (HMAC-SHA256 qua `force_subject_identifier` của Hydra).
- [x] 3.5 Xuất dữ liệu cá nhân thành ZIP trong 72 giờ; webhook `user.revoked` và `user.deleted`; xác minh file xuất đủ các nhóm dữ liệu
  - Đã xác minh bằng E2E `personal_data_export`: ZIP gồm hồ sơ, phiên đăng nhập, ứng dụng đã kết nối, lịch sử đồng ý, nhật ký 90 ngày (và tệp rỗng cho F-Builder, đơn hàng, hóa đơn tới khi có dữ liệu), gửi email liên kết hạn 7 ngày, chỉ chủ tài khoản tải được. Webhook `user.revoked` ký HMAC-SHA256, gửi lại theo backoff trong 72 giờ, đã xác minh chữ ký trong E2E. `user.deleted` được đưa vào hàng đợi khi xóa hẳn tài khoản (chưa có E2E riêng).
- [ ] 3.6 Chính sách quyền riêng tư và điều khoản sử dụng (có luật sư rà soát); xác minh bằng biên bản rà soát pháp lý
  - Chưa làm: cần luật sư soạn và rà soát chính sách quyền riêng tư và điều khoản sử dụng.

## 4. Bảo mật nền tảng (`platform-security`)

- [ ] 4.1 TLS, HSTS, mã hóa dữ liệu khi lưu, Argon2id; xác minh bằng quét cấu hình TLS và test băm mật khẩu
  - Trạng thái: đã xác minh bằng test HSTS (khi địa chỉ công khai là https), chuyển HTTP sang HTTPS ở production (308), header chống nhúng khung và đoán kiểu nội dung; E2E `argon2id_passwords_and_encrypted_webhook_secrets` xác nhận Kratos băm mật khẩu bằng Argon2id; khóa ký webhook và file xuất dữ liệu được mã hóa AES-256-GCM khi lưu (`src/crypto.rs`). Còn lại: quét cấu hình TLS 1.2+ và mã hóa đĩa của PostgreSQL ở staging/production (chờ hạ tầng, task 1.4).
- [x] 4.2 Giới hạn tần suất theo người dùng, app, IP, kèm mã 429 và `Retry-After`; xác minh bằng test tải
  - Đã xác minh: giới hạn theo IP cho mọi yêu cầu (test `ip_rate_limit_returns_429_with_retry_after`), theo app và theo người dùng ở `/v1/me` (E2E `v1_me_rate_limit_per_user`: lần thứ 121 trong một phút nhận 429, `Retry-After`, `error: rate_limited`). Ngưỡng đổi được qua `FECO_RATE_*`. Bộ đếm trong bộ nhớ; cần Valkey khi chạy nhiều bản (QUESTIONS.md).
- [x] 4.3 Nhật ký kiểm toán chỉ ghi thêm, lưu 1 năm, không chứa bí mật; xác minh bằng test thử sửa nhật ký và quét nội dung nhật ký
  - Đã xác minh bằng E2E `audit_log_append_only_and_secret_free`: ghi đăng nhập, đăng nhập sai (chỉ lưu mã băm định danh), đổi hồ sơ và thông tin bảo mật, đồng ý, thu hồi, xóa tài khoản, thiết bị, xuất dữ liệu; UPDATE, DELETE bản ghi chưa quá 1 năm và TRUNCATE bị trigger từ chối; bản ghi quá 1 năm được tác vụ lưu trữ xóa; nội dung không chứa mật khẩu hay token. Chưa có: ghi lại chính lần thử sửa ở mức cơ sở dữ liệu (lỗi làm giao dịch bị hủy); thao tác của quản trị viên sẽ được ghi ở nhóm 14.
- [x] 4.4 Email cảnh báo đăng nhập từ thiết bị lạ, ký HMAC cho webhook đi ra; xác minh bằng các scenario trong spec
  - Đã xác minh bằng E2E `new_device_alert_and_not_me`: lần đăng nhập đầu và thiết bị cũ không gửi email; thiết bị mới gửi email kèm thiết bị, vị trí gần đúng, thời điểm và liên kết "Không phải tôi" (dùng một lần, 7 ngày) đăng xuất mọi thiết bị. Webhook đi ra ký HMAC-SHA256 (đã xác minh ở nhóm 3).
- [ ] 4.5 Quy trình xử lý sự cố lộ dữ liệu bằng văn bản, kèm một buổi diễn tập; xác minh bằng biên bản diễn tập
  - Trạng thái: quy trình bằng văn bản ở `docs/security/incident-response.md` (vai trò, mức độ, 6 bước, truy vấn điều tra, thao tác ngăn chặn, mẫu thông báo, kịch bản diễn tập). Còn lại: luật sư xác nhận thời hạn và mẫu thông báo; tổ chức buổi diễn tập đầu tiên.

## 5. Phân phối và chuỗi tin cậy

- [ ] 5.1 Kho TUF: các vai trò root, targets, snapshot, timestamp; khóa root ngoại tuyến, ngưỡng 2/3; khóa còn lại trong KMS; xác minh bằng test ký và xác minh bằng `tough`
  - Trạng thái: crate `crates/feco-tuf` dựng kho 4 vai trò bằng `tough` (consistent snapshot, Ed25519), root ngưỡng 2/3, targets/snapshot/timestamp ngưỡng 1. Đã xác minh bằng test `tests/chain_of_trust.rs`: dựng kho bị từ chối khi chỉ 1/3 khóa root ký; client `tough` chặn gói bị sửa (sai SHA-256, vượt độ dài), metadata bị sửa, metadata ký bằng khóa lạ, rollback về metadata cũ, metadata bị đóng băng quá hạn timestamp 1 giờ. Quy trình giữ khóa: `docs/security/tuf-keys.md`. Còn lại: đưa khóa targets/snapshot/timestamp vào KMS (chờ chọn nhà cung cấp, task 1.4); bản dev giữ khóa trong file.
- [x] 5.2 Ký target sau khi phiên bản được duyệt, và thu hồi bằng metadata mới; xác minh bằng test thu hồi lan tới client giả lập trong 1 giờ
  - Đã xác minh bằng test `revocation_reaches_client_on_next_hourly_sync`: client đồng bộ mỗi giờ (`SYNC_INTERVAL`) thấy target biến mất và đọc được lý do thu hồi đã ký ở lần đồng bộ kế tiếp, không tải được gói đã thu hồi, không phát hành lại được cùng tên; test `frozen_metadata_expires_within_one_hour` xác nhận timestamp hết hạn sau 1 giờ nên metadata cũ giấu bản thu hồi bị từ chối. `Repo::publish` sẽ được dịch vụ kiểm duyệt (nhóm 9) gọi sau khi duyệt.
- [ ] 5.3 Diễn tập xoay khóa targets và root; xác minh bằng biên bản diễn tập
  - Trạng thái: diễn tập kỹ thuật bằng `scripts/tuf-drill.sh` (CI chạy ở mỗi commit trên Ubuntu và Windows), biên bản `docs/security/drills/2026-11-09-tuf-drill.md`: sau khi xoay, khóa targets cũ không ký được; root mới ký bởi 1/3 khóa root hiện tại bị từ chối, 2/3 được chấp nhận; khóa root bị loại không còn được tính; launcher đang chạy và launcher mới cài (chỉ có `1.root.json`) đều đi theo chuỗi root. Còn lại: buổi diễn tập với người giữ khóa root thật trên máy ngoại tuyến (chờ chỉ định người giữ khóa).

## 6. F-Launcher (`launcher`)

- [ ] 6.1 Ứng dụng Rust + egui: đăng nhập PKCE qua trình duyệt, refresh token trong `keyring`; xác minh trên Windows 11 và macOS 13
  - Trạng thái: `clients/feco-launcher` (Rust + egui 0.37, không đóng gói phông chữ, dùng phông của hệ điều hành). Đăng nhập qua trình duyệt mặc định bằng Authorization Code + PKCE với loopback `127.0.0.1:47201`, không có ô mật khẩu; refresh token lưu trong kho bảo mật của hệ điều hành qua `keyring-core` với kho gốc của Windows (Credential Manager) và macOS (Keychain); không kéo kho Secret Service của Linux vì launcher không phát hành cho Linux; mỗi client một mục, xoay mỗi lần dùng. Đã xác minh: test `tests/auth.rs` (đăng nhập qua "trình duyệt", state sai bị bỏ, khởi động lại vào lại bằng refresh token, ngoại tuyến giữ token, token bị thu hồi thì xóa) và `os_keyring_round_trip` với Windows Credential Manager thật; E2E `launcher_login_and_single_use_launch_codes` (Hydra, Kratos thật: launcher không qua màn hình đồng ý, nhận refresh token, `/v1/me` trả tên); chạy thử bản GUI trên Windows 10: khởi động, đồng bộ TUF, mở API cục bộ. Còn lại: kiểm tra tay giao diện trên Windows 11 và macOS 13 (máy dev là Windows 10, chưa có máy Mac).
- [x] 6.2 "App của tôi" chạy được khi ngoại tuyến, cùng màn hình danh mục; xác minh bằng test ngắt mạng
  - Đã xác minh bằng test `offline_start_keeps_my_apps_usable` (khởi động lại launcher khi máy chủ phân phối không tới được: "App của tôi" vẫn liệt kê và mở được app, tên người dùng lần trước vẫn hiện, lỗi là "Đang ngoại tuyến"), `fake_manifest_is_rejected` (danh mục giữ bản đã xác minh). Giao diện có tab "App của tôi" và "Danh mục"; danh mục báo "Đang ngoại tuyến". Danh mục tạm dựng từ metadata TUF đã xác minh cho tới khi có dịch vụ danh mục (nhóm 7).
- [x] 6.3 Cài, cập nhật nguyên tử, quay lại bản trước, gỡ app; tất cả xác minh qua TUF; xác minh bằng các scenario gói bị sửa và manifest giả
  - Đã xác minh bằng test `tests/launcher.rs` với kho TUF và gói zip thật: `install_verified_package_and_open`, `tampered_package_is_deleted_and_reported` ("Gói cài đặt không hợp lệ", gói bị xóa, ghi sự kiện bảo mật), `fake_manifest_is_rejected` (metadata ký bằng khóa lạ bị từ chối, ghi sự kiện), `package_with_path_traversal_is_rejected`, `interrupted_update_keeps_old_version_and_rollback_needs_no_download` (mất kết nối giữa chừng: bản cũ vẫn mở được, không còn thư mục tạm, lần sau cập nhật thành công; "Quay lại" khi không có mạng), `uninstall_removes_only_that_app` (mặc định giữ dữ liệu cục bộ). Gói được tải thẳng vào file và xóa nếu sai hash (`Client::download_to`). Kiểm tra cập nhật mỗi giờ cùng lúc đồng bộ; tự động cập nhật mỗi 6 giờ nếu bật.
- [x] 6.4 Mở app con bằng mã khởi chạy dùng một lần, chặn xin token của app khác; xác minh bằng test dùng lại mã
  - Đã xác minh bằng E2E `launcher_login_and_single_use_launch_codes`: chỉ token của launcher xin được mã (`/v1/launch-codes`, sống 60 giây, chỉ lưu mã băm); app đổi mã tại `/v1/launch-codes/exchange`, feco-server tự đi luồng ủy quyền của Hydra và chỉ cấp lại scope đã đồng ý (chưa đồng ý: `consent_required`), app đổi code lấy token bằng PKCE verifier của mình, `sub` vẫn là định danh riêng của app; mã dùng lại bị từ chối và ghi `launch_code.reuse` vào nhật ký kiểm toán; mã dùng sai app bị đốt và ghi `launch_code.client_mismatch`; mã quá 60 giây bị từ chối. Phía máy: test `broker_refuses_codes_for_other_apps` (API cục bộ từ chối app A xin mã cho app B, ghi sự kiện) và `launch_command_passes_only_app_specific_values`.
- [x] 6.5 Chặn mở phiên bản bị thu hồi, hiện công bố mức cô lập khi cài; xác minh bằng các scenario trong spec
  - Đã xác minh bằng test `revoked_version_is_blocked_after_next_sync`: sau lần đồng bộ kế tiếp (mỗi giờ khi có mạng, mỗi 5 phút khi đang ngoại tuyến) launcher không cho mở bản bị thu hồi và hiện lý do đã ký; trạng thái thu hồi được lưu nên khởi động lại khi ngoại tuyến vẫn chặn; đề xuất quay lại bản trước, cập nhật hoặc gỡ. Hộp thoại cài và thẻ danh mục có dòng "App này chạy với quyền như mọi ứng dụng khác trên máy bạn" (`ISOLATION_NOTICE`).
- [ ] 6.6 Launcher tự cập nhật, bộ cài ký Authenticode, Developer ID và notarize; xác minh trên máy ảo sạch
  - Trạng thái: chưa làm. Cần chứng thư ký mã Authenticode và tài khoản Apple Developer (QUESTIONS.md, câu 17).

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
