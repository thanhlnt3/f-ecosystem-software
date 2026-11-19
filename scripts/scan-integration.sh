#!/usr/bin/env bash
# Quét repo bên ngoài tại đúng commit trước khi viết bản vá hay dựng (thay đổi chat-and-source-scan,
# design S3, spec source-scan "Quét trước khi tích hợp app bên ngoài").
#
#   scripts/scan-integration.sh <owner/repo> <commit> [allow.toml]
#
# 1. Dựng feco-codescan (cargo build --release), trừ khi FECO_CODESCAN trỏ tới lệnh đã dựng sẵn.
# 2. Quét github:<owner/repo>@<commit>: tải zip từ codeload.github.com, tra lỗ hổng thư viện trên
#    OSV.dev (chỉ gửi hệ sinh thái, tên và phiên bản thư viện).
# 3. Ghi scan-report.json và scan-report.md vào .dev/scans/<repo>-<commit>/.
# 4. Thoát mã 2 nếu có phát hiện critical chưa nằm trong allow.toml (scan-allow.toml của app).
#
# Biến môi trường: FECO_CODESCAN, FECO_OSV=off (không tra OSV), FECO_OSV_URL, CARGO_TARGET_DIR,
# CARGO_NET_OFFLINE=true (dựng không cần mạng).
set -euo pipefail

if [[ $# -lt 2 || $# -gt 3 ]]; then
  echo "cách dùng: $0 <owner/repo> <commit> [allow.toml]" >&2
  exit 1
fi
repo="$1"
commit="$2"
allow="${3:-}"
root="$(cd "$(dirname "$0")/.." && pwd)"

if [[ ! "$repo" =~ ^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$ ]]; then
  echo "repo phải có dạng owner/repo: $repo" >&2
  exit 1
fi
if [[ ! "$commit" =~ ^[0-9a-f]{40}$ ]]; then
  echo "commit phải đủ 40 ký tự hex: $commit" >&2
  exit 1
fi

bin="${FECO_CODESCAN:-}"
if [[ -z "$bin" ]]; then
  cargo build --release -q -p feco-codescan --manifest-path "$root/Cargo.toml" >&2
  target="${CARGO_TARGET_DIR:-$root/target}"
  bin="$target/release/feco-codescan"
  [[ -x "$bin" ]] || bin="$bin.exe"
fi

out="$root/.dev/scans/${repo#*/}-$commit"
mkdir -p "$out"
args=("github:$repo@$commit" --json "$out/scan-report.json" --markdown "$out/scan-report.md" --fail-on critical)
if [[ -n "$allow" ]]; then
  args+=(--allow "$allow")
fi

set +e
"$bin" "${args[@]}"
code=$?
set -e
echo "báo cáo: $out/scan-report.md" >&2
case "$code" in
  0) ;;
  2) echo "$repo@$commit có phát hiện critical chưa được cho phép (xem báo cáo)" >&2 ;;
  *) echo "quét $repo@$commit lỗi (mã $code)" >&2 ;;
esac
exit "$code"
