# Quy trình xử lý sự cố lộ dữ liệu cá nhân

Tài liệu này đáp ứng yêu cầu "Xử lý sự cố lộ dữ liệu" trong `openspec/changes/add-f-ecosystem-platform/specs/platform-security/spec.md` (tasks.md 4.5).

> **Lưu ý pháp lý:** thời hạn và mẫu thông báo dưới đây phải được luật sư xác nhận trước khi ra mắt (QUESTIONS.md). Theo hiểu biết hiện tại:
> - Nghị định 13/2023/NĐ-CP yêu cầu thông báo cho cơ quan chuyên trách bảo vệ dữ liệu cá nhân của Bộ Công an (Cục A05) chậm nhất 72 giờ sau khi xảy ra vi phạm.
> - Luật Bảo vệ dữ liệu cá nhân năm 2025 và các văn bản hướng dẫn có thể thay đổi thời hạn hoặc mẫu thông báo.
>
> Luôn áp dụng văn bản đang có hiệu lực.

## 1. Phạm vi

Áp dụng khi có dấu hiệu dữ liệu cá nhân do F-Eco lưu hoặc xử lý bị truy cập, sao chép, sửa, xóa hoặc tiết lộ trái phép. Các dữ liệu này gồm: tài khoản (Kratos), đồng ý và nhật ký truy cập (PostgreSQL), token OAuth (Hydra), file xuất dữ liệu, email, thông tin hóa đơn và đơn hàng (khi có). Phạm vi gồm cả sự cố tại nhà cung cấp mà F-Eco chuyển dữ liệu cho (email, cổng thanh toán, AI).

Ví dụ: lộ bản sao lưu cơ sở dữ liệu; lộ khóa `FECO_DATA_KEY`, `FECO_PAIRWISE_SECRET`, bí mật của Kratos hoặc Hydra; app con bị phát hiện thu thập dữ liệu trái khai báo; nhân viên truy cập hồ sơ không có lý do.

## 2. Vai trò

| Vai trò | Trách nhiệm |
|---|---|
| Chỉ huy sự cố | Điều phối, quyết định mức độ, giữ dòng thời gian, chịu trách nhiệm đúng thời hạn thông báo |
| Kỹ thuật | Điều tra, ngăn chặn, khắc phục, thu thập bằng chứng |
| Pháp lý và bảo vệ dữ liệu | Đánh giá nghĩa vụ thông báo, soạn thông báo cho cơ quan nhà nước |
| Truyền thông và hỗ trợ | Thông báo người dùng, trả lời câu hỏi |

Mỗi vai trò có một người chính và một người dự phòng. Danh sách liên lạc được giữ ngoài kho mã.

## 3. Mức độ

| Mức | Tiêu chí | Thời gian phản hồi |
|---|---|---|
| Nghiêm trọng | Lộ khóa mã hóa hoặc bí mật hệ thống; lộ dữ liệu của nhiều người dùng; dữ liệu nhạy cảm (thông tin hóa đơn, mã số thuế) | Bắt đầu xử lý ngay, 24/7 |
| Cao | Lộ dữ liệu của một số người dùng; app con vi phạm khai báo | Trong 4 giờ |
| Trung bình | Nghi ngờ chưa xác nhận; truy cập trái phép bị chặn kịp | Trong 1 ngày làm việc |

## 4. Quy trình

### Bước 1: Phát hiện và ghi nhận (giờ 0)

- Nguồn: cảnh báo hệ thống, báo cáo của người dùng hoặc nhà phát triển, kết quả quét, nhà cung cấp báo.
- Mở hồ sơ sự cố và ghi **thời điểm phát hiện**. Thời hạn thông báo tính từ đây.
- Chỉ định chỉ huy sự cố.

### Bước 2: Đánh giá phạm vi (trong 24 giờ đầu)

Dùng nhật ký kiểm toán (bảng `audit_log`, chỉ ghi thêm, lưu 1 năm):

```sql
-- Hoạt động của một tài khoản
SELECT at, action, target, ip, details FROM audit_log
WHERE actor_id = '<id danh tính>' ORDER BY at;

-- Đăng nhập sai và thao tác bảo mật trong khoảng nghi vấn
SELECT at, action, actor_id, ip FROM audit_log
WHERE at BETWEEN '<từ>' AND '<đến>'
  AND action IN ('auth.login_failed', 'auth.login', 'account.security_changed', 'security.not_me')
ORDER BY at;

-- App nào đã đọc dữ liệu của một người dùng (90 ngày)
SELECT client_id, scopes, created_at FROM data_access_log
WHERE subject = '<id danh tính>' ORDER BY created_at DESC;
```

Xác định: loại dữ liệu bị ảnh hưởng, số người dùng, khoảng thời gian, nguyên nhân, và việc dữ liệu đã bị lấy ra ngoài hay chưa.

### Bước 3: Ngăn chặn

Các thao tác có sẵn:

