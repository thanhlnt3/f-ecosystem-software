# Design

## C1. Dữ liệu chat (`0019_chat.sql`)

```sql
CREATE TABLE conversations (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  kind TEXT NOT NULL CHECK (kind IN ('direct','project')),
  customer UUID NOT NULL,           -- người bắt đầu (trực tiếp) hoặc khách hàng của dự án
  developer UUID NOT NULL,          -- nhà phát triển (trực tiếp) hoặc bên nhận dự án
  project_id UUID UNIQUE REFERENCES projects(id) ON DELETE CASCADE,  -- chỉ loại project
  app_id TEXT,                      -- app trong F-Store mà khách hàng bấm "Nhắn nhà phát triển" (ngữ cảnh)
  blocked_at TIMESTAMPTZ,           -- nhà phát triển chặn (chỉ loại direct)
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  last_message_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  customer_read BIGINT NOT NULL DEFAULT 0,   -- id tin cuối cùng đã đọc
  developer_read BIGINT NOT NULL DEFAULT 0,
  customer_notified_at TIMESTAMPTZ,          -- lần cuối báo "có tin nhắn mới"
  developer_notified_at TIMESTAMPTZ,
  CHECK (customer <> developer),
  CHECK ((kind = 'project') = (project_id IS NOT NULL))
);
CREATE UNIQUE INDEX conversations_direct ON conversations (customer, developer) WHERE kind = 'direct';
CREATE INDEX conversations_customer ON conversations (customer, last_message_at DESC);
CREATE INDEX conversations_developer ON conversations (developer, last_message_at DESC);

CREATE TABLE chat_messages (
  id BIGSERIAL PRIMARY KEY,
  conversation_id UUID NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
  sender UUID,                                 -- NULL với tin hệ thống
  side TEXT NOT NULL CHECK (side IN ('customer','developer','admin','system')),
  body TEXT NOT NULL CHECK (char_length(body) <= 5000),
  at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX chat_messages_conv ON chat_messages (conversation_id, id);

CREATE TABLE chat_attachments (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  message_id BIGINT NOT NULL REFERENCES chat_messages(id) ON DELETE CASCADE,
  name TEXT NOT NULL, size BIGINT NOT NULL, sha256 TEXT NOT NULL, path TEXT NOT NULL,
  scan_status TEXT NOT NULL DEFAULT 'pending' CHECK (scan_status IN ('pending','clean','infected','error')),
  scan_note TEXT, scanned_at TIMESTAMPTZ
);
CREATE TABLE chat_reports (
  id BIGSERIAL PRIMARY KEY, conversation_id UUID NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
  reporter UUID NOT NULL, reason TEXT NOT NULL, at TIMESTAMPTZ NOT NULL DEFAULT now(), handled_at TIMESTAMPTZ
);
```

- **Chuyển dữ liệu dự án.** Migration tạo hội thoại `project` cho mọi dự án và chép `project_messages` sang `chat_messages`:
  - `side` `customer`, `admin`, `system` giữ nguyên; `provider` thành `developer`;
  - `sender` là khách hàng hoặc bên nhận theo `side`;
  - `customer_read` và `developer_read` đặt bằng tin cuối, để tin cũ không thành "chưa đọc".

  Bảng `project_messages` giữ lại, nhưng không ghi thêm.
- **Ai là "nhà phát triển" trong hội thoại trực tiếp**: người có vai trò `developer` hoặc `dev_company` (`can_take_projects`), hoặc chủ (owner đang hoạt động) của tài khoản nhà phát triển sở hữu app trong F-Store.
- **Ai bắt đầu hội thoại**: người dùng đăng nhập, email đã xác minh. Người bắt đầu là phía `customer`. Không nhắn cho chính mình.

## C2. Quy tắc

