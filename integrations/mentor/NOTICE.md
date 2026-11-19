# NOTICE: Mentor

- **Ứng dụng**: Mentor, https://github.com/katD2707/agent-mentor-maximum-experience
- **Commit dùng để tạo bản vá và dựng gói**: xem `MENTOR_COMMIT`.
- **Giấy phép**: repo Mentor chưa có tệp LICENSE. Chủ dự án F-Eco xác nhận tác giả đồng ý cho F-Eco:
  - tải mã nguồn và dựng app;
  - sửa phần đăng nhập;
  - dùng logo Mentor (`assets/logos/mentor-*.png`, lấy từ `apps/app/src-tauri/icons`);

  với điều kiện ghi nguồn. Xem QUESTIONS.md.
- **F-Eco không chứa mã Mentor**: thư mục này chỉ có bản vá, hướng dẫn và cấu hình. Gói desktop do CI của F-Eco dựng từ mã Mentor gốc và các bản vá này.

## Thành phần bên thứ ba có trong Mentor

Theo `THIRD_PARTY_NOTICES.md` của Mentor:

| Thành phần | Giấy phép | Vị trí trong Mentor |
|---|---|---|
| DeepTutor (HKUDS), v1.7.2 | Apache License 2.0 | `services/gateway/src/mentor_gateway/research/vendor/deeptutor/` (kèm LICENSE và NOTICE) |
| education-agent-skills (Gareth Manning) | CC BY-SA 4.0 | `services/gateway/src/mentor_gateway/planner/skills/` (tám tệp kỹ năng, xem `ATTRIBUTION.md`) |

Gói desktop của F-Eco chỉ có app Mentor (giao diện React trong Tauri). Hai thành phần trên thuộc gateway; người chạy gateway lấy từ mã nguồn Mentor.

## Thư viện bản vá thêm vào Mentor

| Thư viện | Giấy phép | Dùng cho |
|---|---|---|
| authlib | BSD-3-Clause | Client OIDC của gateway (đăng nhập bằng F-Eco) |
| joserfc | BSD-3-Clause | Authlib dùng để kiểm chữ ký ID token |
| itsdangerous | BSD-3-Clause | Cookie ký của `SessionMiddleware` (state, nonce, PKCE) |

App Mentor không thêm thư viện npm nào.
