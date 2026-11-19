# Spec Delta

## Purpose

Bổ sung cho capability `mobile-launcher`: tab Tin nhắn trên điện thoại.

## ADDED Requirements

### Requirement: Tin nhắn trên điện thoại
App di động SHALL có tab "Tin nhắn" (sáu tab) với huy hiệu số chưa đọc. Tab SHALL có danh sách hội thoại; chạm vào một hội thoại thì mở luồng tin và ô soạn tin. Khi đang mở hội thoại, app SHALL hỏi tin mới mỗi 5 giây và đánh dấu đã đọc. Mọi chuỗi MUST có đủ bốn thứ tiếng.

#### Scenario: Gửi tin từ điện thoại
- **WHEN** người dùng mở hội thoại trong tab Tin nhắn, gõ "Cảm ơn anh" và bấm Gửi
- **THEN** tin hiện ở cuối luồng

#### Scenario: Chưa đăng nhập
- **WHEN** người dùng chưa đăng nhập mở tab Tin nhắn
- **THEN** tab hiện nút "Đăng nhập"
