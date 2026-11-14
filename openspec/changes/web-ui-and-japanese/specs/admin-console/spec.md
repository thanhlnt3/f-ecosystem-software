# Spec Delta

## Purpose

Bổ sung cho capability `admin-console`: trang tổng quan (dashboard) và ngoại lệ passkey ở môi trường dev.

## ADDED Requirements

### Requirement: Dashboard quản trị
Trang chủ bảng quản trị SHALL có thanh bên với các mục theo vai trò con, và vùng tổng quan gồm thẻ số liệu dẫn tới mục tương ứng:
- yêu cầu quyền Lập trình viên chờ duyệt và số người dùng: Quản trị hệ thống;
- phiên bản chờ duyệt thủ công: Người duyệt app;
- đơn sắp trễ: Quản lý dự án;
- lệnh hoàn chờ phê duyệt, doanh thu 30 ngày và biểu đồ doanh thu 14 ngày: Tài chính.

Mỗi thẻ MUST chỉ hiện với người có vai trò con tương ứng.

#### Scenario: Quản trị hệ thống mở bảng quản trị
- **GIVEN** có một yêu cầu quyền Lập trình viên đang chờ
- **WHEN** Quản trị hệ thống mở `/admin`
- **THEN** trang tổng quan có thẻ "Yêu cầu quyền Lập trình viên" ghi 1, bấm vào dẫn tới trang Phân quyền

#### Scenario: Thẻ theo vai trò
- **WHEN** người chỉ có vai trò Hỗ trợ mở `/admin`
- **THEN** trang tổng quan không có thẻ doanh thu hay hoàn tiền

### Requirement: Ngoại lệ passkey ở môi trường dev
Máy chủ SHALL cho phép bỏ yêu cầu passkey của bảng quản trị bằng `FECO_ADMIN_PASSKEY_OPTIONAL=true`, nhưng chỉ khi `FECO_ENV=dev`. Khi bật, mọi trang quản trị MUST hiện cảnh báo rằng passkey không bị bắt buộc. Ở staging và production, đặt biến này MUST làm máy chủ từ chối khởi động.

#### Scenario: Production với cờ bỏ passkey
- **GIVEN** `FECO_ENV=production` và `FECO_ADMIN_PASSKEY_OPTIONAL=true`
- **WHEN** máy chủ khởi động
- **THEN** máy chủ báo lỗi cấu hình và không chạy

#### Scenario: Dev không có passkey
- **GIVEN** `FECO_ENV=dev`, cờ bật, và một quản trị viên đăng nhập bằng mật khẩu
- **WHEN** quản trị viên mở `/admin`
- **THEN** trang tổng quan hiện kèm cảnh báo "Chế độ dev: không bắt buộc passkey"
