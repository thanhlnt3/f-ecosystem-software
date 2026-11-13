#!/usr/bin/env bash
# Đóng gói một bản phát hành F-Eco cho một nền tảng, từ các tệp đã dựng bằng
#   cargo build --release --locked -p feco-launcher -p feco-server -p f-dev -p feco-tuf -p fsdk-c
# Kết quả: dist/feco-<phiên bản>-<target>.zip gồm chương trình, F-SDK, tài liệu, giấy phép,
# THIRD_PARTY_NOTICES và SBOM (CycloneDX) của từng sản phẩm.
#
#   scripts/package-release.sh x86_64-pc-windows-msvc [thư mục target]
set -euo pipefail
target="${1:?cần target triple, ví dụ x86_64-pc-windows-msvc}"
root="$(cd "$(dirname "$0")/.." && pwd)"
build="${2:-${CARGO_TARGET_DIR:-$root/target}}/release"
version="$(sed -n 's/^version = "\(.*\)"/\1/p' "$root/Cargo.toml" | head -1)"
name="feco-$version-$target"
out="$root/dist/$name"
py="$(command -v python3 || command -v python)"

case "$target" in
  *windows*) exe=".exe"; lib="fsdk_c.dll"; extra_lib="fsdk_c.dll.lib" ;;
  *apple*) exe=""; lib="libfsdk_c.dylib"; extra_lib="" ;;
  *) exe=""; lib="libfsdk_c.so"; extra_lib="" ;;
esac

rm -rf "$out" "$out.zip"
mkdir -p "$out/bin" "$out/sdk/c/include" "$out/sdk/c/lib" "$out/docs" "$out/notices" "$out/sbom"

for bin in feco-launcher feco-server feco-catalog f-dev feco-tuf; do
  cp "$build/$bin$exe" "$out/bin/"
done

# F-SDK: C ABI (header + thư viện), Python (gọi qua C ABI), TypeScript, mã nguồn Rust.
cp "$root/sdk/c/include/fsdk.h" "$out/sdk/c/include/"
cp "$build/$lib" "$out/sdk/c/lib/"
[ -n "$extra_lib" ] && [ -f "$build/$extra_lib" ] && cp "$build/$extra_lib" "$out/sdk/c/lib/"
cp -r "$root/sdk/c/examples" "$out/sdk/c/"
mkdir -p "$out/sdk/python" "$out/sdk/typescript" "$out/sdk/rust"
cp -r "$root/sdk/python/fsdk" "$root/sdk/python/examples" "$root/sdk/python/pyproject.toml" "$out/sdk/python/"
cp -r "$root/sdk/typescript/src" "$root/sdk/typescript/examples" "$root/sdk/typescript/package.json" "$out/sdk/typescript/"
cp -r "$root/sdk/rust/src" "$root/sdk/rust/Cargo.toml" "$out/sdk/rust/"
find "$out/sdk" -name "__pycache__" -type d -prune -exec rm -rf {} +

# Tài liệu, cấu hình hạ tầng mẫu cho máy chủ, giấy phép.
cp "$root/docs/integration-guide.md" "$root/docs/catalog-app.example.json" "$root/docs/order-quote.example.json" "$out/docs/"
cp "$root/README.md" "$root/CHANGELOG.md" "$root/LICENSE-MIT" "$root/LICENSE-APACHE" "$out/"
mkdir -p "$out/infra"
cp -r "$root/infra/kratos" "$root/infra/hydra" "$root/infra/docker-compose.yml" "$out/infra/"
rm -rf "$out/infra/.data"

# THIRD_PARTY_NOTICES và SBOM theo cây phụ thuộc thật của từng sản phẩm (task 15.5).
sbom() { cargo run -q --locked -p feco-license-check --bin feco-sbom -- "$@"; }
for pkg in feco-launcher feco-server f-dev fsdk fsdk-c; do
  sbom "$pkg" "$target" "$out/sbom"
done
if [ "${FECO_SBOM_FBUILDER:-1}" = "1" ] && [ -f "$root/clients/f-builder/Cargo.toml" ]; then
  sbom f-builder "$target" "$out/sbom" "$root/clients/f-builder/Cargo.toml"
fi
mv "$out"/sbom/*THIRD_PARTY_NOTICES.md "$out/notices/"

(cd "$root/dist" && "$py" -m zipfile -c "$name.zip" "$name")
echo "$root/dist/$name.zip"
