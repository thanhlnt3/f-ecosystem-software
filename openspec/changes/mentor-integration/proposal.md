# Proposal

## Why

Chủ dự án muốn đưa Mentor (https://github.com/katD2707/agent-mentor-maximum-experience) vào F-Eco: F-Store, F-Launcher và bản web.

Mentor là ứng dụng học tập cho học sinh từ lớp 1 tới đại học và cho giáo viên. Mentor gồm:
- app React 19 + Vite, đóng gói thành app desktop bằng Tauri 2 (`apps/app`);
- gateway FastAPI (`services/gateway`) giữ tài khoản, khóa học, kế hoạch học, AI;
- các tính năng AI gọi FPT AI Factory qua LiteLLM. Chỉ các tính năng này cần khóa `FPT_API_KEY`.

Chủ dự án xác nhận:
- tác giả đồng ý cho F-Eco tải mã, dựng, sửa phần đăng nhập và dùng logo, có ghi nguồn;
- tích hợp cả **app web** lẫn **app desktop**;
- viết **bản vá SSO** để đăng nhập Mentor bằng tài khoản F-Eco.

Hiện trạng:
- Mentor không có đăng ký. Tài khoản do quản trị tạo bằng CLI, đăng nhập bằng tên tài khoản, mật khẩu và vai trò (học sinh hoặc giáo viên).
- Repo Mentor chưa có tệp LICENSE ở thư mục gốc.

## What Changes

- **Bản vá Mentor** (`integrations/mentor/`). Bản vá không đưa mã Mentor vào F-Eco; F-Eco chỉ giữ bản vá, hướng dẫn và NOTICE.
  - **Gateway**:
    - route `/api/auth/feco/login`, `/callback`, `/exchange`, `/status`;
    - client OIDC phía máy chủ bằng Authlib (BSD-3-Clause), không tự viết OAuth;
    - cột `users.feco_subject` và bảng mã đăng nhập dùng một lần;
    - lệnh `set-role` để quản trị Mentor đổi vai trò;
    - cấu hình `FECO_*`.
  - **App**:
    - nút "Đăng nhập bằng F-Eco" ở màn hình đăng nhập;
    - đổi mã dùng một lần lấy phiên khi F-Eco chuyển về;
    - thông báo lỗi tiếng Việt và tiếng Anh.
- **F-Eco**:
  - `scripts/dev-apps.sh mentor`:
    - client OAuth phía máy chủ (scope `openid profile`, chuyển hướng về gateway Mentor);
    - target TUF `web` có `web_url` dẫn thẳng tới đăng nhập F-Eco của gateway;
    - logo từ repo Mentor, mục F-Store;
    - target `windows-x64` khi có gói desktop.
  - **CI**: job `mentor-desktop` lấy Mentor đúng commit, áp bản vá, chạy pytest và vitest của Mentor, dựng app desktop Windows bằng Tauri (`--no-bundle`, tắt trình tự cập nhật của Mentor). `release.yml` đính kèm `mentor-<phiên bản>-windows-x64.zip`.
  - **Logo** `assets/logos/mentor-128.png`.

## Capabilities

### New Capabilities

- `mentor`: Mentor trong F-Eco (đăng nhập bằng F-Eco, app web, app desktop).

### Modified Capabilities

Không có. F-Store và launcher đã hỗ trợ app web và app có cả gói desktop lẫn bản web (thay đổi `launcher-sidebar-and-web-apps`).

## Impact

- **Thư mục mới**: `integrations/mentor/` (README, NOTICE, bản vá), `assets/logos/mentor*`.
- **Script**: `scripts/dev-apps.sh` (lệnh `mentor`).
- **CI**: `.github/workflows/ci.yml`, `release.yml`.
- **Thư viện F-Eco**: không thêm thư viện nào.
- **Thư viện Mentor**: bản vá thêm `authlib`, `itsdangerous` (BSD-3-Clause) và `joserfc` (BSD-3-Clause, Authlib kéo theo) vào gateway Mentor.
- **Chưa có**:
  - khóa FPT AI: người chạy Mentor tự điền;
  - tệp LICENSE của Mentor;
  - ký số app desktop.

  Các điểm này ghi trong QUESTIONS.md.
