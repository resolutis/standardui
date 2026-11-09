#!/usr/bin/env bash
# Wrap a built binary into a macOS .app bundle with an icon, so Finder, the
# Dock and the app switcher show it. Usage:
#
#   tools/bundle-macos.sh <binary> <App Name> <icon.png> [bundle.id]
#
# e.g. after `cargo build --release --example hello`:
#   tools/bundle-macos.sh target/release/examples/hello "Hello StandardUI" assets/icon.png com.example.hello
set -euo pipefail

BIN="$1"; NAME="$2"; PNG="$3"; ID="${4:-com.example.$(echo "$NAME" | tr 'A-Z ' 'a-z-')}"
OUT="target/$NAME.app"
rm -rf "$OUT"
mkdir -p "$OUT/Contents/MacOS" "$OUT/Contents/Resources"

# PNG -> .icns (needs a 1024x1024 PNG for best results; sips resizes).
ICONSET="$(mktemp -d)/icon.iconset"; mkdir -p "$ICONSET"
for s in 16 32 128 256 512; do
  sips -z $s $s "$PNG" --out "$ICONSET/icon_${s}x${s}.png" >/dev/null
  d=$((s*2)); sips -z $d $d "$PNG" --out "$ICONSET/icon_${s}x${s}@2x.png" >/dev/null
done
iconutil -c icns "$ICONSET" -o "$OUT/Contents/Resources/AppIcon.icns"

cp "$BIN" "$OUT/Contents/MacOS/$NAME"
chmod +x "$OUT/Contents/MacOS/$NAME"

cat > "$OUT/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>CFBundleName</key><string>$NAME</string>
  <key>CFBundleDisplayName</key><string>$NAME</string>
  <key>CFBundleIdentifier</key><string>$ID</string>
  <key>CFBundleExecutable</key><string>$NAME</string>
  <key>CFBundleIconFile</key><string>AppIcon</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>0.1.0</string>
  <key>CFBundleVersion</key><string>1</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
</dict></plist>
PLIST

echo "Built $OUT  (open \"$OUT\")"
