#!/usr/bin/env bash
# Diễn tập sao lưu và khôi phục cơ sở dữ liệu (spec platform-support "Sao lưu và khôi phục";
# tasks.md 15.3). Chạy trên một cụm Postgres tạm, không đụng tới cơ sở dữ liệu đang dùng.
#
#   PG_BIN=<thư mục chứa initdb, pg_ctl, psql...> scripts/backup-drill.sh [báo-cáo.md]
#
# Các bước:
#   1. Tạo cụm "chính" có lưu trữ WAL liên tục (archive_mode, archive_timeout = ARCHIVE_TIMEOUT
#      giây; production dùng 300 giây cho RPO 5 phút), nạp bản sao dữ liệu dev (pg_dump của CSDL
#      `feco` nếu có, không thì dữ liệu mẫu).
#   2. Sao lưu nền (pg_basebackup), rồi tiếp tục ghi dữ liệu mới.
#   3. Mô phỏng sự cố: dừng ngay cụm chính (immediate), bỏ mọi WAL chưa kịp lưu trữ.
#   4. Khôi phục vào thư mục mới từ bản sao lưu nền + WAL đã lưu trữ, đo thời gian.
#   5. Kiểm tra dữ liệu: số dòng các bảng, dòng đánh dấu cuối cùng còn lại, khoảng dữ liệu mất.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
bin="${PG_BIN:-}"; [ -n "$bin" ] && bin="$bin/"
report="${1:-}"
ARCHIVE_TIMEOUT="${ARCHIVE_TIMEOUT:-60}"
PRIMARY_PORT=55432
RESTORE_PORT=55433
work="$(mktemp -d)"
native() { if command -v cygpath >/dev/null; then cygpath -w "$1"; else echo "$1"; fi; }
psqlp() { "${bin}psql" -X -q -v ON_ERROR_STOP=1 -h 127.0.0.1 -U drill "$@"; }
now_ms() { date +%s%3N; }
cleanup() {
  "${bin}pg_ctl" -D "$work/primary" -m immediate stop >/dev/null 2>&1 || true
  "${bin}pg_ctl" -D "$work/restored" -m fast stop >/dev/null 2>&1 || true
  rm -rf "$work"
}
trap cleanup EXIT

mkdir -p "$work/archive"
archive_native="$(native "$work/archive")"
if [[ "$archive_native" == *\\* ]]; then
  # Windows: copy của cmd.
  archive_cmd="copy \"%p\" \"${archive_native//\\/\\\\}\\\\%f\""
  restore_cmd="copy \"${archive_native//\\/\\\\}\\\\%f\" \"%p\""
else
  archive_cmd="test ! -f '$work/archive/%f' && cp '%p' '$work/archive/%f'"
  restore_cmd="cp '$work/archive/%f' '%p'"
fi

echo "1. Tạo cụm chính có lưu trữ WAL (archive_timeout = ${ARCHIVE_TIMEOUT} giây)"
"${bin}initdb" -D "$work/primary" -U drill -A trust -E UTF8 --locale=C >/dev/null
cat >> "$work/primary/postgresql.conf" <<EOF
port = $PRIMARY_PORT
listen_addresses = '127.0.0.1'
wal_level = replica
archive_mode = on
archive_command = '$archive_cmd'
archive_timeout = $ARCHIVE_TIMEOUT
EOF
"${bin}pg_ctl" -D "$work/primary" -w -l "$work/primary.log" start >/dev/null
psqlp -p $PRIMARY_PORT -d postgres -c "CREATE DATABASE feco"
source_note="dữ liệu mẫu"
if PGPASSWORD="${FECO_DB_PASSWORD:-feco-dev-only}" "${bin}pg_dump" -h 127.0.0.1 -p 5432 -U feco -d feco \
     --no-owner --no-privileges -f "$work/feco.sql" 2>/dev/null; then
  psqlp -p $PRIMARY_PORT -d feco -f "$work/feco.sql" >/dev/null
  source_note="bản sao CSDL dev \`feco\` (pg_dump)"
fi
psqlp -p $PRIMARY_PORT -d feco -c "CREATE TABLE drill_marker (n INTEGER PRIMARY KEY, at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp())"
psqlp -p $PRIMARY_PORT -d feco -c "INSERT INTO drill_marker (n) SELECT g FROM generate_series(1, 100) g"

