# Logo

Logo của F-Launcher và các app trong danh mục dev.

- F-Launcher hiện logo app trong Thư viện, Cửa hàng và cửa sổ chi tiết.
- F-Store trên web hiện logo ở `/store/{id}/icon.png`.

Logo được nhúng (PNG 128 px, mã hóa base64) vào metadata TUF đã ký và vào danh mục.

| Tệp | Nguồn | Giấy phép |
|---|---|---|
| `f-launcher.svg` | F-Eco tự thiết kế | MIT OR Apache-2.0 (như dự án) |
| `f-notes.svg` | F-Eco tự thiết kế | MIT OR Apache-2.0 |
| `f-timer.svg` | F-Eco tự thiết kế | MIT OR Apache-2.0 |
| `bottom.svg` | `assets/icons/bottom-system-monitor.svg` của [ClementTsang/bottom](https://github.com/ClementTsang/bottom) | MIT, © Clement Tsang |
| `kibi.svg` | `assets/kibi.svg` của [ilai-deutel/kibi](https://github.com/ilai-deutel/kibi) | MIT OR Apache-2.0, © Ilaï Deutel và cộng sự |
| `eframe-template.png` | `assets/favicon-512x512.png` của [emilk/eframe_template](https://github.com/emilk/eframe_template) | MIT OR Apache-2.0, © Emil Ernerfeldt |
| `thesisai.svg` | `public/logo.svg` của [justAchillGuy8386/thesisai-fe](https://github.com/justAchillGuy8386/thesisai-fe); hình mặt trời giống icon `sun` của Lucide | Chưa công bố giấy phép; tác giả cho phép F-Eco dùng (integrations/thesisai/NOTICE.md). Lucide: ISC |

Các tệp `*-256.png` và `*-128.png` được vẽ lại từ tệp gốc:
- SVG: dùng Chrome headless;
- PNG: thu nhỏ bằng Pillow.

## Logo F-Launcher

Chữ F nghiêng ghép từ ba thanh màu: cam, xanh lá, xanh dương. Logo chỉ lấy cảm hứng từ bộ ba màu của Tập đoàn FPT, không dùng lại hình hay chữ trong logo FPT.

Trước khi phát hành thương mại, cần bộ phận thương hiệu của FPT đồng ý dùng yếu tố nhận diện này (QUESTIONS.md, mục 89).

## Thêm logo cho app

Mỗi app có một logo PNG, tối đa 64 KB; nên dùng 128 × 128 px, nền vuông bo góc.

- `feco-tuf publish ... --icon LOGO.png`: phát hành kèm logo.
- `feco-tuf set-icon --app ID --icon LOGO.png`: đặt logo cho các phiên bản đã có.
- Trường `icon_file` trong tệp JSON của `feco-catalog add-app`.
- `scripts/dev-apps.sh icons`: đặt logo cho các app dev.
- `scripts/dev-apps.sh thesisai`: app web ThesisAI, phát hành kèm logo.
