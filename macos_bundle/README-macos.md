# MartyPC macOS `.app` Bundle Builder

This folder contains a script to package **MartyPC** as a native macOS `.app` bundle. This makes it easy to double-click and run MartyPC like any other Mac application.

No more launching from Terminal or worrying about working directories — just click and go!

---

## 🧰 What This Script Does

- Builds the `martypc` binary using Cargo (if not already built)
- Creates a valid macOS `.app` bundle structure
- Copies the entire `install/` directory (including `martypc.toml`, `media/`, `roms/`, etc.) into the bundle
- Adds a launcher script that ensures MartyPC starts in the correct working directory
- Creates a proper `Info.plist` so macOS recognizes the app

---

## 🧪 How to Use

From the root of the MartyPC source tree:

```bash
cd macos_bundle
./make-mac-app.sh