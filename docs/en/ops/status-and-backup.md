# Status page, backup and recovery

> English translation of [the Vietnamese original](../../ops/status-and-backup.md). If they differ, the Vietnamese version is authoritative.

Operations document for the `platform-support` spec ("Availability", "Backup and recovery"; tasks.md 15.3).

## 1. Status page

- **Addresses:**
  - `/status`: public page, available in all 4 languages;
  - `/status.json`: for monitoring tools. The `state` field is `operational` or `outage`; each component has `uptime_24h` and `uptime_30d`.
- **Components checked** every minute (background task of feco-server):

  | Component | How it is checked |
  |---|---|
  | Accounts and sign-in | Kratos `GET /admin/health/alive` |
  | App sign-in and the `/v1/me` API | Hydra `GET /health/alive` |
  | Database | `SELECT 1` |

- **Incidents:**
  - two consecutive failed checks open an incident, and the first passing check closes it;
  - an incident appears on the page within about 2 minutes (the spec requires within 10 minutes);
  - check results are kept for 90 days.
- **Limitation:** the page runs inside feco-server. When the server or the database goes down completely, the page cannot be opened either. In production an external monitoring service is also needed, for example a separate machine that calls `/healthz` and `/status.json` every minute and alerts the on-call person. The choice of monitoring service is still open (QUESTIONS.md).

## 2. Continuous backup

Targets per the spec:
- data loss of no more than 5 minutes (RPO);
- recovery within no more than 4 hours (RTO);
- recovery to any point in time within the last 14 days.

Postgres configuration in production:

```conf
wal_level = replica
archive_mode = on
archive_command = '<copy %p to the archive store, for example object storage>'
archive_timeout = 300          # at most 5 minutes between two WAL archivals: RPO of 5 minutes
```

- Run a base backup (`pg_basebackup` or an equivalent tool) every day.
- Keep base backups and WAL for 14 days.
- Place the archive store in a different region from the database server. The location must comply with the domestic data storage requirements (see `docs/legal/data-inventory.md`).
- Kratos and Hydra in production share this Postgres cluster, so they are backed up together.

## 3. Recovery drill (every quarter)

```bash
PG_BIN=<Postgres binaries directory> scripts/backup-drill.sh docs/ops/drills/<date>-backup-drill.md
```

The script runs on a temporary Postgres cluster and does not touch the live database:
1. enable WAL archiving;
2. load a copy of the data (`pg_dump`);
3. take a base backup;
4. keep writing data;
5. stop the primary cluster abruptly;
6. recover from the base backup and the archived WAL;
7. compare the data.

The results are written as a Markdown table. The script exits with an error code if the row counts do not match, if data loss exceeds the WAL archiving interval, or if recovery takes more than 4 hours.

Records of drills: `docs/ops/drills/`.
