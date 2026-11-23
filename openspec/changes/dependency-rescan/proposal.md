# Proposal

## Why

Rà soát ngày 23/11/2026: quét lại toàn bộ app con trong kho bằng `feco-codescan`. Không app nào có phát hiện `critical`, nhưng có lỗ hổng thư viện **công bố sau khi app đã phát hành**:

- Mentor: `pypdf` 6.21.3 (3 lỗ hổng HIGH) và `soupsieve` 2.10 (2 HIGH);
- eframe-template, Mentor: `url` 2.5.8 (RUSTSEC-2026-0592, công bố 20/11);
- ThesisAI frontend: `zod` 3 (HIGH).

Hệ thống hiện không tự phát hiện được những trường hợp này:

1. **Quét lại 7 ngày một lần chỉ quét gói** (mã độc, chữ ký), không tra lại lỗ hổng thư viện của mã nguồn đã nộp. Lỗ hổng mới công bố sau khi duyệt sẽ không ai biết.
2. **Cảnh báo quản trị** (`admin_alerts`) chỉ được gửi qua email và đọc được bằng `feco-catalog`, không có trong bảng quản trị.
3. **App tích hợp từ bên ngoài** (Mentor, ThesisAI) chỉ được quét khi có commit mới đẩy lên repo F-Eco. Không đẩy gì thì không quét lại.
4. **E2E ghi vào CSDL dev** khi không đặt `FECO_E2E_DATABASE_URL`. Hệ quả là khoảng 270 app thử nằm trên F-Store dev; đã dọn ngày 23/11/2026, có sao lưu.

## What Changes

- **Quét lại định kỳ gồm mã nguồn.** Việc quét lại 7 ngày (`review::run_rescan`) quét lại cả mã nguồn đã nộp: tra lại OSV.dev cho lockfile, cập nhật báo cáo trên trang phiên bản.
- **Báo lỗ hổng mới** khi có phát hiện `high` hoặc `critical` chưa có ở lần quét trước:
  - email cho nhà phát triển (Chủ sở hữu, Quản trị), nêu thư viện, phiên bản, mã lỗ hổng và cách sửa (nâng thư viện rồi nộp bản mới);
  - cảnh báo quản trị `new_vulnerability`, kèm số người đang cài.
  - Không tự thu hồi phiên bản: lỗ hổng thư viện chưa chắc ảnh hưởng tới app, nên người duyệt quyết định.
- **Bảng quản trị**: trang Hàng chờ duyệt có mục "Cảnh báo kiểm duyệt": cảnh báo 30 ngày gần nhất (lỗ hổng mới, mã độc phát hiện muộn), có thanh lọc, sắp xếp, tìm kiếm như các danh sách khác.
- **CI**: workflow `integration-scan` chạy mỗi thứ Hai (và chạy tay được), quét lại Mentor và ThesisAI ở commit đã ghi.
- **E2E**: mặc định dùng CSDL `feco_e2e`, không ghi vào CSDL dev.

## Capabilities

### Modified Capabilities
- `source-scan`: quét lại mã nguồn định kỳ, báo lỗ hổng mới.
- `admin-console`: cảnh báo kiểm duyệt trong bảng quản trị.

## Impact

- `services/feco-server`: `source_scan.rs`, `review.rs`, `admin.rs`, `templates/admin.html`, `i18n*.rs`, E2E.
- `.github/workflows/integration-scan.yml` (mới); job `integration-scan` trong `ci.yml` giữ nguyên.
- Không đổi cơ sở dữ liệu: dùng `source_submissions.report` và `admin_alerts` sẵn có.
