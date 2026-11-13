# Đưa app mã nguồn mở từ GitHub vào F-Eco

Hướng dẫn này dành cho người muốn đưa một app mã nguồn mở có sẵn (thường là trên GitHub) vào
F-Store, để người dùng cài và mở bằng F-Launcher. App đi qua đúng quy trình như mọi app con khác:
manifest, nộp gói, kiểm duyệt, ký TUF. Phần khác biệt nằm ở ba chỗ:
- dựng từ mã nguồn của người khác;
- giữ giấy phép gốc;
- có thể là app chạy trong terminal.

Các khái niệm chung (tài khoản nhà phát triển, manifest, `f-dev`, kiểm duyệt) có trong
[`integration-guide.md`](integration-guide.md). Tài liệu này chỉ nói phần riêng cho app từ GitHub.

Ví dụ xuyên suốt là ba app có sẵn trong danh mục dev:

| App | Kho | Giấy phép | Loại |
|---|---|---|---|
| kibi | `ilai-deutel/kibi` @ `v0.3.3` | MIT OR Apache-2.0 | soạn thảo văn bản, chạy trong terminal |
| bottom | `ClementTsang/bottom` @ `0.15.0` | MIT | theo dõi hệ thống, chạy trong terminal |
| eframe template | `emilk/eframe_template` @ `f108546` | MIT OR Apache-2.0 | app cửa sổ (egui) |

## Mục lục

