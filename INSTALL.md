# Hey work - Installation Guide

## ⚠️ CRITICAL: First Time Installation

Due to macOS Gatekeeper security, the app will be blocked on first launch. **This is expected for unsigned apps.**

---

## 🚀 Quick Fix (Run This First!)

After installing the app, open Terminal and run:

```bash
xattr -cr "/Applications/Hey work.app"
```

Then you can open the app normally.

---

## Installation Steps

### 1. Download & Install
1. Download `Hey work_0.1.0_aarch64.dmg`
2. Double-click to mount the DMG
3. Drag "Hey work" to Applications folder
4. **Important:** Run the Terminal command above

### 2. First Launch
1. Open Terminal and run:
   ```bash
   xattr -cr "/Applications/Hey work.app"
   ```
2. Now open the app from Applications
3. If you see a security warning, right-click the app → Open

---

## 🔴 If App Still Won't Open

### Method 1: Remove Quarantine (Recommended)
```bash
xattr -cr "/Applications/Hey work.app"
```

### Method 2: Right-Click Open
1. Find the app in Applications
2. **Right-click** (or Control+click) on the app
3. Select **"Open"**
4. Click **"Open"** in the security dialog

### Method 3: Disable Gatekeeper Temporarily
```bash
# Disable Gatekeeper
sudo spctl --master-disable

# Open the app
open "/Applications/Hey work.app"

# Re-enable Gatekeeper (recommended)
sudo spctl --master-enable
```

---

## 🔐 Granting Permissions (Required)

After opening the app, grant these permissions:

### Accessibility (Required)
1. System Settings → Privacy & Security → Accessibility
2. Add "Hey work" and enable it
3. Needed for: Mouse/keyboard control

### Screen Recording (Required)
1. System Settings → Privacy & Security → Screen Recording
2. Add "Hey work" and enable it
3. Needed for: Taking screenshots

### Microphone (Optional)
1. System Settings → Privacy & Security → Microphone
2. Add "Hey work"
3. Needed for: Voice commands

---

## 🛠️ Troubleshooting

### "App is damaged" Error
Run:
```bash
xattr -cr "/Applications/Hey work.app"
```

### "Unidentified Developer" Warning
Right-click → Open, or run the xattr command above.

### App Opens But No Window Shows
1. Check menu bar for the app icon (near clock)
2. Press `⌘⇧Space` to trigger spotlight mode
3. Click the menu bar icon

### Still Not Working?
Run from Terminal to see error messages:
```bash
/Applications/Hey\ work.app/Contents/MacOS/hey-work
```

---

## ⌨️ Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| `⌘⇧Space` | Quick spotlight search |
| `⌘⇧H` | Screenshot + help |
| `⌃⇧C` | Voice command (hold) |
| `⌘⇧S` | Stop agent |
| `⌘⇧Q` | Quit app |

---

## 📋 System Requirements

- macOS 10.15 (Catalina) or later
- Apple Silicon (M1/M2/M3) Mac
- Internet connection for AI features

---

## 🆘 Still Having Issues?

1. Check Console.app for crash logs
2. Run the Terminal command to see errors
3. Make sure macOS is up to date

**The `xattr -cr` command fixes 99% of installation issues.**
