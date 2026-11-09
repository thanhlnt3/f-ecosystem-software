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