| Quy tắc | Giá trị |
|---|---|
| Độ dài tin | 1–5.000 ký tự (sau khi bỏ khoảng trắng hai đầu); tin chỉ có tệp đính kèm thì nội dung là tên tệp |
| Đính kèm | 1 tệp mỗi tin, tối đa 20 MB; tên tệp được làm sạch; lưu ở `package_dir/chat/<conversation>/<attachment id>` |
| Quét đính kèm | dùng các engine mã độc của duyệt app (`scan::Engine`: chữ ký đã biết, clamd nếu cấu hình); `pending` thì chưa tải được; `infected` thì chặn tải, báo quản trị (`admin_alert`, loại `chat_malware`) |
| Hội thoại mới | tối đa 20 mỗi người mỗi 24 giờ (đếm hội thoại `direct` do người đó tạo) |
| Tốc độ gửi | tối đa 30 tin mỗi phút mỗi người |
| Chặn | chỉ phía `developer` của hội thoại `direct`. Khi bị chặn, khách hàng không gửi được (`chat_blocked`); bỏ chặn được |
| Hội thoại dự án | khách hàng và bên nhận gửi được ở mọi trạng thái, trừ `cancelled` và `refunded` sau 30 ngày. Quản trị `project_manager` đọc và gửi với `side = 'admin'` |
| Đã đọc | mở hội thoại (web) hoặc gọi `POST …/read` thì cập nhật `*_read` của phía mình thành tin mới nhất |

Mã lỗi mới (thêm vào `errors::CODES`): `chat_not_found`, `chat_forbidden`, `chat_blocked`, `chat_too_long`, `chat_empty`, `chat_rate_limited`, `chat_attachment_too_large`, `chat_attachment_pending`, `chat_attachment_blocked`, `chat_not_developer`, `chat_self`.

## C3. Thời gian thực

- **Báo giữa các máy chủ.** Sau khi ghi tin, máy chủ gọi:

  ```sql
  SELECT pg_notify('feco_chat', '<conversation id>')
  ```

  Mỗi máy chủ có một tác vụ `PgListener` nghe kênh `feco_chat` và phát lại qua `tokio::sync::broadcast` (`AppState.chat_events`). Mất kết nối thì nối lại sau 2 giây.
- **Web.** `GET /messages/events` là Server-Sent Events (axum `Sse`):
  - mỗi sự kiện `message` mang `{"conversation": "<id>"}`, chỉ gửi cho người tham gia hội thoại;
  - nhịp giữ kết nối mỗi 20 giây;
  - trang hội thoại nhận sự kiện thì gọi `GET /messages/{id}/after/{last_id}` (HTML phần tin mới) và chèn vào cuối;
  - trang danh sách thì tải lại danh sách.

  JavaScript nhỏ, không thư viện ngoài. Không có JavaScript thì form gửi tin vẫn chạy (gửi rồi chuyển hướng).
- **Launcher và di động.** Khi đang mở một hội thoại, app gọi `GET /v1/chat/conversations/{id}/messages?after=<id>` mỗi 5 giây. Khi không mở, số chưa đọc cập nhật theo lần đồng bộ:
  - launcher mỗi 60 giây (`GET /v1/chat/unread`);
  - di động khi kéo làm mới hoặc khi mở app.

## C4. API `/v1/chat` (token của F-Launcher máy tính hoặc di động)

| Phương thức, đường dẫn | Vào | Ra |
|---|---|---|
| `GET /v1/chat/unread` | | `{"unread": 3}`: tổng tin chưa đọc |
| `GET /v1/chat/conversations` | | `{"conversations": [Conversation]}`, mới nhất trước, tối đa 100 |
| `POST /v1/chat/conversations` | `{"developer": "<subject>"}` hoặc `{"app_id": "mentor"}` | `Conversation` (201 khi tạo mới, 200 khi đã có) |
| `GET /v1/chat/conversations/{id}/messages?after=<id>&limit=<n>` | `after` mặc định 0, `limit` mặc định và tối đa 200 | `{"messages": [Message]}`, cũ trước |
| `POST /v1/chat/conversations/{id}/messages` | `{"body": "..."}` (đính kèm chỉ qua web) | `Message` (201) |
| `POST /v1/chat/conversations/{id}/read` | `{"up_to": <id>}` | 204 |

