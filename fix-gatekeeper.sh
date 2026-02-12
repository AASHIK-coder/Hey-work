#!/bin/bash

# Hey work - Gatekeeper Fix Script
# This script removes macOS quarantine attributes that block unsigned apps

echo ""
echo "🔧 Hey work - Gatekeeper Fix"
echo "========================================"
echo ""

APP_PATH="/Applications/Hey work.app"

# Check if app exists in Applications
if [ ! -d "$APP_PATH" ]; then
    echo "⚠️  App not found in Applications folder."
    echo ""
    echo "Looking in current directory..."
    
    # Try to find it in current directory
    if [ -d "Hey work.app" ]; then
        APP_PATH="Hey work.app"
        echo "✅ Found: $APP_PATH"
    else
        echo "❌ App not found."
        echo ""
        echo "Please drag 'Hey work.app' to your Applications folder first."
        exit 1
    fi
else
    echo "✅ Found: $APP_PATH"
fi

echo ""
echo "Removing Gatekeeper quarantine attributes..."

# Remove quarantine attribute (the fix!)
if xattr -cr "$APP_PATH" 2>/dev/null; then
    echo "✅ Quarantine attributes removed!"
else
    echo "⚠️  Could not remove attributes (may already be fixed)"
fi

echo ""
echo "========================================"
echo "✅ Fix complete!"
echo ""
echo "You can now open the app by:"
echo "  • Double-clicking from Applications"
echo "  • Or right-click → Open"
echo ""

# Ask if user wants to launch
read -p "Launch Hey work now? (y/n) " -n 1 -r
echo ""
if [[ $REPLY =~ ^[Yy]$ ]]; then
    echo "🚀 Launching..."
    open "$APP_PATH"
fi

echo ""
echo "Happy automating! 🤖"
echo ""
