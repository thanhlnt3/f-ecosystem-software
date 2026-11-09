#!/usr/bin/env bash
# Chạy hạ tầng dev trực tiếp trên máy (không cần Docker), dùng cấu hình trong infra/.
# Cần sẵn các chương trình trong PATH, hoặc đặt KRATOS_BIN, HYDRA_BIN, MAILPIT_BIN,
# PG_BIN (thư mục chứa initdb và postgres).
#   scripts/dev-local.sh mailpit   # SMTP 127.0.0.1:1025, web http://127.0.0.1:8025
#   scripts/dev-local.sh kratos    # public 127.0.0.1:4433, admin 127.0.0.1:4434
#   scripts/dev-local.sh hydra     # public 127.0.0.1:4444, admin 127.0.0.1:4445
#   scripts/dev-local.sh postgres  # 127.0.0.1:5432, người dùng feco, CSDL feco
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
case "${1:-}" in
  mailpit)
    exec "${MAILPIT_BIN:-mailpit}" --smtp 127.0.0.1:1025 --listen 127.0.0.1:8025
    ;;
  kratos)
    # Kratos trên Windows không đọc được file:///C:/..., nên cấu hình được sinh lại
    # với mọi tệp tham chiếu nhúng dạng base64:// (scripts/inline-kratos-config.py).
    native_root="$root"
    if command -v cygpath >/dev/null; then native_root="$(cygpath -m "$root")"; fi
    py="$(command -v python3 || command -v python)"
    local_cfg="$native_root/infra/.data/kratos.local.yml"
    "$py" "$native_root/scripts/inline-kratos-config.py" "$native_root/infra/kratos" "$local_cfg"
    export DSN=memory
    export COURIER_SMTP_CONNECTION_URI="smtp://127.0.0.1:1025/?disable_starttls=true"
    exec "${KRATOS_BIN:-kratos}" serve --config "$local_cfg" --dev --watch-courier
    ;;
  hydra)
    native_root="$root"
    if command -v cygpath >/dev/null; then native_root="$(cygpath -m "$root")"; fi
    export DSN=memory
    exec "${HYDRA_BIN:-hydra}" serve all --config "$native_root/infra/hydra/hydra.yml" --dev
    ;;
  postgres)
    # Giống dịch vụ postgres trong infra/docker-compose.yml (mật khẩu chỉ dùng cho dev).
    bin="${PG_BIN:-}"; [ -n "$bin" ] && bin="$bin/"
    data="$root/infra/.data/pg"
    if [ ! -f "$data/PG_VERSION" ]; then
      mkdir -p "$data"
      pwfile="$(mktemp)"; printf 'feco-dev-only' > "$pwfile"
      "${bin}initdb" -D "$data" -U feco --pwfile="$pwfile" -A scram-sha-256 -E UTF8 --locale=C >/dev/null
      rm -f "$pwfile"
      "${bin}pg_ctl" -D "$data" -o "-p 5432 -c listen_addresses=127.0.0.1" -w start >/dev/null
      PGPASSWORD=feco-dev-only "${bin}createdb" -h 127.0.0.1 -U feco feco
      "${bin}pg_ctl" -D "$data" -w stop >/dev/null
    fi
    exec "${bin}postgres" -D "$data" -p 5432 -c listen_addresses=127.0.0.1
    ;;
  *)
    echo "dùng: $0 mailpit|kratos|hydra|postgres" >&2; exit 2
    ;;
esac
