#!/usr/bin/env bash
# Quét lại mọi app bên ngoài đã tích hợp ở đúng commit đã ghi (thay đổi chat-and-source-scan, design
# S3; dependency-rescan, design R5):
# - Mentor: commit trong integrations/mentor/MENTOR_COMMIT;
# - ThesisAI: repo và commit trong bảng của integrations/thesisai/README.md;
# - BinhAI Office: repo và commit trong bảng của integrations/binhai-office/README.md.
# Thoát mã khác 0 nếu một lần quét có phát hiện critical chưa được cho phép trong
# integrations/<app>/scan-allow.toml, hoặc quét lỗi. Báo cáo ở .dev/scans/*/scan-report.md.
# Dùng trong job integration-scan của ci.yml và workflow integration-scan.yml (hằng tuần).
set -u
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root" || exit 1
status=0
scripts/scan-integration.sh katD2707/agent-mentor-maximum-experience \
  "$(tr -d '\r\n' < integrations/mentor/MENTOR_COMMIT)" integrations/mentor/scan-allow.toml || status=1
targets="$(sed -nE 's#.*https://github\.com/([A-Za-z0-9_.-]+/thesisai-(fe|be)) \| `([0-9a-f]{40})`.*#\1 \3#p' integrations/thesisai/README.md)"
if [ "$(printf '%s\n' "$targets" | grep -c .)" != 2 ]; then
  echo "không đọc được repo và commit ThesisAI từ README" >&2
  exit 1
fi
while read -r repo commit; do
  scripts/scan-integration.sh "$repo" "$commit" integrations/thesisai/scan-allow.toml || status=1
done <<< "$targets"
read -r repo commit <<< "$(sed -nE 's#.*https://github\.com/([A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+) \| `([0-9a-f]{40})`.*#\1 \2#p' integrations/binhai-office/README.md | head -1)"
if [ -z "${commit:-}" ]; then
  echo "không đọc được repo và commit BinhAI Office từ README" >&2
  exit 1
fi
scripts/scan-integration.sh "$repo" "$commit" integrations/binhai-office/scan-allow.toml || status=1
exit "$status"
