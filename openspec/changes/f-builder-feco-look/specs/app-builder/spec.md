## ADDED Requirements

### Requirement: Giao diện F-Builder đồng bộ với F-Launcher
F-Builder SHALL dùng phông giao diện của hệ điều hành theo cùng danh sách với F-Launcher. F-Builder MUST chỉ dùng phông đọc được và có đủ chữ tiếng Việt, và MUST không dừng khi gặp phông hỏng. Bảng màu sáng và tối, bo góc và nút chính của F-Builder SHALL theo F-Launcher (phong cách Trang nhã, màu F-Eco), với chữ đủ tương phản theo WCAG AA. Giao diện mặc định MUST là giao diện sáng, như F-Launcher.

#### Scenario: Chữ tiếng Việt đủ dấu
- **GIVEN** máy có phông giao diện của hệ điều hành (Segoe UI trên Windows)
- **WHEN** người dùng chọn ngôn ngữ Tiếng Việt trong F-Builder
- **THEN** mọi chữ tiếng Việt có dấu và mũi tên hiện bằng cùng một phông, không có ô vuông

#### Scenario: Phông chỉ định bị hỏng
- **GIVEN** `F_BUILDER_UI_FONT` trỏ tới một tệp không phải phông
- **WHEN** người dùng mở F-Builder
- **THEN** F-Builder bỏ qua tệp đó, dùng phông kế tiếp trong danh sách và vẫn mở bình thường

#### Scenario: Giống F-Launcher
- **WHEN** người dùng mở F-Builder lần đầu
- **THEN** F-Builder dùng giao diện sáng, cùng màu nền, thanh bên, thẻ và nút chính màu F-Eco như F-Launcher
