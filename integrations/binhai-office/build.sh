#!/usr/bin/env bash
# Dựng BinhAI Office từ mã nguồn tại commit trong README.md (bảng "Mã nguồn") rồi chép bản dựng ra
# THƯ-MỤC-RA (thay đổi binhai-office-integration). Không dùng app/dist dựng sẵn của tác giả; chỉ
# so với nó để biết bản dựng sẵn có khớp mã nguồn.
#
#   integrations/binhai-office/build.sh THƯ-MỤC-RA [--verify-dist]
#
# --verify-dist: thất bại nếu tệp nào trong bản tự dựng khác app/dist của repo (cùng tên, cùng
# SHA-256). Thư viện npm được cài theo package-lock.json với --ignore-scripts: không chạy script
# cài đặt nào của thư viện bên thứ ba.
set -euo pipefail

out="${1:?cách dùng: $0 THƯ-MỤC-RA [--verify-dist]}"
verify="${2:-}"
here="$(cd "$(dirname "$0")" && pwd)"
read -r repo commit <<< "$(sed -nE 's#.*https://github\.com/([A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+) \| `([0-9a-f]{40})`.*#\1 \2#p' "$here/README.md" | head -1)"
[[ -n "${commit:-}" ]] || { echo "không đọc được repo và commit từ README.md" >&2; exit 1; }

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
git -C "$work" init -q src
# Giữ nguyên kết thúc dòng (Git trên Windows mặc định đổi LF thành CRLF, làm lệch SHA-256 khi so).
git -C "$work/src" config core.autocrlf false
git -C "$work/src" fetch -q --depth 1 "https://github.com/$repo.git" "$commit"
git -C "$work/src" checkout -q FETCH_HEAD
web="$work/src/app/apps/web"
cp -R "$work/src/app/dist" "$work/author-dist"
(cd "$web" && npm ci --ignore-scripts --no-audit --no-fund && npm run build)

if [[ "$verify" == "--verify-dist" ]]; then
  # Bản dựng của Vite có tên tệp chứa mã băm nội dung: so cả danh sách tệp lẫn SHA-256.
  sums() { (cd "$1" && find . -type f -print0 | sort -z | xargs -0 sha256sum); }
  if ! diff <(sums "$work/author-dist") <(sums "$work/src/app/dist"); then
    echo "bản tự dựng khác app/dist của tác giả (xem các dòng trên)" >&2
    exit 3
  fi
  echo "bản tự dựng khớp app/dist của tác giả" >&2
fi

rm -rf "$out"
mkdir -p "$out"
cp -R "$work/src/app/dist/." "$out/"
echo "đã dựng BinhAI Office $repo@${commit:0:12} vào $out" >&2
