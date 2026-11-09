# Installing a standardui app with an icon

Each OS gets its app icon from a different place, so installing is different on each.

| | Running window / taskbar | Finder / Explorer / launcher / dock |
|---|---|---|
| **macOS** | from the `.app` bundle | `.icns` file in the `.app` bundle |
| **Windows** | `App::icon(...)` | `.ico` compiled into the `.exe` |
| **Linux** | `App::icon(...)` plus a matching `.desktop` file | `.desktop` file plus an installed PNG |

Start with a square **1024×1024 PNG** for the icon.

## First: a problem with `App::icon("assets/icon.png")`

That path is looked up from the folder the app is *started from*. It works with `cargo run` from the project folder. An installed app is started from somewhere else, so it won't find the file and will run without an icon, with no error.

The fix in standardui is to add a way to set the icon from bytes built into the binary, using `include_bytes!`:

```rust
// src/app.rs — next to App::icon
pub fn icon_bytes(mut self, png: &'static [u8]) -> Self { … }
// src/icon.rs — load with image::load_from_memory(png) instead of image::open(path)
```

Apps would then write `.icon_bytes(include_bytes!("../assets/icon.png"))`. Until that exists, install the PNG next to the binary and pass an absolute path.

---

## macOS

1. **Build and bundle it** with the existing script, which makes the `.icns` and `Info.plist`:

   ```sh
   cargo build --release --example hello
   tools/bundle-macos.sh target/release/examples/hello "Hello StandardUI" assets/icon.png com.resolutis.hello
   ```

2. **Install it:**

   ```sh
   cp -R "target/Hello StandardUI.app" /Applications/
   ```

   It then shows up in Launchpad, Spotlight and the Dock with its icon. If the Dock shows an old icon, run `killall Dock`.

`cargo-bundle` can do the bundling step instead of the script if you prefer.

### Make a DMG installer

