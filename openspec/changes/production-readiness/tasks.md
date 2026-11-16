# Tasks

## 1. Tài khoản quản trị viên mặc định

- [x] 1.1 `AdminBootstrapConfig`, `bootstrap::ensure_admin` (tạo, bật lại, đủ vai trò, chạy lúc khởi động và mỗi 10 phút), bảo vệ trong `set_role` và xóa tài khoản; xác minh bằng unit test cấu hình và E2E scenario "Lần khởi động đầu", "Tài khoản bị tắt", "Production thiếu mật khẩu", "Hạ vai trò tài khoản mặc định"
  - Xác minh:
    - unit test `admin_bootstrap_settings` (feco-config): dev mặc định `admin@f-eco.local` và tự sinh mật khẩu; production thiếu `FECO_ADMIN_EMAIL` hoặc `FECO_ADMIN_PASSWORD_FILE` thì lỗi; email sai định dạng bị từ chối;
    - unit test `password_file_rules`: dev sinh 32 ký tự một lần rồi đọc lại; production không có tệp thì lỗi; mật khẩu dưới 12 ký tự bị từ chối;
    - E2E `default_admin_is_always_on`: tạo tài khoản đã xác minh, đủ năm vai trò, có `admin.bootstrap`; đăng nhập được bằng mật khẩu đã sinh; lượt sau không đổi gì; tài khoản `inactive` được bật lại; hạ vai trò hoặc bỏ Quản trị hệ thống trả `ProtectedAccount`;
    - test khởi động (`tests/startup.rs`) chạy với biến quản trị viên đặt sẵn.

## 2. Chuyển F-Coin

- [x] 2.1 Migration 0018, `wallet::admin_transfer`, yêu cầu cấp F-Coin và duyệt, biểu mẫu ở `/admin/wallet`; xác minh bằng E2E scenario "Chuyển từ ví của quản trị viên", "Ví của quản trị viên không đủ", "Cấp từ quỹ nền tảng", "Cấp lớn cần người thứ hai", "Gửi lại biểu mẫu", "Chuyển cho chính mình"
  - Xác minh:
    - E2E `admin_transfers_f_coin`: chuyển 50 từ ví (gửi hai lần cùng biểu mẫu vẫn chỉ chuyển một lần, email "Bạn nhận được 50 F-Coin" có lý do); chuyển 190 khi còn 150 trả 402 "còn thiếu 40 F-Coin"; tự cấp cho mình trả 400; cấp 100 từ quỹ ghi ngay; cấp 2.000 tạo yêu cầu chờ, người tạo duyệt bị 403, quản trị viên thứ hai duyệt thì người nhận có tiền; nhật ký kiểm toán đủ; `check_ledger` cân;
    - unit test `transfer_checks`.

## 3. Các task còn lại

- [x] 3.1 CI tự kiểm gitleaks (task 1.3 của add-f-ecosystem-platform); đánh dấu task 2.4 với bằng chứng E2E; ghi rõ task còn chờ yếu tố bên ngoài
  - Xác minh: bước "gitleaks phải bắt được khóa giả" trong job `secrets` của CI (kho thử có token GitHub giả sinh lúc chạy, gitleaks phải trả mã có rò rỉ); task 2.4 ghi bằng chứng E2E; task 2.5 và các task chờ hạ tầng, pháp lý, thiết bị ghi trong design.md K3 và QUESTIONS.md 95–99.

## 4. Rà soát bảo mật và chức năng

- [x] 4.1 Ba lượt rà soát (xác thực và web, luồng tiền, launcher và TUF); sửa mọi lỗi được xác nhận, mỗi lỗi có test
  - Xác minh: 35 lỗi được xác nhận (12 xác thực và web, 10 luồng tiền, 13 launcher và TUF) cùng một lỗi chặn triển khai (production không có thử thách chống bot). Đã sửa tất cả, trừ năm điểm cần hạ tầng hoặc quyết định:
    - gọi thẳng API công khai của Kratos (QUESTIONS.md 96);
    - kho TUF phục vụ gói app có giá và app riêng tư cho mọi người (97);
    - lệnh hoàn chưa rõ kết quả cần thao tác xử lý tay, VNPAY mã 94 (98);
    - đăng xuất từ app con chưa kết thúc phiên F-Eco (100);
    - lệnh rút đang chờ giữ tỷ giá lúc tạo khi tỷ giá giảm. Lệnh rút cần duyệt, và mỗi lần đổi tỷ giá chỉ tối đa 20%, nên chưa sửa.

    Test:
    - xác thực và web: `delivery_link_rejects_script_urls`, `spoofed_forwarded_for_does_not_bypass_ip_rate_limit`, `forwarded_for_ignored_unless_peer_is_trusted_proxy`, `safe_local_path_rejects_tab_open_redirect`, `open_work_statuses_match_migrations`, `oauth_login_honors_prompt_login_and_max_age`, `oauth_accept_login_passes_session_acr_amr`, `role_grant_by_email_requires_verified_address`, `csrf_cookie_uses_host_prefix_on_https`, `reviewer_cannot_approve_own_developer_app`, `bearer_rejects_refresh_tokens`, `turnstile_siteverify_response_parsing`, `production_accepts_turnstile_with_keys`, `turnstile_without_keys_is_rejected`, `trusted_proxies_parse_and_default_empty`;
    - luồng tiền: `rate_steps_and_rolling_approvals`, `topups_expire`, cùng E2E `fcoin_wallet_end_to_end`, `software_projects_end_to_end`, `role_chosen_at_registration`, `six_roles_and_role_requests` sau khi sửa;
    - launcher và TUF: `revoking_a_removed_target_still_blocks_it`, `concurrent_mutations_are_serialized`, `icon_with_huge_dimensions_is_rejected`, `colliding_package_paths`, `launch_needs_a_sync_within_seven_days`, `package_with_colliding_entries_is_rejected`, `entry_with_shell_metacharacters_is_rejected`, `huge_png_header_is_rejected`, `web_links_compare_scheme_and_exact_host`, `release_build_needs_server_and_hydra`, `entry_or_terminal_change_needs_manual_review`, `too_many_entries_are_rejected`, `existing_target_must_match_release`, `packages_with_colliding_entries_are_rejected`, `launch_access`.

## 5. Tài liệu và phát hành

- [x] 5.1 Chuỗi giao diện bốn thứ tiếng, README (biến `FECO_ADMIN_*`), CHANGELOG, QUESTIONS.md; phát hành 1.9.0
  - Xác minh: test `every_key_has_japanese`, `every_key_has_french`, `every_ui_string_has_translations`; `scripts/check-docs-i18n.py`; README bước 11.1, 12.5, bảng cấu hình; CHANGELOG 1.9.0; QUESTIONS.md 95–99; CI và release.yml của tag `v1.9.0`.
