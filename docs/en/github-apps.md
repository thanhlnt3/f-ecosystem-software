# Bringing an open-source app from GitHub into F-Eco

> English translation of [the Vietnamese original](../github-apps.md). If they differ, the Vietnamese version is authoritative.

This guide is for people who want to bring an existing open-source app (usually on GitHub) into
F-Store, so users can install and open it with F-Launcher. The app goes through exactly the same
process as every other sub-app: manifest, package submission, review, TUF signing. The differences
are in three places:
- it is built from someone else's source code;
- the original license is kept;
- it may be an app that runs in a terminal.

The general concepts (developer account, manifest, `f-dev`, review) are in
[`integration-guide.md`](integration-guide.md). This document covers only what is specific to apps
from GitHub.

The running examples are three apps already in the dev catalog:

| App | Repository | License | Type |
|---|---|---|---|
| kibi | `ilai-deutel/kibi` @ `v0.3.3` | MIT OR Apache-2.0 | text editor, runs in a terminal |
| bottom | `ClementTsang/bottom` @ `0.15.0` | MIT | system monitor, runs in a terminal |
| eframe template | `emilk/eframe_template` @ `f108546` | MIT OR Apache-2.0 | windowed app (egui) |

## Table of contents

1. [Choose an app](#1-choose-an-app)
2. [Fetch the right source version](#2-fetch-the-right-source-version)
3. [Build](#3-build)
4. [Package with the original license](#4-package-with-the-original-license)
5. [Write the manifest](#5-write-the-manifest)
6. [Sign, validate and submit](#6-sign-validate-and-submit)
7. [Review and display in F-Launcher](#7-review-and-display-in-f-launcher)
8. [Update when the upstream project releases a new version](#8-update-when-the-upstream-project-releases-a-new-version)
9. [Add F-Eco sign-in to the app (optional)](#9-add-f-eco-sign-in-to-the-app-optional)
10. [Try it on a dev machine with one command](#10-try-it-on-a-dev-machine-with-one-command)
11. [Checklist](#11-checklist)

## 1. Choose an app

- The **license** must be on F-Eco's allow list:
  - allowed: MIT, BSD-2-Clause, BSD-3-Clause, Apache-2.0, ISC, 0BSD, Zlib, BSL-1.0, MPL-2.0 (unmodified files), and the other licenses in the `dependency-licensing` spec;
  - not accepted: GPL, AGPL, SSPL, and licenses that prohibit commercial use.

  Read the repository's `LICENSE` file; do not rely only on the label GitHub shows: the label may reflect only part of the project.
- **Dependencies** must also have permitted licenses. For a Rust project, run `cargo deny check licenses` or `cargo tree` in the source folder to check.
- **The project is still maintained**: not archived, and has tagged releases.
- **Name and logo** belong to the upstream project:
  - keep the app's name;
  - state clearly "built from the source code of …" in the description;
  - do not present the app as a product made by F-Eco;
  - do not use the project's logo as your own logo.

## 2. Fetch the right source version

Always build from a fixed **tag or commit**, never from the `main` branch. That way the build is repeatable and reviewers can cross-check it.

```bash
mkdir -p ~/build-src/kibi && cd ~/build-src/kibi
curl -sSfL https://codeload.github.com/ilai-deutel/kibi/tar.gz/refs/tags/v0.3.3 \
  | tar -xz --strip-components=1
# for a commit instead of a tag: .../tar.gz/<full 40-character sha>
```

- Put the source **outside** your project folder, and outside any other git repository. There are two reasons:
  - inside another Cargo workspace, Cargo reports the error "believes it's in a workspace";
  - many projects get their version number from `git describe` at build time. Inside your git repository, this command reads your repository's tag by mistake. For example: kibi 0.3.3 built inside the F-Eco repository reports itself as "Kibi 1.0.0".
- **Before building, read the scripts that run at build time**, because they run on your machine with your privileges:
  - Rust: `build.rs`;
  - Node: `scripts` in `package.json`;
  - Python: `setup.py`.

  For example: the `build.rs` of kibi and bottom only calls `git describe` to get the version number.

## 3. Build

Build on each platform you declare in the manifest. For a Rust project:

```bash
cd ~/build-src/kibi
RUSTUP_TOOLCHAIN=stable cargo build --release --locked
# executable: target/release/kibi(.exe)
```

- `--locked` uses the upstream project's exact `Cargo.lock`.
- `RUSTUP_TOOLCHAIN=stable` ignores the project's `rust-toolchain` file. For example, eframe template pins Rust 1.95, so without this flag rustup would download that toolchain as well.

For a project written in another language, follow that project's build instructions:
- Go: `go build`;
- Electron or Node: `npm ci && npm run build`, then package with the project's tooling.

The package submitted to F-Eco must run without requiring users to install an extra interpreter.

## 4. Package with the original license

When the manifest has `source`, **the package must contain at least one original license file**: `LICENSE*`, `LICENCE*`, `COPYING*` or `NOTICE*`, in any folder within the package. `f-dev pack` and `f-dev publish` reject a package missing this file.

Recommended structure:

```
build/
├── bin/kibi.exe
├── licenses/LICENSE-APACHE
├── licenses/LICENSE-MIT
├── licenses/COPYRIGHT
└── SOURCE.txt          # "Built from https://github.com/ilai-deutel/kibi at v0.3.3"
```

If the project's license requires including the notices of dependencies, copy that notice file as well (for example `THIRD_PARTY_NOTICES`, or the output of `cargo about`) into `licenses/`.

## 5. Write the manifest

A terminal app that does not use F-Eco sign-in (kibi):

```toml
id = "kibi"
name = "kibi"
version = "0.3.3"                 # follows the upstream project's version
license = "MIT OR Apache-2.0"     # matches the upstream project's license
privacy_policy = "https://<your site>/kibi-privacy"
summary = "A tiny text editor that runs in the terminal"
description = "Built from the source code of Ilaï Deutel (github.com/ilai-deutel/kibi), unmodified."
category = "tools"
source = "https://github.com/ilai-deutel/kibi"   # must be https://
terminal = true                                  # the launcher opens the app in a terminal window
features = [                                     # shown on the detail page in the launcher
  "Syntax highlighting",
  "Incremental search",
  "Run external commands and paste the output",
]
release_notes = "Upstream version 0.3.3: adds Ctrl+/ to comment a line."

[[platforms]]
os = "windows"
arch = "x64"
entry = "bin/kibi.exe"

[permissions]
network = false
user_folders = ["documents"]
```

A windowed app (eframe template): drop `terminal`; the remaining fields are similar.

Explanation of the fields:
- **`source`**:
  - appears as the "Source" button in F-Launcher and F-Store;
  - enables the check for the original license in the package;
  - is where reviewers cross-check the license.
- **`terminal = true`**:
  - Windows: the launcher opens the app in a new console window;
  - macOS: the launcher opens the app in Terminal. Environment variables do not pass through Terminal, so a terminal app cannot receive the launch code. If a terminal app needs to sign in, it must sign in itself through the browser (`fsdk::session`). `f-dev validate` warns about this case.
- **Do not declare `[scopes]`** when the app does not sign in with F-Eco. In that case:
  - users do not see a consent screen;
  - the TUF target has no `client_id`;
  - the launcher opens the app without requesting a launch code.
- **`category`**: use one of the following codes so F-Launcher groups the app and shows the Vietnamese display name:

  | Code | Display name |
  |---|---|
  | `productivity` | Năng suất (Productivity) |
  | `tools` | Công cụ (Tools) |
  | `system` | Hệ thống (System) |
  | `development` | Lập trình (Development) |
  | `education` | Giáo dục (Education) |
  | `games` | Trò chơi (Games) |
  | `graphics` | Đồ họa (Graphics) |
  | `music` | Âm nhạc (Music) |
  | `video` | Video |
  | `business` | Kinh doanh (Business) |

  Other values are shown verbatim.
- **`privacy_policy`** is **your** page, as the publisher on F-Eco. This page states clearly whether the app collects data. For an app with no network connection, a short page saying "the app does not collect data" is enough.

## 6. Sign, validate and submit

Sign the executable **you built**:
- Windows: Authenticode;
- macOS: Developer ID, notarize it.

An unsigned package is rejected, like any other app. The signing key is yours, not the upstream project's.

```bash
f-dev validate --manifest f-app.toml          # errors and warnings (for example a terminal app on macOS)
f-dev pack     --manifest f-app.toml --package build/ --out dist/
                                              # creates dist/windows-x64.zip, checks the original license
f-dev publish  --manifest f-app.toml --package dist/
```

## 7. Review and display in F-Launcher

In addition to the usual review steps (malware scan, code signature, permissions), reviewers:
- open the `source` link;
- cross-check `license` in the manifest against the project's license;
- check that the license file is in the package.

After release, the server signs the following information into the version's TUF metadata:
- short description, category, `source`, `terminal`: taken from the manifest;
- developer name: taken from the catalog.

F-Launcher only shows information from this signed metadata. In addition, the server also signs the introduction,
`features`, `website`, `release_notes`, license and release time, so the launcher can build the app's
detail page:
- **Library and Store**, in grid or list layout: first-letter icon, name, developer, category, short description;
- **Open button**: reads "Open (terminal)" for apps with `terminal`;
- **"..." menu**: has a "View source" item.
- **detail page** (click an app): introduction, features, what's new, information table, and the
  "Source (GitHub)" and "Website" buttons.

Release notes should accurately summarize the upstream project's CHANGELOG for that version, and
state clearly whether it is the original or a modified build.

## 8. Update when the upstream project releases a new version

1. Read the project's changelog. Check whether the license and dependencies have changed.
2. Fetch the new tag (section 2), re-read the build-time scripts, then rebuild.
3. Change `version` in the manifest to the new version. The version must be greater than the released one.
4. Sign, run `f-dev pack`, then `f-dev publish`. An update that adds no scope or permission is usually approved automatically.

## 9. Add F-Eco sign-in to the app (optional)

If you want the app to use F-Eco accounts (for example to sync data per user):
1. Fork the project and add F-SDK. Keep the original license; what you add may use that same license.
2. Declare `[scopes]` in the manifest. From that version on, the launcher issues launch codes to the app.
3. Change `source` to your fork's repository.

See how to use F-SDK in [`integration-guide.md`](integration-guide.md#5-sign-in-within-the-app-with-f-sdk), and the two sample apps `examples/apps/f-notes` (Notes) and `examples/apps/f-timer` (Timer).

## 10. Try it on a dev machine with one command

In the F-Eco repository, after running the infrastructure, `feco-server` and `scripts/dev-launcher.sh setup` (README steps 2, 3 and 6):

```bash
# sample apps that use F-SDK, and the three open-source apps above
scripts/dev-apps.sh all

# any app from GitHub
scripts/dev-apps.sh github ilai-deutel/kibi v0.3.3 --id kibi --name kibi --bin kibi \
  --license "MIT OR Apache-2.0" --category tools --developer "Ilaï Deutel" --terminal \
  --summary "A text editor in the terminal" \
  --description "Long introduction for the detail page" \
  --features "Syntax highlighting|Incremental search" --website https://example.org \
  --notes "Changes in version 0.3.3"
```

The script does the following in order:
1. downloads the source at the exact tag or commit into `~/.feco-dev-ext/src/<id>` (change the location with `FECO_EXT_DIR`);
2. builds with `cargo build --release --locked`;
3. packages `bin/`, `licenses/`, `SOURCE.txt`;
4. signs into the dev TUF repository;
5. writes the app into the dev F-Store catalog.

Reopen the dev F-Launcher (or click Sync) to see the app in the Store. The script is for dev only. Real releases always go through `f-dev`, review, and server signing.

## 11. Checklist

- [ ] The licenses of the project and of its dependencies are all on the allow list.
- [ ] Built from a fixed tag or commit, outside your project folder, after reading the build-time scripts.
- [ ] The package has the original license file, and `SOURCE.txt` records the repository and the tag or commit.
- [ ] `license` matches the upstream project; `source` is the repository's https:// address.
- [ ] `terminal = true` for apps that run in a terminal.
- [ ] No scopes declared if the app does not sign in with F-Eco.
- [ ] The description states whose source code the app is built from; the project's logo is not used as your logo.
- [ ] The executable is code-signed with your key.
