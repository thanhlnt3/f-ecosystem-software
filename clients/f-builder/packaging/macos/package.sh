#!/usr/bin/env bash
# Build, sign and (optionally) notarize the macOS release artifacts:
#
#   $DIST/f-builder-<version>-macos-<arch>.dmg          F-Builder.app on a drag-to-Applications DMG
#   $DIST/f-builder-cli-<version>-macos-<arch>.zip      the headless CLI (when apps/f-builder-cli exists)
#
# Usage: packaging/macos/package.sh [--arch universal|aarch64|x86_64] [--skip-build]
#
# Signing (env):
#   MACOS_SIGN_IDENTITY   codesign identity (name or SHA-1). Default "-" = ad-hoc (local testing;
#                         Gatekeeper will reject the result on other Macs). In CI, import-cert.sh sets it.
#   MACOS_KEYCHAIN        keychain holding the identity (optional)
# Notarization (env; all three needed, and a real identity):
#   APPLE_ID, APPLE_PASSWORD (app-specific password), APPLE_TEAM_ID
set -euo pipefail
# shellcheck source=../env.sh
. "$(dirname "${BASH_SOURCE[0]}")/../env.sh"
HERE="$ROOT/packaging/macos"

ARCH=universal
SKIP_BUILD=0
while [ $# -gt 0 ]; do
  case "$1" in
    --arch) ARCH="$2"; shift 2 ;;
    --skip-build) SKIP_BUILD=1; shift ;;
    -h | --help) sed -n '2,16p' "$0"; exit 0 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done
case "$ARCH" in
  universal) TARGETS=(aarch64-apple-darwin x86_64-apple-darwin) ;;
  aarch64) TARGETS=(aarch64-apple-darwin) ;;
  x86_64) TARGETS=(x86_64-apple-darwin) ;;
  *) echo "unknown --arch $ARCH" >&2; exit 2 ;;
esac

# Keep in sync with LSMinimumSystemVersion in Info.plist.in.
export MACOSX_DEPLOYMENT_TARGET=11.0
IDENTITY="${MACOS_SIGN_IDENTITY:--}"
SHORT_VERSION="${VERSION%%-*}"
WORK="$CARGO_TARGET_DIR/macos-package"
APP="$WORK/F-Builder.app"
DMG="$DIST/f-builder-$VERSION-macos-$ARCH.dmg"
CLI_ZIP="$DIST/f-builder-cli-$VERSION-macos-$ARCH.zip"

NOTARIZE=0
if [ "$IDENTITY" = "-" ]; then
  warn "macOS: ad-hoc signing (no MACOS_SIGN_IDENTITY); artifacts are not notarized"
elif [ -n "${APPLE_ID:-}" ] && [ -n "${APPLE_PASSWORD:-}" ] && [ -n "${APPLE_TEAM_ID:-}" ]; then
  NOTARIZE=1
else
  warn "macOS: APPLE_ID / APPLE_PASSWORD / APPLE_TEAM_ID incomplete; signed but not notarized"
fi

echo "==> F-Builder $VERSION for macOS ($ARCH; ${BINS[*]}), identity: $IDENTITY, notarize: $NOTARIZE"

# ---- build -------------------------------------------------------------------------------------
if [ "$SKIP_BUILD" = 0 ]; then
  args=()
  for t in "${TARGETS[@]}"; do args+=(--target "$t"); done
  (cd "$ROOT" && cargo build --release --locked "${CARGO_PKGS[@]}" "${args[@]}")
fi

rm -rf "$WORK"
mkdir -p "$WORK/bin"
for bin in "${BINS[@]}"; do
  inputs=()
  for t in "${TARGETS[@]}"; do inputs+=("$CARGO_TARGET_DIR/$t/release/$bin"); done
  lipo -create -output "$WORK/bin/$bin" "${inputs[@]}"
  lipo -info "$WORK/bin/$bin"
done

# ---- signing helpers ---------------------------------------------------------------------------
sign() {
  # Hardened runtime + secure timestamp for a real identity; ad-hoc can't be timestamped.
  local ts=(--timestamp)
  [ "$IDENTITY" = "-" ] && ts=(--timestamp=none)
  local kc=()
  [ -n "${MACOS_KEYCHAIN:-}" ] && kc=(--keychain "$MACOS_KEYCHAIN")
  codesign --force --sign "$IDENTITY" ${kc[@]+"${kc[@]}"} "${ts[@]}" "$@"
}