1. [Chọn app](#1-chọn-app)
2. [Tải đúng phiên bản mã nguồn](#2-tải-đúng-phiên-bản-mã-nguồn)
3. [Dựng](#3-dựng)
4. [Đóng gói kèm giấy phép gốc](#4-đóng-gói-kèm-giấy-phép-gốc)
5. [Viết manifest](#5-viết-manifest)
6. [Ký mã, kiểm tra và nộp](#6-ký-mã-kiểm-tra-và-nộp)
7. [Kiểm duyệt và hiển thị trong F-Launcher](#7-kiểm-duyệt-và-hiển-thị-trong-f-launcher)
8. [Cập nhật khi dự án gốc ra bản mới](#8-cập-nhật-khi-dự-án-gốc-ra-bản-mới)
9. [Thêm đăng nhập F-Eco vào app (tùy chọn)](#9-thêm-đăng-nhập-f-eco-vào-app-tùy-chọn)
10. [Thử trên máy dev bằng một lệnh](#10-thử-trên-máy-dev-bằng-một-lệnh)
11. [Danh sách kiểm tra](#11-danh-sách-kiểm-tra)

## 1. Chọn app

- **Giấy phép** phải nằm trong danh sách cho phép của F-Eco:
  - được phép: MIT, BSD-2-Clause, BSD-3-Clause, Apache-2.0, ISC, 0BSD, Zlib, BSL-1.0, MPL-2.0 (không sửa file), và các giấy phép khác trong spec `dependency-licensing`;
  - không nhận: GPL, AGPL, SSPL và giấy phép cấm dùng thương mại.

  Đọc file `LICENSE` của kho, đừng chỉ dựa vào nhãn GitHub hiển thị: nhãn có thể chỉ phản ánh một phần của dự án.
- **Thư viện phụ thuộc** cũng phải có giấy phép cho phép. Với dự án Rust, chạy `cargo deny check licenses` hoặc `cargo tree` trong thư mục mã nguồn để xem.
- **Dự án còn được duy trì**: không bị archive, có bản phát hành gắn tag.
- **Tên và logo** thuộc về dự án gốc:
  - giữ tên app;
  - ghi rõ "dựng từ mã nguồn của …" trong mô tả;
  - không trình bày app như sản phẩm do F-Eco làm ra;
  - không dùng logo của dự án làm logo của bạn.

## 2. Tải đúng phiên bản mã nguồn

Luôn dựng từ một **tag hoặc commit cố định**, không dựng từ nhánh `main`. Có vậy bản dựng mới lặp lại được, và người duyệt mới đối chiếu được.

```bash
mkdir -p ~/build-src/kibi && cd ~/build-src/kibi
curl -sSfL https://codeload.github.com/ilai-deutel/kibi/tar.gz/refs/tags/v0.3.3 \
  | tar -xz --strip-components=1
# commit thay cho tag: .../tar.gz/<sha đủ 40 ký tự>
```

- Đặt mã nguồn **ngoài** thư mục dự án của bạn, và ngoài mọi kho git khác. Có hai lý do:
  - nằm trong một Cargo workspace khác thì Cargo báo lỗi "believes it's in a workspace";
  - nhiều dự án lấy số phiên bản bằng `git describe` lúc dựng. Nằm trong kho git của bạn thì lệnh này đọc nhầm tag của kho bạn. Ví dụ: kibi 0.3.3 dựng trong kho F-Eco tự báo là "Kibi 1.0.0".
- **Trước khi dựng, đọc các script chạy lúc dựng**, vì chúng chạy trên máy bạn với quyền của bạn:
  - Rust: `build.rs`;
  - Node: `scripts` trong `package.json`;
  - Python: `setup.py`.

  Ví dụ: `build.rs` của kibi và bottom chỉ gọi `git describe` để lấy số phiên bản.

## 3. Dựng

Dựng trên từng nền tảng bạn khai trong manifest. Với dự án Rust:

```bash
cd ~/build-src/kibi
RUSTUP_TOOLCHAIN=stable cargo build --release --locked
# tệp chạy: target/release/kibi(.exe)
```

- `--locked` dùng đúng `Cargo.lock` của dự án gốc.
- `RUSTUP_TOOLCHAIN=stable` bỏ qua file `rust-toolchain` của dự án. Ví dụ eframe template ghim Rust 1.95, nên khi không có cờ này, rustup sẽ tự tải thêm toolchain đó.

Dự án viết bằng ngôn ngữ khác thì làm theo hướng dẫn dựng của dự án đó:
- Go: `go build`;
- Electron hoặc Node: `npm ci && npm run build`, rồi đóng gói bằng công cụ của dự án.

Gói nộp cho F-Eco phải chạy được mà không cần người dùng cài thêm trình thông dịch.

## 4. Đóng gói kèm giấy phép gốc

Khi manifest có `source`, **gói phải chứa ít nhất một file giấy phép gốc**: `LICENSE*`, `LICENCE*`, `COPYING*` hoặc `NOTICE*`, ở thư mục nào trong gói cũng được. `f-dev pack` và `f-dev publish` từ chối gói thiếu file này.

Cấu trúc nên dùng:

```
build/
├── bin/kibi.exe
├── licenses/LICENSE-APACHE
├── licenses/LICENSE-MIT
├── licenses/COPYRIGHT
└── SOURCE.txt          # "Dựng từ https://github.com/ilai-deutel/kibi tại v0.3.3"
```

Nếu giấy phép của dự án yêu cầu kèm thông báo của thư viện phụ thuộc, chép cả file thông báo đó (ví dụ `THIRD_PARTY_NOTICES`, hoặc kết quả `cargo about`) vào `licenses/`.

## 5. Viết manifest

App terminal không dùng đăng nhập F-Eco (kibi):

```toml
id = "kibi"
name = "kibi"
version = "0.3.3"                 # theo phiên bản của dự án gốc
license = "MIT OR Apache-2.0"     # khớp giấy phép của dự án gốc
privacy_policy = "https://<trang của bạn>/kibi-privacy"
summary = "Trình soạn thảo văn bản siêu nhỏ chạy trong terminal"
description = "Dựng từ mã nguồn của Ilaï Deutel (github.com/ilai-deutel/kibi), không sửa đổi."
category = "tools"
source = "https://github.com/ilai-deutel/kibi"   # bắt buộc phải là https://
terminal = true                                  # launcher mở app trong cửa sổ terminal
features = [                                     # hiện ở trang chi tiết trong launcher
  "Tô màu cú pháp",
  "Tìm kiếm tăng dần",
  "Chạy lệnh ngoài và dán kết quả",
]
release_notes = "Bản 0.3.3 của dự án gốc: thêm Ctrl+/ để chú thích dòng."

[[platforms]]
os = "windows"
arch = "x64"
entry = "bin/kibi.exe"

[permissions]
network = false
user_folders = ["documents"]
```

App cửa sổ (eframe template): bỏ `terminal`, các trường còn lại tương tự.

Giải thích các trường:
- **`source`**:
  - hiện thành nút "Mã nguồn" trong F-Launcher và F-Store;
  - bật kiểm tra giấy phép gốc trong gói;
  - là nơi người duyệt đối chiếu giấy phép.
- **`terminal = true`**:
  - Windows: launcher mở app trong một cửa sổ console mới;
  - macOS: launcher mở app trong Terminal. Biến môi trường không đi qua Terminal, nên app terminal không nhận được mã khởi chạy. Nếu app terminal cần đăng nhập, app phải tự đăng nhập qua trình duyệt (`fsdk::session`). `f-dev validate` cảnh báo trường hợp này.
- **Không khai `[scopes]`** khi app không đăng nhập bằng F-Eco. Khi đó:
  - người dùng không thấy màn hình đồng ý;
  - target TUF không có `client_id`;
  - launcher mở app mà không xin mã khởi chạy.
- **`category`**: nên dùng một mã sau để F-Launcher gom nhóm và hiện tên tiếng Việt:

  | Mã | Tên hiển thị |
  |---|---|
  | `productivity` | Năng suất |
  | `tools` | Công cụ |
  | `system` | Hệ thống |
  | `development` | Lập trình |
  | `education` | Giáo dục |
  | `games` | Trò chơi |
  | `graphics` | Đồ họa |
  | `music` | Âm nhạc |
  | `video` | Video |
  | `business` | Kinh doanh |

  Giá trị khác được hiện nguyên văn.
- **`privacy_policy`** là trang của **bạn**, người phát hành trên F-Eco. Trang này nói rõ app có thu thập dữ liệu hay không. Với app không kết nối mạng, một trang ngắn ghi "app không thu thập dữ liệu" là đủ.

## 6. Ký mã, kiểm tra và nộp

Ký tệp chạy **bạn đã dựng**:
- Windows: Authenticode;
- macOS: Developer ID, nên notarize.

Gói chưa ký bị từ chối, như mọi app khác. Khóa ký là của bạn, không phải của dự án gốc.

```bash
f-dev validate --manifest f-app.toml          # lỗi và cảnh báo (ví dụ app terminal trên macOS)
f-dev pack     --manifest f-app.toml --package build/ --out dist/
                                              # tạo dist/windows-x64.zip, kiểm tra giấy phép gốc
f-dev publish  --manifest f-app.toml --package dist/
```

## 7. Kiểm duyệt và hiển thị trong F-Launcher

Ngoài các bước kiểm duyệt thường (quét mã độc, chữ ký mã, quyền), người duyệt:
- mở liên kết `source`;
- đối chiếu `license` trong manifest với giấy phép của dự án;
- kiểm tra file giấy phép có trong gói.

Sau khi phát hành, máy chủ ký vào metadata TUF của phiên bản các thông tin sau:
- mô tả ngắn, nhóm, `source`, `terminal`: lấy từ manifest;
- tên nhà phát triển: lấy từ danh mục.

F-Launcher chỉ hiện thông tin từ metadata đã ký này. Ngoài ra, máy chủ còn ký giới thiệu,
`features`, `website`, `release_notes`, giấy phép và thời điểm phát hành, để launcher dựng trang
chi tiết của app:
- **Thư viện và Cửa hàng**, ở bố cục lưới hoặc danh sách: biểu tượng chữ cái đầu, tên, nhà phát triển, nhóm, mô tả ngắn;
- **nút Mở**: ghi "Mở (terminal)" cho app có `terminal`;
- **menu "..."**: có mục "Xem mã nguồn".
- **trang chi tiết** (bấm vào app): giới thiệu, tính năng, có gì mới, bảng thông tin, nút
  "Mã nguồn (GitHub)" và "Website".

Ghi chú phát hành nên tóm tắt đúng CHANGELOG của dự án gốc cho phiên bản đó, và ghi rõ là bản gốc
hay đã sửa đổi.

## 8. Cập nhật khi dự án gốc ra bản mới

1. Đọc changelog của dự án. Kiểm tra giấy phép và thư viện phụ thuộc có đổi không.
2. Tải tag mới (mục 2), xem lại script chạy lúc dựng, rồi dựng lại.
3. Đổi `version` trong manifest theo phiên bản mới. Phiên bản phải lớn hơn bản đã phát hành.
4. Ký mã, chạy `f-dev pack`, rồi `f-dev publish`. Bản cập nhật không thêm scope hay quyền thường được duyệt tự động.

## 9. Thêm đăng nhập F-Eco vào app (tùy chọn)

Muốn app dùng tài khoản F-Eco (ví dụ đồng bộ dữ liệu theo người dùng):
1. Fork dự án và thêm F-SDK. Giữ nguyên giấy phép gốc; phần bạn thêm có thể dùng cùng giấy phép đó.
2. Khai `[scopes]` trong manifest. Từ phiên bản đó, launcher cấp mã khởi chạy cho app.
3. Đổi `source` sang kho fork của bạn.

Xem cách dùng F-SDK trong [`integration-guide.md`](integration-guide.md#5-đăng-nhập-trong-app-bằng-f-sdk), và hai app mẫu `examples/apps/f-notes` (Ghi chú) và `examples/apps/f-timer` (Bấm giờ).

## 10. Thử trên máy dev bằng một lệnh

Trong kho F-Eco, sau khi đã chạy hạ tầng, `feco-server` và `scripts/dev-launcher.sh setup` (README bước 2, 3 và 6):

```bash
# app mẫu dùng F-SDK và ba app mã nguồn mở ở trên
scripts/dev-apps.sh all

# một app bất kỳ từ GitHub
scripts/dev-apps.sh github ilai-deutel/kibi v0.3.3 --id kibi --name kibi --bin kibi \
  --license "MIT OR Apache-2.0" --category tools --developer "Ilaï Deutel" --terminal \
  --summary "Trình soạn thảo văn bản trong terminal" \
  --description "Giới thiệu dài cho trang chi tiết" \
  --features "Tô màu cú pháp|Tìm kiếm tăng dần" --website https://example.org \
  --notes "Các thay đổi của bản 0.3.3"
```

Script làm lần lượt các việc sau:
1. tải mã nguồn đúng tag hoặc commit vào `~/.feco-dev-ext/src/<id>` (đổi chỗ bằng `FECO_EXT_DIR`);
2. dựng bằng `cargo build --release --locked`;
3. đóng gói `bin/`, `licenses/`, `SOURCE.txt`;
4. ký vào kho TUF dev;
5. ghi app vào danh mục F-Store dev.

Mở lại F-Launcher dev (hoặc bấm Đồng bộ) để thấy app trong Cửa hàng. Script chỉ dành cho dev. Bản phát hành thật luôn đi qua `f-dev`, kiểm duyệt, và máy chủ ký.

## 11. Danh sách kiểm tra

- [ ] Giấy phép của dự án và của thư viện phụ thuộc đều nằm trong danh sách cho phép.
- [ ] Dựng từ tag hoặc commit cố định, ngoài thư mục dự án của bạn, sau khi đã đọc script chạy lúc dựng.
- [ ] Gói có file giấy phép gốc, và `SOURCE.txt` ghi kho cùng tag hoặc commit.
- [ ] `license` khớp dự án gốc; `source` là địa chỉ https:// của kho.
- [ ] `terminal = true` cho app chạy trong terminal.
- [ ] Không khai scope nếu app không đăng nhập bằng F-Eco.
- [ ] Mô tả ghi rõ app dựng từ mã nguồn của ai; không dùng logo dự án như logo của bạn.
- [ ] Tệp chạy đã ký mã bằng khóa của bạn.
