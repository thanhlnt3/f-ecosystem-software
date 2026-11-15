# Hồ sơ dữ liệu cá nhân và chuyển dữ liệu ra nước ngoài (bản nháp)

> **Bản nháp kỹ thuật, chưa phải ý kiến pháp lý.** Tài liệu liệt kê dữ liệu cá nhân F-Eco xử lý, nơi lưu và các luồng có thể ra nước ngoài, làm đầu vào cho luật sư lập hồ sơ theo pháp luật Việt Nam về bảo vệ dữ liệu cá nhân và an ninh mạng (tasks.md 15.4; spec platform-support "Lưu trữ dữ liệu"). Các văn bản dưới đây phải được luật sư xác nhận còn hiệu lực và áp dụng đúng, trước khi ra mắt (QUESTIONS.md).

## 1. Văn bản cần đối chiếu

- Luật Bảo vệ dữ liệu cá nhân và các nghị định hướng dẫn: đánh giá tác động xử lý dữ liệu cá nhân, đánh giá tác động chuyển dữ liệu ra nước ngoài, thông báo vi phạm.
- Nghị định 13/2023/NĐ-CP về bảo vệ dữ liệu cá nhân: còn áp dụng tới đâu khi Luật đã có hiệu lực.
- Luật An ninh mạng 2018 và Nghị định 53/2022/NĐ-CP: yêu cầu lưu trữ dữ liệu tại Việt Nam với doanh nghiệp trong nước.

## 2. Dữ liệu cá nhân F-Eco xử lý

| Nhóm | Dữ liệu | Nơi lưu | Mục đích | Thời gian giữ |
|---|---|---|---|---|
| Tài khoản | email, tên, ngôn ngữ, múi giờ, số điện thoại (tùy chọn), mật khẩu (băm), TOTP, passkey | Kratos (Postgres) | đăng nhập, liên lạc | tới khi xóa tài khoản + 30 ngày chờ |
| Phiên và thiết bị | địa chỉ IP, user agent, vị trí gần đúng | Kratos, `audit_log` | bảo mật, cảnh báo thiết bị mới | nhật ký truy cập 90 ngày |
| Đồng ý cho app | app, scope, thời điểm đồng ý | Hydra (Postgres) | đăng nhập một lần, API `/v1/me` | tới khi thu hồi |
| Hóa đơn | tên, mã số thuế, địa chỉ xuất hóa đơn | Postgres (mã hóa AES-256-GCM) | hóa đơn điện tử | theo luật kế toán |
| Đơn hàng | mô tả yêu cầu, tệp đính kèm, tin nhắn | Postgres, thư mục tệp | đơn xây app theo yêu cầu | theo hợp đồng |
| Thanh toán | mã giao dịch, số tiền (không lưu thẻ) | Postgres | đối soát, hoàn tiền | theo luật kế toán |
| Nhà phát triển | tên cá nhân/tổ chức, tên miền, thành viên | Postgres | cổng nhà phát triển | tới khi rời nền tảng |
| Thông báo | tiêu đề, nội dung, tùy chọn nhận | Postgres | trung tâm thông báo, email | 1 năm (đề xuất) |
| Kiểm duyệt, quản trị | nhật ký kiểm toán, lý do xem hồ sơ | `audit_log` | trách nhiệm giải trình | đề xuất 2 năm |

## 3. Bên thứ ba và luồng có thể ra nước ngoài

| Bên | Dữ liệu gửi đi | Ở đâu | Ghi chú |
|---|---|---|---|
| Nhà cung cấp email (SMTP) | email, nội dung thông báo | **chưa chọn** | nếu máy chủ ở nước ngoài: cần hồ sơ chuyển dữ liệu và nêu trong chính sách quyền riêng tư |
| ZaloPay, VNPAY, MoMo | mã đơn, số tiền | Việt Nam | không gửi dữ liệu thẻ (trang thanh toán của cổng) |
| MISA meInvoice | tên, mã số thuế, địa chỉ hóa đơn | Việt Nam | |
| Nhà cung cấp AI cho F-Builder | mô tả yêu cầu của người dùng | **chưa chọn** (QUESTIONS.md) | có thể ở nước ngoài |
| App con (nhà phát triển) | dữ liệu theo scope người dùng đồng ý, `sub` riêng cho từng app | theo nhà phát triển | người dùng đồng ý từng scope; thu hồi được |
| Hạ tầng máy chủ | toàn bộ cơ sở dữ liệu, bản sao lưu | **chưa chọn** | đề xuất trung tâm dữ liệu tại Việt Nam |

## 4. Việc cần làm trước khi ra mắt

1. Luật sư xác nhận văn bản áp dụng và nghĩa vụ của F-Eco: bên kiểm soát dữ liệu hay bên xử lý, với từng luồng.
2. Chọn hạ tầng máy chủ và vị trí lưu bản sao lưu, ưu tiên trong nước.
3. Chọn nhà cung cấp email và AI; nếu ở nước ngoài, lập hồ sơ đánh giá tác động chuyển dữ liệu và nộp theo quy định.
4. Lập hồ sơ đánh giá tác động xử lý dữ liệu cá nhân.
5. Cập nhật chính sách quyền riêng tư: nêu các luồng ở mục 3.
6. Quy trình thông báo vi phạm dữ liệu: nối với `docs/security/incident-response.md`.
