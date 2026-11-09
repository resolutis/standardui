# Changelog

All notable changes to standardui are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.2] - 2026-11-09

### Added

- `TextArea`, a multi-line text input.
  - Text wraps at the widget's width. When it's taller than the widget, it
    scrolls with the mouse wheel or trackpad, and follows the caret as you
    type or move. A thin indicator on the right shows the scroll position.
  - Height is `.rows(n)` lines (default 4), or use `.flex(1.0)` in a `Column`
    to fill the available height.
  - Same API as `TextField` where it overlaps: `.bind(..)`, `.on_change(..)`,
    `.placeholder(..)`, `.text(..)`, `.id(..)`, `.min_width(..)`, and
    `ui.set_text` / `ui.set_enabled` by id.
  - Keys: Enter inserts a new line. ↑/↓ move between lines and keep the
    column. Home/End (⌘←/⌘→ on macOS) go to the start or end of the visual
    line. Ctrl+Home/End (⌘↑/⌘↓ on macOS) go to the start or end of the text.
  - No text selection or clipboard yet, the same as `TextField`.
- `Event::Scroll { pos, delta }` for the mouse wheel and trackpad. `delta` is
  in logical pixels; positive `y` scrolls towards the top. Code that matches
  on `Event` exhaustively needs a new arm (or a `_` arm).
- `notes` example showing a `TextArea` bound to the state.

## [0.1.1] - 2026-11-09

This release fixes how `Row` and `Column` share out space. Before, a widget
placed after a stretching child in a `Row` could end up just past the window's
right edge, so it never appeared, however wide the window was.

### What was wrong

Widgets reported *how much space they were offered* as their size, rather than
*how much they needed*. Once one widget did that, every container above it did
too, and a single greedy child filled the whole row. Three things combined:

1. `TextField::measure` asked for all the width it was offered:
   `avail.width.max(min_width)`.
2. A `Column` stretches its children by default, so its `measure` also reported
   its full available width rather than the width of its widest child.
3. `Stack::measure` gave every child of a `Row` the Row's full inner width. It
   never took off what earlier children had already used.

For example, take a `Row` with a `Column` (holding a `TextField` and a button)
followed by a second `Column` holding another button. In a 420px window, the
first column reported a width of about 420px, so `layout` placed the second
column at `x = 420 + gap`, just past the right edge.

Making the window wider didn't help. The first column's width was never fixed;
it was always the window's full width. At 1000px it asked for about 1000px, and
the second column started at about 1008px, still just off-screen. Adding
`.flex()` didn't help either: the fixed children already used all the space,
so the leftover to share out was 0.

Nothing warned about this, so the widget just wasn't there.

### Fixed

- `Row` and `Column` no longer offer every child the full available space.
  Measuring now happens in two passes: fixed-size children are measured first,
  each offered only the space the earlier ones left. Flex children then split
  what remains by their flex weights.
- A `Column` now reports the width of its widest child instead of claiming all
  the width it is offered. With the default `CrossAlign::Stretch` it still
  stretches its children to its full width when laid out.
- A stack that its parent stretches or gives a flex share is now measured
  again against its final size, so its children fit the space they actually
  get.
- A wrapped `Label` now reports the width of its widest line instead of the
  full available width.

### Added

- `TextField::flex(f32)` lets a text field take a share of the leftover space
  in a `Row` or `Column`, for example `.flex(1.0)` to fill the rest of a row.
- Overflow warning in debug builds. When a `Row` or `Column`'s fixed-size
  children don't fit, standardui prints a warning to stderr instead of placing
  them out of sight without saying anything:

  ```
  standardui: Row overflowed by 700px (children need 1080px, have 380px); use .flex() on a child to let it take the remaining space instead
  ```

  It prints once each time the overflow amount changes, not every frame.

### Changed

- **A `TextField` in a `Row` no longer fills the row by default.** It now takes
  its natural width (`min_width`, 120px unless set). Add `.flex(1.0)` to the
  field, or to the `Column` containing it, to make it fill the row again. A
  `TextField` in a `Column` still fills the column's width, as before.
