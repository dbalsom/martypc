#!/bin/bash

set -e

APP_NAME="Martypc"
APP_BUNDLE="${APP_NAME}.app"
APP_CONTENTS="${APP_BUNDLE}/Contents"
APP_MACOS="${APP_CONTENTS}/MacOS"
APP_RESOURCES="${APP_CONTENTS}/Resources"
INSTALL_DIR="install"
BINARY_PATH="target/release/martypc"

# Check binary exists
if [ ! -f "$BINARY_PATH" ]; then
  echo "⚠️  Binary not found at $BINARY_PATH"
  echo "🛠  Building with cargo..."
  cargo build --release
fi

echo "🚀 Creating bundle: $APP_BUNDLE"
rm -rf "$APP_BUNDLE"
mkdir -p "$APP_MACOS"
mkdir -p "$APP_RESOURCES"

echo "📦 Copying binary..."
cp "$BINARY_PATH" "$APP_MACOS/"

echo "📁 Copying install directory..."
cp -a "$INSTALL_DIR/." "$APP_RESOURCES/"

echo "🐇 Creating launcher script..."
cat > "$APP_MACOS/launcher.sh" <<'EOF'
#!/bin/bash
cd "$(dirname "$0")/../Resources"
exec ../MacOS/martypc --configfile martypc.toml
EOF

chmod +x "$APP_MACOS/launcher.sh"

echo "📝 Creating Info.plist..."
cat > "$APP_CONTENTS/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
 "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key>
  <string>$APP_NAME</string>
  <key>CFBundleExecutable</key>
  <string>launcher.sh</string>
  <key>CFBundleIdentifier</key>
  <string>com.mark.martypc</string>
  <key>CFBundleVersion</key>
  <string>1.0</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
</dict>
</plist>
EOF

echo "✅ Build complete!"
echo "➡️  Run it with: open $APP_BUNDLE"