```json
Conversation = {"id": "uuid", "kind": "direct|project", "role": "customer|developer",
  "peer_name": "Tên hiển thị của bên kia", "title": "Tên dự án hoặc tên app hoặc tên bên kia",
  "project_id": "uuid|null", "app_id": "mentor|null", "blocked": false,
  "last_message": "120 ký tự đầu|null", "last_message_at": "RFC 3339", "unread": 2,
  "web_url": "https://…/messages/<id>"}
Message = {"id": 42, "side": "customer|developer|admin|system", "mine": true, "body": "...",
  "at": "RFC 3339", "attachment": {"name": "a.pdf", "size": 1234, "status": "pending|clean|infected|error",
  "url": "https://…/messages/<conv>/files/<att>"} | null}
```

- **Lỗi** dạng `{"error": "<mã>"}`:
  - 401 token sai;
  - 403 `chat_forbidden` (không phải người tham gia);
  - 404 `chat_not_found`;
  - 409 `chat_blocked`;
  - 422 `chat_empty`, `chat_too_long`, `chat_not_developer`, `chat_self`;
  - 429 `chat_rate_limited`.
- **Phạm vi token**: như các API dành cho launcher khác (`launcher_subject`, nhận client máy tính và di động).

## C5. Trang web

- **`GET /messages`**: danh sách hội thoại (tên bên kia, tiêu đề, dòng cuối, số chưa đọc), có lọc "Trực tiếp" và "Dự án".
- **`GET /messages/{id}`**: luồng tin.
  - Form gửi gồm ô chữ và một tệp đính kèm, gửi bằng `POST /messages/{id}`.
  - Phía nhà phát triển có nút "Chặn" và "Bỏ chặn" (`POST /messages/{id}/block`, `/unblock`).
  - Có nút "Báo cáo" (`POST /messages/{id}/report`).
  - Tệp đính kèm tải qua `GET /messages/{id}/files/{att}`.
- **`POST /messages/start`** (`developer` hoặc `app_id`): tạo hoặc mở hội thoại trực tiếp. Nút "Nhắn nhà phát triển" ở trang app F-Store và ở danh sách nhà phát triển trong Dự án.
- **Trang dự án**: luồng tin thay bằng hội thoại dự án, dùng cùng mẫu.
- **Thanh trên cùng**: mục "Tin nhắn" kèm số chưa đọc.
- **Quản trị**: `/admin/chat-reports` (support) xem báo cáo, mở hội thoại bị báo cáo ở chế độ chỉ đọc, đánh dấu đã xử lý.
- **Bốn thứ tiếng**: mọi chuỗi có tiếng Việt, English, Français, 日本語.

## C6. Thông báo

- **Nhóm mới `messages`**, tùy chọn: bật mặc định, tắt được ở cài đặt thông báo.
- **Khi có tin mới**: người nhận được thông báo "Tin nhắn mới từ {tên}" kèm đường dẫn `/messages/{id}`, không có nội dung, nếu thỏa cả ba điều kiện:
  - chưa đọc tin đó;
  - không đang mở hội thoại, tức là không có kết nối SSE đang xem hội thoại này;
  - lần báo trước cho hội thoại này đã quá 15 phút.
- **Email**: theo cài đặt của nhóm `messages`.
- **Thông báo tin nhắn dự án cũ** (`projects::message`) bỏ phần trích nội dung.

## S1. `feco-codescan`

