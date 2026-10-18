#!/usr/bin/env bash
# Builds target/neoteletext.app: the browser as a macOS application, with
# its helpers inside the same executable, signed for this machine only
# (ad hoc). Developer ID signing and notarisation are release work
# (prototypes/browser/docs/desktop-browser.md section 13).
set -euo pipefail
cd "$(dirname "$0")/.."

cargo build --release -p neoteletext-gui

app=target/neoteletext.app
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp target/release/neoteletext "$app/Contents/MacOS/neoteletext"

cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key><string>neoteletext</string>
    <key>CFBundleDisplayName</key><string>neoteletext</string>
    <key>CFBundleIdentifier</key><string>com.crowdwave.neoteletext</string>
    <key>CFBundleExecutable</key><string>neoteletext</string>
    <key>CFBundlePackageType</key><string>APPL</string>
    <key>CFBundleShortVersionString</key><string>${version}</string>
    <key>CFBundleVersion</key><string>${version}</string>
    <key>LSMinimumSystemVersion</key><string>13.0</string>
    <key>NSHighResolutionCapable</key><true/>
    <key>CFBundleDocumentTypes</key>
    <array>
        <dict>
            <key>CFBundleTypeName</key><string>Teletext page file</string>
            <key>CFBundleTypeRole</key><string>Viewer</string>
            <key>CFBundleTypeExtensions</key><array><string>t42</string><string>tti</string></array>
        </dict>
    </array>
</dict>
</plist>
PLIST

# Ad hoc: runs on this machine; other machines need Developer ID signing.
codesign --force --sign - --options runtime "$app"
codesign --verify --verbose=1 "$app"
echo "built $app"
