# Spec Delta

## Purpose

Bổ sung cho capability `launcher`: tin nhắn trong F-Launcher máy tính.

## ADDED Requirements

### Requirement: Tin nhắn trong F-Launcher
Thanh bên SHALL có mục "Tin nhắn" (icon ở chế độ thu gọn) với huy hiệu số tin chưa đọc, cập nhật ít nhất mỗi 60 giây khi đã đăng nhập. Trang Tin nhắn SHALL có danh sách hội thoại và luồng tin của hội thoại đang chọn. Người dùng SHALL gửi được tin văn bản. Khi đang mở hội thoại, launcher SHALL hỏi tin mới mỗi 5 giây và đánh dấu đã đọc. Tệp đính kèm SHALL mở bằng trình duyệt. Chưa đăng nhập thì trang hiện nút "Đăng nhập".

#### Scenario: Huy hiệu chưa đọc
- **GIVEN** người dùng có 3 tin chưa đọc
- **WHEN** launcher đồng bộ
- **THEN** mục "Tin nhắn" có huy hiệu "3"

#### Scenario: Trả lời trong launcher
- **WHEN** người dùng mở hội thoại, gõ "Em gửi bản sửa nhé" và bấm Gửi
- **THEN** tin hiện ở cuối luồng, và bên kia thấy tin trên web
