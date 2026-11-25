# Proposal

## Why

Chủ dự án yêu cầu (25/11/2026) tích hợp bản mới của Mentor từ gói phát hành của tác giả: `mentor-0.2.4-windows-x64.zip` (tag `v0.2.4`).

Mentor 0.2.4 đổi cách đóng gói. `Mentor.exe` tự bật gateway đi kèm (PyInstaller), không cần chạy gateway riêng. Tác giả viết lại giao diện (Làng gà) và gateway, nên hai bản vá "Đăng nhập bằng F-Eco" của F-Eco (viết cho 0.2.2) không áp được nữa. Gói 0.2.4 dùng tài khoản Mentor tạo ngay trên máy.

## What Changes

- **Quét trước khi tích hợp** ở commit `c7ac6f2` của tag: critical 0, high 0, medium 9 (`SCAN.md`).
- **Ghim gói của tác giả**: `integrations/mentor/DESKTOP_PACKAGE` ghi phiên bản, tag, commit, địa chỉ tải và SHA-256.
- **`fetch-desktop.sh`**: quét mã nguồn ở commit của tag, tải gói, so SHA-256, kiểm cấu trúc gói (có `Mentor.exe`, gateway; không có đường dẫn tuyệt đối hay `..`).
- **Bản phát hành F-Eco**: job `mentor` của `release.yml` dùng `fetch-desktop.sh`, đính gói 0.2.4 nguyên vẹn, thay cho bản 0.2.2 tự dựng.
- **Quét định kỳ**: `scan-integrations-all.sh` quét thêm commit của gói desktop (CI và mỗi thứ Hai).
- **Kho dev**: `scripts/dev-apps.sh mentor` nhận `MENTOR_DESKTOP_VERSION` (phiên bản riêng của bản desktop) và `MENTOR_SKIP_WEB=1` (chỉ đăng bản desktop). Mô tả trên F-Store nói đúng hai dạng.
- **Bản web giữ 0.2.2** có vá đăng nhập F-Eco; job CI `mentor-desktop` vẫn kiểm các bản vá đó.

## Capabilities

### Modified Capabilities
- `app-catalog`: gói desktop Mentor 0.2.4 ghim theo mã băm.

## Impact

- App desktop Mentor trên Windows chưa đăng nhập bằng F-Eco. Câu hỏi gửi chủ dự án: `QUESTIONS.md`.
- Gói chưa ký Authenticode: kho dev nhận; kiểm duyệt thật của F-Eco sẽ từ chối cho tới khi tác giả ký.
