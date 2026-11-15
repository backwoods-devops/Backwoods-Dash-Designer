#!/bin/bash
# macOS only: wraps the release build in a real Mac app bundle so the 3Dconnexion driver treats it as an application.
set -e
cd "$(dirname "$0")"
cargo build --release
APP="Backwoods Dash Designer.app"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp assets/DashDesigner.icns "$APP/Contents/Resources/DashDesigner.icns"
cp target/release/bw-dash-designer "$APP/Contents/MacOS/bw-dash-designer"
cat > "$APP/Contents/Info.plist" <<'P'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleName</key><string>Backwoods Dash Designer</string>
<key>CFBundleDisplayName</key><string>Backwoods Dash Designer</string>
<key>CFBundleIdentifier</key><string>com.backwoodsfab.dashdesigner</string>
<key>CFBundleIconFile</key><string>DashDesigner</string>
<key>CFBundleExecutable</key><string>bw-dash-designer</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleVersion</key><string>0.3.0</string>
<key>CFBundleShortVersionString</key><string>0.3.0</string>
<key>NSHighResolutionCapable</key><true/>
<key>LSMinimumSystemVersion</key><string>11.0</string>
</dict></plist>
P
codesign --force --deep -s - "$APP" 2>/dev/null || true
echo "Built: $PWD/$APP"
open "$APP"
