# Proposal

## Why

Rà soát tính năng "phát hiện app con có bản mới và báo người dùng" trên F-Launcher máy tính, web và di động (21/11/2026). Mỗi nơi đều có phần báo bản mới, nhưng dữ liệu giữa các nơi không khớp và thông báo gửi sai người:

1. **Máy chủ không biết người dùng đã cập nhật.** F-Launcher chỉ báo phiên bản khi cài hoặc mua. Sau khi người dùng cập nhật, cập nhật tất cả, tự cập nhật hay quay lại bản trước, máy chủ vẫn giữ phiên bản cũ. Vì vậy "Ứng dụng của tôi" trên web và di động vẫn hiện "Có bản mới".
2. **Gỡ app không được báo cho máy chủ.** Người đã gỡ vẫn thấy app trong "Ứng dụng của tôi" và vẫn nhận thông báo cập nhật.
3. **Thông báo gửi quá sớm, gửi trùng và gửi sai người.** Mỗi lần nhà phát triển bấm phát hành (cả kênh beta, cả mức 10%, cả lần đổi phần trăm) đều báo cho mọi người đã cài, kể cả người đã có bản đó. Trong khi đó, F-Launcher chỉ đề nghị bản này cho đúng phần trăm thiết bị trên kênh ổn định.
4. **Phiên bản phát hành bằng `feco-catalog release` không có thông báo.**
5. **Web và di động lấy "bản mới nhất" không theo kênh và phần trăm.** Chúng hiện "Có bản mới" cho cả bản beta, bản đang phát hành dần và bản đã tạm dừng, là những bản mà F-Launcher không cài.
6. **F-Launcher đếm không khớp.** Huy hiệu ở Thư viện và nút "Cập nhật tất cả (N)" tính cả bản người dùng đã "Bỏ qua bản này", nên "Cập nhật tất cả" vẫn cài bản đó. Hộp thoại và dải thông báo thì không tính.
7. **Di động chỉ biết bản mới khi mở app hoặc kéo làm mới.** Quay lại app từ nền chỉ tải lại tin nhắn. Danh sách không có dòng tóm tắt như web. Với app web, nhãn "Có bản mới" vô nghĩa vì app web luôn mở bản mới nhất.

## What Changes

- **Máy chủ:**
  - `DELETE /v1/catalog/installs/{app}`: F-Launcher báo đã gỡ app.
  - "Bản mới nhất" trong `/v1/library` và trang `/library` chỉ tính bản đã phát hành trên kênh ổn định cho 100% thiết bị.
  - Thông báo cập nhật gửi một lần cho mỗi phiên bản, khi phiên bản đó tới tay mọi người trên kênh ổn định. Chỉ gửi cho người đang cài bản cũ hơn. Bảng mới `app_update_notices` chống gửi trùng.
  - Việc định kỳ mỗi 10 phút gửi thông báo cho cả bản phát hành bằng `feco-catalog release`.
- **F-Launcher máy tính:**
  - báo máy chủ phiên bản mới sau khi cập nhật, cập nhật tất cả, tự cập nhật hoặc quay lại bản trước;
  - báo máy chủ khi gỡ app;
  - huy hiệu Thư viện, nút "Cập nhật tất cả" và tự cập nhật bỏ qua phiên bản người dùng đã bỏ qua.
- **Di động:**
  - quay lại app từ nền thì tải lại "Ứng dụng của tôi" và thông báo;
  - dòng tóm tắt "Số app có bản mới: N. Mở F-Launcher trên máy tính để cập nhật.";
  - huy hiệu số bản mới trên tab "Ứng dụng của tôi";
  - không hiện "Có bản mới" cho app web.

## Capabilities

### Modified Capabilities
- `notifications`: điều kiện gửi thông báo cập nhật app.
- `app-catalog`: "bản mới nhất" của "Ứng dụng của tôi"; báo gỡ app.
- `launcher`: báo máy chủ sau khi cập nhật hoặc gỡ; đếm bản cập nhật.
- `mobile-launcher`: báo bản mới trên di động.

## Impact

- `services/feco-server`:
  - `catalog.rs`, `notifications.rs`, `developer.rs`, `store_web.rs`, `lib.rs`, `main.rs`;
  - migration `0022_app_update_notices.sql`;
  - E2E `app_update_notices`.
- `clients/feco-launcher`: `main.rs`, `install.rs`, `auth.rs`, kèm test.
- `clients/feco-mobile`: `app_state.dart`, `ui.dart`, `i18n.dart`, kèm widget test.
- Không đổi API đang có. Chỉ thêm `DELETE /v1/catalog/installs/{app}`.
