# AGENTS.md

Nguồn yêu cầu duy nhất của F-EcoSystem-Software là OpenSpec trong `openspec/changes/add-f-ecosystem-platform/`.

- Đọc `proposal.md`, `design.md`, rồi từng `specs/*/spec.md`. Mỗi `### Requirement` là yêu cầu bắt buộc; mỗi `#### Scenario` là một tiêu chí chấp nhận và cần có test tương ứng.
- Làm theo thứ tự nhóm trong `tasks.md`, đánh dấu `- [x]` kèm một dòng ghi cách đã xác minh.
- Không tự viết OAuth/OIDC, mật mã hay xử lý thẻ: dùng Ory Kratos/Hydra, `tough` (TUF), và trang thanh toán của cổng như `design.md` quyết định.
- Chỉ dùng thư viện có license trong `specs/dependency-licensing/spec.md`.
- Không dùng tên, logo hay tài sản của ArtCraft, Craft Anything hay sản phẩm thương mại khác. F-Builder phát triển từ Craft Anything phải giữ `LICENSE` và `NOTICE` gốc.
- Không đưa khóa thật của cổng thanh toán, nhà cung cấp AI hay chứng thư ký mã vào mã nguồn.
- Chỗ nào spec chưa rõ thì ghi câu hỏi vào `QUESTIONS.md`, không tự đoán.
