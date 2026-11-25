## ADDED Requirements

### Requirement: Gói desktop bên ngoài ghim theo mã băm
Khi F-Eco dùng nguyên gói desktop do tác giả một app bên ngoài dựng, F-Eco SHALL ghim phiên bản, commit mã nguồn và SHA-256 của gói. Trước khi đưa gói vào kho hay bản phát hành, F-Eco MUST quét mã nguồn ở đúng commit đó, MUST so SHA-256 của tệp tải về với giá trị đã ghim, và MUST từ chối gói có đường dẫn tuyệt đối hoặc `..`.

#### Scenario: Gói khớp mã băm
- **GIVEN** `DESKTOP_PACKAGE` của Mentor ghim gói 0.2.4 và SHA-256 của nó
- **WHEN** job phát hành chạy `fetch-desktop.sh`
- **THEN** mã nguồn ở commit của tag được quét, gói được tải và kiểm, rồi đính nguyên vẹn vào bản phát hành

#### Scenario: Gói bị thay
- **GIVEN** tệp tải về có SHA-256 khác giá trị đã ghim
- **WHEN** `fetch-desktop.sh` chạy
- **THEN** script xóa tệp, báo mã băm không khớp và thoát lỗi; job phát hành dừng
