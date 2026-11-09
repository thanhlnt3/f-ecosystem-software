# F-EcoSystem-Software (F-Eco)

Nền tảng mở để nhà phát triển bên thứ ba phát hành **app con** qua một launcher chung, với tài khoản người dùng dùng chung, công cụ tự xây app (**F-Builder**) và dịch vụ **xây app theo yêu cầu** có thanh toán. Ý tưởng lấy cảm hứng từ mô hình launcher + bộ ứng dụng của ArtCraft, nhưng mở cho bên thứ ba.

> Trạng thái: **đang xây dựng**. Nhóm task 1 (nền tảng dự án) gần xong; tiếp theo là nhóm 2 (tài khoản). Chưa dùng được cho người dùng cuối.

## Yêu cầu

Toàn bộ yêu cầu viết theo [OpenSpec](https://github.com/Fission-AI/OpenSpec) trong
[`openspec/changes/add-f-ecosystem-platform/`](openspec/changes/add-f-ecosystem-platform/):

- `proposal.md`: lý do, phạm vi
- `specs/*/spec.md`: 15 capability (tài khoản, đồng ý dữ liệu, launcher, danh mục, cổng nhà phát triển, kiểm duyệt, SDK, F-Builder, đơn xây app, thanh toán, quản trị…)
- `design.md`: quyết định kỹ thuật (Rust toàn bộ, Ory Kratos/Hydra, TUF, VNPAY/MoMo…)
- `tasks.md`: kế hoạch theo nhóm, đánh dấu tiến độ

## Cấu trúc

| Thư mục | Nội dung |
|---|---|
| `crates/` | Thư viện dùng chung (cấu hình, kiểu dữ liệu) |
| `services/` | Dịch vụ backend (axum) |
| `infra/` | Hạ tầng phát triển (docker compose: PostgreSQL, Valkey, Ory Kratos, Ory Hydra) |
| `third_party/` | Khai báo license của dịch vụ và ảnh container đi kèm |

## Phát triển

```bash
cargo build --workspace
cargo test --workspace
```

## Giấy phép

Phát hành theo lựa chọn của bạn một trong hai giấy phép: MIT ([LICENSE-MIT](LICENSE-MIT)) hoặc Apache License 2.0 ([LICENSE-APACHE](LICENSE-APACHE)).
Trừ khi bạn nói rõ khác đi, mọi đóng góp bạn gửi vào dự án được cấp phép theo cả hai giấy phép trên, không kèm điều khoản bổ sung.

## Chạy hạ tầng phát triển

Cần Docker:

```bash
docker compose -f infra/docker-compose.yml up -d --wait
FECO_ENV=dev cargo run -p feco-server   # http://127.0.0.1:8080/healthz
```

Kratos: http://127.0.0.1:4433 · Hydra: http://127.0.0.1:4444 · Mailpit: http://127.0.0.1:8025

## Kiểm tra license

```bash
cargo deny check                  # crate Rust
cargo run -p feco-license-check   # dịch vụ và ảnh container (third_party/native.toml)
```
