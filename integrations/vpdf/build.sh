#!/usr/bin/env bash
# Dựng VPdf (Windows x64) từ mã nguồn tại tag và commit trong README.md, rồi sắp gói vào THƯ-MỤC-GÓI
# (thay đổi vpdf-integration). Gói gồm vpdf-app.exe, pdfium.dll, các DLL của qpdf và mọi tệp giấy
# phép; F-Eco ký thư mục này thành gói TUF bằng scripts/dev-apps.sh vpdf.
#
#   integrations/vpdf/build.sh THƯ-MỤC-GÓI
#
# PDFium (bblanchon/pdfium-binaries) và qpdf dựng sẵn được tải bằng scripts/fetch-native.sh của
# chính VPdf, có kiểm SHA-256 ghim trong script đó. Đặt CARGO_TARGET_DIR để dùng chung thư mục dựng.
set -euo pipefail

out="${1:?cách dùng: $0 THƯ-MỤC-GÓI}"
here="$(cd "$(dirname "$0")" && pwd)"
read -r repo tag commit <<< "$(sed -nE 's#.*https://github\.com/([A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+) \| `([A-Za-z0-9._-]+)` \| `([0-9a-f]{40})`.*#\1 \2 \3#p' "$here/README.md" | head -1)"
[[ -n "${commit:-}" ]] || { echo "không đọc được repo, tag và commit từ README.md" >&2; exit 1; }

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
git -C "$work" init -q src
git -C "$work/src" config core.autocrlf false
git -C "$work/src" fetch -q --depth 1 "https://github.com/$repo.git" "$commit"
git -C "$work/src" checkout -q FETCH_HEAD
src="$work/src"

(cd "$src" && bash scripts/fetch-native.sh win-x64)
target="${CARGO_TARGET_DIR:-$src/target}"
cargo build --release --locked --manifest-path "$src/Cargo.toml" -p vpdf-app
exe="$target/release/vpdf-app.exe"
[[ -f "$exe" ]] || { echo "không thấy $exe" >&2; exit 1; }

rm -rf "$out"
mkdir -p "$out/licenses/pdfium" "$out/licenses/qpdf"
cp "$exe" "$out/"
cp "$src/third_party/pdfium/win-x64/bin/pdfium.dll" "$out/"
qpdf_bin="$(dirname "$(find "$src/third_party/qpdf/win-x64" -name 'qpdf30.dll' | head -1)")"
cp "$qpdf_bin"/*.dll "$out/"
# Giấy phép: VPdf (MIT hoặc Apache-2.0), PDFium và các thành phần bên trong, qpdf, phông Noto Sans.
cp "$src/LICENSE-MIT" "$src/LICENSE-APACHE" "$out/licenses/"
cp "$src/assets/fonts/OFL.txt" "$out/licenses/NotoSans-OFL.txt"
cp "$src/third_party/pdfium/win-x64/LICENSE" "$out/licenses/pdfium/"
cp "$src/third_party/pdfium/win-x64/licenses/"* "$out/licenses/pdfium/"
find "$src/third_party/qpdf/win-x64" -type f \( -iname 'LICENSE*' -o -iname 'NOTICE*' -o -iname 'Artistic*' \) \
  -exec cp {} "$out/licenses/qpdf/" \;
if ! ls "$out/licenses/qpdf/"* >/dev/null 2>&1; then
  echo "không thấy giấy phép qpdf trong gói qpdf đã tải" >&2
  find "$src/third_party/qpdf/win-x64" -maxdepth 4 >&2
  exit 1
fi
cp "$src/third_party/native.toml" "$src/third_party/license-exceptions.toml" "$out/licenses/"
cp "$here/NOTICE.md" "$out/licenses/F-ECO-NOTICE.md"
echo "đã dựng VPdf $repo@$tag (${commit:0:12}) vào $out:" >&2
ls "$out" >&2
