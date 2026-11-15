# Guide to integrating a sub-app into F-Eco

> English translation of [the Vietnamese original](../integration-guide.md). If they differ, the Vietnamese version is authoritative.

This document is for teams that want to bring a desktop application (Windows, macOS) into the F-Eco
ecosystem: users find the app on F-Store, install and open it with F-Launcher, sign in once with
their F-Eco account, and the app receives exactly the data the user agreed to share.

Applies to F-Eco 1.1. Compared with 1.0:
- the manifest adds `source`, `terminal`, `website`, `features`, `release_notes`;
- there is an `f-dev pack` command;
- F-Launcher shows apps as a grid or a list, and has a detail page for each app.

The full list of error codes and the server address are on the `/developer/docs` page of the F-Eco
server you use.

If you want to bring an existing open-source app (for example one on GitHub) into F-Eco, see also
[`github-apps.md`](github-apps.md).

## Table of contents

1. [How a sub-app runs in F-Eco](#1-how-a-sub-app-runs-in-f-eco)
2. [Prepare a developer account](#2-prepare-a-developer-account)
3. [Create the app and get the client ID](#3-create-the-app-and-get-the-client-id)
4. [Write the `f-app.toml` manifest](#4-write-the-f-apptoml-manifest)
5. [Sign in within the app with F-SDK](#5-sign-in-within-the-app-with-f-sdk)
6. [Call the F-Eco API](#6-call-the-f-eco-api)
7. [Receive events via webhook](#7-receive-events-via-webhook)
8. [Package, code-sign and submit with `f-dev`](#8-package-code-sign-and-submit-with-f-dev)
9. [Review and release](#9-review-and-release)
10. [Integrating without F-SDK (standard OIDC)](#10-integrating-without-f-sdk-standard-oidc)
11. [Try it on your machine](#11-try-it-on-your-machine)
12. [Pre-submission checklist](#12-pre-submission-checklist)
13. [How apps appear in F-Launcher](#13-how-apps-appear-in-f-launcher)

## 1. How a sub-app runs in F-Eco

```
Developer               F-Eco (server)                     User's machine
─────────               ──────────────                     ──────────────
f-app.toml + package ──f-dev──▶ Developer portal
                            │ automated checks, review
                            ▼
                         Release ── sign TUF target ──▶ F-Launcher syncs, verifies signature,
                         F-Store (catalog)                installs into the user's folder
                                                          │ user clicks "Open"
                         /v1/launch-codes ◀── request code ┤ (launcher already signed in to F-Eco)
                                                          ▼
                         /v1/launch-codes/exchange ◀── Sub-app (F-SDK) exchanges code for tokens
                         /v1/me, /v1/entitlements  ◀── calls the API with the access token
                         webhook (signed) ─────────▶ the app's server (if any)
```

- **Secure installation:** the app package is signed as a TUF target at release time; F-Launcher
  only installs packages with a valid signature and the correct SHA-256, and blocks revoked
  versions.
- **Single sign-on:** the user signs in to F-Launcher once. When opening an app, the launcher
  requests a single-use **launch code** (valid for 60 seconds) for the app's own client ID and
  passes it through an environment variable. The app exchanges the code for its own access token
  and refresh token. The launcher never hands its own tokens to the app.
- **Per-scope consent:** the first time, the user sees a consent screen listing each type of data
  and the purpose you declared in the manifest. Optional scopes can be declined. From then on,
  opening the app from the launcher does not ask again, unless you add a new **required** scope.
- **Per-app identifier (pairwise):** the `sub` an app receives differs from that of every other
  app. Two apps cannot join data about the same user through `sub`.

## 2. Prepare a developer account

1. Create an F-Eco account, **verify the email** and **enable two-factor authentication** (TOTP
   app) in Settings. The developer portal rejects accounts missing either one (`email_unverified`,
   `second_factor_required`).
2. Open `/developer`, register as an **individual** or an **organization**, and accept the Developer
   Agreement. An organization can verify its domain with a DNS TXT record
   `feco-domain-verification=<code>` so its apps show a "Verified" badge.
3. Invite members to the team if needed. Roles: Owner, Admin (can release), Developer (can submit
   versions, cannot release).
4. Create a **CLI token** (shown once) for `f-dev` and your CI.

## 3. Create the app and get the client ID

On `/developer`, create an app with:

| Field | Meaning |
|---|---|
| App ID (`app_id`) | Lowercase letters, digits, `.`, `_`, `-`; at most 63 characters. Matches `id` in the manifest. |
| Type | **Desktop**: public client, uses PKCE, no client secret (most apps). **Own server**: has a client secret that can be rotated (shown once). |
| Redirect URI | The callback address for browser sign-in. F-SDK defaults to `http://127.0.0.1:47280/callback`; declare exactly this address if you do not change it. |

The app page shows the **client ID**. This is also where you set the webhook address and receive
the webhook signing key.

## 4. Write the `f-app.toml` manifest

Each submitted version comes with a manifest. A complete example:

```toml
id = "quanlykho"
name = "Quản Lý Kho"
version = "1.2.0"                       # SemVer, must be greater than the released version
license = "MIT"
privacy_policy = "https://kho.example/privacy"
webhook = "https://kho.example/feco/webhook"   # optional
summary = "Manage stock receipts, issues and inventory"         # optional
description = "Long description shown on the F-Store detail page"
category = "productivity"               # category code, see section 13
source = "https://github.com/kho/quanlykho"    # optional; if present, the package must include LICENSE
terminal = false                        # true: the launcher opens the app in a terminal window
website = "https://kho.example"         # optional, must be https://
features = [                            # optional, at most 12 lines, each ≤ 120 characters
  "Receive and issue stock with barcodes",
  "Daily inventory reports",
]
release_notes = "Added inventory report"  # changes in this version, ≤ 2,000 characters

[[platforms]]
os = "windows"          # windows | macos
arch = "x64"            # x64 | arm64
entry = "QuanLyKho.exe" # path to the executable, relative to the package root

[[platforms]]
os = "macos"
arch = "arm64"
entry = "QuanLyKho.app/Contents/MacOS/QuanLyKho"

[scopes.profile]
required = true
purpose = "Show who entered the stock"
purpose_en = "Shows who entered the stock"   # optional

[scopes.purchases]
purpose = "Check licenses"

[permissions]
network = true
camera = false
microphone = false
user_folders = ["documents"]   # documents, downloads, pictures, music, videos, desktop
```

**Data scopes** (each scope must have a `purpose`, shown verbatim on the consent screen and in
F-Store):

| Scope | Fields the app receives in `/v1/me` |
|---|---|
| `profile` | `name` (display name, avatar) |
| `email` | `email` |
| `locale` | `locale` (language, time zone) |
| `phone` | `phone_number` |
| `billing` | `billing` (invoicing information) |
| `purchases` | permission to read the app's own `/v1/entitlements` |

`openid` and `offline_access` are protocol scopes and need no declaration. Request only the scopes
you actually use: reviewers compare scopes against the app's functionality (the "Undeclared data
collection" clause).

Validate the manifest before submitting:

```bash
f-dev validate --manifest f-app.toml
```

All errors are listed at once, and each one names the faulty field (for example
`scopes.email.purpose là bắt buộc`, "is required").

## 5. Sign in within the app with F-SDK

F-SDK is available for Rust (`fsdk`), C (`fsdk-c`, stable ABI), Python (`fsdk`, via the C ABI) and
TypeScript/Node.js (`@f-eco/sdk`, Node ≥ 22.6). Every version does the same thing:

1. If the app was opened by F-Launcher (`FECO_LAUNCH_CODE` is in the environment): exchange the
   launch code for tokens.
2. Otherwise (run directly, or the user has not yet consented to the scopes): open the browser to
   sign in (OAuth 2.0 PKCE) and receive the callback on `127.0.0.1`.
3. Automatically refresh the access token (15 minutes) using the refresh token.

**Environment variables F-Launcher sets when opening the app** (every other `FECO_*` variable of
the launcher is removed):

| Variable | Content |
|---|---|
| `FECO_LAUNCH_CODE` | Single-use launch code, valid for 60 seconds |
| `FECO_CLIENT_ID` | The app's client ID |
| `FECO_SERVER_URL` | The F-Eco server (`/v1/...` API) |
| `FECO_ISSUER_URL` | The OAuth 2.0 / OIDC server |
| `FECO_APP_DATA_DIR` | The app's own data folder (kept across updates) |
| `FECO_LAUNCHER_URL`, `FECO_LAUNCHER_SECRET` | The launcher's local API for requesting a new launch code |

F-SDK's own variables (optional): `FSDK_REDIRECT_URI`, `FSDK_SCOPES` (default
`openid offline_access profile`; add the scopes you declared in the manifest).

**Rust**

```rust
let config = fsdk::Config::from_env()?.scopes("openid offline_access profile purchases");
let session = fsdk::session(&config).await?;      // launcher or browser
let me = session.me().await?;                     // sub, name...
let licenses = session.entitlements().await?;     // requires the purchases scope
```

When running outside the launcher, create `Config::new(client_id, issuer, server)` instead of
`from_env()`. Store `session.refresh_token()` in the operating system's secure storage so you can
use `Session::resume(&config, &refresh_token)` next time.

**C**

```c
#include "fsdk.h"
fsdk_session *s = getenv("FECO_LAUNCH_CODE")
    ? fsdk_session_from_launch_code(NULL, NULL, NULL, NULL, NULL, NULL)  /* NULL: read the environment variables */
    : fsdk_login_browser(NULL, NULL, NULL, NULL, NULL);
if (!s) { fprintf(stderr, "%s\n", fsdk_last_error()); return 1; }
char *me = fsdk_me_json(s);          /* JSON, free with fsdk_string_free */
fsdk_string_free(me);
fsdk_session_free(s);
```

Check `fsdk_abi_version() == FSDK_ABI_VERSION` at startup. See `sdk/c/examples/login.c`.

**Python**

```python
import fsdk
with fsdk.Session.auto() as session:     # launcher or browser
    me = session.me()
    licenses = session.entitlements()   # requires the purchases scope
```

Set `FSDK_LIBRARY` to point to the `fsdk_c` library shipped with the app. See
`sdk/python/examples/login.py`.

**TypeScript / Node.js**

```ts
import { Session, configFromEnv } from "@f-eco/sdk";
const session = await Session.auto(configFromEnv());
const me = await session.me();
```

See `sdk/typescript/examples/login.ts`.

**Requesting a new launch code:** a code lives for only 60 seconds. If the app needs to sign in
again while running (for example the user signs out inside the app), call the launcher's local API:

```
POST {FECO_LAUNCHER_URL}v1/launch-code
Authorization: Bearer {FECO_LAUNCHER_SECRET}
{"client_id": "<the app's client ID>"}
→ {"code": "...", "expires_in": 60}
```

The launcher only issues codes for the app it opened (a request for another app gets 403 and is
logged as a security event).

## 6. Call the F-Eco API

Call with the app's `Authorization: Bearer <access token>`.

| Endpoint | Scope | Content |
|---|---|---|
| `GET /v1/me` | `profile`, `email`, ... | User information according to the granted scopes; `sub` is the app's pairwise identifier. Use `?fields=name,email` to fetch fewer fields. |
| `GET /v1/entitlements` | `purchases` | The user's entitlements for the app: `owner`, `license`, `subscription` (with source, grant date, expiry). |
| `POST /v1/apps/crashes` | (any) | Report that the app crashed: `{"version": "1.2.0"}`. Counted per version only, no user data is sent. F-SDK: `report_crash`. |
| `GET /v1/catalog/{app}/update` | (public) | The version this device is eligible to receive (staged rollout). |

**Errors** are always JSON:

```json
{"error": "insufficient_scope", "message": "...", "message_vi": "...", "request_id": "..."}
```

Common codes: `invalid_token` (401, refresh the token), `insufficient_scope` (403, declare the
scope and request consent), `consent_required` (403, F-SDK opens the browser automatically),
`invalid_grant` (400, the launch code is wrong or older than 60 seconds), `rate_limited` (429, wait
according to `Retry-After`). Default limits: 120 requests/minute per user, 1,200 requests/minute
per app and per IP. The full list is at `/developer/docs`. Record the `request_id` when contacting
support.

## 7. Receive events via webhook

Set the webhook address (HTTPS) on the app page in the Developer Portal; store the signing key,
which is shown once. F-Eco sends a JSON `POST`:

```json
{"event": "user.revoked", "client_id": "...", "sub": "<pairwise sub>", "occurred_at": "2026-11-13T08:00:00Z"}
```

| Event | What you need to do |
|---|---|
| `user.revoked` | The user revoked the app's access: stop using the tokens, delete the session. |
| `user.deleted` | The user deleted their F-Eco account: delete their personal data in your system. |
| `entitlement.changed` | The entitlement changed: re-read `/v1/entitlements`. |
| `app.version.revoked` | A version of the app was pulled urgently: check the moderation notice. |

**Verify the signature** before processing: header
`F-Eco-Signature: t=<unix seconds>,v1=<hex>`, where `v1 = HMAC-SHA256(signing key, "<t>.<request body>")`.
Reject if `t` is off by more than 5 minutes. F-SDK has ready-made functions: `fsdk::verify_webhook`
(Rust), `fsdk.verify_webhook` (Python), `verifyWebhook` (TypeScript).

Return 2xx once received. If no 2xx is received, F-Eco retries after 1, 5 and 30 minutes and then at
increasing intervals, for at most 72 hours. Process idempotently: the same event may arrive more
than once.

## 8. Package, code-sign and submit with `f-dev`

A **package** is a folder (or `.zip` file) containing the executable and all resources. `entry` in
the manifest is the path to the executable relative to the package root. For multiple platforms:
subfolders or zip files named `<os>-<arch>`:

```
dist/
├── windows-x64/QuanLyKho.exe, data/...
└── macos-arm64/QuanLyKho.app/...
```

For a single platform, point directly at the package folder. A package is at most 2 GB. Do not put
`../`-style paths in the zip: the launcher rejects such a package.

**Code signing is mandatory:** Windows executables are signed with Authenticode, macOS apps with
Developer ID (notarize them so Gatekeeper does not warn).
An unsigned package is rejected (`invalid_package`).

**Submit:**

```bash
export FDEV_TOKEN=<CLI token>          # or --token
export FDEV_SERVER=https://<F-Eco server>   # or --server
f-dev publish --manifest f-app.toml --package dist/
# → "Pending review", with the version ID
```

`f-dev` validates the manifest, compresses each platform, uploads with SHA-256 and submits for
review. It can be used in CI.

To preview the package that will be submitted without uploading, use `f-dev pack --manifest f-app.toml --package build/
--out dist/`. This command creates `dist/<os>-<arch>.zip` and runs exactly the same checks as `publish`.
If the manifest has `source`, each package must contain the original license file (`LICENSE*`, `COPYING*` or
`NOTICE*`), otherwise it is rejected.
You can also upload the package and submit for review on the app page in the Developer Portal.

## 9. Review and release

1. **Automated checks** (a few minutes): malware scanning with two engines, code signature
   verification, executable architecture against the manifest, packer detection.
2. **Manual review** when: it is the first version, a new scope or permission is added, or the
   automated checks need a human to look. Reviewers clearly see the scope and permission
   differences compared with the previous version.
3. **Rejections** always cite a clause of the Content Policy (`/policy/content`) and how to fix it.
   Each rejection can be appealed once.
4. **Release** (Admin or Owner role): choose the `stable` or `beta` channel and the staged rollout
   percentage; increase gradually up to 100% or pause. Releasing signs a TUF target; users'
   F-Launcher receives the new version at its next sync (within 1 hour at most), and users who have
   installed the app receive an update notification (if they enabled it).
5. Every version is rescanned periodically every 7 days. If a serious violation is found, that
   version is urgently revoked: removed from F-Store, blocked from opening by the launcher, and you
   receive an email and the `app.version.revoked` webhook.

Aggregate statistics (installs, 7-day and 30-day active users, runtime crashes, ratings) are on
the app page; F-Eco never tells developers the identity of users.

## 10. Integrating without F-SDK (standard OIDC)

F-Eco is a standard OpenID Connect provider; any OIDC library can be used:

- Discovery: `{FECO_ISSUER_URL}.well-known/openid-configuration`
- Flow: Authorization Code + PKCE (desktop apps), or with a client secret (apps with their own
  server).
- Scopes as in section 4; `sub` in the ID token is pairwise.
- When opened by the launcher, exchange the launch code with
  `POST {FECO_SERVER_URL}v1/launch-codes/exchange` with JSON `{"code", "client_id", "redirect_uri",
  "code_challenge", "scope"}` (`code_challenge` is the S256 of the PKCE verifier generated by the
  app) to obtain an authorization code, then exchange the code for tokens at the token endpoint
  with the verifier as usual.
  (F-SDK does these steps for you.)

A runnable example with Authlib (Python): `examples/standard-oidc/authlib_login.py`.

## 11. Try it on your machine

The repository ships dev infrastructure (Kratos, Hydra, PostgreSQL, Mailpit) and a script that
sets up a dev launcher; see the README section "Hướng dẫn chạy hệ thống" (system run guide):

1. Start the infrastructure and `feco-server` (README steps 2–3).
2. Create a developer account and an app at `http://127.0.0.1:8080/developer`.
3. `scripts/dev-launcher.sh setup` creates the dev TUF repository and the launcher's client;
   `scripts/dev-launcher.sh
   publish` signs your package into the dev repository so the dev launcher can install it (README
   step 6).
4. Open the app from the dev F-Launcher: the app receives the variables in section 5. Or run the
   app directly with `FECO_CLIENT_ID`, `FECO_ISSUER_URL=http://127.0.0.1:4444/`,
   `FECO_SERVER_URL=http://127.0.0.1:8080/` to try browser sign-in.
5. Sample apps:
   - `examples/apps/f-notes` (Notes) and `examples/apps/f-timer` (Timer): egui windowed apps that
     use F-SDK, sign in automatically when opened by the launcher, and run in guest mode when
     opened directly;
   - `tools/feco-demo-app` (Rust) and `sdk/*/examples`.

   `scripts/dev-apps.sh samples` signs the two sample apps into the dev repository.
   `scripts/dev-apps.sh github ...` builds and signs an open-source app from GitHub (see
   [`github-apps.md`](github-apps.md)).

## 12. Pre-submission checklist

- [ ] Email verified, two-factor authentication enabled, Developer Agreement accepted.
- [ ] `id` in the manifest matches the app ID; `version` is greater than the released version.
- [ ] Each scope has a clear `purpose`; only scopes actually used are requested; non-essential
      scopes have `required = false`.
- [ ] `privacy_policy` is an accessible HTTPS page.
- [ ] The executable is signed with Authenticode / Developer ID; `entry` is the correct path in the
      package.
- [ ] The app uses F-SDK (or standard OIDC) to sign in; it never asks for the F-Eco password itself.
- [ ] The refresh token is stored in the operating system's secure storage, not written to plain
      files.
- [ ] The app's own data lives in `FECO_APP_DATA_DIR`.
- [ ] The webhook verifies signatures and processes idempotently; data is deleted on receiving
      `user.deleted`.
- [ ] There is a concise `summary` and `category` is one of the codes in section 13, so the app
      appears in the right group in the launcher.
- [ ] Apps that run in a terminal have `terminal = true`. Apps built from external source code have
      `source`, and the package includes the original license.

## 13. How apps appear in F-Launcher

F-Launcher shows apps in the Library (installed apps) and the Store, as a **grid** or a **list**.
Users search (accent-insensitive), filter by category, and sort by name, last opened, install date,
category or size.

Click an app (tile or row) to open its **detail page**. The page has:
- icon, name, developer, category, short description;
- introduction (`description`) and features (`features`);
- "What's new in version …" (`release_notes`) with the release date;
- an information table: latest version, installed version, size, platforms with packages, license, source, website;
- buttons Install, Update, Open, Uninstall, Roll back to the previous version;
- buttons and arrow keys to move to the previous or next app.

Items with no data are not shown. `release_notes` is also the version's change history on F-Store.

All displayed information comes from the TUF metadata that the server signs at release time:

| Displayed | Taken from |
|---|---|
| Name | `name` |
| Short description | `summary` |
| Category | `category` |
| Developer | name in the F-Store catalog |
| "Source" button | `source` |
| "Open (terminal)" | `terminal = true` |
| Introduction, features | `description`, `features` |
| Website | `website` |
| What's new, release date | `release_notes`, the time the server signed the version |
| License | `license` |
| Runs on | the `[[platforms]]` that have signed packages |

The icon is a tile with the first letter of the name, with a fixed color derived from the app ID.
Developer-uploaded icons are not supported yet.

Recommended category codes (other values are shown verbatim):

| Code | Display name | Code | Display name |
|---|---|---|---|
| `productivity` | Năng suất (Productivity) | `games` | Trò chơi (Games) |
| `tools` | Công cụ (Tools) | `graphics` | Đồ họa (Graphics) |
| `system` | Hệ thống (System) | `music` | Âm nhạc (Music) |
| `development` | Lập trình (Development) | `video` | Video |
| `education` | Giáo dục (Education) | `business` | Kinh doanh (Business) |

An app that declares no `[scopes]` is treated as not signing in with F-Eco: the target has no
`client_id`, and the launcher opens the app without requesting a launch code. Users adjust the
launcher's appearance (theme, accent color, font size, layout, grid tile size) on the Settings
page. These settings are stored only on their machine.
- [ ] Handle `rate_limited` according to `Retry-After`, and `invalid_token` by refreshing the token.
- [ ] `f-dev validate` reports no errors.
