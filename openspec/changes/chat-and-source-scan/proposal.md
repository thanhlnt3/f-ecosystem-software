# Proposal

## Why

Chủ dự án muốn hai tính năng mới:

1. **Chat giữa khách hàng và nhà phát triển.**
2. **Quét mã trước khi tích hợp.**

Hiện trạng:
- **Tin nhắn**: chỉ có luồng tin nhắn trong trang dự án (`project_messages`). Luồng này:
  - phải gửi rồi tải lại trang;
  - không có API JSON, không đánh dấu đã đọc, không đính kèm tệp;
  - không dùng được trong F-Launcher.

  Khách hàng không nhắn được cho nhà phát triển trước khi có dự án. Thông báo tin nhắn dự án còn chép 300 ký tự nội dung vào thông báo và email, trái quy tắc "Không lộ thông tin nhạy cảm" của `notifications`.
- **Quét gói app**: duyệt app đã quét gói (mã độc, chữ ký số, packer), nhưng chưa xem mã nguồn. Bí mật lộ trong mã, thư viện có lỗ hổng đã công bố, mã đáng ngờ (đào tiền ảo, lấy mật khẩu trình duyệt) đều lọt qua nếu gói sạch.
- **Tích hợp app bên ngoài** (ThesisAI, Mentor) chưa có bước quét repo trước khi viết bản vá và dựng.

Chủ dự án chọn:
- chat **trực tiếp và trong dự án**, trên **web, F-Launcher máy tính và F-Launcher di động**;
- quét **cả hai**: mã nguồn app nhà phát triển nộp, và repo bên ngoài trước khi tích hợp;
- công cụ quét là **luật viết bằng Rust**, tra lỗ hổng thư viện qua **API công khai OSV.dev**. Chỉ gửi tên và phiên bản thư viện; không tải phần mềm quét mới.

## What Changes

### Chat

- **Hội thoại** giữa một khách hàng và một nhà phát triển, có hai loại:
  - **trực tiếp**: khách hàng bấm "Nhắn nhà phát triển" ở trang app trong F-Store hoặc ở hồ sơ nhà phát triển trong Dự án;
  - **dự án**: mỗi dự án có một hội thoại. Tin nhắn cũ trong `project_messages` được chuyển sang; bên quản lý dự án vẫn xem và nhắn được khi giải quyết tranh chấp.
- **Tin nhắn**:
  - văn bản tối đa 5.000 ký tự;
  - đính kèm tối đa 20 MB, được quét mã độc trước khi tải được;
  - đánh dấu đã đọc; đếm số chưa đọc.
- **Cập nhật gần như tức thời**:
  - web dùng Server-Sent Events;
  - máy chủ báo nhau qua PostgreSQL `LISTEN/NOTIFY`, chạy được nhiều máy chủ;
  - launcher và app di động hỏi máy chủ 5 giây một lần khi đang mở hội thoại.
- **Chống làm phiền**:
  - nhà phát triển chặn được hội thoại trực tiếp;
  - mỗi người tạo tối đa 20 hội thoại mới mỗi ngày, gửi tối đa 30 tin mỗi phút;
  - báo cáo hội thoại tới quản trị.
- **Thông báo**: nhóm mới "Tin nhắn" (tắt được). Thông báo và email chỉ ghi người gửi và đường dẫn, không chép nội dung; mỗi hội thoại báo tối đa một lần mỗi 15 phút khi còn tin chưa đọc. Thông báo tin nhắn dự án cũng bỏ nội dung.
- **Ở đâu**:
  - web: trang `/messages`, ô chat trong trang dự án;
  - F-Launcher máy tính: mục "Tin nhắn" ở thanh bên có huy hiệu số chưa đọc;
  - F-Launcher di động: tab "Tin nhắn".
- **API** `/v1/chat/*` cho launcher và app di động.

### Quét mã