notarize() {
  local file="$1" out id status
  echo "==> notarizing $(basename "$file") (this can take a few minutes)"
  out="$(xcrun notarytool submit "$file" --apple-id "$APPLE_ID" --password "$APPLE_PASSWORD" \
    --team-id "$APPLE_TEAM_ID" --wait --timeout 1h --output-format json)" || true
  echo "$out"
  id="$(printf '%s' "$out" | plutil -extract id raw -o - - 2>/dev/null || true)"
  status="$(printf '%s' "$out" | plutil -extract status raw -o - - 2>/dev/null || true)"
  if [ "$status" != "Accepted" ]; then
    if [ -n "$id" ]; then
      xcrun notarytool log "$id" --apple-id "$APPLE_ID" --password "$APPLE_PASSWORD" --team-id "$APPLE_TEAM_ID" || true
    fi
    echo "error: notarization of $(basename "$file") failed (status: ${status:-unknown})" >&2
    exit 1
  fi
}

# ---- icon --------------------------------------------------------------------------------------
# The committed .icns (assets/app-icon/f-builder.icns, rendered from the icon source).
ICNS="$ROOT/assets/app-icon/f-builder.icns"

# ---- F-Builder.app ----------------------------------------------------------------------
echo "==> assembling $APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
# The executable keeps the machine name (CFBundleExecutable); the bundle carries the display name.
cp "$WORK/bin/f-builder" "$APP/Contents/MacOS/f-builder"
# The CLI goes beside it: the app starts it as the MCP server its coding agents use, and finds it
# next to its own executable (`self_cli_path`). Without it every Claude Code build fails (#91).
if [ "$HAS_CLI" = 1 ]; then cp "$WORK/bin/f-builder-cli" "$APP/Contents/MacOS/f-builder-cli"; fi
cp "$ICNS" "$APP/Contents/Resources/f-builder.icns"
# Licences of the embedded fonts (assets/fonts, OFL-1.1).
mkdir -p "$APP/Contents/Resources/Licenses"
copy_font_licences "$APP/Contents/Resources/Licenses"
sed -e "s/@VERSION@/$VERSION/g" -e "s/@SHORT_VERSION@/$SHORT_VERSION/g" \
  -e "s/@BUILD_SHA@/${F_BUILDER_BUILD_SHA:-unknown}/g" \
  "$HERE/Info.plist.in" >"$APP/Contents/Info.plist"
plutil -lint "$APP/Contents/Info.plist"
printf 'APPL????' >"$APP/Contents/PkgInfo"

# Sign inside-out: nested code first, then the bundle itself (no --deep on the final signature).
# The nested code is the main executable and the CLI; frameworks/helpers would be signed here too.
# The CLI keeps the identifier its zip has.
if [ "$HAS_CLI" = 1 ]; then
  sign --options runtime --identifier io.github.thanhlnt3.f-builder-cli "$APP/Contents/MacOS/f-builder-cli"
fi
sign --options runtime --entitlements "$HERE/entitlements.plist" "$APP/Contents/MacOS/f-builder"
sign --options runtime --entitlements "$HERE/entitlements.plist" "$APP"
codesign --verify --strict --deep --verbose=2 "$APP"

if [ "$NOTARIZE" = 1 ]; then
  ditto -c -k --keepParent "$APP" "$WORK/f-builder-notarize.zip"
  notarize "$WORK/f-builder-notarize.zip"
  retry xcrun stapler staple "$APP"
  retry xcrun stapler validate "$APP"
  spctl --assess --type execute -vvv "$APP"
fi

