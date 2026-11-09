# standardui

Easy, native-looking desktop UIs in Rust. Think Swing's simplicity with a look
that follows the OS: self-drawn widgets themed as Adwaita (Linux), Aqua (macOS)
or Fluent (Windows), the real global menu bar on macOS, and callbacks that
just take your state.

**Status: v0.1 — first cut.** Built and screenshotted on Linux headlessly; the
macOS-specific code (muda menu bar) has not yet been compiled on a Mac.

## Hello world

```rust
use standardui::prelude::*;

struct State { name: String }

fn main() {
    App::new("Hello", State { name: String::new() })
        .icon("assets/icon.png")
        .menu(MenuBar::new()
            .menu("File", |m| m
                .item("New", Shortcut::cmd('N'), |s: &mut State, ui| { s.name.clear(); ui.focus("name"); })
                .separator()
                .quit())
            .menu("Edit", |m| m.standard_edit()))
        .window(Window::new("Hello").size(420.0, 240.0)
            .content(Column::new()
                .add(TextField::new().id("name").placeholder("Your name")
                    .bind(|s: &mut State| &mut s.name))
                .add(Button::new("Greet").primary()
                    .on_click(|s: &mut State, ui| ui.alert("Hi", format!("Hello, {}!", s.name))))))
        .run();
}
```

Run the full example (menus, text field, checkbox, bound label, buttons):

```sh
cargo run --example hello
```

## How it works

| Piece | Approach |
|---|---|
| Windowing & input | [winit] |
| Rendering | software: [tiny-skia] → [softbuffer]; HiDPI-aware |
| Text | [cosmic-text] (system fonts, shaping) |
| Menus | macOS: native global menu bar via [muda]. Linux/Windows: in-window bar drawn by standardui |
| Dialogs | [rfd] (XDG portal on Linux, so no GTK headers needed to build) |
| Theme | picked from the OS at startup; follows light/dark live |

### Callbacks

Every callback gets `&mut S` (your state) and `&mut Ui<S>`. Widgets are owned by
the tree, so you never capture one widget inside another's closure:

```rust
Button::new("Clear").on_click(|state: &mut State, ui| {
    state.name.clear();        // bound field updates on the next frame
    ui.set_text("status", ""); // or address a widget by id
    ui.focus("name");
})
```

`Ui` queues commands (`alert`, `quit`, `set_text`, `set_enabled`, `set_checked`,
`focus`, `set_title`) and applies them after the callback returns, so callbacks
are re-entrancy-safe. `ui.spawn(work, then)` runs `work` on a thread and `then`
back on the UI thread; `ui.handle()` gives a `Send` handle (`invokeLater`).

### Bindings

`TextField::bind(|s| &mut s.name)`, `Checkbox::bind(|s| &mut s.flag)` are two-way.
`Label::bind(|s| format!(..))` recomputes each frame.

### Menus

Standard roles land where each OS expects them: `.quit()`, `.about(..)`,
`.preferences(..)` move to the app menu on macOS (with ⌘Q / ⌘,), and stay where
you put them elsewhere. `.standard_edit()` adds Undo/Redo/Cut/Copy/Paste/Select
All with platform shortcuts. `Shortcut::cmd('N')` is ⌘N on macOS, Ctrl+N elsewhere.

### Icon

`App::icon("icon.png")` sets the window/taskbar icon on Linux and Windows. On
macOS the icon comes from the `.app` bundle:

```sh
cargo build --release --example hello
tools/bundle-macos.sh target/release/examples/hello "Hello StandardUI" assets/icon.png
open "target/Hello StandardUI.app"
```

On Linux, install a `hello-standardui.desktop` file whose name matches the app id
(`standardui::icon::app_id("Hello StandardUI")` → `hello-standardui`) for docks to show the icon.

### Screenshots without a display

`App::snapshot(theme, scale)` renders the window off-screen. The example does
this for every theme:

```sh
STANDARDUI_SNAPSHOT=shots cargo run --example hello
```

## Not yet done (v0.1)

- Clipboard (cut/copy/paste in `TextField`), text selection, caret blink
- More widgets: ComboBox, Slider, List, TextArea, Tabs, ScrollView, Image
- Accessibility (AccessKit) and IME input
- Multiple windows, context menus, file dialogs wrapper (`rfd` is available)
- Native-control backend (real NSButton etc.) — API is designed so it can slot in

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.

[winit]: https://crates.io/crates/winit
[tiny-skia]: https://crates.io/crates/tiny-skia
[softbuffer]: https://crates.io/crates/softbuffer
[cosmic-text]: https://crates.io/crates/cosmic-text
[muda]: https://crates.io/crates/muda
[rfd]: https://crates.io/crates/rfd