```rust
pub struct ScanOptions { pub osv: Option<OsvClient>, pub max_bytes: u64, pub max_files: usize, pub allow: AllowList }
pub fn scan_dir(root: &Path, opts: &ScanOptions) -> Report;          // đồng bộ: bí mật, mã đáng ngờ, giấy phép, đọc lockfile
pub async fn check_vulnerabilities(report: &mut Report, osv: &OsvClient); // tra OSV theo lô
pub fn scan_zip(zip: &Path, opts: &ScanOptions) -> Result<Report, ScanError>; // giải nén an toàn vào thư mục tạm rồi scan_dir
Report { files_scanned, bytes_scanned, findings: Vec<Finding>, dependencies: Vec<Dependency>,
         licenses: Vec<DetectedLicense>, osv: OsvStatus /* checked | skipped | failed(reason) */, truncated: bool }
Finding { rule: "secret.aws_access_key", category: secret|vulnerability|suspicious|license, severity: critical|high|medium|info,
          path: "src/main.rs", line: 12, message: "…", excerpt: "AKIA****" }
```

- **Giới hạn**:
  - tối đa 200 MB và 50.000 tệp;
  - bỏ tệp nhị phân và tệp lớn hơn 2 MB khi quét nội dung;
  - bỏ các thư mục `.git`, `node_modules`, `target`, `vendor`, `dist`, `build`, `.venv`, `__pycache__`;
  - giải nén zip dùng `enclosed_name`, chặn symlink và bom zip (tổng giải nén tối đa 1 GB).
- **Bí mật**: luật regex có mức `critical`.
  - Tệp kiểm thử (`test`, `tests`, `fixtures`, `examples`) hạ xuống `medium`, vì thường là khóa giả.
  - Chuỗi mẫu rõ ràng (`EXAMPLE`, `xxxx`, `changeme`, `<...>`) không tính.
  - Trích đoạn chỉ giữ 4 ký tự đầu.
- **Mã đáng ngờ**:
  - `critical`: đào tiền ảo (`stratum+tcp://`, `xmrig`); đọc kho mật khẩu hoặc cookie của trình duyệt (`Login Data`, `Cookies` trong thư mục hồ sơ Chrome/Edge/Firefox) kết hợp gửi mạng; tắt Defender (`Set-MpPreference -Disable…`); móc bàn phím (`SetWindowsHookEx` với `WH_KEYBOARD_LL`).
  - `high`: tải rồi chạy ngay (`curl|wget … | sh|bash`, `iex (iwr|irm …)`); khối base64 trên 4 KB rồi giải mã và chạy (`eval`, `exec`, `Function(`); webhook Discord trong mã.
- **Lỗ hổng**: đọc lockfile thành `(hệ sinh thái OSV, tên, phiên bản)`: crates.io, npm, PyPI, Go. Rồi gửi:
  - `POST {FECO_OSV_URL}/v1/querybatch`, lô 500;
  - mỗi mã lỗ hổng gọi `GET /v1/vulns/{id}` (có bộ nhớ đệm) để lấy mức nghiêm trọng (`database_specific.severity` hoặc điểm CVSS).

  Mức của phát hiện:

  | Lỗ hổng | Mức phát hiện |
  |---|---|
  | CRITICAL | `high` |
  | HIGH | `high` |
  | MODERATE, LOW, không rõ | `medium` |

  Lỗ hổng thư viện không tự từ chối app, vì có thể không dùng tới đoạn mã lỗi; người duyệt quyết định.
- **Giấy phép**: nhận dạng SPDX từ tệp LICENSE (MIT, Apache-2.0, BSD-2/3, ISC, MPL-2.0, GPL-2/3, AGPL-3, LGPL, Unlicense) và từ trường `license` của manifest gói. Giấy phép của F-Eco manifest không khớp với mã nguồn thì phát hiện `high` `license.mismatch`.
- **`AllowList`** (`scan-allow.toml`): mỗi mục có `rule`, `path` (glob) và `reason` (bắt buộc), để bỏ phát hiện đã xem xét.
- **Lệnh**:

  ```bash
  feco-codescan <thư-mục|tệp.zip|github:owner/repo@commit> [--allow scan-allow.toml] [--json out.json] [--markdown out.md] [--no-osv] [--fail-on critical|high]
  ```

  Mã thoát 0 khi không vượt ngưỡng, 2 khi vượt.

