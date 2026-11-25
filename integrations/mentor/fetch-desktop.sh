#!/usr/bin/env bash
# Lấy gói desktop Mentor đã ghim trong DESKTOP_PACKAGE (thay đổi mentor-desktop-0.2.4).
#
#   integrations/mentor/fetch-desktop.sh <thư-mục-ra>
#
# 1. Quét mã nguồn Mentor tại commit của tag bằng scripts/scan-integration.sh (spec source-scan);
#    dừng nếu có phát hiện critical chưa được cho phép trong scan-allow.toml.
# 2. Tải gói, so SHA-256 với giá trị đã ghim; lệch thì dừng.
# 3. Kiểm gói: Mentor.exe ở gốc, có gateway/mentor-gateway.exe, không có đường dẫn tuyệt đối hay "..".
# Kết quả: <thư-mục-ra>/mentor-<phiên bản>-windows-x64.zip (giữ nguyên tệp của tác giả).
set -euo pipefail

out="$1"
here="$(cd "$(dirname "$0")" && pwd)"
field() { sed -n "s/^$1=//p" "$here/DESKTOP_PACKAGE" | tr -d '\r\n'; }
version="$(field version)"; commit="$(field commit)"; url="$(field url)"; sha="$(field sha256)"
[[ -n "$version" && -n "$commit" && -n "$url" && ${#sha} -eq 64 ]] || { echo "DESKTOP_PACKAGE thiếu trường" >&2; exit 1; }

"$here/../../scripts/scan-integration.sh" katD2707/agent-mentor-maximum-experience "$commit" "$here/scan-allow.toml"

mkdir -p "$out"
zip="$(cd "$out" && pwd)/mentor-$version-windows-x64.zip"
curl -sSfL -o "$zip" "$url"
got="$( (sha256sum "$zip" 2>/dev/null || shasum -a 256 "$zip") | cut -c1-64)"
if [[ "$got" != "$sha" ]]; then
  echo "SHA-256 không khớp: $got (ghim $sha)" >&2
  rm -f "$zip"
  exit 1
fi
py="$(command -v python3 || command -v python)"
PYTHONIOENCODING=utf-8 "$py" -X utf8 -I - "$zip" <<'PYEOF'
import sys, zipfile
names = zipfile.ZipFile(sys.argv[1]).namelist()
bad = [n for n in names if n.startswith(("/", "\\")) or ".." in n.replace("\\", "/").split("/") or ":" in n]
assert not bad, f"đường dẫn không an toàn: {bad[:3]}"
assert "Mentor.exe" in names, "thiếu Mentor.exe ở gốc gói"
assert "gateway/mentor-gateway.exe" in names, "thiếu gateway/mentor-gateway.exe"
print(f"gói Mentor hợp lệ: {len(names)} tệp")
PYEOF
echo "$zip"
