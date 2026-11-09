# Câu hỏi còn mở

Ghi lại những điểm spec chưa rõ hoặc cần chủ dự án quyết định (theo AGENTS.md).

## Nhóm 2: tài khoản

1. **Nhà cung cấp thử thách chống bot cho production.** Hiện chỉ có thử thách thử nghiệm
   (ô đánh dấu) cho dev và CI; production bị chặn khởi động cho tới khi chọn nhà cung cấp
   thật. Ứng viên: Cloudflare Turnstile, hCaptcha. Cần chọn và cân nhắc việc gửi dữ liệu
   ra nước ngoài (spec platform-support, "Lưu trữ dữ liệu").
2. **Khôi phục mật khẩu dùng mã, không dùng liên kết.** Spec ghi "liên kết email dùng một
   lần, có hạn 30 phút". Kratos chỉ cho một thời hạn chung cho mọi liên kết (xác minh email
   cần 24 giờ), nên khôi phục dùng **mã 30 phút** gửi qua email. Mức an toàn tương đương.
   Cần xác nhận, hoặc sửa spec cho khớp.
3. **Bộ đếm đăng nhập sai nằm trong bộ nhớ.** Đúng khi chạy một bản feco-server. Khi chạy
   nhiều bản cần chuyển sang Valkey (đã có trong hạ tầng dev).
4. **Đăng nhập Google và GitHub (task 2.3).** Cần chủ dự án tạo ứng dụng OAuth trên Google
   Cloud Console và GitHub (Settings → Developer settings → OAuth Apps), với địa chỉ chuyển
   hướng `http://127.0.0.1:4433/self-service/methods/oidc/callback/google` (và `.../github`)
   cho dev, rồi cung cấp client ID và secret qua biến môi trường. Không đưa secret vào mã nguồn.
5. **Passkey khi phát triển.** WebAuthn không chạy trên địa chỉ IP, nên muốn thử passkey ở máy
   cần chuyển dev sang tên miền `localhost` (Kratos, feco-server, cookie). Có làm ngay không?
6. **Kratos ở staging và production dùng PostgreSQL.** Dev và CI dùng SQLite trong bộ nhớ, vốn
   báo "database table is locked" khi ghi đồng thời (test E2E vì vậy chạy lần lượt).

## Nhóm 3: dữ liệu người dùng và đồng ý

7. **Chính sách quyền riêng tư và điều khoản sử dụng (task 3.6)** cần luật sư soạn và rà soát
   theo pháp luật Việt Nam về bảo vệ dữ liệu cá nhân. Mã nguồn chưa có văn bản này.
8. **Đăng ký webhook và scope của app** hiện được tạo trực tiếp qua API quản trị Hydra và bảng
   `app_webhooks` (dùng trong test). Cổng nhà phát triển (nhóm 8) sẽ ghi các thông tin này từ
   `f-app.toml`.
9. **File xuất dữ liệu lưu trong PostgreSQL** (cột `bytea`). Khi dữ liệu lớn (dự án F-Builder),
   nên chuyển sang kho đối tượng (design D11).

## Nhóm 4: bảo mật

10. **Bộ đếm giới hạn tần suất nằm trong bộ nhớ** (giống bộ đếm đăng nhập sai). Khi chạy nhiều bản
    feco-server cần chuyển sang Valkey. Ngưỡng mặc định: 1200 yêu cầu/phút mỗi IP, 1200 lần/phút
    mỗi app và 120 lần/phút mỗi người dùng ở `/v1/me`. Cần chốt ngưỡng cho production.
11. **Vai trò cơ sở dữ liệu ở production**: feco-server phải kết nối bằng vai trò không được
    `ALTER TABLE`/`DISABLE TRIGGER`, nếu không trigger bảo vệ nhật ký kiểm toán có thể bị tắt.
12. **Thời hạn thông báo sự cố** trong `docs/security/incident-response.md` (hiện ghi 72 giờ theo
    Nghị định 13/2023/NĐ-CP) cần luật sư xác nhận theo văn bản đang có hiệu lực.
13. **TLS**: feco-server chạy sau proxy TLS (ví dụ của nhà cung cấp đám mây). Quét cấu hình TLS 1.2+
    làm khi có staging.

## Nhóm 5: phân phối và chuỗi tin cậy

14. **Người giữ khóa root**: cần chỉ định 3 người (khác bộ phận, không cùng đi công tác) và thiết bị
    giữ khóa (USB HSM hay máy ngoại tuyến mã hóa đĩa) trước buổi ký root thật và buổi diễn tập 5.3.
15. **KMS cho khóa targets, snapshot, timestamp**: phụ thuộc nhà cung cấp hạ tầng (task 1.4).
    `tough` có sẵn nguồn khóa cho AWS KMS; nếu chọn nhà cung cấp trong nước không có KMS tương thích
    thì cần HSM riêng.
16. **Thời hạn timestamp 1 giờ** (để thu hồi tới launcher trong 1 giờ kể cả khi bị đóng băng metadata)
    buộc máy chủ ký lại timestamp liên tục và launcher có đồng hồ đúng. Nếu máy người dùng lệch giờ
    hơn khoảng đó, launcher sẽ báo metadata hết hạn. Cần quyết định có nới lên (ví dụ 4 giờ) và
    chấp nhận cửa sổ đóng băng dài hơn không.
