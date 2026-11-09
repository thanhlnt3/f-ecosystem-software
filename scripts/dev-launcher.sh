#!/usr/bin/env bash
# Môi trường dev cho F-Launcher (README, bước 7). Cần Hydra đang chạy (bước 2).
#
#   scripts/dev-launcher.sh setup     # khóa + kho TUF cục bộ, client OAuth của launcher và Demo App
#   scripts/dev-launcher.sh publish   # ký một phiên bản Demo App mới (để thử cập nhật, quay lại)
#   scripts/dev-launcher.sh revoke    # thu hồi phiên bản Demo App mới nhất
#   scripts/dev-launcher.sh refresh   # ký lại timestamp (hết hạn sau 1 giờ)
#   scripts/dev-launcher.sh env       # in biến môi trường cho feco-server và launcher
#
# setup và publish cũng ghi Demo App vào danh mục F-Store (cần PostgreSQL đang chạy).
#
# Mọi thứ nằm trong .dev/ (không commit). Khóa ở đây chỉ dùng cho dev.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && (pwd -W 2>/dev/null || pwd))"
DEV="$ROOT/.dev"
KEYS="$DEV/tuf/keys"
REPO="$DEV/tuf/repo"
HYDRA_ADMIN="${HYDRA_ADMIN:-http://127.0.0.1:4445}"
TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target}"
PY="$(command -v python3 || command -v python)"

exe() { if [[ -f "$1.exe" ]]; then echo "$1.exe"; else echo "$1"; fi; }

build() {
  cargo build -q --manifest-path "$ROOT/Cargo.toml" -p feco-tuf -p feco-demo-app >&2
  cargo build -q --manifest-path "$ROOT/Cargo.toml" -p feco-server --bin feco-catalog >&2
  TUF="$(exe "$TARGET_DIR/debug/feco-tuf")"
  DEMO="$(exe "$TARGET_DIR/debug/feco-demo-app")"
  CATALOG="$(exe "$TARGET_DIR/debug/feco-catalog")"
}

# Ghi vào danh mục F-Store; không chặn nếu PostgreSQL chưa chạy.
catalog() { FECO_ENV="${FECO_ENV:-dev}" "$CATALOG" "$@" || echo "(bỏ qua danh mục: PostgreSQL chưa chạy?)" >&2; }

catalog_app() {
  local json="$DEV/catalog-demo-app.json"
  cat > "$json" <<EOF
{"id":"feco-demo-app","client_id":"$(cat "$DEV/demo-client-id")","name":"Demo App",
 "developer":"F-Eco","developer_verified":true,"first_party":true,
 "summary":"App con mẫu: đăng nhập một lần bằng tài khoản F-Eco và đọc hồ sơ.",
 "description":"Demo App cho thấy cách một app con dùng tài khoản F-Eco: đăng nhập không cần trình duyệt khi được F-Launcher mở, rồi gọi /v1/me.",
 "category":"Công cụ","price_vnd":0,"license":"MIT OR Apache-2.0","screenshots":[]}
EOF
  catalog add-app "$json"
}

platform() {
  local os arch
  case "$(uname -s)" in
    MINGW* | MSYS* | CYGWIN*) os=windows ;;
    Darwin) os=macos ;;
    *) os=linux ;;
  esac
  case "$(uname -m)" in
    x86_64 | amd64) arch=x64 ;;
    arm64 | aarch64) arch=arm64 ;;
    *) arch="$(uname -m)" ;;
  esac
  echo "$os-$arch"
}

k() { echo "$KEYS/$1.pk8"; }
RELEASE_KEYS() { echo "$(k targets),$(k snapshot),$(k timestamp)"; }

