# Spec Delta

## Purpose

Bổ sung cho capability `app-catalog`: giá app bằng F-Coin.

## ADDED Requirements

### Requirement: Giá app bằng F-Coin
Giá app trong F-Store SHALL tính bằng nguyên F-Coin; 0 là miễn phí.
- Trang app hiện giá F-Coin kèm số tiền quy đổi theo tỷ giá hiện hành, và nút "Mua · N F-Coin" khi người dùng chưa có quyền sử dụng.
- Nhà phát triển SHALL đặt giá ở trang app trong Cổng nhà phát triển, từ 0 đến 1.000.000 F-Coin.
- Giá cũ bằng đồng MUST được đổi sang F-Coin theo 1.000 ₫ một F-Coin, làm tròn lên.

#### Scenario: Trang app có giá
- **GIVEN** app giá 49 F-Coin, tỷ giá 1.000 ₫
- **WHEN** người dùng chưa mua mở trang app
- **THEN** trang ghi "49 F-Coin (≈ 49.000 ₫)" và có nút "Mua · 49 F-Coin"

#### Scenario: Nhà phát triển đặt giá
- **WHEN** chủ tài khoản nhà phát triển đặt giá app là 99 F-Coin
- **THEN** F-Store hiện giá 99 F-Coin, và bộ lọc "Miễn phí" không còn liệt kê app đó
