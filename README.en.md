# F-EcoSystem-Software (F-Eco)

An open platform for third-party developers to publish **sub-apps** through a shared launcher, with shared user accounts, a self-service app-building tool (**F-Builder**), and a paid **build-to-order app** service. The idea is inspired by the launcher + app suite model of ArtCraft, but open to third parties.

> **Version 1.20.0** (Tiếng Việt: [README.md](README.md)) ([changelog](CHANGELOG.md)). Integrating a sub-app into F-Eco: [developer guide](docs/en/integration-guide.md); bringing an open-source app from GitHub into F-Eco: [docs/en/github-apps.md](docs/en/github-apps.md); configuring the payment gateways, e-invoicing and email: [docs/en/payments-setup.md](docs/en/payments-setup.md). The release package (binaries, F-SDK, documentation, `THIRD_PARTY_NOTICES`, SBOM) is built with `scripts/package-release.sh`, or automatically when a `v*` tag is pushed (`.github/workflows/release.yml`, which creates a draft release).
>
> Working today (open items are listed at the end of the changelog and in `QUESTIONS.md`):
> - Accounts: sign-up, email verification, sign-in, password-guessing protection, two-factor authentication (TOTP, recovery codes), signed-in devices, password recovery, account deletion after a 30-day waiting period.
> - Single sign-on for sub-apps via OAuth 2.0/OIDC, a scope-based consent screen, the `/v1/me` API, a connected-apps page, permission revocation, webhooks, personal data export.
> - Web UI in Vietnamese, English, French and Japanese; the Appearance (Giao diện) page (`/preferences`) lets you choose language, theme, accent color, the same 10 styles as F-Launcher, density and font size. A profile page for signed-in users (role, stat cards, security, privacy). Documentation has English versions in `docs/en/`.
> - Public status page (`/status`, `/status.json`), backup and restore drill (`scripts/backup-drill.sh`), `/v1/me` load test ([report](docs/reports/2026-11-15-performance.md)).
> - Security: HSTS and protective headers, encryption of sensitive data at rest (AES-256-GCM), rate limiting, an append-only audit log, alerts for sign-ins from new devices. Incident response procedure: [`docs/en/security/incident-response.md`](docs/en/security/incident-response.md).
> - Chain of trust for installation packages based on TUF (using `tough`): signing approved versions, revocation reaching the launcher within 1 hour, blocking tampered packages or metadata, rollback, freeze; rotation of the targets and root keys (2-of-3 threshold). The `feco-tuf` tool and the key custody procedure: [`docs/en/security/tuf-keys.md`](docs/en/security/tuf-keys.md).
> - F-Launcher (Rust + egui):
>   - browser-based sign-in, staying signed in across restarts;
>   - install, update, roll back and uninstall sub-apps with TUF verification; works offline; blocks revoked versions;
>   - opens sub-apps with a one-time launch code; opens apps that run in a terminal in a separate window;
>   - Library and Store views as a grid (uniform tiles) or a list, with diacritic-insensitive search, category filter and sorting;
>   - icon-style toolbar buttons with tooltips; Settings and the app detail page open in a separate window;
>   - appearance settings page: language (Vietnamese, English, French, Japanese), style (Plain, Elegant,
>     Parchment, Fresco, Starry night, Lotus pond, Cubist, Lily, Old quarter, Lacquer),
>     theme, accent color, font size, layout;
>   - shows the platform role, with role-based shortcuts to the Developer Portal and the Admin Console;
>   - app detail page (introduction, features, what's new, version info, platform, the
>     install, open and uninstall buttons), switching to the previous or next app with a button or the arrow keys.
> - F-Store (`/store`): catalog of approved apps, search that ignores Vietnamese diacritics, filter by category, operating system, price, verified developer; the detail page shows data scopes with their purpose, license, privacy policy, change history; reviews only from people who installed the app, developer replies, abuse reports.
>
> - Developer Portal (`/developer`): individual or organization registration (requires a verified email and TOTP), Developer Agreement, domain verification via DNS TXT, teams and roles, client ID and redirect URIs, the `f-app.toml` manifest, package submission via the web or the `f-dev` CLI, code-signature checks, stable/beta channels and percentage-based gradual rollout, aggregate metrics.
> - Moderation: automated check queue (malware scanning with the F-Eco signature engine and ClamAV, code signature, architecture, packer), manual or automatic review rules, content policy (`/policy/content`), rejections that cite the clause and how to fix it, appeals, emergency takedown, 7-day rescan (both packages and library vulnerabilities in the submitted source; new vulnerabilities are reported to the developer and listed under "Review alerts" on the review queue page).
> - F-SDK for Rust, C (stable ABI), Python and TypeScript/Node.js: sign-in with the launcher's launch code or through the browser (PKCE), automatic token refresh, `/v1/me`, entitlement checks (`/v1/entitlements`), runtime error reporting, webhook signature verification. Apps that use a standard OIDC library do not need F-SDK. Unified error codes, listed at `/developer/docs`.
>
> - Build-to-order app requests (`/orders`): submit a request with files (up to 200 MB) and accept the app-building Terms of Service (draft), milestone-based quotes (deposit ≤ 30%, 7-day validity, counter-offers), each milestone delivered as a private app in F-Launcher, acceptance, free revisions, automatic acceptance after 14 days, messages with attachments, order cancellation, disputes, final delivery download once paid in full.
>
> - Payments through ZaloPay (sandbox): choose VietQR, ATM card, ZaloPay wallet or international card on the gateway's own page (F-Eco never receives card numbers), signed callbacks, active status queries after 15 minutes, no double charging, refunds (partial; from 10 million VND they need two approvers), invoices (draft, awaiting an e-invoicing provider), payment history, daily reconciliation.
>
> - Payments also through VNPAY (QR, ATM card, international card) and MoMo (wallet); e-invoicing through MISA meInvoice when configured (no test account yet).
> - Admin Console (`/admin`) with 5 roles, requiring a passkey or security key; notifications by email and through the notification center on the web and in F-Launcher, optional categories can be turned off, unsubscribing without signing in.
>
> - F-Coin wallet (top-up, withdrawal, buying apps, statistics), user roles with approval, software projects (work profiles, contracts, escrow), messaging between customers and developers (web, F-Launcher, phone).
> - Admin Console: a default administrator account is always present; every admin list has search, status and date-range filters, and sorting by time or sender, shared by the web, F-Launcher and the phone app.
> - App update notifications on the web, in F-Launcher and on the phone; F-Launcher updates itself through TUF.
> - Source scanning of sub-apps on submission and every 7 days (leaked secrets, suspicious code, library vulnerabilities via OSV.dev); outside apps are scanned explicitly before integration and rescanned weekly.
> - Apps integrated into the dev repository: ThesisAI and BinhAI Office (web apps), Mentor (web and desktop), VPdf (Windows desktop), plus the Notes and Timer sample apps and three open-source apps.
> - F-Launcher for mobile (Flutter, Android and iOS) as a companion to the desktop version.
> - Verification and notification emails can go to real inboxes over SMTP (for example Gmail) on a dev machine.
> - New look (1.20.0): a landing page on the web for visitors who are not signed in; F-Launcher defaults to the Elegant style, light theme and large grid tiles, with a "Your workspace" header showing installed apps and available updates; a shared teal palette across web, desktop and phone.
>
> Not yet available: a virtual machine for trial runs of apps, a sandbox environment, an email domain (SPF, DKIM, DMARC), test accounts for VNPAY, MoMo and MISA. Not yet usable by end users.

## Table of contents