create_client() {
  local id body="$DEV/client.json"
  # Gửi qua file UTF-8: trên Windows, tham số dòng lệnh của curl bị đổi sang bảng mã ANSI
  # và làm hỏng chữ có dấu.
  printf '%s' "$1" > "$body"
  id="$(curl -sf -X POST "$HYDRA_ADMIN/admin/clients" -H 'content-type: application/json; charset=utf-8' \
    --data-binary "@$body" | sed -n 's/.*"client_id":"\([^"]*\)".*/\1/p')"
  rm -f "$body"
  [[ -n "$id" ]] || { echo "không tạo được client trong Hydra ($HYDRA_ADMIN)" >&2; exit 1; }
  echo "$id"
}

setup() {
  build
  mkdir -p "$KEYS"
  for name in root-1 root-2 root-3 targets snapshot timestamp; do
    [[ -f "$(k "$name")" ]] || "$TUF" keygen "$(k "$name")" >/dev/null
  done
  if [[ ! -f "$REPO/metadata/1.root.json" ]]; then
    "$TUF" init --repo "$REPO" --root "$(k root-1),$(k root-2),$(k root-3)" --targets "$(k targets)" \
      --snapshot "$(k snapshot)" --timestamp "$(k timestamp)" --sign "$(k root-1),$(k root-2),$(RELEASE_KEYS)"
  fi
  if [[ ! -s "$DEV/launcher-client-id" ]]; then
    create_client '{"client_name":"F-Launcher","grant_types":["authorization_code","refresh_token"],
      "response_types":["code"],"scope":"openid offline_access profile",
      "redirect_uris":["http://127.0.0.1:47201/callback"],"token_endpoint_auth_method":"none"}' \
      > "$DEV/launcher-client-id"
  fi
  if [[ ! -s "$DEV/demo-client-id" ]]; then
    create_client '{"client_name":"Demo App","grant_types":["authorization_code","refresh_token"],
      "response_types":["code"],"scope":"openid offline_access profile email locale phone",
      "redirect_uris":["http://127.0.0.1:9999/cb"],"token_endpoint_auth_method":"none",
      "subject_type":"pairwise","policy_uri":"https://demo.example/privacy",
      "metadata":{"developer":"F-Eco (app mẫu)","scopes":{
        "profile":{"required":true,"purpose_vi":"Hiển thị tên của bạn trong app","purpose_en":"Show your name in the app"},
        "email":{"required":false,"purpose_vi":"Gửi thông báo về tài khoản","purpose_en":"Send account notifications"}}}}' \
      > "$DEV/demo-client-id"
  fi
  catalog_app
  publish
  env_vars
}

publish() {
  build
  local plat version name entry zipfile
  plat="$(platform)"
  version="0.1.$(date +%Y%m%d%H%M%S)"
  name="feco-demo-app-$version-$plat.zip"
  entry="bin/$(basename "$DEMO")"
  zipfile="$DEV/uploads/$name"
  mkdir -p "$DEV/uploads"
  "$PY" -I - "$zipfile" "$DEMO" "$entry" <<'EOF'
import sys, zipfile
out, exe, entry = sys.argv[1:4]
with zipfile.ZipFile(out, "w", zipfile.ZIP_DEFLATED) as z:
    info = zipfile.ZipInfo(entry)
    info.external_attr = 0o755 << 16
    info.compress_type = zipfile.ZIP_DEFLATED
    with open(exe, "rb") as f:
        z.writestr(info, f.read())
EOF
  "$TUF" publish --repo "$REPO" --file "$zipfile" --name "$name" --app feco-demo-app --version "$version" \
    --platform "$plat" --title "Demo App" --entry "$entry" --client-id "$(cat "$DEV/demo-client-id")" \
    --sign "$(RELEASE_KEYS)"
  catalog release feco-demo-app "$version" "$plat" "$(wc -c < "$zipfile" | tr -d ' ')" "$name" \
    "Bản dựng dev lúc $(date '+%H:%M %d/%m/%Y')"
}

revoke() {
  build
  local latest
  latest="$(ls "$DEV/uploads" | sort | tail -1)"
  "$TUF" revoke --repo "$REPO" --name "$latest" --reason "${1:-Diễn tập thu hồi}" --sign "$(RELEASE_KEYS)"
  local version="${latest#feco-demo-app-}"
  catalog revoke feco-demo-app "${version%-*-*.zip}"
}

refresh() {
  build
  "$TUF" refresh --repo "$REPO" --sign "$(k snapshot),$(k timestamp)"
}

env_vars() {
  local repo_native="$REPO"
  command -v cygpath >/dev/null && repo_native="$(cygpath -w "$REPO")"
  cat <<EOF

# feco-server (thêm vào lệnh chạy ở bước 3):
export FECO_LAUNCHER_CLIENT_ID=$(cat "$DEV/launcher-client-id")

# F-Launcher:
export FECO_LAUNCHER_CLIENT_ID=$(cat "$DEV/launcher-client-id")
export FECO_LAUNCHER_TUF='$repo_native'
cargo run -p feco-launcher
EOF
}

case "${1:-}" in
  setup) setup ;;
  publish) publish ;;
  revoke) revoke "${2:-}" ;;
  refresh) refresh ;;
  env) env_vars ;;
  *) sed -n '2,10p' "$0"; exit 2 ;;
esac
