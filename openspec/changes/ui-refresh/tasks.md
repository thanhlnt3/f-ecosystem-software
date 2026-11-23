# Tasks

## 1. Gộp PR #1

- [x] 1.1 Kiểm PR #1 không xung đột với `main`, rà nội dung, chạy clippy và unit test; xác minh bằng `git merge-tree` và kết quả test, scenario "Khôi phục mặc định", "Cài đặt cũ thiếu phong cách"
  - Xác minh: PR dựa trên `7dd4d12` (đỉnh `main`), `git merge-tree` không xung đột, gộp kiểu fast-forward. Rà diff 11 tệp: chỉ giao diện, chuỗi dịch, CSS; không thêm tài nguyên ngoài, lệnh hệ thống hay gọi mạng. `cargo fmt --check`, `cargo clippy -p feco-launcher -p feco-server -p feco-core --all-targets -D warnings` sạch; unit test `feco-launcher`, `feco-core`, `feco-server` qua, gồm `prefs::tests::language_and_style_are_saved` (cài đặt cũ thiếu phong cách và khôi phục mặc định → Trang nhã). CI của PR: lượt `push` xanh; job `secrets` của lượt `pull_request` lỗi vì gitleaks-action thiếu quyền đọc PR, không phải phát hiện bí mật.

## 2. Tài liệu và phát hành

- [x] 2.1 README hai thứ tiếng lên 1.20.0, CHANGELOG; phát hành 1.20.0
  - Xác minh: `scripts/check-docs-i18n.py` (đủ 8 cặp); CHANGELOG 1.20.0; CI và release.yml của tag `v1.20.0`.
