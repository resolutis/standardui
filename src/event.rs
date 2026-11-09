//! Toolkit-level input events (already converted from the windowing layer to
//! logical pixels and platform-neutral keys).

use crate::geometry::Point;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Modifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    /// ⌘ on macOS, the Windows/Super key elsewhere.
    pub meta: bool,
}

impl Modifiers {
    /// The platform "command" modifier: ⌘ on macOS, Ctrl elsewhere.
    pub fn command(&self) -> bool {
        if cfg!(target_os = "macos") {
            self.meta
        } else {
            self.ctrl
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Key {
    Character(String),
    Enter,
    Tab,
    Escape,
    Backspace,
    Delete,
    Space,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    MouseMove(Point),
    MouseDown { pos: Point, button: MouseButton },
    MouseUp { pos: Point, button: MouseButton },
    MouseLeave,
    KeyDown { key: Key, mods: Modifiers, text: Option<String> },
    KeyUp { key: Key, mods: Modifiers },
}