echo "2. Sao lưu nền"
"${bin}pg_basebackup" -h 127.0.0.1 -p $PRIMARY_PORT -U drill -D "$work/base" -X fetch -c fast >/dev/null
# Dữ liệu ghi sau bản sao lưu nền: mỗi giây một dòng, trong ARCHIVE_TIMEOUT + 5 giây.
for i in $(seq 101 $((100 + ARCHIVE_TIMEOUT + 5))); do
  psqlp -p $PRIMARY_PORT -d feco -c "INSERT INTO drill_marker (n) VALUES ($i)"
  sleep 1
done
counts_sql="SELECT string_agg(format('%s=%s', relname, n_live_tup), ', ' ORDER BY relname) FROM pg_stat_user_tables WHERE n_live_tup > 0 AND relname <> 'drill_marker'"
psqlp -p $PRIMARY_PORT -d feco -c "ANALYZE" >/dev/null
before_counts="$(psqlp -p $PRIMARY_PORT -d feco -At -c "$counts_sql")"
last_written="$(psqlp -p $PRIMARY_PORT -d feco -At -c "SELECT max(n) FROM drill_marker")"
crash_at="$(psqlp -p $PRIMARY_PORT -d feco -At -c "SELECT clock_timestamp()")"

echo "3. Mô phỏng sự cố: dừng ngay cụm chính"
"${bin}pg_ctl" -D "$work/primary" -m immediate stop >/dev/null

echo "4. Khôi phục từ bản sao lưu nền và WAL đã lưu trữ"
t0=$(now_ms)
cp -r "$work/base" "$work/restored"
cat >> "$work/restored/postgresql.conf" <<EOF
port = $RESTORE_PORT
archive_mode = off
restore_command = '$restore_cmd'
recovery_target_action = 'promote'
EOF
touch "$work/restored/recovery.signal"
"${bin}pg_ctl" -D "$work/restored" -w -t 600 -l "$work/restored.log" start >/dev/null
until [ "$(psqlp -p $RESTORE_PORT -d postgres -At -c 'SELECT NOT pg_is_in_recovery()')" = t ]; do sleep 1; done
t1=$(now_ms)
rto_s=$(( (t1 - t0) / 1000 ))

echo "5. Kiểm tra dữ liệu"
psqlp -p $RESTORE_PORT -d feco -c "ANALYZE" >/dev/null
after_counts="$(psqlp -p $RESTORE_PORT -d feco -At -c "$counts_sql")"
last_restored="$(psqlp -p $RESTORE_PORT -d feco -At -c "SELECT max(n) FROM drill_marker")"
lost_s="$(psqlp -p $RESTORE_PORT -d feco -At -c "SELECT round(extract(epoch FROM ('$crash_at'::timestamptz - max(at))))::int FROM drill_marker")"
counts_ok=$([ "$before_counts" = "$after_counts" ] && echo "khớp" || echo "KHÁC")
rpo_ok=$([ "$lost_s" -le $((ARCHIVE_TIMEOUT + 5)) ] && echo "đạt" || echo "KHÔNG ĐẠT")
rto_ok=$([ "$rto_s" -le 14400 ] && echo "đạt" || echo "KHÔNG ĐẠT")

out="$(cat <<EOF
| Mục | Kết quả |
|---|---|
| Ngày | $(date '+%d/%m/%Y %H:%M %z') |
| Dữ liệu | $source_note |
| Chu kỳ lưu trữ WAL (archive_timeout) | ${ARCHIVE_TIMEOUT} giây |
| Dòng đánh dấu ghi trước sự cố / sau khôi phục | $last_written / $last_restored |
| Dữ liệu mất (từ dòng cuối còn lại tới lúc sự cố) | ${lost_s} giây (giới hạn: chu kỳ lưu trữ WAL) — $rpo_ok |
| Thời gian khôi phục (RTO) | ${rto_s} giây (mục tiêu 4 giờ) — $rto_ok |
| Số dòng các bảng nghiệp vụ trước/sau | $counts_ok |
EOF
)"
echo
echo "$out"
if [ -n "$report" ]; then echo "$out" > "$report"; fi
[ "$counts_ok" = "khớp" ] && [ "$rpo_ok" = "đạt" ] && [ "$rto_ok" = "đạt" ]