# ---- DMG ---------------------------------------------------------------------------------------
echo "==> building $DMG"
STAGE="$WORK/dmg"
mkdir -p "$STAGE"
ditto "$APP" "$STAGE/F-Builder.app"
ln -s /Applications "$STAGE/Applications"
# Finder window layout: background, icon size and positions (packaging/macos/dmg/README.md).
mkdir -p "$STAGE/.background"
cp "$HERE/dmg/background.tiff" "$STAGE/.background/background.tiff"
# DS_Store phải dựng lại bằng dmg/generate.py (cần resvg, ds_store) sau khi đổi tên F-Builder.
if [[ -f "$HERE/dmg/DS_Store" ]]; then cp "$HERE/dmg/DS_Store" "$STAGE/.DS_Store"; fi
rm -f "$DMG"
# `hdiutil create -srcfolder` writes the files as they are. `makehybrid -hfs` gives every file a
# com.apple.FinderInfo, which `codesign --verify --strict` rejects as "Finder information, or
# similar detritus" on the app in the image and on a copy installed from it. `create` attaches a
# device while it copies, which fails now and then on CI runners ("Resource busy"), so it gets a
# few tries. The volume name has no version: .DS_Store finds the background through an alias that
# includes it.
# Usage: hdiutil_retry <what> <hdiutil args…>; tries five times with growing pauses.
hdiutil_retry() {
  local what="$1" attempt
  shift
  for attempt in 1 2 3 4 5; do
    if hdiutil "$@"; then
      return 0
    fi
    [ "$attempt" = 5 ] && { echo "hdiutil $what failed 5 times" >&2; return 1; }
    echo "hdiutil $what failed (attempt $attempt of 5); retrying in $((attempt * 10))s" >&2
    sleep $((attempt * 10))
  done
}
hdiutil_retry create create -srcfolder "$STAGE" -volname "F-Builder" -fs HFS+ -format UDZO -imagekey zlib-level=9 -ov "$DMG"
sign "$DMG"
codesign --verify --strict --verbose=2 "$DMG"
# The app inside the image must still pass --strict, so a makehybrid-style regression can't ship.
MOUNT="$WORK/dmg-check"
rm -rf "$MOUNT"
mkdir -p "$MOUNT"
hdiutil_retry attach attach -nobrowse -readonly -mountpoint "$MOUNT" "$DMG" >/dev/null
if ! codesign --verify --strict --deep --verbose=2 "$MOUNT/F-Builder.app"; then
  hdiutil_retry detach detach "$MOUNT" >/dev/null || true
  exit 1
fi
if [ "$HAS_CLI" = 1 ] && ! "$MOUNT/F-Builder.app/Contents/MacOS/f-builder-cli" --version; then
  echo "error: the app in the DMG has no working f-builder-cli beside its executable" >&2
  hdiutil_retry detach detach "$MOUNT" >/dev/null || true
  exit 1
fi
hdiutil_retry detach detach "$MOUNT" >/dev/null
if [ "$NOTARIZE" = 1 ]; then
  notarize "$DMG"
  retry xcrun stapler staple "$DMG"
  retry xcrun stapler validate "$DMG"
  spctl --assess --type open --context context:primary-signature -vvv "$DMG"
fi

# ---- CLI ---------------------------------------------------------------------------------------
if [ "$HAS_CLI" = 1 ]; then
  echo "==> building $CLI_ZIP"
  CLI_DIR="$WORK/f-builder-cli-$VERSION-macos-$ARCH"
  mkdir -p "$CLI_DIR"
  cp "$WORK/bin/f-builder-cli" "$CLI_DIR/"
  copy_docs "$CLI_DIR"
  # Same Developer ID and hardened runtime as the app, with an explicit reverse-DNS identifier
  # (codesign would otherwise use the bare file name).
  sign --options runtime --identifier io.github.thanhlnt3.f-builder-cli "$CLI_DIR/f-builder-cli"
  codesign --verify --strict --verbose=2 "$CLI_DIR/f-builder-cli"
  rm -f "$CLI_ZIP"
  ditto -c -k --keepParent "$CLI_DIR" "$CLI_ZIP"
  # notarytool accepts a zip of the bare binary. A bare Mach-O can't carry a stapled ticket, so
  # Gatekeeper looks the notarization up online when a quarantined copy first runs.
  if [ "$NOTARIZE" = 1 ]; then notarize "$CLI_ZIP"; fi
  "$WORK/bin/f-builder-cli" --version
else
  warn "apps/f-builder-cli is not in this workspace: no CLI zip"
  rm -f "$CLI_ZIP"
fi

"$WORK/bin/f-builder" --version
echo "==> done (check the artifacts as shipped with packaging/macos/verify.sh --arch $ARCH)"
ls -lh "$DIST"/f-builder*-"$VERSION"-macos-"$ARCH".*