- Flex children now get exactly their weighted share of the leftover space.
  Before, they got their natural size plus a share of the leftover.

### Upgrading from 0.1.0

If a layout relied on a `TextField` or a stretching `Column` filling a `Row`,
add `.flex(1.0)` to that child:

```rust
Row::new()
    .add(Column::new().flex(1.0).add(text_field).add(greet))
    .add(Column::new().add(other_button))
```

If you see an overflow warning in a debug build, the stack's fixed-size
children are wider (or taller) than the space available. Give one of them
`.flex(..)`, or reduce its `min_width`.

## [0.1.0] - 2026-11-09

First release. standardui builds easy, native-looking desktop UIs in Rust:
self-drawn widgets themed to match the OS, with callbacks that take your app's
state.

### Added

- **App core.** `App`, `Window` and `Ui`. Every callback receives `&mut S`
  (your state) and `&mut Ui<S>`.
  - `Ui` queues commands (`alert`, `quit`, `set_text`, `set_enabled`,
    `set_checked`, `focus`, `set_title`) and applies them after the callback
    returns, so callbacks are safe to re-enter.
  - `ui.spawn(work, then)` runs `work` on a background thread and `then` back
    on the UI thread. `ui.handle()` gives a `Send` handle for posting work to
    the UI thread from elsewhere.
- **Widgets.** `Label`, `Button`, `Checkbox`, `TextField`, `Row` and `Column`
  stacks, and `Spacer`. Widgets can be given an id and addressed through `Ui`.
- **Bindings.** `TextField::bind` and `Checkbox::bind` bind two ways to a field
  in your state. `Label::bind` recomputes its text every frame.
- **Menus.**
  - macOS uses the real global menu bar (via muda). Linux and Windows get an
    in-window menu bar drawn by standardui.
  - Standard roles go where each OS expects them: `.quit()`, `.about(..)` and
    `.preferences(..)` move to the app menu on macOS, with ⌘Q and ⌘,.
  - `.standard_edit()` adds Undo, Redo, Cut, Copy, Paste and Select All with
    the platform's shortcuts.
  - `Shortcut::cmd('N')` is ⌘N on macOS and Ctrl+N elsewhere.
- **Themes.** Aqua (macOS), Fluent (Windows) and Adwaita (Linux) looks, each in
  light and dark. The theme is picked from the OS at startup and follows
  light/dark changes live.
- **Rendering and text.** Software rendering with tiny-skia and softbuffer,
  HiDPI-aware. Text uses cosmic-text, with system fonts and shaping.
- **Dialogs** through rfd. On Linux this uses the XDG desktop portal, so
  building doesn't need the GTK development headers.
- **App icon.** `App::icon(..)` sets the window and taskbar icon on Linux and
  Windows. `tools/bundle-macos.sh` builds a macOS `.app` bundle with the icon.
- **Headless screenshots.** `App::snapshot` and `App::snapshot_with` render a
  window off-screen to an image.
- **Examples.** `hello` shows menus, a text field, a checkbox, a bound label
  and buttons. Run it with `STANDARDUI_SNAPSHOT=<dir>` to write screenshots in
  every theme instead of opening a window.
- **Docs.** `INSTALL.md` covers making a release build, a macOS `.app` and a
  DMG installer.
- Dual-licensed under MIT or Apache-2.0.

### Known limitations

- Clipboard (cut, copy and paste in `TextField`), text selection and caret
  blink are not implemented yet.
- No ComboBox, Slider, List, TextArea, Tabs, ScrollView or Image widgets yet.
- No accessibility (AccessKit) or IME input yet.
- One window per app; no context menus; no wrapper for file dialogs yet (rfd
  is available directly).

[Unreleased]: https://github.com/resolutis/standardui/compare/v0.1.2...HEAD
[0.1.2]: https://github.com/resolutis/standardui/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/resolutis/standardui/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/resolutis/standardui/releases/tag/v0.1.0
