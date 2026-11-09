#!/usr/bin/env bash
# Chạy Kratos và Mailpit trực tiếp trên máy (không cần Docker), dùng cấu hình
# trong infra/. Cần sẵn kratos và mailpit trong PATH hoặc đặt KRATOS_BIN, MAILPIT_BIN.
#   scripts/dev-local.sh mailpit   # SMTP 127.0.0.1:1025, web http://127.0.0.1:8025
#   scripts/dev-local.sh kratos    # public 127.0.0.1:4433, admin 127.0.0.1:4434
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
  *)
    echo "dùng: $0 mailpit|kratos" >&2; exit 2
    ;;
esac
