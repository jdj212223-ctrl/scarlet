#!/bin/sh
# Builds Scarlet.app (release build + Info.plist + icon) next to this repo's README. Usage: macos/bundle.sh [--install]
# --install also copies it to /Applications.
set -e
cd "$(dirname "$0")/.."
command -v cargo >/dev/null || PATH="$(brew --prefix rustup 2>/dev/null)/bin:$PATH"
cargo build --release
[ -f macos/Scarlet.icns ] || { swift macos/make-icon.swift macos/Scarlet.iconset && iconutil -c icns macos/Scarlet.iconset -o macos/Scarlet.icns; }
APP=Scarlet.app
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp target/release/scarlet "$APP/Contents/MacOS/scarlet"
cp macos/Info.plist "$APP/Contents/Info.plist"
cp macos/Scarlet.icns "$APP/Contents/Resources/Scarlet.icns"
printf 'APPL????' > "$APP/Contents/PkgInfo"
# Ad-hoc signature so macOS treats it as one app (no developer account needed on this Mac).
codesign --force --deep --sign - "$APP" 2>/dev/null || true
echo "built $(pwd)/$APP"
if [ "$1" = "--install" ]; then rm -rf "/Applications/$APP"; cp -R "$APP" /Applications/; echo "installed /Applications/$APP"; fi