| Tình huống | Thao tác |
|---|---|
| Tài khoản bị chiếm | Kratos admin: `DELETE /admin/identities/{id}/sessions` (đăng xuất mọi thiết bị); yêu cầu người dùng đặt lại mật khẩu |
| App con vi phạm | Hydra admin: `DELETE /admin/oauth2/auth/sessions/consent?subject=...&client=...` (thu hồi đồng ý và token của app); vô hiệu client; gỡ khẩn cấp app (nhóm task 9) |
| Lộ `FECO_DATA_KEY` | Tạo khóa mới; mã hóa lại khóa webhook (các nhà phát triển cấp lại khóa) và xóa file xuất cũ; đổi khóa trong kho bí mật |
| Lộ `FECO_PAIRWISE_SECRET` | Đánh giá ảnh hưởng trước khi đổi: đổi khóa làm `sub` của mọi app thay đổi |
| Lộ bí mật Kratos hoặc Hydra (`secrets.cookie`, `secrets.cipher`, `secrets.system`) | Xoay khóa theo hướng dẫn của Ory (thêm khóa mới lên đầu danh sách); đăng xuất mọi phiên nếu cần |
| Lộ khóa cổng thanh toán | Thu hồi khóa tại cổng; đổi khóa; đối soát giao dịch |

Giữ bằng chứng (nhật ký, bản chụp cấu hình) trước khi thay đổi hệ thống.

### Bước 4: Thông báo (theo thời hạn pháp luật, hiện hiểu là 72 giờ)

- **Cơ quan nhà nước:** pháp lý gửi thông báo theo mẫu quy định. Nội dung gồm: mô tả vi phạm, thời gian, loại và số lượng dữ liệu cùng chủ thể, biện pháp đã thực hiện, đầu mối liên hệ. Thông tin chưa đủ thì vẫn gửi đúng hạn và bổ sung sau.
- **Người dùng bị ảnh hưởng:** gửi email (mẫu ở mục 6), và đăng thông báo trên trang trạng thái nếu ảnh hưởng rộng.
- **Nhà phát triển app con:** khi app của họ liên quan.

### Bước 5: Khắc phục và khôi phục

Vá nguyên nhân gốc, kiểm tra không còn truy cập trái phép, khôi phục dịch vụ, theo dõi tăng cường trong ít nhất 30 ngày.

### Bước 6: Rút kinh nghiệm (trong 2 tuần)

Họp xem lại, không tìm người để đổ lỗi. Ghi nguyên nhân gốc, dòng thời gian, việc làm tốt và chưa tốt, và việc cần làm kèm người phụ trách. Cập nhật tài liệu này và spec nếu cần.

## 5. Dòng thời gian mẫu

| Thời điểm | Việc |
|---|---|
| Giờ 0 | Phát hiện, mở hồ sơ, chỉ định chỉ huy |
| Giờ 0 đến 4 | Ngăn chặn ban đầu, giữ bằng chứng |
| Giờ 4 đến 24 | Đánh giá phạm vi, quyết định nghĩa vụ thông báo |
| Trước giờ 72 | Gửi thông báo cho cơ quan nhà nước; thông báo người dùng bị ảnh hưởng |
| Tuần 1 đến 2 | Khắc phục, theo dõi, họp rút kinh nghiệm |

## 6. Mẫu email cho người dùng

> **Tiêu đề:** Thông báo về sự cố bảo mật liên quan tới tài khoản F-Eco của bạn
>
> Ngày [ngày], F-Eco phát hiện [mô tả ngắn, dễ hiểu]. Dữ liệu có thể bị ảnh hưởng: [liệt kê]. Mật khẩu của bạn [được / không được] lưu ở dạng rõ (F-Eco chỉ lưu mã băm Argon2id).
>
> Chúng tôi đã [biện pháp]. Bạn nên: đặt lại mật khẩu, bật xác thực hai lớp, kiểm tra trang "Thiết bị đang đăng nhập" và "Ứng dụng đã kết nối".
>
> Liên hệ: [đầu mối]. Chúng tôi xin lỗi vì sự cố này.

## 7. Diễn tập

Diễn tập ít nhất mỗi năm một lần và trước khi ra mắt công khai (tasks.md 4.5). Kịch bản gợi ý:

1. **Lộ bản sao lưu PostgreSQL** lên kho lưu trữ công khai trong 6 giờ.
2. **App con "SoundLab"** bị phát hiện dùng email người dùng để gửi quảng cáo, trái với mục đích đã khai báo.
3. **Khóa `FECO_DATA_KEY` bị commit nhầm** vào một kho mã công khai.

Biên bản diễn tập ghi: ngày, người tham gia, kịch bản, dòng thời gian thực tế so với mục 5, các bước không làm được hoặc chậm, và việc cần cải thiện. Lưu biên bản cùng hồ sơ bảo mật, không đưa vào kho mã công khai nếu có thông tin nội bộ.
