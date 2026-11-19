# Spec Delta

## Purpose

Bổ sung cho capability `notifications`: thông báo tin nhắn chat.

## ADDED Requirements

### Requirement: Thông báo tin nhắn
Nhóm thông báo "Tin nhắn" SHALL bật mặc định và tắt được. Thông báo và email tin nhắn MUST chỉ ghi tên người gửi và đường dẫn tới hội thoại, không chép nội dung tin. Mỗi hội thoại SHALL báo tối đa một lần mỗi 15 phút cho mỗi người nhận, và MUST không báo khi người nhận đang mở hội thoại đó trên web.

#### Scenario: Không lộ nội dung
- **WHEN** khách hàng gửi "Mật khẩu máy chủ là abc123" trong hội thoại dự án
- **THEN** thông báo và email gửi bên nhận chỉ ghi "Tin nhắn mới từ {tên khách hàng}" kèm đường dẫn, không có "abc123"

#### Scenario: Gộp thông báo
- **GIVEN** nhà phát triển chưa mở hội thoại
- **WHEN** khách hàng gửi 5 tin trong 10 phút
- **THEN** nhà phát triển nhận đúng một thông báo
