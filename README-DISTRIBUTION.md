# Hey work - Distribution Notes

## Windows release artifact (for friends)

After pushing a tag like `v0.1.0`, GitHub Actions publishes Windows installers to Releases:

- `src-tauri/target/release/bundle/nsis/*.exe` (recommended share file)
- `src-tauri/target/release/bundle/msi/*.msi` (enterprise/manual install option)

Share the generated `.exe` installer with Windows users.

## ⚠️ Known Issue with Signed Builds

The bundled app (`.app` bundle) may crash with `Trace/BPT trap: 5` error when code-signed. This is due to macOS hardened runtime restrictions affecting the NSPanel plugin.

### Solutions:

#### Option 1: Use Development Build (Recommended for Testing)
Run the app without bundling:
```bash
cd src-tauri
cargo run --release
```

#### Option 2: Run Binary Directly
Execute the binary inside the app bundle directly:
```bash
"/Applications/Hey work.app/Contents/MacOS/hey-work"
```

#### Option 3: Fix the Bundle (Advanced)
Remove code signing and run:
```bash
# Remove signature
codesign --remove-signature "/Applications/Hey work.app"

# Run without signature
"/Applications/Hey work.app/Contents/MacOS/hey-work"
```

---

## 📦 Creating a Working Distribution

### Method 1: ZIP Distribution (Recommended)
Instead of DMG, distribute as ZIP with instructions:

1. Build the app: `npm run tauri build`
2. Create a ZIP with the app and fix script
3. Users extract and run the fix script first

### Method 2: Provide Build from Source
Include instructions to build:
```bash
git clone <repo>
cd taskhomie
npm install
npm run tauri build
```

---

## 🔧 Technical Details

The issue is caused by:
1. Tauri's bundler applies ad-hoc code signing with hardened runtime
2. The hardened runtime blocks certain operations required by:
   - The NSPanel macOS plugin
   - WebKit subprocesses
   - Accessibility APIs

### Why Direct Binary Works
Running the binary directly bypasses LaunchServices and Gatekeeper checks that enforce hardened runtime restrictions.

---

## ✅ Recommended User Instructions

Include this in your distribution:

```
⚠️  First Time Setup:

1. Drag "Hey work" to Applications
2. Open Terminal and run:
   
   "/Applications/Hey work.app/Contents/MacOS/hey-work"

3. The app will start and create a menu bar icon
4. For subsequent launches, you can use Spotlight or Finder

If you get "Unidentified Developer" warning:
- Right-click the app → Open
- Or run: xattr -cr "/Applications/Hey work.app"
```