- **Thư viện và lệnh mới `feco-codescan`** (`crates/feco-codescan`, MIT OR Apache-2.0) quét một thư mục, tệp zip hoặc repo GitHub tại một commit:
  - **bí mật lộ**: khóa riêng PEM, khóa AWS, token GitHub/GitLab/Slack/Stripe, webhook Discord, tệp `.env` có giá trị, gán `password`/`secret`/`api_key` bằng chuỗi dài;
  - **thư viện có lỗ hổng**: đọc `Cargo.lock`, `package-lock.json`, `pnpm-lock.yaml`, `yarn.lock`, `requirements*.txt`, `poetry.lock`, `uv.lock`, `go.sum`; tra OSV.dev theo lô;
  - **mã đáng ngờ**: đào tiền ảo, đọc kho mật khẩu hoặc cookie của trình duyệt, móc bàn phím, tắt Windows Defender, tải rồi chạy ngay (`curl … | sh`, `iex (iwr …)`), khối base64 rất dài được giải mã rồi chạy;
  - **giấy phép**: giấy phép khai trong mã nguồn (LICENSE, `Cargo.toml`, `package.json`, `pyproject.toml`) so với giấy phép khai trong manifest.

  Mỗi phát hiện có mức `critical`, `high`, `medium` hoặc `info`, kèm tệp và dòng. Bí mật chỉ hiện vài ký tự đầu.
- **Duyệt app**:
  - phiên bản mới có thể kèm mã nguồn, dạng tệp zip hoặc repo + commit trong `f-app.toml` (`[source]`);
  - duyệt tự động quét mã nguồn sau khi gói sạch:
    - có phát hiện `critical` thì từ chối;
    - có `high`, lỗ hổng mức cao hoặc nghiêm trọng, giấy phép không khớp, hay OSV không trả lời thì chuyển duyệt tay;
    - không kèm mã nguồn thì ghi "Không có mã nguồn" trong báo cáo;
  - nhà phát triển và người duyệt xem báo cáo ở trang phiên bản;
  - `f-dev publish --source <thư-mục>` đóng gói và tải mã nguồn lên.
- **Tích hợp app bên ngoài**: `scripts/scan-integration.sh <owner/repo> <commit>` ghi báo cáo JSON và Markdown.
  - `integrations/mentor/build-desktop.sh` quét Mentor trước khi dựng.
  - Job CI `integration-scan` quét Mentor và ThesisAI ở commit đã ghi. Có phát hiện `critical` mới thì CI thất bại; phát hiện đã xem xét ghi trong `integrations/<app>/scan-allow.toml` kèm lý do.

## Capabilities

### New Capabilities

- `messaging`: chat giữa khách hàng và nhà phát triển.
- `source-scan`: quét mã nguồn trước khi duyệt và trước khi tích hợp.

### Modified Capabilities

- `app-review`: thêm bước quét mã nguồn.
- `notifications`: nhóm "Tin nhắn", không chép nội dung tin nhắn.
- `launcher`: mục "Tin nhắn".
- `mobile-launcher`: tab "Tin nhắn".

## Impact

- **Máy chủ**:
  - migration `0019_chat.sql`, `0020_source_scan.sql`;
  - module `chat.rs`, `chat_web.rs`, `chat_api.rs`, `source_scan.rs`;
  - `projects.rs` dùng hội thoại;
  - `review.rs` gọi quét mã nguồn;
  - `developer.rs`, `dev_web.rs` nhận mã nguồn;
  - `notifications.rs` thêm nhóm `messages`.
- **Crate mới** `crates/feco-codescan` (thư viện và lệnh `feco-codescan`). Thư viện thêm, đều có giấy phép được phép:
  - `regex` (MIT/Apache-2.0);
  - `serde_yaml_ng` hoặc trình đọc riêng cho `pnpm-lock.yaml`.

  Không dùng thư viện GPL.
- **F-Launcher máy tính**: trang "Tin nhắn"; `auth.rs` gọi `/v1/chat/*`.
- **F-Launcher di động**: tab "Tin nhắn" (sáu tab).
- **f-dev**: `--source`.
- **Script và CI**: `scripts/scan-integration.sh`, job `integration-scan`, `integrations/*/scan-allow.toml`.
- **Dịch vụ ngoài**: OSV.dev (`FECO_OSV_URL`, tắt bằng `FECO_OSV=off`). Chỉ gửi hệ sinh thái, tên và phiên bản thư viện.
