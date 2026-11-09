#!/usr/bin/env bash
# Diễn tập kỹ thuật chuỗi tin cậy TUF (tasks.md 5.1–5.3) bằng công cụ feco-tuf, trên một kho tạm
# với khóa tạm (xóa khi xong). In biên bản dạng Markdown ra stdout; dừng với mã lỗi nếu có bước
# không đúng kết quả mong đợi.
#
#   scripts/tuf-drill.sh > docs/security/drills/<ngày>-tuf-drill.md
#
# Đặt FECO_TUF_BIN để dùng một bản feco-tuf đã build sẵn.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
if [[ -z "${FECO_TUF_BIN:-}" ]]; then
  cargo build -q -p feco-tuf --bin feco-tuf --manifest-path "$ROOT/Cargo.toml" >&2
  FECO_TUF_BIN="${CARGO_TARGET_DIR:-$ROOT/target}/debug/feco-tuf"
  [[ -f "$FECO_TUF_BIN.exe" ]] && FECO_TUF_BIN="$FECO_TUF_BIN.exe"
fi

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
cd "$WORK"

tuf() { "$FECO_TUF_BIN" "$@"; }

# step ok|fail <mô tả> <tham số feco-tuf...>
step() {
  local expect="$1" title="$2"
  shift 2
  local out code
  set +e
  out="$(tuf "$@" 2>&1)"
  code=$?
  set -e
  echo
  echo "**$title**"
  echo
  echo '```text'
  echo "\$ feco-tuf $*"
  echo "$out"
  echo '```'
  if [[ "$expect" == ok && $code -ne 0 ]] || [[ "$expect" == fail && $code -eq 0 ]]; then
    echo "KẾT QUẢ SAI ở bước \"$title\": mong đợi $expect, mã thoát $code" >&2
    exit 1
  fi
  if [[ $code -eq 0 ]]; then echo "Kết quả: thành công, đúng mong đợi."; else echo "Kết quả: bị từ chối, đúng mong đợi."; fi
}

k() { echo "keys/$1.pk8"; }
RELEASE="$(k targets),$(k snapshot),$(k timestamp)"
ONLINE="$(k snapshot),$(k timestamp)"

echo "# Biên bản diễn tập chuỗi tin cậy TUF"
echo
echo "- Thời điểm: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
REV="$(git -C "$ROOT" rev-parse --short HEAD 2>/dev/null || echo 'không rõ')"
[[ -n "$(git -C "$ROOT" status --porcelain 2>/dev/null)" ]] && REV="$REV (cộng thay đổi chưa commit)"
echo "- Mã nguồn: $REV"
echo "- Công cụ: \`feco-tuf\` (crate \`crates/feco-tuf\`, xác minh bằng \`tough\`)"
echo "- Loại: diễn tập kỹ thuật với khóa tạm sinh ra trong thư mục tạm và bị xóa khi xong."
echo
echo "## Khóa dùng trong buổi diễn tập"
echo
echo "| Vai trò | Key ID |"
echo "|---|---|"
for name in root-1 root-2 root-3 root-4 targets targets-2 snapshot timestamp; do
  echo "| $name | \`$(tuf keygen "$(k "$name")")\` |"
done

echo
echo "## 1. Dựng kho: root ngưỡng 2/3"
step fail "Chỉ 1 người giữ khóa root ký" init --repo other \
  --root "$(k root-1),$(k root-2),$(k root-3)" --targets "$(k targets)" --snapshot "$(k snapshot)" \
  --timestamp "$(k timestamp)" --sign "$(k root-1),$RELEASE"
step ok "2 trong 3 người giữ khóa root ký" init --repo repo \
  --root "$(k root-1),$(k root-2),$(k root-3)" --targets "$(k targets)" --snapshot "$(k snapshot)" \
  --timestamp "$(k timestamp)" --sign "$(k root-1),$(k root-2),$RELEASE"

echo
echo "## 2. Ký phiên bản đã duyệt và xác minh ở launcher"
printf 'SoundLab 1.2.0' > soundlab-1.2.0.zip
printf 'SoundLab 1.3.0' > soundlab-1.3.0.zip
step ok "Ký SoundLab 1.2.0" publish --repo repo --file soundlab-1.2.0.zip \
  --name soundlab-1.2.0-windows-x64.zip --app soundlab --version 1.2.0 --platform windows-x64 --sign "$RELEASE"
mkdir -p launcher
step ok "Launcher đồng bộ (chỉ tin 1.root.json đi kèm bộ cài)" status --repo repo --datastore launcher

echo
echo "## 3. Xoay khóa targets"
step ok "Phát hành root mới đổi khóa targets (root-1, root-3 ký)" rotate --repo repo --role targets \
  --keys "$(k targets-2)" --sign "$(k root-1),$(k root-3),$(k targets-2),$ONLINE"
step fail "Khóa targets cũ không ký được nữa" publish --repo repo --file soundlab-1.3.0.zip \
  --name soundlab-1.3.0-windows-x64.zip --app soundlab --version 1.3.0 --platform windows-x64 --sign "$RELEASE"
step ok "Khóa targets mới ký được" publish --repo repo --file soundlab-1.3.0.zip \
  --name soundlab-1.3.0-windows-x64.zip --app soundlab --version 1.3.0 --platform windows-x64 \
  --sign "$(k targets-2),$ONLINE"
step ok "Launcher đang chạy theo root mới" status --repo repo --datastore launcher

echo
echo "## 4. Xoay khóa root: root-3 bị lộ, thay bằng root-4"
step fail "Chỉ 1 khóa root hiện tại ký (root-1 và root-4 mới)" rotate --repo repo --role root \
  --keys "$(k root-1),$(k root-2),$(k root-4)" --sign "$(k root-1),$(k root-4),$(k targets-2),$ONLINE"
step ok "2 khóa root hiện tại ký (root-1, root-2) cùng root-4" rotate --repo repo --role root \
  --keys "$(k root-1),$(k root-2),$(k root-4)" --sign "$(k root-1),$(k root-2),$(k root-4),$(k targets-2),$ONLINE"
step fail "root-3 đã bị loại không còn được tính (root-2 + root-3)" rotate --repo repo --role timestamp \
  --keys "$(k timestamp)" --sign "$(k root-2),$(k root-3),$(k targets-2),$ONLINE"
step ok "Launcher đang chạy theo root mới" status --repo repo --datastore launcher
step ok "Launcher mới cài đi theo chuỗi root 1 → 3" status --repo repo

echo
echo "## 5. Thu hồi và làm mới timestamp"
step ok "Thu hồi SoundLab 1.2.0" revoke --repo repo --name soundlab-1.2.0-windows-x64.zip \
  --reason "Diễn tập thu hồi" --sign "$(k targets-2),$ONLINE"
step ok "Ký lại snapshot và timestamp chỉ bằng khóa trực tuyến" refresh --repo repo --sign "$ONLINE"
step ok "Gia hạn targets bằng khóa targets mới" resign --repo repo --sign "$(k targets-2),$ONLINE"
step ok "Launcher thấy bản thu hồi ở lần đồng bộ kế tiếp" status --repo repo --datastore launcher

echo
echo "## Kết luận"
echo
echo "Mọi bước cho kết quả đúng mong đợi."