- [Installing from a release](#installing-from-a-release)
  - [Windows](#windows) · [macOS](#macos) · [Linux](#linux) · [Phone](#phone)
  - [Running the F-Eco server from the package](#running-the-f-eco-server-from-the-package)
- [Running the system](#running-the-system)
  - [1. Prerequisites](#1-prerequisites)
  - [2. Start the infrastructure](#2-start-the-infrastructure)
  - [3. Run feco-server](#3-run-feco-server)
  - [4. Try the account features](#4-try-the-account-features)
  - [5. Try single sign-on with the sample app](#5-try-single-sign-on-with-the-sample-app)
  - [6. Try F-Launcher](#6-try-f-launcher)
  - [7. Try the Developer Portal](#7-try-the-developer-portal)
  - [8. Write a sub-app with F-SDK](#8-write-a-sub-app-with-f-sdk)
  - [9. Try build-to-order requests](#9-try-build-to-order-requests)
  - [10. Try ZaloPay payments](#10-try-zalopay-payments)
  - [11. Try the Admin Console and notifications](#11-try-the-admin-console-and-notifications)
  - [12. Try the F-Coin wallet, roles and projects](#12-try-the-f-coin-wallet-roles-and-projects)
  - [13. Stop and wipe data](#13-stop-and-wipe-data)
- [Service addresses](#service-addresses)
- [Configuration](#configuration)
- [Testing](#testing)
- [Troubleshooting](#troubleshooting)
- [Requirements and design](#requirements-and-design)
- [Directory structure](#directory-structure)
- [License](#license)

## Installing from a release

Every release on [GitHub Releases](https://github.com/thanhlnt3/f-ecosystem-software/releases/latest) ships prebuilt packages for each operating system. You do not need Rust or the source code.

| File | Operating system | Contents |
|---|---|---|
| `feco-<version>-x86_64-pc-windows-msvc.zip` | Windows 10/11, x64 | F-Launcher, the server and tools (`bin/*.exe`), F-SDK, sample infrastructure configuration, documentation |
| `feco-<version>-aarch64-apple-darwin.zip` | macOS on Apple silicon (M1 or later) | Same as above, macOS build. There is no build for Intel Macs yet |
| `feco-<version>-x86_64-unknown-linux-gnu.zip` | Linux x64 (glibc) | Same as above, Linux build. F-Launcher on Linux cannot sign in yet (see [Linux](#linux)) |
| `feco-mobile-<version>.apk` | Android 10+ | F-Launcher for mobile, test build (see [Phone](#phone)) |
| `feco-mobile-<version>-ios-simulator.zip` | iOS Simulator on macOS | F-Launcher for mobile, for the iOS simulator |
| `mentor-<version>-windows-x64.zip` | Windows x64 | The Mentor desktop app, installed through F-Launcher |
| `SHA256SUMS.txt` | | SHA-256 checksums of every file |

Inside each desktop package:

| Folder | Contents |
|---|---|
| `bin/` | `feco-launcher` (F-Launcher), `feco-server` (the server), `feco-catalog` (catalog administration), `f-dev` (developer CLI), `feco-tuf` (TUF repository) |
| `sdk/` | F-SDK: C (header and prebuilt library), Python, TypeScript, Rust source |
| `infra/` | `docker-compose.yml` and the Kratos and Hydra configuration for running the infrastructure |
| `docs/`, `notices/`, `sbom/` | Integration guide, third-party licenses, SBOM (CycloneDX) |

The programs are not yet signed with an Authenticode certificate or an Apple Developer ID, so Windows and macOS warn on first launch, as described below. Verify the checksums before running anything, to make sure the package has not been tampered with.

### Download and verify

Download the files you need together with `SHA256SUMS.txt` into one folder, then check them. On Linux:

```bash
sha256sum --ignore-missing -c SHA256SUMS.txt
```

On macOS:

```bash
shasum -a 256 --ignore-missing -c SHA256SUMS.txt
```

On Windows (PowerShell), compare the printed hash with the matching line in `SHA256SUMS.txt`:

```powershell
Get-FileHash .\feco-1.20.0-x86_64-pc-windows-msvc.zip -Algorithm SHA256
```

Every file must report `OK` (Linux, macOS) or a matching hash (Windows). If it does not match, do not run that package.

### Connecting F-Launcher to an F-Eco server

F-Launcher needs to know which F-Eco server to connect to. The server administrator provides these values:

| Variable | Meaning |
|---|---|
| `FECO_LAUNCHER_SERVER` | Address of `feco-server`, for example `https://f-eco.example.com/` |
| `FECO_LAUNCHER_HYDRA` | Sign-in address (Hydra) |
| `FECO_LAUNCHER_CLIENT_ID` | F-Launcher's client ID |
| `FECO_LAUNCHER_TUF` | Address of the TUF repository holding the app packages |
| `FECO_LAUNCHER_TRUSTED_ROOT` | The trusted `root.json` of the TUF repository |

Every address must use HTTPS unless the server runs on this machine (`127.0.0.1`, `localhost`). For a dev server run from source on the same machine ([Running the system](#running-the-system)), `scripts/dev-launcher.sh env` prints the values. Also set `FECO_LAUNCHER_DEV=1` to use the default addresses `http://127.0.0.1:8080/` and `http://127.0.0.1:4444/`.

### Windows

1. Extract the package, for example to `C:\F-Eco`.
2. Open PowerShell, set the variables from the section above, then run F-Launcher:

   ```powershell
   cd C:\F-Eco\feco-1.20.0-x86_64-pc-windows-msvc
   $env:FECO_LAUNCHER_SERVER = "https://<feco-server>/"
   $env:FECO_LAUNCHER_HYDRA = "https://<feco-hydra>/"
   $env:FECO_LAUNCHER_CLIENT_ID = "<client-id>"
   $env:FECO_LAUNCHER_TUF = "https://<feco-tuf>/"
   $env:FECO_LAUNCHER_TRUSTED_ROOT = "C:\F-Eco\root.json"
   .\bin\feco-launcher.exe
   ```

3. If Windows shows "Windows protected your PC" (SmartScreen), click **More info**, then **Run anyway**. Only do this after the checksum matched.
4. Click **Sign in**: the browser opens the F-Eco sign-in page. Your sign-in is stored in Windows Credential Manager. Apps install into `%LOCALAPPDATA%\F-Eco\Launcher`, with no administrator rights needed.

### macOS

The package is for Macs with Apple silicon and runs from Terminal (there is no `.app` bundle yet).

```bash
cd ~/Downloads
unzip feco-1.20.0-aarch64-apple-darwin.zip
cd feco-1.20.0-aarch64-apple-darwin
xattr -dr com.apple.quarantine .
chmod +x bin/*
export FECO_LAUNCHER_SERVER=https://<feco-server>/
export FECO_LAUNCHER_HYDRA=https://<feco-hydra>/
export FECO_LAUNCHER_CLIENT_ID=<client-id>
export FECO_LAUNCHER_TUF=https://<feco-tuf>/
export FECO_LAUNCHER_TRUSTED_ROOT=$HOME/F-Eco/root.json
./bin/feco-launcher
```

- The `xattr` command removes the "downloaded from the Internet" mark. Without it, macOS refuses to open the program because the developer cannot be verified (the programs have no Apple Developer ID yet).
- On first sign-in, macOS may ask whether F-Launcher may use the Keychain: choose **Allow** or **Always Allow**. Apps install into `~/Library/Application Support/F-Eco/Launcher`.

### Linux

The Linux package is built on Ubuntu 26.04 (x64), so it needs a similarly recent glibc. On older distributions the programs may report a missing `GLIBC_2.xx`; in that case build from source ([Running the system](#running-the-system)).

```bash
unzip feco-1.20.0-x86_64-unknown-linux-gnu.zip
cd feco-1.20.0-x86_64-unknown-linux-gnu
chmod +x bin/*
```

On Linux, the package covers the following:
- **Server and tools** (`feco-server`, `feco-catalog`, `feco-tuf`, `f-dev`) work fully. See [Running the F-Eco server from the package](#running-the-f-eco-server-from-the-package).
- **F-SDK** works for sub-apps on Linux (`sdk/c/lib/libfsdk_c.so`, Python, TypeScript).
- **The F-Eco website and F-Store** work in a browser.
- **F-Launcher** opens its window but **cannot sign in yet**. F-Launcher only stores your sign-in in the operating system's secure store, and today supports Windows Credential Manager and macOS Keychain only. On Linux it reports that F-Launcher only supports Windows and macOS. Support for the Linux secure store (Secret Service) is planned for a later release.

### Running the F-Eco server from the package

The quickest way to try it on one machine (Windows, macOS or Linux) is to start the infrastructure with the bundled Docker setup, then run `feco-server` in `dev` mode:

```bash
docker compose -f infra/docker-compose.yml up -d --wait
FECO_ENV=dev ./bin/feco-server
```

On Windows (PowerShell):

```powershell
docker compose -f infra/docker-compose.yml up -d --wait
$env:FECO_ENV = "dev"; .\bin\feco-server.exe
```

- When you see `feco-server đang chạy`, open http://127.0.0.1:8080. Emails from the dev system land in Mailpit at http://127.0.0.1:8025.
- The password of the default administrator account (`admin@f-eco.local`) is generated into `.dev/admin-password`, under the folder you ran the command from.
- `dev` mode is for trying things out only. For a real deployment, set `FECO_ENV=production` and the required variables in [Configuration](#configuration).
- Stop and delete the trial data: `docker compose -f infra/docker-compose.yml down`.

For administration, use `feco-catalog` from `bin/` instead of `cargo run -p feco-server --bin feco-catalog --` in the sections below. Developers use `bin/f-dev` instead of `cargo run -p f-dev --`.

### Phone

The two mobile packages in a release are **test builds**, meant for a dev server running on your computer:

- **Android** (`feco-mobile-<version>.apk`): signed with a debug key, connects to `http://10.0.2.2:8080`, which is the computer running the Android emulator. Install it into the emulator with `adb install feco-mobile-1.20.0.apk`. On a real phone the app cannot reach a server yet; rebuild it with your server's `FECO_SERVER` and `FECO_ISSUER` (see [clients/feco-mobile/README.md](clients/feco-mobile/README.md)).
- **iOS** (`feco-mobile-<version>-ios-simulator.zip`): runs only in Xcode's iOS Simulator. Extract it, then run `xcrun simctl install booted Runner.app`. There is no package for a real iPhone yet (it needs signing with an Apple Developer account).

## Running the system

The system consists of `feco-server` (Rust, port 8080) and four infrastructure services: PostgreSQL (F-Eco data), Ory Kratos (accounts, passwords, sign-in sessions), Ory Hydra (OAuth 2.0/OIDC for sub-apps) and Mailpit (a fake mailbox for viewing email during development).

```text
 Browser ──► feco-server :8080 ──► Kratos :4433/:4434 ──► Mailpit :1025/:8025
     │                │
     │                ├──► Hydra :4445 (admin)
     │                └──► PostgreSQL :5432
     └──► Hydra :4444 (OAuth for sub-apps) ──► sub-app (e.g. feco-demo-app :9999)
```

### 1. Prerequisites

You need:

| Tool | Version | Notes |
|---|---|---|
| [Rust](https://rustup.rs) | 1.85 or later | Run `rustc --version` to check |
| Git | any | On Windows, use Git Bash for the bash commands below |
| **Option A:** [Docker](https://docs.docker.com/get-docker/) | Docker Compose v2 | Simplest: one command starts the whole infrastructure |
| **Option B:** native programs | PostgreSQL 17, [Ory Kratos v1.3.1](https://github.com/ory/kratos/releases/tag/v1.3.1), [Ory Hydra v2.3.0](https://github.com/ory/hydra/releases/tag/v2.3.0), [Mailpit](https://github.com/axllent/mailpit/releases), Python 3 | When Docker is not available. On Windows, download the `windows_sqlite_64bit` build of Kratos and Hydra |

Get the source code:

```bash
git clone https://github.com/thanhlnt3/f-ecosystem-software.git
cd f-ecosystem-software
```

### 2. Start the infrastructure

**Option A: Docker**

```bash
docker compose -f infra/docker-compose.yml up -d --wait
```

The command waits until PostgreSQL, Valkey, Mailpit, Kratos and Hydra are all ready. To check:

```bash
curl http://127.0.0.1:4433/health/ready   # Kratos: {"status":"ok"}
curl http://127.0.0.1:4444/health/ready   # Hydra:  {"status":"ok"}
```

**Option B: run natively, no Docker needed**

Open four terminal windows (Git Bash on Windows) in the project directory, each running one service. If the programs are not on your `PATH`, point to them through environment variables as in the example:

```bash
PG_BIN=/path/to/pgsql/bin scripts/dev-local.sh postgres     # creates the feco database on first run
MAILPIT_BIN=/path/to/mailpit scripts/dev-local.sh mailpit
KRATOS_BIN=/path/to/kratos scripts/dev-local.sh kratos
HYDRA_BIN=/path/to/hydra scripts/dev-local.sh hydra
```

`scripts/dev-local.sh` uses exactly the configuration in `infra/`, so behavior is identical to option A. Data lives in `infra/.data` (not committed): PostgreSQL in `infra/.data/pg`, Kratos and Hydra in `kratos.sqlite` and `hydra.sqlite`, so accounts and OAuth clients survive restarts. To run Kratos and Hydra in memory as before (every start is clean), set `FECO_DEV_MEMORY=1`.

### 3. Run feco-server

```bash
FECO_ENV=dev cargo run -p feco-server
```

On PowerShell:

```powershell
$env:FECO_ENV = "dev"; cargo run -p feco-server
```

The first build takes a few minutes. When you see the line `feco-server đang chạy` (feco-server is running), check:

```bash
curl http://127.0.0.1:8080/healthz
# {"status":"ok","environment":"dev","version":"1.20.0"}
```

In the `dev` environment, every address has a default value pointing to the infrastructure from step 2, and the database schema is created automatically at startup. See also the [Configuration](#configuration) section.

### 4. Try the account features

Open http://127.0.0.1:8080 in a browser. When you are not signed in, the home page is a landing page introducing F-Store, the Developer Portal and privacy, with **Sign up** and **Sign in** buttons. The UI follows the browser language; change it with the **VI · EN · FR · 日本語** button in the top-right corner. The **Appearance** (Giao diện) button next to it opens the customization page: style (Plain, Elegant, Parchment, Fresco, Starry night, Lotus pond, Cubist, Lily, Old quarter, Lacquer), light/dark theme, accent color, density, font size. Customizations are stored in the browser (cookie `feco_ui`).

1. **Sign up** at http://127.0.0.1:8080/registration. The password must be at least 10 characters long and must not appear in known data breaches, so for example `Password123!` will be rejected.
   - Choose a **Role**: General user (default), Individual customer, Business customer, Individual developer or Software company.
   - Business customers and Software companies must enter a legal name and a tax code (10 digits, for example `0312345678`).
   - Roles other than General user wait for a System administrator to approve or reject them at http://127.0.0.1:8080/admin/members. The request shows "Chosen at sign-up" and whether the email is verified. The result arrives by email; until then, the account is a General user.
2. **Verify your email**: open Mailpit at http://127.0.0.1:8025. By default all emails from the dev system are kept here and never sent out. Open the email "Xác minh email tài khoản F-Eco" (Verify your F-Eco account email) and click the link. To deliver emails to a real inbox (for example Gmail), see [Sending email to real inboxes](#sending-email-to-real-inboxes).
3. **Sign in** at http://127.0.0.1:8080/login. The home page is the **profile page**: name, email and verification status, role, join date; stat cards (connected apps, devices, orders, unread notifications); shortcuts to security (including two-factor status), privacy, shopping, and role-based tools.
4. **Settings** at http://127.0.0.1:8080/settings:
   - edit your profile (name, photo, phone number, invoicing details), change your password;
   - enable an **authenticator app (TOTP)**: scan the QR code with Google Authenticator, Microsoft Authenticator or a similar app, then enter the 6-digit code;
   - generate **recovery codes** and save them. From your next sign-in on, you must enter a TOTP code or a recovery code.
5. **Signed-in devices** at http://127.0.0.1:8080/sessions: sign in with a second browser to see two devices, then try "Sign out this device".
6. **Forgot password** at http://127.0.0.1:8080/recovery: a 6-digit recovery code is sent to Mailpit.
7. **New-device alert**: sign in with a different browser (a different browser or operating system) to receive the email "Đăng nhập từ thiết bị mới" (New sign-in to your F-Eco account) in Mailpit. The "This wasn't me" link in the email signs out all devices.
8. **Password-guessing protection**: enter a wrong password 5 times in a row. Each failure is slowed down progressively, and from the 5th failure an "I am not a robot" box appears (a test challenge, available only in dev). The account owner receives a warning email in Mailpit.
9. **Data export** at http://127.0.0.1:8080/account/export: a link to download a ZIP file is sent to Mailpit.
10. **Delete account** at http://127.0.0.1:8080/account/delete: the account waits 30 days before deletion; sign in again during that time to cancel.
11. **Sign in with Google or GitHub**: the buttons appear on the sign-in and sign-up pages once the providers are configured (Configuration section).
    - A new account gets the email verified by the provider. An email the provider has not verified is not used.
    - If the email matches an existing account, F-Eco asks you to sign in with the old password once to link them.
12. **Refresh token reuse alert**: when an old refresh token of F-Launcher or a child app is used again, that session is signed out and you get the email "Your … session may have been copied" with the device and IP.

### 5. Try single sign-on with the sample app

`tools/feco-demo-app` is a sample sub-app. It registers itself with Hydra (declaring the `profile` scope as required; `email`, `locale`, `phone` as optional), starts an OAuth 2.0 flow with PKCE, receives the result at `http://127.0.0.1:9999/cb`, then calls `GET /v1/me`.

```bash
cargo run -p feco-demo-app
```

1. The app prints a `client_id` and an address starting with `http://127.0.0.1:4444/oauth2/auth?...`. Open that address in a browser.
2. If you are not yet signed in to F-Eco, you are taken to the sign-in page, then automatically returned to the flow.
3. The **consent screen** shows the app name, the developer, and each type of data with its purpose. "Display name and avatar" is required; email is optional and unchecked by default. Choose as you like, then click **Allow** (or **Deny**).
4. Back in the terminal: the app prints the granted scopes and the `/v1/me` result, for example:

   ```json
   {
     "email": "ban@example.com",
     "email_verified": true,
     "name": "Lan",
     "picture": null,
     "sub": "PYUOrskLAhLL-pdLQKJqyUHw8B719Zbi7--ZuwdfaSw"
   }
   ```

   `sub` is an identifier specific to this app: each app receives a different value for the same person. If you did not allow email, the result has no `email`.
5. Open http://127.0.0.1:8080/apps to see "Demo App" under **Connected apps**, with the data granted and the most recent access. Click **Revoke access**: the app's tokens become invalid immediately, and the app has to ask again next time.

Run again with the same app (this time the already granted scopes are not asked again):

```bash
cargo run -p feco-demo-app -- --client-id <client_id printed last time>
cargo run -p feco-demo-app -- --client-id <client_id> --scope "openid profile phone"   # request an additional scope
```

### 6. Try F-Launcher

F-Launcher (`clients/feco-launcher`) is a desktop application. The dev build reads its catalog from a local
TUF repository that the script below sets up. Apps are added to the repository with `scripts/dev-apps.sh` (step 9): the
sample apps Notes (Ghi chú) and Stopwatch (Bấm giờ), which use F-SDK, and the open-source apps from GitHub. Demo App is no longer in the
app repository; it is used only to try single sign-on in step 5.

1. Preparation (Hydra must be running). The script creates the keys and the TUF repository in `.dev/` (not committed), registers
   the OAuth clients of the launcher, of Demo App and of the sample apps in Hydra, then prints the environment
   variables you need. A dev environment from version 1.x that still has Demo App in the repository can remove it with
   `scripts/dev-launcher.sh remove-demo`.

   ```bash
   scripts/dev-launcher.sh setup
   ```

2. Run `feco-server` again (step 3) with the `FECO_LAUNCHER_CLIENT_ID` that the script printed. This variable enables launch
   codes and lets the launcher sign in without a consent screen (the launcher is a platform app).
3. Run the launcher with the two variables the script printed:

   ```bash
   export FECO_LAUNCHER_CLIENT_ID=<launcher client_id>
   export FECO_LAUNCHER_TUF=<path to .dev/tuf/repo>
   cargo run -p feco-launcher
   ```

4. Click **Sign in**: the default browser opens the F-Eco sign-in page, after which the launcher shows your name.
   The refresh token is stored in Windows Credential Manager (macOS: Keychain), so you stay signed in
   after closing and reopening the launcher or restarting the machine.
5. Add apps to the repository (`scripts/dev-apps.sh all`, step 9), click **Sync** (Đồng bộ), then go to the **Store** page
   (left sidebar) → **Install** Notes. The confirmation dialog contains the line "This app runs with the same
   permissions as any other application on your machine". The package is checked for signature and SHA-256 through TUF and then installed to
   `%LOCALAPPDATA%\F-Eco\Launcher\apps` (macOS: `~/Library/Application Support/F-Eco/Launcher`).
6. On the **Library** page → **Open** Notes: the launcher requests a one-time launch code (valid for 60 seconds) and then
   runs the app. The app uses F-SDK to exchange the code for a token, needs no password, and greets you by name. The first time,
   if you have not yet allowed the app to read your profile, F-SDK opens the browser to ask.
7. Try update, rollback and revocation:

   ```bash
   scripts/dev-launcher.sh publish   # signs a new Notes version → the launcher shows "Update to ..."
   scripts/dev-launcher.sh revoke    # revokes the latest Notes version → after the next sync it cannot be opened, and the reason is shown
   scripts/dev-launcher.sh refresh   # the TUF timestamp expires after 1 hour: run again when the launcher reports expired metadata
   ```

   Try F-Launcher self-update: build a release with a version newer than the running one, then sign it into the dev repository. The older F-Launcher downloads it after the next sync, shows a **Restart to update** button, and installs the new version on restart.

   ```bash
   cargo build --release -p feco-launcher
   scripts/dev-launcher.sh publish-self   # package feco-launcher-<version>-<platform>.zip, app id feco-launcher
   ```

   Click **Sync** to make the launcher sync right away (normally it syncs every hour). **Roll back** (Quay lại) restores the previous version
   without downloading it again. **Uninstall** asks whether to delete local data (kept by default) and reminds you that data permissions
   can only be revoked on the Connected apps page.
8. Disconnect the network (or stop serving the repository directory) and reopen the launcher: the Library can still open installed apps, and the
   sidebar shows "Offline".
9. Add apps to see the grid, list and filters clearly:

   ```bash
   scripts/dev-apps.sh all
   ```

   This command signs five apps into the dev repository; then click **Sync** to see them in the Store:
   - two sample apps using F-SDK: **Notes** (Ghi chú) and **Stopwatch** (Bấm giờ) (`examples/apps`);
   - three open-source apps built from GitHub: **kibi** (terminal text editor), **bottom** (terminal system
     monitor), **eframe template** (windowed app).

   Step-by-step instructions are in [docs/en/github-apps.md](docs/en/github-apps.md). Terminal apps open in their own
   console window.
10. The launcher UI (hover over an icon to see its tooltip):
    - the **Grid** / **List** icons at the right of the toolbar; in grid mode all tiles have the same size;
    - the search box (diacritic-insensitive, for example "ghi chu"), the category filter and the sort order (name, recently
      opened, recently installed, category, size);
    - **Settings** (the gear icon in the sidebar) opens in its own window: light, dark or system theme; accent color; font size 80–150%; default
      layout; grid tile size; start page; short descriptions; compact mode; restore defaults.
    - defaults (since 1.20.0): Elegant style, light theme, large grid tiles, a 1280 × 800 window.
      The Library page opens with a "Your workspace" header showing a greeting, installed apps and available updates.

    All choices are saved in `%LOCALAPPDATA%\F-Eco\Launcher\settings.json` and take effect immediately.
11. Click an app to open its detail page in its own window (one window per app, up to 6; clicking again brings that window to the front): introduction, features, what's new, an info table, the buttons
    Install, Open, Uninstall, Source code, Website. The **‹ Previous** / **Next ›** buttons (or the left and right arrow keys)
    switch to the neighboring app in the order currently displayed; **Esc** returns to the list.
12. **Language and style**: Settings → Appearance. Choose "English", "Français" or "日本語" to
    change the language immediately. Click a preview tile to change the style: Plain (Chân phương), Elegant, Parchment, Fresco,
    Starry night, Lotus pond, Cubist, Lily, Old quarter, Lacquer. The last eight styles use their own color palettes, so the
    theme and accent color controls are disabled.
13. **Role**: below the user name in the sidebar is the platform role (App user, Developer or
    Administrator). Developers get a **Developer portal** button; Administrators additionally get
    **Admin console**. Change the role at `/admin/members` (System administrator) or with
    `feco-catalog role-set <email> developer "<reason>"`.
14. **Collapsed sidebar**: click the button next to the logo (or press Ctrl+B) to show only icons
    in the sidebar. Hover an icon to see its name; unread notifications and available updates show
    as badges. The launcher remembers your choice; windows narrower than 840 px collapse the
    sidebar automatically.
15. **ThesisAI web app**: a web app that runs in the browser and signs in with your F-Eco account.

    ```bash
    scripts/dev-apps.sh thesisai
    ```

    Click **Sync**. ThesisAI shows a "Web app" label in the Store: **Add**, then **Open** to open
    `http://localhost:3000/auth/feco` in your browser. To run ThesisAI itself, follow
    [integrations/thesisai/README.md](integrations/thesisai/README.md) (apply the "Sign in with
    F-Eco" patch and set the `FECO_*` variables the command above prints).
16. **Mobile F-Launcher** (`clients/feco-mobile`, Flutter): a companion app for Android 10+ and iOS 18+.
    - Five tabs: My apps, Store (open web apps, browse desktop apps), Wallet (buy apps), Notifications, Account.
    - Sign in with your F-Eco account through the system browser.
    - `scripts/dev-launcher.sh setup` creates the mobile OAuth client. Run `feco-server` with the `FECO_MOBILE_CLIENT_ID` the script prints, then run the app on an Android emulator:

    ```bash
    cd clients/feco-mobile && flutter run --dart-define=FECO_CLIENT_ID=$(cat ../../.dev/mobile-client-id)
    ```

    Add `--dart-define=FECO_DEMO=true` to try it with sample data and no server. Details in [clients/feco-mobile/README.md](clients/feco-mobile/README.md).
17. **Mentor**: a learning app for students and teachers, as both a web app and a Windows desktop app, signing in with your F-Eco account.

    ```bash
    scripts/dev-apps.sh mentor
    ```

    - The command creates a server-side OAuth client, signs Mentor's web target and writes the catalog. Set `MENTOR_DESKTOP_ZIP` to the release's `mentor-<version>-windows-x64.zip` to add the desktop app.
    - Run the Mentor gateway (port 8100) and web app (port 5173) as in [integrations/mentor/README.md](integrations/mentor/README.md), with the `FECO_*` variables the command prints.
    - In the launcher: **Sync**, Store → Mentor → **Add** → **Open**. Or open `http://localhost:5173` and click "Sign in with F-Eco".
    - On first sign-in Mentor creates a student account. A Mentor admin promotes teachers.
    - Before integrating an outside app, scan its repository at the exact commit you will use. Reports go to `.dev/scans/`; reviewed findings are recorded in `integrations/<app>/scan-allow.toml` and summarized in `integrations/<app>/SCAN.md`:

      ```bash
      scripts/scan-integration.sh katD2707/agent-mentor-maximum-experience "$(cat integrations/mentor/MENTOR_COMMIT)" integrations/mentor/scan-allow.toml
      ```
18. **BinhAI Office web app**: a prototype "AI office" with 18 agents and a 3D office, running entirely in the browser with no sign-in. F-Eco builds the app from the scanned source, then compares it with the author's prebuilt copy:

    ```bash
    integrations/binhai-office/build.sh ~/.feco-dev-ext/binhai-office/site --verify-dist
    python -m http.server 5180 --bind 127.0.0.1 --directory ~/.feco-dev-ext/binhai-office/site
    scripts/dev-apps.sh binhai
    ```

    Click **Sync**, then in the Store choose BinhAI Office → **Add** → **Open**. The security scan results are in [integrations/binhai-office/SCAN.md](integrations/binhai-office/SCAN.md).
19. **VPdf desktop app** (Windows x64): an offline PDF reader with Vietnamese search that ignores diacritics. F-Eco builds the package from the scanned source, with PDFium and qpdf (SHA-256 checked):

    ```bash
    integrations/vpdf/build.sh ~/.feco-dev-ext/vpdf/pkg
    scripts/dev-apps.sh vpdf
    ```

    Click **Sync**, then in the Store choose VPdf → **Install** → **Open**. Security scan and licenses: [integrations/vpdf/SCAN.md](integrations/vpdf/SCAN.md), [integrations/vpdf/NOTICE.md](integrations/vpdf/NOTICE.md).
20. **Messages**: the sidebar has a "Messages" item with the unread count (refreshed every 60 seconds). The Messages page lists conversations and shows the thread; type and click **Send** or press Ctrl+Enter. While a conversation is open, the launcher checks for new messages every 5 seconds. Attachments are sent on the web only; scanned, clean files open in the browser. The mobile F-Launcher has a "Messages" tab (six tabs).

Launcher variables: `FECO_LAUNCHER_SERVER` (default `http://127.0.0.1:8080/` in debug builds only; required in release builds unless `FECO_LAUNCHER_DEV=1`), `FECO_LAUNCHER_HYDRA`
(`http://127.0.0.1:4444/`, same rule), `FECO_LAUNCHER_DATA_DIR` (data directory), `FECO_LAUNCHER_TRUSTED_ROOT`
(the trusted root.json; by default `metadata/1.root.json` of the local repository), `FECO_LAUNCHER_FONT` (a `.ttf`
font if the machine has no Segoe UI/Arial). Outside dev, the addresses must be HTTPS.

F-Store: `scripts/dev-apps.sh` also puts the apps into the catalog; see them at
http://127.0.0.1:8080/store. After installing an app with the launcher (signed in), you can write a
review on its detail page. Until the Developer Portal and Admin Console exist (groups 8 and 14), manage the
catalog with the command-line tool (it reads the same configuration as `feco-server`):

```bash
cargo run -p feco-server --bin feco-catalog -- add-app docs/catalog-app.example.json
cargo run -p feco-server --bin feco-catalog -- release <app> <version> windows-x64 <size in bytes> <TUF target name> "<changes>"
cargo run -p feco-server --bin feco-catalog -- remove <app> "<reason>"           # remove from the catalog
cargo run -p feco-server --bin feco-catalog -- reports                          # abuse report queue
cargo run -p feco-server --bin feco-catalog -- resolve <id> upheld "<note>"  # resolve, and email the reporter
```

### 7. Try the Developer Portal

1. Sign in at http://127.0.0.1:8080, verify your email (the link is in Mailpit), then go to **Account
   settings** to enable an authenticator app (TOTP). The portal requires both before it lets you register.
2. Open http://127.0.0.1:8080/developer, register as an **Individual** or an **Organization** and tick to accept the Developer
   Agreement. An organization is issued a DNS TXT record to verify its domain; the dev build has
   no real DNS, so this step is skipped (apps can still be released, they just lack the "Verified" badge).
3. **Create a new app**: app ID, name, type (desktop: a public client using PKCE; with its own server: has a
   client secret, shown once and rotatable) and redirect URIs. The app page shows the client ID.
4. **Create a token** for `f-dev` (shown once), write `f-app.toml` (a template is at
   http://127.0.0.1:8080/developer/docs), then submit a version:

   ```bash
   cargo run -p f-dev -- validate --manifest f-app.toml
   FDEV_TOKEN=fdev_... cargo run -p f-dev -- publish --manifest f-app.toml --package dist/
   ```

   The command prints "Pending review" and the version ID. A Windows package with an executable that is not Authenticode-signed is rejected;
   a macOS package must be signed with a Developer ID and have the notarization ticket attached. You can also submit through the app page (paste the
   manifest, upload a `.zip` file for each platform, click **Submit for review**).
5. Moderation. `feco-server` runs the automated check queue every 10 seconds (it can also be run manually with
   `scan`). An app's first version, or a version that adds scopes or system permissions, goes into the manual
   review queue; other versions that pass the checks are approved automatically. Until the Admin Console exists (group 14):

   ```bash
   cargo run -p feco-server --bin feco-catalog -- scan                     # run the queue, schedule a 7-day rescan
   cargo run -p feco-server --bin feco-catalog -- queue                    # manual review queue
   cargo run -p feco-server --bin feco-catalog -- approve <app> <version> "Passed"
   cargo run -p feco-server --bin feco-catalog -- reject <app> <version> brand_impersonation "<note>"
   cargo run -p feco-server --bin feco-catalog -- appeals                  # open appeals
   cargo run -p feco-server --bin feco-catalog -- appeal <app> <version> overturn "<note>"
   cargo run -p feco-server --bin feco-catalog -- emergency <app> all "<reason>"   # emergency takedown
   cargo run -p feco-server --bin feco-catalog -- alerts                   # malware alerts, late detections
   ```

   For the clause used on rejection, see http://127.0.0.1:8080/policy/content. The developer receives an
   email (Mailpit) stating the clause and how to fix it, and clicks **Appeal** on the app page (once per
   rejection). Without ClamAV only the F-Eco signature engine runs (enough to try with an EICAR sample).

   **Source code scan.** A version that comes with source code is scanned after its packages pass:
   - secrets in the code, such as a private key or a token, get the version rejected;
   - libraries with published vulnerabilities (looked up on OSV.dev) or suspicious code send it to manual review;
   - the report shows on the app page and the review page.

   Attach the source in one of three ways: upload `source.zip` or enter a GitHub repository and commit on the app page; declare `[source]` in `f-app.toml`; or use `--source`:

   ```bash
   FDEV_TOKEN=fdev_... cargo run -p f-dev -- publish --manifest f-app.toml --package dist/ --source .
   ```

   `f-dev` leaves out `.git`, `target`, `node_modules` and real `.env` files when it packs the source.

6. On the app page, choose the `stable` or `beta` channel and a percentage (10%, 50%, 100%), then click **Release**
   (requires the Admin or Owner role). The app appears in F-Store; the scopes and purposes on the
   consent screen come from the manifest. **Stop gradual rollout** so machines that have not yet received it no longer receive it.
7. For the launcher to be able to install the version you just released, run `feco-server` with the dev TUF repository so that the server signs the package
   on release (the path is created by `scripts/dev-launcher.sh setup`):

   ```bash
   export FECO_TUF_REPO=.dev/tuf/repo
   export FECO_TUF_SIGN_KEYS=.dev/tuf/keys/targets.pk8,.dev/tuf/keys/snapshot.pk8,.dev/tuf/keys/timestamp.pk8
   ```

### 8. Write a sub-app with F-SDK

F-SDK (`sdk/`, licensed MIT OR Apache-2.0) wraps sign-in and the API for sub-apps. When F-Launcher opens an
app, it passes `FECO_LAUNCH_CODE`, `FECO_CLIENT_ID`, `FECO_ISSUER_URL` and `FECO_SERVER_URL`; F-SDK
exchanges the launch code for a token. When the user opens the app directly, F-SDK opens the browser to sign in with
PKCE and receives the result at a loopback address (by default `http://127.0.0.1:47280/callback`, changed with
`FSDK_REDIRECT_URI`; this address must be in the app's redirect list). The default scope is
`openid offline_access profile`; add `purchases` to read entitlements (`FSDK_SCOPES`).

```rust
let config = fsdk::Config::from_env()?;           // or fsdk::Config::new(client_id, issuer, server)
let session = fsdk::session(&config).await?;      // launcher if there is a launch code, otherwise the browser
let me = session.me().await?;                     // only the fields in the consented scopes
let owner = session.entitlements().await?.iter().any(|e| e.kind == "owner");
```

| Language | Directory | Sample |
|---|---|---|
| Rust | `sdk/rust` (crate `fsdk`) | `fsdk::session_from_launcher()`, `fsdk::login_with_browser()` |
| C | `sdk/c` (library `fsdk_c`, header `sdk/c/include/fsdk.h`) | `sdk/c/examples/login.c` |
| Python ≥ 3.9 | `sdk/python` (package `fsdk`, calls the C library through `ctypes`) | `sdk/python/examples/login.py` |
| TypeScript, Node.js ≥ 22.6 | `sdk/typescript` (package `@f-eco/sdk`, written purely on top of OIDC) | `sdk/typescript/examples/login.ts` |

Run the samples. `scripts/dev-launcher.sh setup` (step 6) has already created the app "QuanLyKho (app mẫu
F-SDK)" (the F-SDK sample app) in Hydra; `scripts/dev-launcher.sh env` prints the variables to set:

```bash
cargo build -p fsdk-c
eval "$(scripts/dev-launcher.sh env | sed -n '/App mẫu F-SDK/,$p')"     # sets FECO_CLIENT_ID, FECO_ISSUER_URL, ...
FSDK_LIBRARY=target/debug/libfsdk_c.so python sdk/python/examples/login.py   # Windows: fsdk_c.dll, macOS: libfsdk_c.dylib
node sdk/typescript/examples/login.ts
```

The first time, the app opens the browser for you to sign in and consent to "QuanLyKho" seeing your name and entitlements;
the terminal prints `/v1/me` (with the app's own `sub`, no email) and `"entitlements": []`. Grant the
`owner` entitlement and run again to see it:

```bash
cargo run -p feco-server --bin feco-catalog -- grant "$FECO_CLIENT_ID" <your email> owner custom-build:DEMO-1
```

To integrate without F-SDK: use any standard OIDC library pointed at
`http://127.0.0.1:4444/.well-known/openid-configuration` (for example Authlib:
`examples/standard-oidc/authlib_login.py`). To receive the events `user.revoked`, `user.deleted`,
`app.version.revoked`, `entitlement.changed`: enter the webhook address on the app page in the developer
portal, store the signing key (shown once) and verify the signature with `fsdk::verify_webhook` (or `verify_webhook`
in the Python version, `verifyWebhook` in the TypeScript version). The list of error codes and the API versioning policy:
http://127.0.0.1:8080/developer/docs.

Delivering a milestone of a build-to-order request (step 9) automatically grants the `license` entitlement to the customer. Until payments exist (group 13), other entitlements are granted by hand:

```bash
cargo run -p feco-server --bin feco-catalog -- grant <app or client ID> <email> owner custom-build:<order ID>
cargo run -p feco-server --bin feco-catalog -- ungrant <app or client ID> <email> owner
```

### 9. Try build-to-order requests

1. Sign in with an account whose email is verified, and open http://127.0.0.1:8080/orders (or the "App build orders" link on the home page). Fill in the request, attach the spec (up to 200 MB in total), read and accept the app-building Terms of Service (http://127.0.0.1:8080/policy/custom-build), and click **Submit request**. Mailpit receives a confirmation email with the order ID (`DH-000001`), and the recipients in `FECO_ADMIN_EMAILS` receive the quoting task.
2. Play the F-Eco team with `feco-catalog` (the assignee's name comes from `FECO_REVIEWER`):

   ```bash
   cargo run -p feco-server --bin feco-catalog -- orders submitted                 # orders awaiting a quote
   cargo run -p feco-server --bin feco-catalog -- order-quote DH-000001 docs/order-quote.example.json
   ```

3. On the order page, review the quote (scope, exclusions, source code ownership, milestones), and click **Accept quote** or **Request changes**. Accepting creates a payment request for the deposit; pay through ZaloPay as in step 10, or (when no ZaloPay keys are set) mark it as paid by hand:

   ```bash
   cargo run -p feco-server --bin feco-catalog -- order DH-000001                  # view the payment request ID
   cargo run -p feco-server --bin feco-catalog -- order-paid <payment-request-id>
   ```

4. Deliver a milestone: the build is an app in the catalog, set to private for the customer **before** release (steps 6 and 7), then delivered together with the check results. The app appears only in that customer's F-Launcher.

   ```bash
   cargo run -p feco-server --bin feco-catalog -- private <app> <customer email>
   cargo run -p feco-server --bin feco-catalog -- order-deliver DH-000001 1 <app> <version> ket-qua-kiem-tra.txt "Change notes"
   cargo run -p feco-server --bin feco-catalog -- order-message DH-000001 "Do you receive stock by batch or item by item?"
   ```

5. The customer clicks **Accept this milestone**, **Request a fix** (which uses up a free revision) or **Open a dispute** on the order page. After 7 days without a response the platform sends a reminder, and after 14 days it accepts automatically (the server runs this every hour; `feco-catalog order-due` runs it immediately). A dispute is resolved with `order-resolve DH-000001 continue|full|partial <amount>`.
6. When the order is Completed and paid in full, set the final handover package for the customer to download: `feco-catalog order-handover DH-000001 ban-giao.zip`.

### 10. Try ZaloPay payments

1. Get the ZaloPay sandbox keys (`app_id`, `key1`, `key2`) from the documentation at https://docs.zalopay.vn (the sample set with `app_id` 2554 worked on 11 Nov 2026) or from your own sandbox account. Set them before running `feco-server`; do not write keys into files in the repo:

   ```bash
   export FECO_ZALOPAY_APP_ID=<app_id> FECO_ZALOPAY_KEY1=<key1> FECO_ZALOPAY_KEY2=<key2>
   export FECO_ZALOPAY_ENDPOINT=https://sb-openapi.zalopay.vn
   ```

   Outside production only the sandbox endpoint is accepted; if you point to `openapi.zalopay.vn` the server refuses to start.
2. On the order page (step 9), after accepting the quote, click **Pay** on the deposit line: the page shows the amount, the milestone description and four payment methods. Choose a method to go to ZaloPay's page (pay with the ZaloPay sandbox app or a test card from the ZaloPay documentation). The link expires after 15 minutes.
3. On a dev machine, ZaloPay cannot call back to `127.0.0.1`: the status is updated when you return to the redirect page (the server queries the gateway), or within a minute thanks to the active status query. The order changes to "In progress", and an email with the invoice goes to Mailpit.
4. **Payment history** (http://127.0.0.1:8080/payments): transactions, refunds, re-downloading invoices, and the invoicing details (individual or business with a tax code).
5. Play the administrator:

   ```bash
   cargo run -p feco-server --bin feco-catalog -- payments DH-000001                       # the order's transactions
   FECO_REVIEWER=lan cargo run -p feco-server --bin feco-catalog -- refund <transaction-id> 4000000 "Milestone 2 dispute"
   FECO_REVIEWER=minh cargo run -p feco-server --bin feco-catalog -- refund-approve <refund-id>   # from 10 million a second person is needed
   cargo run -p feco-server --bin feco-catalog -- reconcile                                # reconcile yesterday
   ```

Without ZaloPay keys the payment page reports that the gateway is not configured; in dev you can record a payment by hand with `feco-catalog order-paid <payment-request-id>`. To test signatures against the real sandbox on their own: set `FECO_ZALOPAY_SANDBOX_APP_ID`, `_KEY1`, `_KEY2`, then run `cargo test -p feco-server --test zalopay_sandbox`.

### 11. Try the Admin Console and notifications

1. **The default administrator account** is always there: the server creates or restores it at every start and every 10 minutes, with all five roles; it cannot be demoted or deleted.
   - In dev, the email is `admin@f-eco.local`; a random password is generated into `.dev/admin-password` (not committed) on the first run.
   - In staging and production, set `FECO_ADMIN_EMAIL` and `FECO_ADMIN_PASSWORD_FILE` (a secret file, password of at least 12 characters); without them the server does not start.
   - Like every administrator, add a passkey before entering the admin console (step 2).

   To give admin roles to your own account (the email must be verified):

   ```bash
   cargo run -p feco-server --bin feco-catalog -- admin-set <email> project_manager,finance,sysadmin
   ```

2. The Admin Console needs a passkey or security key: go to Settings (http://127.0.0.1:8080/settings), click **Add passkey**, then sign out and sign in again (password, then passkey). Open http://127.0.0.1:8080/admin: the **Overview** page has stat cards by role (Developer role requests, users, versions awaiting review, orders due soon, refunds awaiting approval, 30-day revenue and a 14-day chart); the sidebar shows only the sections for the roles you have (review queue, build-to-order requests, refunds, user support which requires entering a reason, role assignment).

   Every admin list (review queue, build-to-order requests, refunds, role assignment, projects, chat reports, F-Coin withdrawal and grant queues) has a toolbar:
   - search that ignores case and Vietnamese diacritics;
   - status and date-range filters;
   - sorting by newest, oldest, sender A–Z or Z–A;
   - "Clear filters" and a "Showing N of M" line.

   The filters live in the page address, for example http://127.0.0.1:8080/admin/orders?q=an&status=all&sort=sender. The order list shows open orders by default; choose "All" to include completed or cancelled orders.

   In F-Launcher, the **Admin console** button opens a menu of the sections you can enter. On the phone, the **Account** tab has an **Admin console** card. Both open the matching web page.

   Browsers usually do not allow creating a passkey on the `127.0.0.1` address. On a dev machine, set `FECO_ADMIN_PASSKEY_OPTIONAL=true` (together with `FECO_ENV=dev`) to enter the Admin Console with a password only; every admin page then shows a warning. This variable makes staging and production refuse to start.
3. Notification center: http://127.0.0.1:8080/notifications (and the **Notifications** tab in F-Launcher). Turn app update notifications, app suggestions and the newsletter on or off; newsletter emails have an unsubscribe link that works without signing in.
4. Add the VNPAY and MoMo gateways and MISA e-invoicing through environment variables (your own sandbox keys, never written into the repo): `FECO_VNPAY_TMN_CODE`, `FECO_VNPAY_HASH_SECRET`, `FECO_VNPAY_PAYMENT_URL`; `FECO_MOMO_PARTNER_CODE`, `FECO_MOMO_ACCESS_KEY`, `FECO_MOMO_SECRET_KEY`, `FECO_MOMO_ENDPOINT`; `FECO_MISA_CLIENT_ID`, `FECO_MISA_CLIENT_SECRET`, `FECO_MISA_TAX_CODE`, `FECO_MISA_USERNAME`, `FECO_MISA_PASSWORD`, `FECO_MISA_INV_SERIES`, `FECO_MISA_VAT_RATE`. The gateways call back to `FECO_PAYMENT_CALLBACK_BASE` (a public address, for example a tunnel).
5. Check the email-sending domain: `cargo run -p feco-server --bin feco-catalog -- mail-dns <domain> <DKIM selector>`. Production accepts only an `FECO_SMTP_URL` of the form `smtps://` or `smtp://...?tls=starttls`.

### 12. Try the F-Coin wallet, roles and projects

F-Coin is a payment unit used only inside F-Eco. The initial rate is 1 F-Coin = 1,000 VND; a Finance administrator can change it.

1. **Wallet**: open http://127.0.0.1:8080/wallet.
   - Top up: enter an amount of F-Coin (10–50,000) and choose a payment gateway (configure the ZaloPay, VNPAY or MoMo sandbox from steps 10 and 11). The F-Coin is added when the gateway confirms the payment.
   - On a dev machine without a gateway: record the payment with `feco-catalog` or the manual payment page.
   - Withdraw: refund to an earlier top-up (through the gateway) or bank transfer. The F-Coin is held at once and waits for Finance to approve. `FECO_WALLET_WITHDRAWALS=false` stops new withdrawal requests.
   - See the transaction history (filter by type) and monthly income and spending at http://127.0.0.1:8080/wallet/stats.
2. **Buy apps**: paid apps on F-Store and in F-Launcher show **Buy · N F-Coin**. Developers set the price on the app page of the Developer Portal.
3. **Roles**: http://127.0.0.1:8080/account/role to request Individual customer, Business customer, Individual developer or Software company. A System administrator approves at `/admin/members`, or assigns directly:

   ```bash
   cargo run -p feco-server --bin feco-catalog -- role-set <email> customer "Try projects"
   ```

4. **Projects**: http://127.0.0.1:8080/projects.
   - Developers and companies turn on a provider profile.
   - Customers find a provider, send a request and negotiate the price.
   - A business or company draws up a contract, and both sides sign.
   - The customer funds escrow in F-Coin, receives the delivery, and accepting it pays the provider.
   - A Project manager resolves disputes at `/admin/projects`.
5. **Wallet admin**: http://127.0.0.1:8080/admin/wallet (Finance, System administrator) shows F-Coin totals, rate (each change limited to 20%), fees, withdrawal approvals, wallet freezing and transaction lookup.
   - **Transfer F-Coin** (Finance): enter the recipient's (verified) email, the amount and a reason, and choose **My wallet** or **Platform fund**. Platform-fund grants of 1,000,000 VND or more wait for another Finance administrator under "F-Coin grant requests awaiting approval".
   - Administrators cannot approve or mark as paid their own withdrawals. A withdrawal needs two approvals when the user's withdrawals in 24 hours total 10,000,000 VND or more.

   Check the ledger:

   ```bash
   cargo run -p feco-server --bin feco-catalog -- wallet-check
   ```

6. **My apps**: http://127.0.0.1:8080/library lists apps installed through F-Launcher and apps you bought, with an "Update available" label. F-Launcher suggests updates after every sync.
   - "Update available" only counts versions released on the stable channel to every device. Staged, beta and paused versions do not count.
   - F-Launcher tells the server after updating, rolling back or uninstalling an app. The web and the phone then stop showing "Update available" once you have updated on the computer.
   - The "update available" notification goes out once per version, when it reaches everyone, and only to people on an older version. Versions released with `feco-catalog release` are announced within 10 minutes.
   - On the phone, the "My apps" tab shows a badge with the number of apps that have an update, plus a line "Open F-Launcher on your computer to update". The app reloads the list when you come back to it.

7. **Messages**: http://127.0.0.1:8080/messages.
   - A customer clicks **Message the developer** on an F-Store app page or on the developer list in Projects.
   - Each project has one conversation between the customer and the provider; old project messages are moved into it.
   - Messages arrive without reloading the page (Server-Sent Events). Each message can carry one file of up to 20 MB, scanned for malware before it can be downloaded.
   - Developers can block a direct conversation; anyone can report one. Support admins review reports at `/admin/chat-reports`.
   - Notifications and emails only say "New message from …", never the content, at most once every 15 minutes per conversation. Turn the emails off in the notification settings, group "Messages".

### 13. Stop and wipe data

- Stop `feco-server` and the sample app with `Ctrl+C`.
- Option A: `docker compose -f infra/docker-compose.yml down`. This stops everything and deletes all dev data, because the compose file uses no volumes.
- Option B: `Ctrl+C` each window. To delete the PostgreSQL data, delete the `infra/.data/pg` directory.

## Service addresses

| Service | Address | Used for |
|---|---|---|
| feco-server | http://127.0.0.1:8080 | Account pages, the consent screen, F-Store (`/store`), the `/v1/...` API |
| Mailpit | http://127.0.0.1:8025 (SMTP 1025) | Viewing the dev system's email |
| Kratos public / admin | http://127.0.0.1:4433 / :4434 | Accounts (called by feco-server; do not open directly) |
| Hydra public / admin | http://127.0.0.1:4444 / :4445 | OAuth 2.0/OIDC for sub-apps; `/.well-known/openid-configuration` |
| PostgreSQL | 127.0.0.1:5432, user `feco`, database `feco` | Consents, access log, webhooks, data export, audit log |
| Valkey | 127.0.0.1:6379 | Not used yet (reserved for shared counters when running multiple instances) |

The main `feco-server` APIs:

| Path | Description |
|---|---|
| `GET /healthz` | Health check |
| `/registration`, `/login`, `/verification`, `/recovery`, `/settings`, `/logout` | Account pages (via Kratos) |
| `/sessions`, `/apps`, `/account/export`, `/account/delete`, `/lang/{vi\|en}` | Pages handled by feco-server |
| `/security/not-me/{token}` | The "This wasn't me" button in the new-device alert email |
| `/oauth/login`, `/oauth/consent`, `/oauth/logout` | Hydra's login & consent provider |
| `POST /oauth2/token` | Token endpoint: forwards to Hydra and records refresh token hashes to detect reuse. Hydra's discovery document points `token_endpoint` here |
| `GET /auth/after-login?next=` | Destination after sign-in and sign-up: records the audit log and new-device check for Google/GitHub sign-ins, then redirects to `next` (inside F-Eco only) |
| `POST /v1/launch-codes` | F-Launcher tokens only: `{"client_id"}` → a one-time launch code, valid for 60 seconds |
| `/store`, `/store/{id}` | F-Store: catalog, search (`?q=&category=&os=windows\|macos&price=free\|paid&verified=1`), detail page; `POST /store/{id}/reviews`, `/store/{id}/reviews/{rid}/reply`, `/store/{id}/report` |
| `GET /v1/catalog`, `GET /v1/catalog/{id}` | The catalog as JSON (same search parameters); the detail includes scopes, purposes, versions, reviews |
| `POST /v1/catalog/installs` | F-Launcher tokens only: `{"app_id", "version"}` records an install, update or rollback (a prerequisite for reviewing) |
| `DELETE /v1/catalog/installs/{app}` | F-Launcher tokens only: the app was uninstalled; it leaves "My apps" (unless bought) and no longer gets update notifications |
| `GET /v1/catalog/{id}/update?platform=&device=&channel=` | The version a launcher should use, by channel and gradual rollout percentage |
| `/developer`, `/developer/apps/{id}`, `/developer/docs` | The Developer Portal and its documentation |
| `POST /v1/dev/apps/{app}/versions`, `PUT /v1/dev/versions/{id}/packages/{platform}`, `POST /v1/dev/versions/{id}/submit` | API for the `f-dev` CLI (token `fdev_...`): submit `f-app.toml`, upload a package (header `X-Feco-Sha256`, up to 2 GB), submit for review |
| `/policy/content` | Content policy (the clauses cited when rejecting a version) |
| `POST /v1/apps/crashes` | An app reports a runtime crash (`{"version"}`, the app's access token); used only for aggregate metrics |
| `POST /v1/launch-codes/exchange` | Sub-app: `{"code", "client_id", "redirect_uri", "code_challenge", "scope"}` → an `authorization_code` to exchange for a token at Hydra with PKCE. Only re-issues scopes the user has already consented to; if not yet consented it returns `consent_required` |
| `/wallet`, `/wallet/stats`, `POST /wallet/topup`, `POST /wallet/withdraw` | F-Coin wallet: balance, top-up, withdrawal, history, statistics |
| `GET /v1/library` | F-Launcher token only (desktop or mobile): "My apps" as JSON |
| `GET /v1/chat/unread` | F-Launcher token: total unread messages |
| `GET`, `POST /v1/chat/conversations` | F-Launcher token: list conversations; start one with `{"developer"}` or `{"app_id"}` |
| `GET`, `POST /v1/chat/conversations/{id}/messages` | F-Launcher token: messages after `?after=<id>`; send `{"body"}` |
| `POST /v1/chat/conversations/{id}/read` | F-Launcher token: `{"up_to"}`, mark as read |
| `PUT /v1/dev/versions/{id}/source` | `fdev_` token: source code as a zip (header `X-Feco-Sha256`) |
| `GET /v1/wallet`, `POST /v1/wallet/purchases` | F-Launcher token only: balance, owned apps, prices; buy an app `{"app_id", "expected_price"}` |
| `/account/role`, `/projects`, `/projects/{id}`, `/library` | Role requests, software projects, my apps |
| `/admin/wallet`, `/admin/projects` | F-Coin wallet admin (Finance) and projects (Project manager) |
| `GET /store/{id}/icon.png`, `GET /favicon.svg` | App logos and the F-Launcher logo |
| `GET /v1/me` | User data according to the access token's scopes (`Authorization: Bearer …`); `?fields=email,phone_number` selects fields. Errors return JSON `{error, message, message_vi, request_id}` |

## Configuration

`feco-server` reads its configuration from environment variables and **refuses to start** (exit code 78) if the configuration is wrong. In `dev` and `staging`, the variables below have defaults pointing to the infrastructure from step 2; in `production`, the variables marked "required" must be set.

| Variable | Default (dev) | Notes |
|---|---|---|
| `FECO_ENV` | (required) | `dev`, `staging` or `production` |
| `FECO_HTTP_ADDR` | `127.0.0.1:8080` | |
| `FECO_PUBLIC_URL` | `http://127.0.0.1:8080/` | Required in production, must be `https` |
| `FECO_DATABASE_URL` | `postgres://feco:feco-dev-only@127.0.0.1:5432/feco` | Required in production |
| `FECO_KRATOS_PUBLIC_URL` / `FECO_KRATOS_ADMIN_URL` | `http://127.0.0.1:4433/` / `:4434/` | Required in production |
| `FECO_HYDRA_PUBLIC_URL` / `FECO_HYDRA_ADMIN_URL` | `http://127.0.0.1:4444/` / `:4445/` | Required in production |
| `FECO_PAIRWISE_SECRET` | fixed dev key | Required in production, at least 32 characters; changing the key changes every app's `sub` |
| `FECO_SMTP_URL` | `smtp://127.0.0.1:1025` | Required in production |
| `FECO_SMTP_URL_FILE` | not set | A secret file holding the SMTP URL (with its password), instead of `FECO_SMTP_URL`. `FECO_SMTP_URL` wins if both are set |
| `FECO_EMAIL_FROM` | `F-Eco <no-reply@f-eco.example>` | |
| `FECO_CHALLENGE_PROVIDER` | `test` | `test` (dev/staging only) or `turnstile` (Cloudflare Turnstile). Production forbids `test` |
| `FECO_TURNSTILE_SITE_KEY` / `FECO_TURNSTILE_SECRET_KEY` | | Required when `FECO_CHALLENGE_PROVIDER=turnstile` |
| `FECO_TRUSTED_PROXIES` | empty | Comma-separated IPs of the reverse proxy in front. `X-Forwarded-For` is trusted only on connections from these IPs |
| `FECO_DATA_KEY` | fixed dev key | AES-256 key (32 bytes, base64) that encrypts webhook keys and data export files at rest. Required in production. Generate with: `openssl rand -base64 32` |
| `FECO_RATE_IP_PER_MIN` | `1200` | Requests per minute from one IP; beyond that it returns 429 with `Retry-After` |
| `FECO_RATE_APP_PER_MIN` / `FECO_RATE_USER_PER_MIN` | `1200` / `120` | `/v1/me` calls per minute per app / per user |
| `FECO_DNS_SERVERS` | `1.1.1.1:53,8.8.8.8:53` | DNS servers used to look up TXT records when verifying a developer's domain |
| `FECO_PACKAGE_DIR` | `infra/.data/packages` | Directory that stores packages uploaded by developers. Required in production |
| `FECO_CLAMD_ADDR` | not set | ClamAV (`clamd`, for example `127.0.0.1:3310`), the second scanning engine in moderation. Required in production |
| `FECO_ADMIN_EMAILS` | not set | Emails that receive moderation alerts (malware, late detections), comma-separated. Required in production |
| `FECO_TUF_REPO` / `FECO_TUF_SIGN_KEYS` | not set | The TUF repository and the targets, snapshot and timestamp key files used to sign packages on release. When set, the server also re-signs metadata before it expires (at startup and every 5 minutes). If not set, the signing step is skipped |
| `FECO_ADMIN_PASSKEY_OPTIONAL` | `false` | `true`: the Admin Console does not require a passkey. Can be set only when `FECO_ENV=dev`; other environments refuse to start |
| `FECO_LAUNCHER_CLIENT_ID` | not set | The F-Launcher client ID in Hydra. If set, it enables sub-app launch codes (`/v1/launch-codes`) and the launcher does not have to go through the consent screen |
| `FECO_VNPAY_*`, `FECO_MOMO_*` | not set | Dev/staging may only point to the payment gateway's sandbox |
| `FECO_MOBILE_CLIENT_ID` | | OAuth client of the mobile F-Launcher (first party: no consent screen, may call launcher APIs) |
| `FECO_OSV_URL` | `https://api.osv.dev` | Service used to look up library vulnerabilities when scanning source code. Only the ecosystem, name and version of each library are sent |
| `FECO_OSV` | on | `off` skips the vulnerability lookup (versions with source code go to manual review) |
| `FECO_SOURCE_FETCH_URL` | `https://codeload.github.com` | Where source code declared as a GitHub repository and commit is downloaded from |
| `FECO_ADMIN_EMAIL` | dev: `admin@f-eco.local` | Email of the default administrator account; required outside dev |
| `FECO_ADMIN_PASSWORD_FILE` | dev: `.dev/admin-password` (generated) | Secret file with the initial password (≥ 12 characters), used only when the account is created; required outside dev |
| `FECO_WALLET_WITHDRAWALS` | `true` | `false`: stop accepting new F-Coin withdrawals (pending ones are still processed) |
| `RUST_LOG` | `info` | Log level, for example `debug` |

**Kratos and Hydra in deployment** (not `feco-server` variables):

- `SELFSERVICE_METHODS_OIDC_CONFIG_PROVIDERS` (Kratos): a JSON array of the Google and GitHub providers with `client_id`, `client_secret` and `mapper_url: file:///etc/config/kratos/oidc/mapper.jsonnet`. Example in `openspec/changes/oidc-login-and-token-reuse/design.md` (O2). Without it there are no external sign-in buttons.
- `WEBFINGER_OIDC_DISCOVERY_TOKEN_URL` (Hydra): `https://<F-Eco server>/oauth2/token`, so apps using standard OIDC libraries exchange tokens through F-Eco.

All secrets (database password, keys) are printed to the log as `***`. Do not commit real secrets to the repository; CI has a secret-scanning step.

## Testing

```bash
cargo test --workspace                          # unit tests (no infrastructure needed)
cargo clippy --workspace --all-targets -- -D warnings
cargo deny check                                # licenses, vulnerabilities, crate sources
cargo run -p feco-license-check                 # service and container image licenses
npx -y @fission-ai/openspec@1.16.1 validate add-f-ecosystem-platform --strict
```

**End-to-end tests** run against real Kratos, Hydra, PostgreSQL and Mailpit, following each scenario in the spec (sign-up, verification, password-guessing protection, TOTP, devices, account deletion, recovery, scope-based consent, pairwise, revocation, webhooks, data export, catalog, Developer Portal, moderation, F-SDK, build-to-order requests with simulated time, payments with a fake ZaloPay gateway speaking the same protocol). The infrastructure from step 2 must be running, and port 8080 must be free (stop `feco-server` first):

```bash
FECO_E2E=1 cargo test -p feco-server --test e2e_auth
```

Each E2E suite uses its own database, created automatically: `e2e_auth` uses `feco_e2e` (set `FECO_E2E_DATABASE_URL` to change it), chat uses `feco_chat_e2e`, source scanning uses `feco_scan_e2e`. No suite writes to the dev database `feco`.

The E2E suites read email from Mailpit. If Kratos is sending real email (`.dev/smtp-url` exists), restart Kratos with Mailpit before running E2E so the tests do not send mail out:

```bash
FECO_SMTP_URL="smtp://127.0.0.1:1025/?disable_starttls=true" scripts/dev-local.sh kratos
```


```bash
FECO_E2E=1 cargo test -p feco-server --test e2e_chat --test e2e_source_scan
```

Google/GitHub sign-in runs against a fake OIDC provider inside the test (`127.0.0.1:4499`). Kratos needs `infra/kratos/kratos.e2e-oidc.yml` as well; without it the test is skipped, unless `FECO_E2E_OIDC=1` is set (CI):

```bash
FECO_KRATOS_E2E_OIDC=1 scripts/dev-local.sh kratos                                # Kratos run directly on the machine
docker compose -f infra/docker-compose.yml -f infra/docker-compose.e2e.yml up -d  # or with docker
FECO_E2E=1 FECO_E2E_OIDC=1 cargo test -p feco-server --test e2e_auth -- external_login refresh_token_reuse
```

The F-SDK E2E (`sdk_end_to_end`) also runs the Python sample app (needs `python` and the `fsdk_c` library; the test builds it if missing) and the TypeScript one (needs `node` ≥ 22.6); it is skipped on machines without them, unless `FECO_E2E_SDKS=1` is set. `standard_oidc_library_integration` (a Python app using Authlib, not F-SDK) runs once you have done `pip install authlib requests`. Each binding's own tests:

```bash
cargo build -p fsdk-c
FSDK_LIBRARY=target/debug/libfsdk_c.so python -m unittest discover -s sdk/python/tests
(cd sdk/typescript && node --test test/fsdk.test.ts)
```

**TUF chain of trust**: `cargo test -p feco-tuf` runs the attack scenarios (tampered package, tampered metadata, unknown key, rollback, freeze) together with revocation and key rotation. Drill the whole procedure with the command-line tool, printing a Markdown report:

```bash
scripts/tuf-drill.sh > /tmp/tuf-drill.md
cargo run -p feco-tuf -- help                   # commands: keygen, init, publish, revoke, refresh, resign, rotate, status
```

**Performance, backup, status** (task group 15):

```bash
# /v1/me load test: 1,000 requests/second for 30 seconds, passes when p95 < 300 ms and no errors
FECO_E2E=1 cargo test --release -p feco-server --test e2e_auth -- --ignored load_v1_me --nocapture
# F-Launcher startup time (under 3 seconds), Windows
powershell -File scripts\measure-launcher-startup.ps1 -Exe target\release\feco-launcher.exe
# Continuous backup and restore drill (a temporary Postgres cluster, does not touch live data)
PG_BIN=/path/to/pgsql/bin scripts/backup-drill.sh docs/ops/drills/<date>-backup-drill.md
# Every document has a complete English version
python3 scripts/check-docs-i18n.py
```

Status page: http://127.0.0.1:8080/status. How to operate backups and monitoring: [docs/en/ops/status-and-backup.md](docs/en/ops/status-and-backup.md).

CI (GitHub Actions) runs all of the steps above (except the load test, the startup measurement and the backup drill, which are run by hand every quarter), including the end-to-end tests with Docker infrastructure and the TUF drill, on every push.

## Sending email to real inboxes

By default, Kratos (verification and password-recovery emails) and feco-server (notifications, alerts) send email to Mailpit. To deliver to real inboxes, both use the same SMTP server:

1. **Gmail (for a dev machine)**:
   - turn on 2-Step Verification for the Google account;
   - go to https://myaccount.google.com/apppasswords and create an "app password" (16 characters).
2. Write the SMTP URL yourself into the secret file `.dev/smtp-url` (the `.dev` folder is never committed). Do not share this password with anyone, including in a chat:

   ```text
   smtps://<account-name>%40gmail.com:<app-password-without-spaces>@smtp.gmail.com:465
   ```

   Write the sender address (the same Gmail account) into `.dev/smtp-from`:

   ```text
   <account-name>@gmail.com
   ```

3. Restart Kratos (`scripts/dev-local.sh kratos`) and feco-server.
   - Kratos reads `FECO_SMTP_URL` or `.dev/smtp-url`, and `FECO_EMAIL_FROM` or `.dev/smtp-from`.
   - feco-server reads `FECO_SMTP_URL_FILE=.dev/smtp-url` and `FECO_EMAIL_FROM`.
   - With `.dev/smtp-url` present, Kratos prints "Kratos gửi email qua SMTP đã cấu hình" at startup.
4. Sign up with a real email address: the verification email reaches that inbox, not Mailpit.

Limits:
- Gmail allows about 500 emails a day.
- Verification links point to Kratos's public address (`http://127.0.0.1:4433` on a dev machine), so they only open on that same machine.
- For a real deployment, use your own sending domain with SPF, DKIM and DMARC (check with `feco-catalog mail-dns`). Set `COURIER_SMTP_CONNECTION_URI` and `COURIER_SMTP_FROM_ADDRESS` for Kratos and `FECO_SMTP_URL_FILE` for feco-server.

## Troubleshooting

| Symptom | Fix |
|---|---|
| Windows reports `LINK : fatal error LNK1104: cannot open file ...` when building | The project directory path is too long (260-character limit). Set a shorter build directory: `export CARGO_TARGET_DIR=C:/t/feco` (PowerShell: `$env:CARGO_TARGET_DIR="C:\t\feco"`), or clone into a shallow directory such as `C:\src` |
| `feco-server` exits immediately with "Cấu hình không hợp lệ" (Invalid configuration) | Read the message: it points to exactly the missing or wrong variable. In dev you only need `FECO_ENV=dev` |
| "Không cập nhật được lược đồ cơ sở dữ liệu" (Could not update the database schema) | PostgreSQL is not running or `FECO_DATABASE_URL` is wrong |
| The page says "Dịch vụ tài khoản đang tạm gián đoạn" (The account service is temporarily unavailable) | Kratos is not running; check `curl http://127.0.0.1:4433/health/ready` |
| No email arrives | By default every dev email goes to Mailpit at http://127.0.0.1:8025. To deliver to real inboxes, see [Sending email to real inboxes](#sending-email-to-real-inboxes) |
| F-Launcher says "The server's distribution data has expired" | The TUF `timestamp` metadata is only valid for 1 hour. Run the server with `FECO_TUF_REPO` and `FECO_TUF_SIGN_KEYS` so it re-signs metadata itself (at startup and every 5 minutes). Without the server, run `cargo run -p feco-tuf -- refresh --repo .dev/tuf/repo --sign .dev/tuf/keys/snapshot.pk8,.dev/tuf/keys/timestamp.pk8`. If your computer's clock is wrong, correct it |
| The sample app reports "cổng 9999 đang bận" (port 9999 is busy) | Another run of the sample app is still waiting; stop it with `Ctrl+C` |
| The end-to-end tests report "cổng 8080 phải trống" (port 8080 must be free) | Stop the running `feco-server` before running the tests |
| Kratos reports `database table is locked` | Dev Kratos uses in-memory SQLite, which cannot handle many concurrent writes; the tests already run sequentially. Staging and production must use PostgreSQL for Kratos |
| The launcher reports "Metadata phân phối không hợp lệ ... ExpiredMetadata" (Invalid distribution metadata ... ExpiredMetadata) | The dev TUF repository's timestamp expires after 1 hour: `scripts/dev-launcher.sh refresh`, then click **Sync** |
| The launcher reports "cổng 47201 đang bận" (port 47201 is busy) when signing in | Another sign-in is still waiting for the browser; close the other launcher or wait 10 minutes for it to expire |
| The launcher exits immediately with "không tìm thấy phông chữ hệ thống" (system font not found) | Set `FECO_LAUNCHER_FONT` to a `.ttf` file that has Vietnamese diacritics |
| Opening Demo App reports `consent_required` | The user has never consented for the app; run the app through the browser once (step 6.6) |
| A valid password is still rejected as "too similar to the email" | Kratos blocks passwords similar to the identifier; choose a password completely different from the email |

## Requirements and design

All requirements are written in [OpenSpec](https://github.com/Fission-AI/OpenSpec) under
[`openspec/changes/add-f-ecosystem-platform/`](openspec/changes/add-f-ecosystem-platform/):

- `proposal.md`: rationale, scope
- `specs/*/spec.md`: 15 capabilities (accounts, data consent, launcher, catalog, Developer Portal, moderation, SDK, F-Builder, build-to-order requests, payments, administration…)
- `design.md`: technical decisions (all Rust, Ory Kratos/Hydra, TUF, VNPAY/MoMo…)
- `tasks.md`: the plan by group, with progress marked and how each item was verified

Questions awaiting a decision are in [`QUESTIONS.md`](QUESTIONS.md).

## Directory structure

| Directory | Contents |
|---|---|
| `crates/feco-core` | Shared types: VND money, data scopes, environments |
| `crates/feco-config` | Reads and validates configuration at startup |
| `crates/feco-manifest` | The schema and validator for `f-app.toml` (shared by the portal, the CLI and moderation) |
| `crates/feco-tuf` | The TUF repository (build, sign, revoke, rotate keys) and the verifying client for the launcher; the `feco-tuf` tool |
| `crates/feco-codescan` | Source code scanning (leaked secrets, suspicious code, library vulnerabilities via OSV.dev, licenses); the `feco-codescan` command |
| `services/feco-server` | The HTTP service (axum): account pages, OAuth login/consent, `/v1/me`, webhooks, data export, F-Store, build-to-order requests (`/orders`); the `feco-catalog` tool; `migrations/` is the PostgreSQL schema; `templates/` is the UI |
| `clients/f-builder` | F-Builder: a tool for building apps yourself with a coding agent, developed from Craft Anything 0.6.0 (its own Cargo workspace; see `clients/f-builder/README.md` and `NOTICE`) |
| `clients/feco-mobile` | Mobile F-Launcher (Flutter, Android 10+, iOS 18+) |
| `clients/feco-launcher` | F-Launcher: the library (sign-in, install/update/uninstall through TUF, launch codes, local API) and the egui UI |
| `tools/feco-demo-app` | Sample sub-app: single sign-on and calling `/v1/me`; when opened by the launcher it signs in with a launch code |
| `sdk/` | F-SDK: `rust` (crate `fsdk`), `c` (C ABI `fsdk_c` and header), `python` (`ctypes` binding), `typescript` (package `@f-eco/sdk`) |
| `examples/standard-oidc` | An app integrated with a standard OIDC library (Authlib), not using F-SDK |
| `examples/apps` | Sample apps using F-SDK: `f-notes` (Notes), `f-timer` (Stopwatch), and the shared part `sample-common` |
| `tools/f-dev` | CLI for developers: `f-dev validate`, `f-dev pack`, `f-dev publish` |
| `tools/feco-license-check` | Checks service and container image licenses |
| `infra/` | Dev infrastructure: `docker-compose.yml`, the Kratos configuration (schema, email templates) and Hydra |
| `scripts/` | `dev-local.sh`: runs the infrastructure without Docker; `tuf-drill.sh`: drills the TUF chain of trust; `dev-launcher.sh`: the TUF repository and OAuth clients for trying the launcher; `dev-apps.sh`: puts sample apps, open-source apps from GitHub, the ThesisAI web app, Mentor, BinhAI Office and VPdf into the dev repository; `scan-integration.sh`: scans an outside repository before integrating it; `scan-integrations-all.sh`: rescans every integrated app |
| `integrations/` | Integrations with outside apps: `thesisai/` (the "Sign in with F-Eco" patch for ThesisAI, guide, provenance); `mentor/` (the "Sign in with F-Eco" patch for Mentor, desktop build script, guide, provenance); `binhai-office/` and `vpdf/` (build-from-source scripts, scan results, licenses) |
| `docs/security/` | Incident response procedure, TUF key management, drill reports |
| `third_party/` | License declarations and approved exceptions |
| `openspec/` | Requirements, design, plan |

## License

Released under your choice of one of two licenses: MIT ([LICENSE-MIT](LICENSE-MIT)) or Apache License 2.0 ([LICENSE-APACHE](LICENSE-APACHE)).
Unless you explicitly state otherwise, any contribution you submit to the project is licensed under both licenses above, without any additional terms.
