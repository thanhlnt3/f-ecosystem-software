# source-scan

## Purpose

Quét mã nguồn để tìm bí mật lộ, thư viện có lỗ hổng, mã đáng ngờ và giấy phép không khớp, trước khi duyệt app và trước khi tích hợp app bên ngoài vào F-Eco.

## ADDED Requirements

### Requirement: Bộ quét mã nguồn
`feco-codescan` SHALL quét một thư mục, tệp zip hoặc repo GitHub tại một commit, và trả danh sách phát hiện (luật, nhóm, mức `critical`/`high`/`medium`/`info`, tệp, dòng) cùng danh sách thư viện. Các nhóm phát hiện SHALL gồm: bí mật lộ, mã đáng ngờ, lỗ hổng thư viện (tra OSV.dev từ lockfile) và giấy phép không khớp với manifest.

#### Scenario: Khóa riêng trong mã
- **GIVEN** mã nguồn có tệp `deploy/key.pem` chứa `-----BEGIN RSA PRIVATE KEY-----`
- **WHEN** bộ quét chạy
- **THEN** có phát hiện `secret.private_key` mức `critical` ở `deploy/key.pem` dòng 1, trích đoạn đã che

#### Scenario: Khóa giả trong test
- **GIVEN** khóa AWS mẫu nằm trong `tests/fixtures/`
- **WHEN** bộ quét chạy
- **THEN** phát hiện có mức `medium`, không phải `critical`

#### Scenario: Thư viện có lỗ hổng
- **GIVEN** `package-lock.json` có một thư viện mà OSV báo lỗ hổng mức HIGH
- **WHEN** bộ quét chạy với OSV
- **THEN** có phát hiện `vulnerability` mức `high` ghi tên thư viện, phiên bản và mã lỗ hổng

#### Scenario: Đào tiền ảo
- **GIVEN** mã nguồn có `stratum+tcp://pool.example:3333`
- **WHEN** bộ quét chạy
- **THEN** có phát hiện `suspicious.miner` mức `critical`


### Requirement: Quét an toàn và kín đáo
Bí mật trong báo cáo MUST bị che, chỉ giữ 4 ký tự đầu. Bộ quét MUST chỉ gửi ra ngoài hệ sinh thái, tên và phiên bản thư viện. Giải nén MUST chặn đường dẫn thoát ra ngoài, symlink và bom zip.

#### Scenario: Zip độc
- **WHEN** tệp zip có mục `../../evil.sh`
- **THEN** bộ quét từ chối tệp mà không ghi gì ra ngoài thư mục tạm

### Requirement: Quét mã nguồn khi duyệt app
Nhà phát triển SHALL kèm được mã nguồn khi nộp phiên bản: tải tệp zip, hoặc khai repo GitHub và commit. Duyệt tự động SHALL quét mã nguồn sau khi gói sạch:
- có phát hiện `critical` thì phiên bản MUST bị từ chối với `reject_code` `source_scan`;
- có phát hiện `high`, hoặc không tra được OSV, thì phiên bản SHALL chuyển duyệt tay;
- báo cáo SHALL hiện ở trang phiên bản cho nhà phát triển và trang duyệt cho người duyệt.

#### Scenario: Bí mật lộ bị từ chối
- **GIVEN** nhà phát triển nộp phiên bản có gói sạch và mã nguồn chứa token GitHub thật
- **WHEN** duyệt tự động chạy
- **THEN** phiên bản bị từ chối với `reject_code` `source_scan`, và email cho nhà phát triển ghi luật và tệp mà không chép token

#### Scenario: Lỗ hổng cần người duyệt
- **GIVEN** bản cập nhật vốn được tự duyệt, mã nguồn có thư viện lỗ hổng mức HIGH
- **WHEN** duyệt tự động chạy
- **THEN** phiên bản chuyển duyệt tay với lý do "Quét mã nguồn: 1 phát hiện cần xem"

#### Scenario: Không kèm mã nguồn
- **WHEN** nhà phát triển nộp phiên bản không kèm mã nguồn
- **THEN** báo cáo kiểm tra ghi "Không có mã nguồn", và cách duyệt không đổi

### Requirement: Quét trước khi tích hợp app bên ngoài
Trước khi viết bản vá hay dựng app bên ngoài (ví dụ ThesisAI, Mentor), F-Eco SHALL quét repo tại đúng commit sẽ dùng, bằng `scripts/scan-integration.sh`. Phát hiện đã xem xét SHALL ghi trong `integrations/<app>/scan-allow.toml` kèm lý do. CI SHALL quét lại mọi app đã tích hợp và MUST thất bại khi có phát hiện `critical` chưa được cho phép. Script dựng Mentor MUST dừng khi quét thất bại.

#### Scenario: Commit mới có bí mật
- **GIVEN** `MENTOR_COMMIT` được đổi sang commit có khóa riêng
- **WHEN** CI chạy `integration-scan`
- **THEN** job thất bại và báo cáo ghi tệp chứa khóa
