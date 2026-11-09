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
