# Spec Delta

## Purpose

Bổ sung cho capability `admin-console`: cảnh báo kiểm duyệt hiện trong bảng quản trị.

## ADDED Requirements

### Requirement: Cảnh báo kiểm duyệt trong bảng quản trị
Trang Hàng chờ duyệt SHALL hiện các cảnh báo kiểm duyệt trong 30 ngày gần nhất (lỗ hổng thư viện mới, mã độc phát hiện muộn), mới nhất trước, với app, phiên bản, nội dung và thời gian. Danh sách SHALL lọc, sắp xếp và tìm kiếm được như các danh sách quản trị khác.

#### Scenario: Xem cảnh báo lỗ hổng mới
- **GIVEN** việc quét lại vừa phát hiện lỗ hổng mới trong một app đang phát hành
- **WHEN** người duyệt mở Hàng chờ duyệt
- **THEN** mục "Cảnh báo kiểm duyệt" có cảnh báo đó với tên app và phiên bản