A `.dmg` is a disk image holding the `.app` next to a shortcut to the Applications folder. People open it and drag the app across. [`create-dmg`](https://github.com/create-dmg/create-dmg) builds one with a tidy installer window.

1. **Build the `.app`** with step 1 above, so `target/Hello StandardUI.app` exists.

2. **Install `create-dmg`** (one time):

   ```sh
   brew install create-dmg
   ```

3. **Create the DMG:**

   ```sh
   create-dmg \
     --volname "Hello StandardUI" \
     --volicon "target/Hello StandardUI.app/Contents/Resources/AppIcon.icns" \
     --window-size 540 380 \
     --icon-size 128 \
     --icon "Hello StandardUI.app" 140 180 \
     --app-drop-link 400 180 \
     --hide-extension "Hello StandardUI.app" \
     "target/HelloStandardUI-0.1.0.dmg" \
     "target/Hello StandardUI.app"
   ```

   | Option | What it does |
   |---|---|
   | `--volname` | Name of the mounted disk, shown in Finder's sidebar and window title |
   | `--volicon` | Icon of the mounted disk; reuses the app's `.icns` |
   | `--window-size 540 380` | Size of the Finder window that opens |
   | `--icon-size 128` | Size of the icons inside the window |
   | `--icon "…app" 140 180` | Places the app's icon at x=140, y=180 (left) |
   | `--app-drop-link 400 180` | Adds the Applications shortcut at x=400, y=180 (right) |
   | `--hide-extension` | Shows "Hello StandardUI" instead of "Hello StandardUI.app" |
   | 1st path | The DMG file to create |
   | 2nd path | The `.app` to put in it |

   Add `--background assets/dmg-background.png` (a 540×380 image, e.g. with an arrow from the app to Applications) for a custom background.

   Create-dmg briefly opens the disk in Finder to lay out the window. Don't click it while it runs. If it fails partway, eject any leftover "Hello StandardUI" disk from Finder before trying again.

4. **Test it:** double-click `target/HelloStandardUI-0.1.0.dmg`, drag the app onto Applications, then open it from Launchpad or Spotlight.

### Sign and notarize (needed to share it)

Without this, the DMG works on your own Mac, but on other Macs Gatekeeper says the app "can't be opened because Apple cannot check it for malicious software". It needs a paid Apple Developer account.

1. **Sign the app**, before building the DMG:

   ```sh
   codesign --force --deep --options runtime --timestamp \
     -s "Developer ID Application: Your Name (TEAMID)" \
     "target/Hello StandardUI.app"
   ```

2. **Build the DMG** as above, then sign it too:

   ```sh
   codesign --timestamp -s "Developer ID Application: Your Name (TEAMID)" target/HelloStandardUI-0.1.0.dmg
   ```

   Or pass `--codesign "Developer ID Application: Your Name (TEAMID)"` to `create-dmg`.

3. **Notarize it.** Save your credentials once, then submit:

   ```sh
   xcrun notarytool store-credentials notary --apple-id you@example.com --team-id TEAMID
   xcrun notarytool submit target/HelloStandardUI-0.1.0.dmg --keychain-profile notary --wait
   ```

   Or pass `--notarize notary` to `create-dmg`.

4. **Staple the approval to the DMG** so it opens offline too:

   ```sh
   xcrun stapler staple target/HelloStandardUI-0.1.0.dmg
   ```

5. **Check the result:**

   ```sh
   spctl -a -t open --context context:primary-signature -v target/HelloStandardUI-0.1.0.dmg
   ```

   It should say "accepted, source=Notarized Developer ID".

### Notes

- `tools/bundle-macos.sh` hard-codes the app version as `0.1.0` in `Info.plist`. Update it, and the DMG file name, for each release.
- For one app that runs on both Intel and Apple Silicon Macs, build both targets and join them with `lipo` before bundling:

  ```sh
  rustup target add x86_64-apple-darwin aarch64-apple-darwin
  cargo build --release --example hello --target aarch64-apple-darwin
  cargo build --release --example hello --target x86_64-apple-darwin
  lipo -create -output target/hello-universal \
    target/aarch64-apple-darwin/release/examples/hello \
    target/x86_64-apple-darwin/release/examples/hello
  tools/bundle-macos.sh target/hello-universal "Hello StandardUI" assets/icon.png com.resolutis.hello
  ```

## Windows

1. **Hide the console window.** Put this at the top of the app's `main.rs` (or `examples/hello.rs`):

   ```rust
   #![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
   ```

   Without it, a black terminal window opens behind the app.

2. **Build the icon into the `.exe`** so Explorer, the Start menu and shortcuts show it:
   - Convert the PNG to a multi-size `.ico` (16, 32, 48 and 256 px). For example, with ImageMagick: `magick icon.png -define icon:auto-resize=256,48,32,16 icon.ico`.
   - Add a build script with the `winresource` crate:

     ```toml
     # Cargo.toml of the app
     [target.'cfg(windows)'.build-dependencies]
     winresource = "0.1"
     ```

     ```rust
     // build.rs
     fn main() {
         #[cfg(windows)]
         winresource::WindowsResource::new().set_icon("assets/icon.ico").compile().unwrap();
     }
     ```

3. **Build it:** `cargo build --release` on Windows.

4. **Install it:**
   - **Simple:** copy the `.exe` to `%LOCALAPPDATA%\Programs\HelloStandardUI\`, then right-click → *Send to → Desktop (create shortcut)* or pin it to Start.
   - **Proper installer:** `cargo install cargo-wix` and then `cargo wix` makes an `.msi` that adds a Start-menu entry and an uninstaller. Inno Setup is a popular alternative for a `setup.exe`.
   - Sign the `.exe`/`.msi` with a code-signing certificate, or SmartScreen will warn people the first time they run it.

## Linux

The icon only shows in docks and task switchers when the window's app id matches a `.desktop` file name. standardui sets the app id from the app name: `app_id("Hello StandardUI")` gives `hello-standardui`.

1. **Build it:** `cargo build --release --example hello`.

2. **Install the binary and icon** (per user, no root needed):

   ```sh
   install -Dm755 target/release/examples/hello ~/.local/bin/hello-standardui
   install -Dm644 assets/icon.png ~/.local/share/icons/hicolor/256x256/apps/hello-standardui.png
   ```

   The icon's folder name should match its real size. A 512 px icon goes in `512x512`.

3. **Create the desktop entry** at `~/.local/share/applications/hello-standardui.desktop`. The file name must equal the app id:

   ```ini
   [Desktop Entry]
   Type=Application
   Name=Hello StandardUI
   Exec=hello-standardui
   Icon=hello-standardui
   StartupWMClass=hello-standardui
   Categories=Utility;
   Terminal=false
   ```

4. **Refresh the caches:**

   ```sh
   update-desktop-database ~/.local/share/applications
   gtk-update-icon-cache ~/.local/share/icons/hicolor 2>/dev/null || true
   ```

   It will then appear in the app launcher (GNOME, KDE and others) with its icon. For a system-wide install, use `/usr/bin`, `/usr/share/icons/hicolor/...` and `/usr/share/applications` with `sudo`.

5. **To give it to other people:**
   - `cargo deb` makes a `.deb` for Debian and Ubuntu.
   - `cargo generate-rpm` makes an `.rpm` for Fedora.
   - An AppImage (via `linuxdeploy`) runs on most distros.
   - Flatpak is the most universal option.

   All of these package the same three things: the binary, the `.desktop` file and the icon.
