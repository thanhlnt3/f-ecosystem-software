<p align="center">
  <img src="assets/app-icon/hicolor/256x256/apps/io.github.thanhlnt3.f-builder.png" width="128" height="128" alt="Biểu tượng F-Builder: chữ F trắng và tia sáng trên nền xanh F-Eco">
</p>

# F-Builder

F-Builder là app con có sẵn của **F-EcoSystem-Software (F-Eco)**: giúp bạn tự xây một app mới từ
mô tả bằng lời, từ một bộ spec (ví dụ OpenSpec) hoặc từ một trang web, ứng dụng để tham khảo, bằng
coding agent và tài khoản AI của chính bạn. Mỗi lượt xây đi qua bốn bước **Tìm hiểu → Xây → Kiểm
tra → Sẵn sàng**; khi xong, bạn chạy thử app ngay trong F-Builder hoặc nộp lên F-Store.

Yêu cầu của F-Builder nằm trong spec `app-builder` của F-Eco
(`openspec/changes/add-f-ecosystem-platform/specs/app-builder/spec.md`, nhóm task 11).

## Nguồn gốc

F-Builder phát triển tiếp từ **Craft Anything 0.6.0** của nhóm ArtCraft và các người đóng góp
(https://github.com/storytold/craft-anything, MIT OR Apache-2.0). F-Builder giữ nguyên `LICENSE-MIT`,
`LICENSE-APACHE` và `NOTICE` gốc (trong `NOTICE`), ghi rõ các thay đổi, và không dùng tên, logo hay
biểu tượng của ArtCraft hay Craft Anything. F-Builder không liên kết với và không được nhóm
ArtCraft bảo trợ.

## Đã có

- App desktop (egui), CLI `f-builder-cli` và máy chủ MCP, cùng một engine.
- Tạo dự án từ mô tả, từ thư mục có sẵn hoặc từ một trang web hay ứng dụng tham khảo; mỗi dự án
  một thư mục (`Documents\F-Builder\<tên>`) có yêu cầu, dữ liệu thu thập, `app/`, kết quả kiểm tra,
  câu hỏi mở và nhật ký.
- Bốn bước Tìm hiểu, Xây, Kiểm tra, Sẵn sàng, có Dừng, Tiếp tục và yêu cầu sửa.
- Chế độ clean room mặc định (chỉ quan sát như người dùng); chế độ đọc sâu là tùy chọn.
- Chọn coding agent đã cài (Claude Code, Codex, Gemini CLI, Aider, OpenCode…) hoặc nhà cung cấp AI
  bằng khóa API, khóa lưu trong kho bảo mật của hệ điều hành.
- Hỏi trước khi chạy lệnh có rủi ro, ghi nhớ lựa chọn "luôn cho phép" theo dự án.

F-Eco thêm (nhóm task 11.2–11.6):

- Xây theo OpenSpec: chọn thư mục có `openspec/changes/<tên>/`, spec là nguồn yêu cầu bắt buộc, app
  dựng trong `app/` bên cạnh. Dự án chỉ có mô tả thì được hỏi nền tảng và ngôn ngữ trước khi xây.
  Mỗi dự án có lịch sử các lượt (`runs.jsonl`).
- Không bị treo im lặng: một bước 10 phút không có tiến triển (nhật ký, tệp, CPU) thì báo kèm Tiếp
  tục chờ, Chạy lại bước, Dừng; lượt kết thúc khi agent đã bàn giao kết quả.
- Đọc sâu cần xác nhận quyền phân tích (ghi vào dự án) và dự án đọc sâu không nộp được lên F-Store;
  bước Kiểm tra cảnh báo khi app mới chứa tên sản phẩm tham khảo.
- Khóa API chỉ ở kho bảo mật của hệ điều hành. Cài thư viện, lệnh mạng, chạy app luôn hỏi trước (kể
  cả chế độ tự động) trừ khi dự án cho phép; không ghi ngoài thư mục dự án.
- Nộp lên F-Store (manifest nháp để xem lại, rồi nộp qua cổng nhà phát triển, "Chờ duyệt") và "Nhờ
  F-Eco xây hộ" (đơn nháp kèm spec).

Việc còn mở: `docs/ROADMAP.md`.

## Dựng và chạy

F-Builder là một Cargo workspace riêng trong repo F-Eco (edition 2024, cần Rust 1.97 trở lên).

```bash
cd clients/f-builder
cargo run -p f-builder            # app desktop
cargo run -p f-builder-cli -- --help
cargo test --workspace
cargo xtask ci                     # mọi kiểm tra như CI
```

Trên Linux cần thêm thư viện hệ thống cho cửa sổ: `libxkbcommon-dev libwayland-dev libx11-dev
libxrandr-dev libxi-dev libgl1-mesa-dev libgtk-3-dev`. F-Eco phát hành F-Builder cho Windows và
macOS.

Dữ liệu: cài đặt trong thư mục cấu hình (`%APPDATA%\f-builder`, đổi bằng `F_BUILDER_CONFIG_DIR`),
dự án trong `Documents\F-Builder`.

## Tài liệu

- `AGENTS.md`: quy tắc cho người đóng góp và coding agent (luôn tuân theo).
- `docs/STATUS.md`: những gì đang chạy được; `docs/tools.md`: danh sách công cụ.
- `docs/extending.md`: thêm công cụ, loại mục tiêu, coding agent; `docs/pitfalls.md`: các lỗi đã gặp.

## Giấy phép

MIT OR Apache-2.0 (`LICENSE-MIT`, `LICENSE-APACHE`). Phông chữ và biểu tượng đi kèm: `ATTRIBUTION.md`.
