#!/usr/bin/env bash
# Môi trường dev cho F-Launcher (README, bước 7). Cần Hydra đang chạy (bước 2).
#
#   scripts/dev-launcher.sh setup     # khóa + kho TUF cục bộ, client OAuth của launcher, của Demo
#                                     # App (thử đăng nhập một lần, README bước 5) và app mẫu F-SDK
#   scripts/dev-launcher.sh publish   # ký phiên bản mới của app mẫu Ghi chú (thử cập nhật, quay lại)
#   scripts/dev-launcher.sh revoke    # thu hồi phiên bản Ghi chú mới nhất
#   scripts/dev-launcher.sh remove-demo  # gỡ Demo App khỏi kho TUF và danh mục (môi trường dev cũ)
#   scripts/dev-launcher.sh refresh   # ký lại timestamp (hết hạn sau 1 giờ)
#   scripts/dev-launcher.sh env       # in biến môi trường cho feco-server, launcher và app mẫu F-SDK
#
# Demo App không còn nằm trong kho ứng dụng (thay đổi platform-roles); thêm app vào kho bằng
# scripts/dev-apps.sh. publish, revoke cũng ghi danh mục F-Store (cần PostgreSQL đang chạy).
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
  cargo build -q --manifest-path "$ROOT/Cargo.toml" -p feco-tuf >&2
  cargo build -q --manifest-path "$ROOT/Cargo.toml" -p feco-server --bin feco-catalog >&2
  TUF="$(exe "$TARGET_DIR/debug/feco-tuf")"
  CATALOG="$(exe "$TARGET_DIR/debug/feco-catalog")"
}

# Ghi vào danh mục F-Store; không chặn nếu PostgreSQL chưa chạy.
catalog() { FECO_ENV="${FECO_ENV:-dev}" "$CATALOG" "$@" || echo "(bỏ qua danh mục: PostgreSQL chưa chạy?)" >&2; }

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

# Tạo client nếu file chưa có client ID, hoặc client đó không còn trong Hydra (Hydra dev giữ
# dữ liệu trong bộ nhớ nên khởi động lại là mất).
ensure_client() {
  local file="$DEV/$1" id
  id="$(cat "$file" 2>/dev/null || true)"
  if [[ -n "$id" ]] && curl -sf -o /dev/null "$HYDRA_ADMIN/admin/clients/$id"; then
    return
  fi
  create_client "$2" > "$file"
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
  ensure_client launcher-client-id '{"client_name":"F-Launcher","grant_types":["authorization_code","refresh_token"],
      "response_types":["code"],"scope":"openid offline_access profile",
      "redirect_uris":["http://127.0.0.1:47201/callback"],"token_endpoint_auth_method":"none"}'
  ensure_client demo-client-id '{"client_name":"Demo App","grant_types":["authorization_code","refresh_token"],
      "response_types":["code"],"scope":"openid offline_access profile email locale phone",
      "redirect_uris":["http://127.0.0.1:9999/cb"],"token_endpoint_auth_method":"none",
      "subject_type":"pairwise","policy_uri":"https://demo.example/privacy",
      "metadata":{"developer":"F-Eco (app mẫu)","scopes":{
        "profile":{"required":true,"purpose_vi":"Hiển thị tên của bạn trong app","purpose_en":"Show your name in the app"},
        "email":{"required":false,"purpose_vi":"Gửi thông báo về tài khoản","purpose_en":"Send account notifications"}}}}'
  # App mẫu F-SDK (sdk/*/examples): nhận kết quả đăng nhập ở địa chỉ loopback mặc định của F-SDK.
  ensure_client sdk-client-id '{"client_name":"QuanLyKho (app mẫu F-SDK)","grant_types":["authorization_code","refresh_token"],
      "response_types":["code"],"scope":"openid offline_access profile email purchases",
      "redirect_uris":["http://127.0.0.1:47280/callback"],"token_endpoint_auth_method":"none",
      "subject_type":"pairwise","policy_uri":"https://demo.example/privacy",
      "metadata":{"developer":"F-Eco (app mẫu)","scopes":{
        "profile":{"required":true,"purpose_vi":"Hiển thị tên của bạn trong app","purpose_en":"Show your name in the app"},
        "purchases":{"required":true,"purpose_vi":"Kiểm tra bạn có quyền dùng app","purpose_en":"Check that you may use the app"}}}}'
  echo "Thêm app vào kho bằng scripts/dev-apps.sh all (app mẫu và app mã nguồn mở từ GitHub)." >&2
  env_vars
}

publish() {
  # Một phiên bản mới của app mẫu Ghi chú (số phiên bản theo thời điểm dựng).
  "$(dirname "${BASH_SOURCE[0]}")/dev-apps.sh" samples
}

revoke() {
  build
  local latest
  latest="$(ls "$DEV/uploads" | grep '^f-notes-' | sort | tail -1)"
  [[ -n "$latest" ]] || { echo "chưa có phiên bản Ghi chú nào: chạy publish trước" >&2; exit 1; }
  "$TUF" revoke --repo "$REPO" --name "$latest" --reason "${1:-Diễn tập thu hồi}" --sign "$(RELEASE_KEYS)"
  local version="${latest#f-notes-}"
  catalog revoke f-notes "${version%-*-*.zip}"
}

# Gỡ Demo App khỏi kho TUF (không thu hồi: máy đã cài vẫn mở được) và khỏi danh mục F-Store.
remove_demo() {
  build
  "$TUF" remove --repo "$REPO" --app feco-demo-app --sign "$(RELEASE_KEYS)" || true
  catalog remove feco-demo-app "Gỡ app thử nghiệm khỏi kho ứng dụng"
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

# App mẫu F-SDK (README, bước 8):
export FECO_CLIENT_ID=$(cat "$DEV/sdk-client-id")
export FECO_ISSUER_URL=http://127.0.0.1:4444/
export FECO_SERVER_URL=http://127.0.0.1:8080/
export FSDK_SCOPES="openid offline_access profile purchases"
EOF
}

# `source scripts/dev-launcher.sh` (ví dụ từ scripts/dev-apps.sh) chỉ nạp các hàm trên.
[[ "${BASH_SOURCE[0]}" != "$0" ]] && return 0

case "${1:-}" in
  setup) setup ;;
  publish) publish ;;
  remove-demo) remove_demo ;;
  revoke) revoke "${2:-}" ;;
  refresh) refresh ;;
  env) env_vars ;;
  *) sed -n '2,13p' "$0"; exit 2 ;;
esac
