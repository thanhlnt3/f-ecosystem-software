# Proposal

## Why

Chủ dự án muốn hai việc:

1. **Thanh bên gọn hơn.** Thanh bên bên trái của F-Launcher ẩn hiện được. Các mục có chữ (Thư viện, Cửa hàng, Thông báo, Cài đặt...) thay được bằng icon nhỏ gọn.
2. **Tích hợp ThesisAI.** ThesisAI là ứng dụng luyện bảo vệ đồ án tốt nghiệp với hội đồng AI, gồm hai repo:
   - frontend: https://github.com/justAchillGuy8386/thesisai-fe (Next.js);
   - backend: https://github.com/justAchillGuy8386/thesisai-be (FastAPI, PostgreSQL pgvector, Ollama).

   Chủ dự án xác nhận tác giả đồng ý cho F-Eco tích hợp và sửa phần đăng nhập. Chủ dự án chọn cách tích hợp "app web trong F-Store".

Hiện trạng:
- Thanh bên luôn rộng 210 px và mọi mục đều có chữ.
- F-Eco chỉ phân phối app desktop: launcher tải gói zip đã ký qua TUF rồi chạy tệp trong gói. Không có cách nào đưa một ứng dụng web vào F-Store và launcher.
- ThesisAI có tài khoản riêng (email, mật khẩu, JWT của nó), chưa đăng nhập được bằng tài khoản F-Eco.

## What Changes

- **Thanh bên thu gọn được**:
  - nút ẩn/hiện ở đầu thanh bên và phím tắt Ctrl+B;
  - chế độ thu gọn rộng 56 px, chỉ có icon (Thư viện, Cửa hàng, Thông báo, Cài đặt, đồng bộ, F-Eco trên web, bảng quản trị, cổng nhà phát triển, ví, đăng nhập, đăng xuất); mỗi icon có tooltip ghi tên, số thông báo chưa đọc và số bản cập nhật hiện thành huy hiệu;
  - launcher nhớ lựa chọn trong `settings.json`; cửa sổ hẹp hơn 720 px thì tự thu gọn.
- **App web**:
  - metadata TUF thêm trường `web_url` (chỉ HTTPS, hoặc `http://localhost`, `http://127.0.0.1` cho môi trường dev), ký cùng target; target của app web có nền tảng `web`;
  - launcher hiện app web trên mọi hệ điều hành, gắn nhãn "App web". Nút "Thêm" ghi app vào Thư viện (gói nhỏ chỉ chứa thông tin app), nút "Mở" mở địa chỉ đó bằng trình duyệt;
  - app web tự đăng nhập bằng tài khoản F-Eco theo chuẩn OIDC (client OAuth phía máy chủ, có secret). Trình duyệt đã đăng nhập F-Eco thì không phải nhập lại mật khẩu;
  - F-Store lưu `web_url` (`catalog_apps.web_url`), trang app có nút "Mở ứng dụng web", nền tảng ghi "Web", bộ lọc hệ điều hành vẫn hiện app web;
  - `feco-tuf publish --web-url`, `feco-catalog add-app` hiểu trường `web_url`.
- **ThesisAI trong môi trường dev**:
  - `scripts/dev-apps.sh thesisai`: client OAuth phía máy chủ (scope `openid profile email`, chuyển hướng về backend ThesisAI), gói app web, logo lấy từ repo gốc, danh mục F-Store;
  - `integrations/thesisai/`: hướng dẫn và bản vá cho hai repo ThesisAI để thêm "Đăng nhập bằng F-Eco". Backend dùng Authlib (BSD-3-Clause), không tự viết OAuth. Bản vá không đưa mã của ThesisAI vào F-Eco; tác giả áp dụng vào repo của mình.

## Capabilities

### New Capabilities

Không có.

### Modified Capabilities

- `launcher`: thanh bên thu gọn; app web.
- `app-catalog`: app web trên F-Store.

## Impact

- `crates/feco-tuf`: `TargetMeta::web_url`, kiểm địa chỉ, `publish --web-url`.
- Máy chủ: migration `0017_web_apps.sql`, `Listing::web_url`, trang app và trang "Ứng dụng của tôi", bộ lọc nền tảng.
- F-Launcher: `Prefs::sidebar_collapsed`, thanh bên hai chế độ, icon mới `Menu`; catalog nhận nền tảng `web`; mở app web bằng trình duyệt.
- `scripts/dev-apps.sh thesisai`, `assets/logos/thesisai*`, `integrations/thesisai/`.
- Không thêm thư viện nào vào F-Eco. Bản vá ThesisAI thêm `authlib` và `itsdangerous` (BSD-3-Clause) vào backend ThesisAI.
