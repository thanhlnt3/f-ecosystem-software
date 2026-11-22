# Spec Delta

## Purpose

Bổ sung cho capability `launcher`: metadata phân phối được gia hạn tự động, và F-Launcher báo đúng khi metadata bị từ chối.

## ADDED Requirements

### Requirement: Tự gia hạn metadata phân phối
Máy chủ có kho phân phối SHALL ký lại metadata trước khi hết hạn, không cần người vận hành. Metadata `timestamp` SHALL được ký lại khi còn dưới một nửa thời hạn, `snapshot` cũng vậy, và `targets` khi còn dưới một phần ba thời hạn. Metadata đã hết hạn (ví dụ máy chủ tắt nhiều ngày) MUST được ký lại trong vòng 5 phút sau khi máy chủ chạy.

#### Scenario: Timestamp sắp hết hạn
- **GIVEN** metadata `timestamp` còn 20 phút trước khi hết hạn 1 giờ
- **WHEN** việc định kỳ của máy chủ chạy
- **THEN** `timestamp` được ký lại với hạn mới, và F-Launcher đồng bộ thành công

#### Scenario: Máy chủ tắt nhiều ngày
- **GIVEN** máy chủ tắt 3 ngày, `timestamp` đã hết hạn
- **WHEN** máy chủ khởi động lại
- **THEN** metadata được ký lại, F-Launcher đồng bộ thành công mà không báo lỗi

### Requirement: Báo đúng khi metadata bị từ chối
Khi metadata phân phối hết hạn hoặc không hợp lệ, F-Launcher MUST không báo "Đang ngoại tuyến" nếu vẫn kết nối được máy chủ. Thông báo SHALL nói rõ danh mục chưa cập nhật được, các app đã cài vẫn mở được, F-Launcher sẽ tự thử lại, và nhắc kiểm tra giờ của máy tính khi metadata hết hạn. Thông báo MUST không chứa chuỗi lỗi kỹ thuật. Lần đồng bộ thành công tiếp theo SHALL xóa thông báo.

#### Scenario: Metadata hết hạn
- **GIVEN** máy chủ trả metadata `timestamp` đã hết hạn
- **WHEN** F-Launcher đồng bộ
- **THEN** F-Launcher không ở trạng thái ngoại tuyến
- **AND** thông báo "Dữ liệu phân phối của máy chủ đã hết hạn…" không chứa `ExpiredMetadata`

#### Scenario: Máy chủ đã gia hạn
- **GIVEN** F-Launcher đang báo metadata hết hạn
- **WHEN** máy chủ ký lại metadata và F-Launcher đồng bộ lại
- **THEN** thông báo biến mất và danh mục được cập nhật
