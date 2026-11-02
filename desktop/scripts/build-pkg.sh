#!/bin/bash
# Builds dist/<App>-<version>.pkg (Apple silicon) and its .sha256; the package installs <App>.app to
# /Applications and replaces an installed version (same bundle ID).
#   scripts/build-pkg.sh [releasepilot|sitepilot]   which app (default: releasepilot)
#   VERSION             app/package version (default: version in the app's tauri.conf.json)
#   SIGN_IDENTITY       codesign identity (default "-" = ad hoc; "Developer ID Application: …" for release)
#   INSTALLER_IDENTITY  optional "Developer ID Installer: …" to sign the package
set -euo pipefail
cd "$(dirname "$0")/.."
case "${1:-releasepilot}" in
  releasepilot) DIR=src-tauri; APP_NAME=OpenReleasePilot; PKG_ID=io.github.bl0rb.openreleasepilot.pkg ;;
  sitepilot) DIR=sitepilot; APP_NAME=OpenSitePilot; PKG_ID=io.github.bl0rb.opensitepilot.pkg ;;
  *) echo "Unknown app: $1 (releasepilot or sitepilot)" >&2; exit 2 ;;
esac
VERSION="${VERSION:-$(node -p "require('./$DIR/tauri.conf.json').version")}"
IDENTITY="${SIGN_IDENTITY:--}"

mkdir -p target dist
printf '{"version":"%s"}' "$VERSION" > target/version.conf.json
# Two Tauri apps share this workspace: the CLI takes the one whose folder it runs in.
(cd "$DIR" && npx tauri build --bundles app --config ../target/version.conf.json)
APP="target/release/bundle/macos/$APP_NAME.app"

OPTS=(--force --timestamp=none)
[ "$IDENTITY" != "-" ] && OPTS=(--force --timestamp --options runtime)
codesign "${OPTS[@]}" -s "$IDENTITY" "$APP"
codesign --verify --strict --deep "$APP"

ROOT="$(mktemp -d)"
mkdir -p "$ROOT/Applications"
ditto --noextattr --noqtn "$APP" "$ROOT/Applications/$APP_NAME.app"
COMPONENTS="$(mktemp -d)/components.plist"
pkgbuild --analyze --root "$ROOT" "$COMPONENTS" >/dev/null
# Always install to /Applications, even if another copy with the same bundle ID exists elsewhere.
plutil -replace 0.BundleIsRelocatable -bool NO "$COMPONENTS"

SIGN=()
[ -n "${INSTALLER_IDENTITY:-}" ] && SIGN=(--sign "$INSTALLER_IDENTITY")
NAME="$APP_NAME-$VERSION.pkg"
rm -f "dist/$NAME" "dist/$NAME.sha256"
pkgbuild --root "$ROOT" --component-plist "$COMPONENTS" --install-location / \
  --identifier "$PKG_ID" --version "$VERSION" ${SIGN[@]+"${SIGN[@]}"} "dist/$NAME"
(cd dist && shasum -a 256 "$NAME" > "$NAME.sha256")
echo "Built dist/$NAME"
