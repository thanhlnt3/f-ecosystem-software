#!/usr/bin/env bash
# Dựng app desktop Mentor cho Windows từ mã gốc và bản vá F-Eco (thay đổi mentor-integration).
#
#   integrations/mentor/build-desktop.sh <thư-mục-ra> <phiên-bản-F-Eco>
#
# 0. Quét Mentor tại đúng commit bằng scripts/scan-integration.sh (spec source-scan); dừng nếu có
#    phát hiện critical chưa được cho phép trong scan-allow.toml.
# 1. Tải Mentor đúng commit trong MENTOR_COMMIT, áp mentor-gateway.patch và mentor-app.patch.
# 2. Chạy test của gateway (pytest) và của app (vitest), trừ khi MENTOR_SKIP_TESTS=1.
# 3. Dựng app Tauri (--no-bundle) với VITE_API_BASE (mặc định http://localhost:8100/api) và
#    tauri.feco.conf.json (bỏ địa chỉ cập nhật của Mentor: F-Launcher cập nhật qua TUF).
# 4. Đóng gói Mentor.exe, NOTICE.md, SOURCE.txt thành <thư-mục-ra>/mentor-<phiên-bản>-windows-x64.zip.
#
# Cần: git, curl, uv, Node 22 + pnpm 9, Rust stable, Python 3.11+. Chạy trên Windows (CI: windows-latest).
set -euo pipefail

out="$1"
version="$2"
here="$(cd "$(dirname "$0")" && pwd)"
commit="$(tr -d '\r\n' < "$here/MENTOR_COMMIT")"
repo="katD2707/agent-mentor-maximum-experience"
work="${MENTOR_WORK:-${RUNNER_TEMP:-/tmp}/mentor-src}"
# Trên Windows, RUNNER_TEMP là đường dẫn có ổ đĩa (D:...): đổi sang dạng POSIX, vì tar hiểu "D:" là máy từ xa.
command -v cygpath >/dev/null && work="$(cygpath -u "$work")"
mkdir -p "$out"
out="$(cd "$out" && pwd)"

# Quét trước khi dựng: set -e dừng script khi quét thất bại (mã 2: có phát hiện critical mới).
"$here/../../scripts/scan-integration.sh" "$repo" "$commit" "$here/scan-allow.toml"

rm -rf "$work"
mkdir -p "$work"
cd "$work"
curl -sSfL "https://codeload.github.com/$repo/tar.gz/$commit" | tar -xz --strip-components=1
git apply "$here/mentor-gateway.patch"
git apply "$here/mentor-app.patch"

if [[ "${MENTOR_SKIP_TESTS:-}" != 1 ]]; then
  uv sync --frozen --all-packages --python 3.11
  uv run --frozen pytest services/gateway -q -p no:cacheprovider
fi

pnpm -C apps/app install --frozen-lockfile
if [[ "${MENTOR_SKIP_TESTS:-}" != 1 ]]; then
  pnpm -C apps/app test
fi

cp "$here/tauri.feco.conf.json" apps/app/src-tauri/tauri.feco.conf.json
(
  cd apps/app
  VITE_API_BASE="${MENTOR_API_BASE:-http://localhost:8100/api}" VITE_API_MOCK="" \
    pnpm tauri build --no-bundle --config src-tauri/tauri.feco.conf.json
)

pkg="$work/feco-pkg"
rm -rf "$pkg"
mkdir -p "$pkg"
cp apps/app/src-tauri/target/release/mentor-app.exe "$pkg/Mentor.exe"
cp "$here/NOTICE.md" "$pkg/NOTICE.md"
cp THIRD_PARTY_NOTICES.md "$pkg/THIRD_PARTY_NOTICES.md"
printf 'Mentor: https://github.com/%s tại commit %s\nĐã áp bản vá integrations/mentor của F-Eco %s (đăng nhập bằng F-Eco).\nApp gọi gateway Mentor ở %s.\n' \
  "$repo" "$commit" "$version" "${MENTOR_API_BASE:-http://localhost:8100/api}" > "$pkg/SOURCE.txt"
python -I - "$pkg" "$out/mentor-$version-windows-x64.zip" <<'EOF'
import os, sys, zipfile
src, dst = sys.argv[1:3]
with zipfile.ZipFile(dst, "w", zipfile.ZIP_DEFLATED) as z:
    for name in sorted(os.listdir(src)):
        z.write(os.path.join(src, name), name)
print(dst)
EOF
