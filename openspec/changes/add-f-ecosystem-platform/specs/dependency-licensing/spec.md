# Spec Delta

## Purpose

Bảo đảm mọi thành phần bên thứ ba mà F-Eco phân phối (launcher, F-Builder, F-SDK, cổng web) có license cho phép dùng thương mại, được ghi công đầy đủ, và tách bạch với license của app con của bên thứ ba.

## ADDED Requirements

### Requirement: Danh sách license cho phép
Thành phần bên thứ ba đi kèm sản phẩm F-Eco SHALL có license thuộc danh sách cho phép: MIT, BSD-2-Clause, BSD-3-Clause, Apache-2.0, ISC, Zlib, BSL-1.0, Unicode-3.0, OFL-1.1, CC0-1.0, MPL-2.0 (không sửa file). License khác MUST chỉ được dùng khi có ngoại lệ đã duyệt, ghi người duyệt, ngày duyệt và lý do. GPL, AGPL, SSPL và license cấm dùng thương mại MUST không được dùng.

#### Scenario: Thêm thư viện AGPL
- **WHEN** một thay đổi thêm thư viện AGPL-3.0 vào F-Launcher
- **THEN** bước kiểm tra license của CI thất bại và chỉ rõ thư viện cùng license của nó

#### Scenario: Ngoại lệ thiếu người duyệt
- **WHEN** file ngoại lệ có một mục không ghi người duyệt
- **THEN** bước kiểm tra license của CI thất bại

### Requirement: Kiểm tra cả cây phụ thuộc
Việc kiểm tra license SHALL áp dụng cho cả phụ thuộc gián tiếp, thư viện native, gói npm của cổng web, và ảnh container của backend. Thành phần không rõ license MUST bị coi là không được phép.

#### Scenario: Phụ thuộc gián tiếp vi phạm
- **GIVEN** gói A (MIT) phụ thuộc gói B (GPL-3.0)
- **WHEN** thêm A vào cổng web
- **THEN** CI thất bại và chỉ ra chuỗi phụ thuộc A → B

### Requirement: Ghi công và SBOM
Mỗi bản phát hành của F-Launcher, F-Builder và F-SDK SHALL kèm `THIRD_PARTY_NOTICES` và SBOM (CycloneDX hoặc SPDX) sinh tự động từ cây phụ thuộc thực tế. Launcher MUST có mục "Giấy phép bên thứ ba".

#### Scenario: Xem giấy phép trong launcher
- **WHEN** người dùng mở Giới thiệu → Giấy phép bên thứ ba trong F-Launcher
- **THEN** danh sách hiện đủ các thành phần có trong bản phát hành đang chạy

### Requirement: License của app con là trách nhiệm của nhà phát triển
License của app con bên thứ ba SHALL do nhà phát triển khai báo trong manifest và hiện trên trang chi tiết. Thỏa thuận nhà phát triển MUST yêu cầu nhà phát triển có đủ quyền với mọi thành phần trong gói của họ.

#### Scenario: Hiện license của app con
- **WHEN** người dùng xem trang chi tiết của SoundLab
- **THEN** trang hiện license nhà phát triển đã khai báo, ví dụ "Proprietary" hoặc "MIT"
