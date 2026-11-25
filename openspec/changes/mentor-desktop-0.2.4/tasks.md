# Tasks

## 1. Quét và tích hợp

- [x] 1.1 Quét Mentor ở tag `v0.2.4` (`c7ac6f2`), tải gói và kiểm; ghi `SCAN.md`
  - Xác minh: `scripts/scan-integration.sh` (25/11/2026): 2.078 tệp, 681 thư viện, OSV đã tra, critical 0, high 0, medium 9. Gói 160.452.772 byte, SHA-256 `3032e27f…ae369` khớp mã băm GitHub công bố; 948 tệp, không có đường dẫn tuyệt đối hay `..`; Windows Defender không phát hiện mối đe dọa. Hai bản vá F-Eco không áp được (`git apply --check`: giao diện và gateway đã viết lại).
- [x] 1.2 `DESKTOP_PACKAGE`, `fetch-desktop.sh`, job `mentor` của `release.yml`, `scan-integrations-all.sh`; xác minh bằng chạy `fetch-desktop.sh` trên máy dev, scenario "Gói khớp mã băm", "Gói bị thay"
  - Xác minh: `fetch-desktop.sh /c/t/mentor-fetch-test` quét lại, tải, khớp SHA-256, báo "gói Mentor hợp lệ: 948 tệp". Đổi một ký tự SHA-256 trong bản sao `DESKTOP_PACKAGE`: script báo "SHA-256 không khớp", xóa tệp, thoát mã 1.
- [x] 1.3 Kho dev: `MENTOR_DESKTOP_VERSION`, `MENTOR_SKIP_WEB`; đưa Mentor 0.2.4 vào kho dev và cài bằng F-Launcher
  - Xác minh: `dev-apps.sh mentor` ký `mentor-0.2.4-windows-x64.zip`, ghi danh mục; F-Launcher sau khi Đồng bộ hiện Mentor 0.2.4 (153,7 MB), bấm Cập nhật thì tải, kiểm TUF và cài vào `apps\mentor\0.2.4` (`Mentor.exe`, `gateway\`). Chưa mở `Mentor.exe` (người dùng thử trên máy).

## 2. Tài liệu và phát hành

- [x] 2.1 `integrations/mentor/README.md`, README hai thứ tiếng, CHANGELOG, QUESTIONS.md; phát hành cùng 1.21.0
