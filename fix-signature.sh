#!/bin/bash

# Fix code signature for unsigned distribution
# Run this after building the app to make it work without notarization

APP_PATH="/Users/aktheboss/project super agents/computer control use project/taskhomie/src-tauri/target/release/bundle/macos/Hey work.app"

echo "Fixing code signature for Hey work..."

# Remove existing signature
codesign --remove-signature "$APP_PATH" 2>/dev/null

# Re-sign with ad-hoc (no hardened runtime)
codesign --force --deep --sign - "$APP_PATH"

# Remove quarantine
xattr -cr "$APP_PATH"

echo "Done! The app should now work without crashing."