## S2. Quét mã nguồn trong duyệt app (`0020_source_scan.sql`)

```sql
CREATE TABLE source_submissions (
  version_id UUID PRIMARY KEY REFERENCES app_versions(id) ON DELETE CASCADE,
  origin TEXT NOT NULL CHECK (origin IN ('upload','git')),
  repository TEXT, commit TEXT,            -- origin git: https://github.com/<owner>/<repo>, commit 40 ký tự hex
  storage_path TEXT, size_bytes BIGINT, sha256 TEXT,  -- origin upload
  status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending','scanned','failed')),
  report JSONB, scanned_at TIMESTAMPTZ
);
```

- **Nộp mã nguồn**:
  - Web: trang phiên bản bản nháp có ô tải `source.zip` (tối đa 200 MB), hoặc ô repo và commit.
  - CLI: `PUT /v1/dev/versions/{id}/source` (thân là zip, header `X-Feco-Sha256`).
  - Manifest: `[source] repository = "https://github.com/owner/repo"` và `commit = "<40 hex>"` trong `f-app.toml`; khi tạo phiên bản thì ghi thành `origin = 'git'`.
  - Chỉ nhận repo GitHub công khai. Máy chủ tải `https://codeload.github.com/<owner>/<repo>/zip/<commit>` với giới hạn kích thước như tệp tải lên.
- **Thứ tự trong `run_submit`**:
  1. quét gói (như cũ);
  2. quét mã nguồn, nếu có;
  3. `manual_reason`.

  Kết quả quét mã nguồn:

  | Kết quả | Xử lý |
  |---|---|
  | Có phát hiện `critical` | Từ chối theo điều khoản kỹ thuật, `reject_code = 'source_scan'`; báo nhà phát triển kèm danh sách phát hiện (bí mật đã che) |
  | Có `high`, hoặc OSV `failed` | Duyệt tay, `manual_reason` "Quét mã nguồn: N phát hiện cần xem" |
  | Không kèm mã nguồn | Ghi "Không có mã nguồn" trong `checks.source`; không đổi cách duyệt |
  | Tải repo thất bại (mạng) | Engine lỗi, xếp hàng lại như clamd |

- **Báo cáo** lưu trong `source_submissions.report` và tóm tắt trong `app_versions.checks.source`:

  ```json
  {"status": "...", "counts": {"critical": 0, "high": 1}, "osv": "checked"}
  ```

  Trang phiên bản (nhà phát triển) và trang duyệt (người duyệt) hiện bảng phát hiện.
- **Quét lại định kỳ** (`run_rescan`): không quét lại mã nguồn. Mã nguồn không đổi; lỗ hổng mới được tra lại khi có phiên bản mới.

## S3. Quét trước khi tích hợp app bên ngoài

- `scripts/scan-integration.sh <owner/repo> <commit> [allow.toml]`:
  1. dựng `feco-codescan`;
  2. quét `github:owner/repo@commit`;
  3. ghi `scan-report.json` và `scan-report.md` vào `.dev/scans/<repo>-<commit>/`;
  4. thoát mã 2 nếu có phát hiện `critical` chưa nằm trong `allow.toml`.
- **`integrations/<app>/`**:
  - `SCAN.md`: tóm tắt lần quét đã xem xét (ngày, số phát hiện theo mức, kết luận);
  - `scan-allow.toml`: phát hiện đã xem xét kèm lý do.
- **CI `integration-scan`**: quét Mentor và ThesisAI ở commit đã ghi, đính kèm báo cáo Markdown làm artifact.
- **`build-desktop.sh`**: quét Mentor trước khi dựng, dừng nếu có phát hiện `critical` chưa được cho phép.
