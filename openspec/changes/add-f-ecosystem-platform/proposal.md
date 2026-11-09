# Proposal

## Why

Các bộ ứng dụng như ArtCraft (Crafting Apps) cho thấy mô hình một launcher cài đặt, cập nhật và mở nhiều app con là hiệu quả, nhưng danh mục app ở đó do một nhóm kiểm soát. Việc thêm app của bên thứ ba là quyết định riêng của maintainer, và không có tài khoản người dùng hay thanh toán. **F-EcoSystem-Software** (gọi tắt: **F-Eco**) là một nền tảng mở cho bên thứ ba (lập trình viên độc lập, công ty) phát hành app con qua một launcher chung. Nền tảng có tài khoản người dùng dùng chung cho mọi app, công cụ tự xây app (tương tự Craft Anything), và dịch vụ xây app theo yêu cầu có thanh toán.

## What Changes

Tạo mới nền tảng F-Eco gồm:

- **Tài khoản và dữ liệu người dùng**: đăng ký, đăng nhập (email, Google, GitHub), xác thực hai lớp, đăng nhập một lần cho mọi app con. App con chỉ được đọc dữ liệu người dùng khi người dùng đồng ý theo từng phạm vi (scope), có thể thu hồi, xuất và xóa dữ liệu.
- **F-Launcher** (desktop): duyệt danh mục, cài đặt, cập nhật, gỡ, mở app con. Mọi gói cài đặt phải được xác minh chữ ký và mã băm trước khi chạy.
- **Danh mục app (F-Store)**: tìm kiếm, phân loại, trang chi tiết, đánh giá, báo cáo vi phạm.
- **Cổng nhà phát triển**: đăng ký nhà phát triển, ký thỏa thuận, khai báo app bằng manifest, nộp phiên bản, theo dõi kết quả duyệt, phát hành và thu hồi.
- **Kiểm duyệt app**: quét mã độc, kiểm tra manifest và quyền, duyệt thủ công, cơ chế gỡ khẩn cấp (kill switch).
- **F-SDK**: API cho app con: đăng nhập một lần, lấy dữ liệu người dùng theo scope đã đồng ý, kiểm tra quyền sử dụng.
- **F-Builder**: app con có sẵn, giúp người dùng tự xây app từ mô tả hoặc spec, theo quy trình Tìm hiểu → Xây → Kiểm tra → Sẵn sàng, mặc định ở chế độ clean room.
- **Đơn xây app theo yêu cầu**: người dùng gửi yêu cầu, nhận báo giá, thanh toán theo mốc, nhận bàn giao, nghiệm thu hoặc yêu cầu sửa.
- **Thanh toán**: thanh toán qua cổng thanh toán Việt Nam và thẻ quốc tế, đặt cọc, thanh toán theo mốc, hoàn tiền, hóa đơn điện tử. Nền tảng không lưu số thẻ.
- **Quản trị**: duyệt app, quản lý đơn hàng, hoàn tiền, hỗ trợ người dùng, nhật ký kiểm toán.
- **Thông báo**: email và thông báo trong launcher cho đơn hàng, cập nhật app, bảo mật.
- **Yêu cầu chung**: chạy trên Windows và macOS, giao diện tiếng Việt và tiếng Anh, ngưỡng hiệu năng và độ sẵn sàng, chỉ dùng thư viện có license cho phép dùng thương mại.

## Capabilities

### New Capabilities

- `account-identity`: Đăng ký, đăng nhập, xác thực hai lớp, phiên đăng nhập, khôi phục tài khoản, đăng nhập một lần cho app con.
- `user-data-consent`: Hồ sơ người dùng, cho app con truy cập dữ liệu theo scope có đồng ý, thu hồi quyền, xuất và xóa dữ liệu.
- `platform-security`: Mã hóa, giới hạn tần suất, nhật ký kiểm toán, phát hiện đăng nhập bất thường, xử lý sự cố.
- `launcher`: Ứng dụng desktop cài đặt, cập nhật, gỡ, xác minh và mở app con.
- `app-catalog`: Danh mục app, tìm kiếm, trang chi tiết, đánh giá, báo cáo vi phạm.
- `developer-portal`: Tài khoản nhà phát triển, thỏa thuận, manifest, nộp và phát hành phiên bản.
- `app-review`: Quét tự động, duyệt thủ công, chính sách nội dung, gỡ khẩn cấp.
- `app-sdk`: API và SDK cho app con: xác thực, dữ liệu người dùng, quyền sử dụng.
- `app-builder`: F-Builder, công cụ tự xây app từ mô tả hoặc spec.
- `custom-build-orders`: Vòng đời đơn xây app theo yêu cầu: yêu cầu, báo giá, mốc, bàn giao, nghiệm thu.
- `payments`: Thanh toán, đặt cọc, theo mốc, hoàn tiền, hóa đơn, đối soát.
- `admin-console`: Công cụ cho đội vận hành: duyệt, đơn hàng, hoàn tiền, hỗ trợ, kiểm toán.
- `notifications`: Thông báo email và trong launcher, tùy chọn nhận thông báo.
- `platform-support`: Hệ điều hành, ngôn ngữ, hiệu năng và độ sẵn sàng.
- `dependency-licensing`: License của thư viện bên thứ ba và nghĩa vụ ghi công.

### Modified Capabilities

(Không có. Đây là dự án mới.)

## Impact

- **Thành phần mới**: dịch vụ backend (tài khoản, danh mục, đơn hàng, thanh toán), cổng web (người dùng, nhà phát triển, quản trị), ứng dụng F-Launcher, ứng dụng F-Builder, F-SDK (Rust, kèm binding cho ngôn ngữ khác) (xem `design.md`).
- **Bên thứ ba**: nhà cung cấp đăng nhập (Google, GitHub), cổng thanh toán, nhà cung cấp hóa đơn điện tử, dịch vụ email, nhà cung cấp mô hình AI cho F-Builder.
- **Pháp lý**: xử lý dữ liệu cá nhân theo pháp luật Việt Nam về bảo vệ dữ liệu cá nhân; điều khoản sử dụng cho người dùng; thỏa thuận nhà phát triển; điều khoản dịch vụ xây app theo yêu cầu; quy định về hóa đơn điện tử.
- **Ngoài phạm vi phiên bản đầu**: chia doanh thu và bán app trả phí của bên thứ ba; app con chạy trên điện thoại; Linux; ví điện tử nội bộ hay điểm thưởng; tích hợp các launcher hiện có như ArtCraft Launcher.
