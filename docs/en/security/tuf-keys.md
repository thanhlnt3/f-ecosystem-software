# TUF repository keys and operations

> English translation of [the Vietnamese original](../../security/tuf-keys.md). If they differ, the Vietnamese version is authoritative.

F-Eco's installation packages are distributed using The Update Framework (TUF) model, design D3. The code is in
`crates/feco-tuf` (the `feco-tuf` library and tool); every signing and verification step uses the `tough` crate.

## Roles and key custody

| Role | What it signs | Where the key is kept | Threshold | Metadata expiry | Re-signing |
|---|---|---|---|---|---|
| `root` | The key list of every role | Offline, 3 holders, each on a separate device | 2/3 | 365 days | Every year, or on key rotation |
| `targets` | Hash, length, app, version and platform of each package | KMS/HSM, used only by the release service after moderation is complete | 1 | 90 days | On release and revocation; `resign` every month |
| `snapshot` | The version of targets | KMS, online key of the distribution server | 1 | 7 days | `refresh` every 15 minutes |
| `timestamp` | The version and hash of snapshot | KMS, online key of the distribution server | 1 | 1 hour | `refresh` every 15 minutes |

The dev build keeps keys in Ed25519 files (PKCS#8). Production replaces them with a KMS key source (the same
`KeySource` trait of `tough`) once an infrastructure provider has been chosen (`QUESTIONS.md`, task 1.4).

Never put key files in the code repository. CI runs gitleaks on every commit.

## Commitments to the launcher

- The launcher installer ships with `1.root.json`. The launcher follows the chain `2.root.json`, `3.root.json`... on its own;
  each new root must carry enough signatures to meet the threshold of both the previous root and itself.
- The launcher syncs every hour (`client::SYNC_INTERVAL`) and stores the metadata it has already trusted to block rollback.
- The timestamp expires after 1 hour, so a revocation reaches the launcher within 1 hour, even if an attacker
  holds on to old metadata (a "freeze" attack). In exchange, the server must run `refresh` regularly: if
  `refresh` stops for more than 1 hour, no launcher can install or update anything.
- Each package may be used only if its length and SHA-256 match the signed targets. A target name is unique
  and its content cannot be replaced after release, even after it has been revoked.

## Routine tasks

```bash
# Every 15 minutes, on the distribution server (online keys):
feco-tuf refresh --repo /srv/feco-tuf --sign <snapshot>,<timestamp>

# Every month, by the release service (targets key):
feco-tuf resign --repo /srv/feco-tuf --sign <targets>,<snapshot>,<timestamp>

# Check the repository as a freshly installed launcher would:
feco-tuf status --repo /srv/feco-tuf
```

The repository's `metadata/` and `targets/` directories are served statically over HTTPS (a CDN is fine). The web server
only needs read access; keys are not on the web server.

## Release and revocation

```bash
# After a version passes moderation (task group 9 will call this step automatically):
feco-tuf publish --repo DIR --file soundlab.zip --name soundlab-1.2.0-windows-x64.zip \
  --app soundlab --version 1.2.0 --platform windows-x64 --sign <targets>,<snapshot>,<timestamp>

# Emergency revocation (the reason is signed with the metadata and shown in the launcher):
feco-tuf revoke --repo DIR --name soundlab-1.2.0-windows-x64.zip --reason "Malware detected" \
  --sign <targets>,<snapshot>,<timestamp>
```

## Root signing ceremony

1. Prepare an offline machine with `feco-tuf` preinstalled, with the build's hash verified.
2. Each root key holder generates a key on their own device (`feco-tuf keygen`) and reads the key ID aloud.
   The secretary records the key IDs in the minutes.
3. Build the repository (`feco-tuf init`) with 3 root keys; at least 2 people sign. The secretary records the key IDs of the
   targets, snapshot and timestamp keys.
4. Copy `metadata/1.root.json` out to include in the launcher installer. Each root key leaves the machine with its holder.
5. Two people sign the minutes together.

## Key rotation

| Situation | Command | Who must be present |
|---|---|---|
| A targets, snapshot or timestamp key is compromised, or due for replacement | `feco-tuf rotate --role targets --keys <new key>` | 2 root key holders, plus the targets, snapshot and timestamp keys that are valid under the new root |
| A root key is compromised or a holder leaves | `feco-tuf rotate --role root --keys <3 new root keys> --threshold 2` | 2 current root key holders (not counting the compromised key) and the holder of the new key |
| Root is about to expire | `feco-tuf rotate --role timestamp --keys <current timestamp key>` (keeps the key, only increments the version and extends the expiry) | 2 root key holders |

`rotate` refuses to issue a root with missing signatures, and re-signs targets, snapshot and timestamp under the new root
in the same step, so the launcher never sees the repository in an inconsistent state. When a key is compromised, record an incident per
`docs/security/incident-response.md`.

## Drill

`scripts/tuf-drill.sh` runs the whole procedure on temporary keys: build a 2/3 repository, release, rotate the
targets key, rotate the root key (the removed key no longer counts), revoke, refresh the timestamp. The script prints a
Markdown report and stops with an error code if any step does not match expectations. CI runs this script on Ubuntu and
Windows on every commit. Reports are stored in `docs/security/drills/`.

A drill with real key holders (offline machine, real keys, signatures on the minutes) is done once the root key
holders have been designated.
