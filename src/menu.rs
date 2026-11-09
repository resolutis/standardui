//! Menu model. Build it once with the fluent API; standardui decides how to show it:
//! the real global menu bar on macOS, an in-window bar on Linux and Windows.
//! Standard roles (`quit`, `about`, `preferences`, `standard_edit`) land where
//! each OS expects them, with the right names and shortcuts.

use crate::event::{Key, Modifiers};
use crate::ui::Ui;
use crate::widget::Callback;

/// A keyboard shortcut. `Shortcut::cmd('N')` is ⌘N on macOS and Ctrl+N elsewhere.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shortcut {
    pub key: ShortcutKey,
    pub command: bool,
    pub shift: bool,
    pub alt: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ShortcutKey {
    Char(char),
    Enter,
    Delete,
    Backspace,
    Escape,
}

impl Shortcut {
    pub fn cmd(c: char) -> Self {
        Self { key: ShortcutKey::Char(c.to_ascii_uppercase()), command: true, shift: false, alt: false }
    }
    pub fn cmd_shift(c: char) -> Self {
        Self { shift: true, ..Self::cmd(c) }
    }
    pub fn cmd_alt(c: char) -> Self {
        Self { alt: true, ..Self::cmd(c) }
    }
    pub fn key(key: ShortcutKey) -> Self {
        Self { key, command: false, shift: false, alt: false }
    }

    pub fn matches(&self, key: &Key, mods: &Modifiers) -> bool {
        if self.command != mods.command() || self.shift != mods.shift || self.alt != mods.alt {
            return false;
        }
        match (&self.key, key) {
            (ShortcutKey::Char(c), Key::Character(s)) => s.chars().next().map(|k| k.to_ascii_uppercase()) == Some(*c),
            (ShortcutKey::Enter, Key::Enter)
            | (ShortcutKey::Delete, Key::Delete)
            | (ShortcutKey::Backspace, Key::Backspace)
            | (ShortcutKey::Escape, Key::Escape) => true,
            _ => false,
        }
    }

    /// Text shown at the right of a menu item in the in-window menu bar.
    pub fn display(&self) -> String {
        let mut parts = Vec::new();
        if self.command {
            parts.push(if cfg!(target_os = "macos") { "⌘" } else { "Ctrl+" });
        }
        if self.shift {
            parts.push(if cfg!(target_os = "macos") { "⇧" } else { "Shift+" });
        }
        if self.alt {
            parts.push(if cfg!(target_os = "macos") { "⌥" } else { "Alt+" });
        }
        let k = match &self.key {
            ShortcutKey::Char(c) => c.to_string(),
            ShortcutKey::Enter => "Enter".into(),
            ShortcutKey::Delete => "Del".into(),
            ShortcutKey::Backspace => "Backspace".into(),
            ShortcutKey::Escape => "Esc".into(),
        };
        format!("{}{}", parts.concat(), k)
    }

    /// muda-style accelerator string, e.g. `CmdOrCtrl+Shift+N`.
    #[allow(dead_code)]
    pub(crate) fn accelerator_string(&self) -> String {
        let mut s = String::new();
        if self.command {
            s.push_str("CmdOrCtrl+");
        }
        if self.shift {
            s.push_str("Shift+");
        }
        if self.alt {
            s.push_str("Alt+");
        }
        match &self.key {
            ShortcutKey::Char(c) => s.push(*c),
            ShortcutKey::Enter => s.push_str("Enter"),
            ShortcutKey::Delete => s.push_str("Delete"),
            ShortcutKey::Backspace => s.push_str("Backspace"),
            ShortcutKey::Escape => s.push_str("Escape"),
        }
        s
    }
}

/// Standard menu items whose placement, label and shortcut differ per OS.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Quit,
    About,
    Preferences,
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    SelectAll,
    Minimize,
    CloseWindow,
}

pub(crate) type MenuItemId = usize;

pub enum MenuEntry<S: 'static> {
    Item {
        id: MenuItemId,
        label: String,
        shortcut: Option<Shortcut>,
        action: Option<Callback<S>>,
        enabled: bool,
        role: Option<Role>,
    },
    Separator,
}

pub struct Menu<S: 'static> {
    pub title: String,
    pub entries: Vec<MenuEntry<S>>,
}

impl<S: 'static> Menu<S> {
    fn new(title: impl Into<String>) -> Self {
        Self { title: title.into(), entries: Vec::new() }
    }

    fn push_item(
        &mut self,
        label: impl Into<String>,
        shortcut: Option<Shortcut>,
        action: Option<Callback<S>>,
        role: Option<Role>,
    ) {
        self.entries.push(MenuEntry::Item { id: 0, label: label.into(), shortcut, action, enabled: true, role });
    }

    /// A menu item with an optional shortcut (`Shortcut::cmd('N')` or `None`) and a `(state, ui)` callback.
    pub fn item(
        mut self,
        label: impl Into<String>,
        shortcut: impl Into<Option<Shortcut>>,
        action: impl FnMut(&mut S, &mut Ui<S>) + 'static,
    ) -> Self {
        self.push_item(label, shortcut.into(), Some(Box::new(action)), None);
        self
    }

    pub fn separator(mut self) -> Self {
        self.entries.push(MenuEntry::Separator);
        self
    }

    /// "Quit <App>" on macOS (⌘Q, in the app menu), "Quit"/"Exit" with Ctrl+Q elsewhere.
    pub fn quit(mut self) -> Self {
        let label = if cfg!(target_os = "windows") { "Exit" } else { "Quit" };
        self.push_item(label, Some(Shortcut::cmd('Q')), None, Some(Role::Quit));
        self
    }

    /// "About <App>". On macOS this moves to the app menu.
    pub fn about(mut self, action: impl FnMut(&mut S, &mut Ui<S>) + 'static) -> Self {
        self.push_item("About", None, Some(Box::new(action)), Some(Role::About));
        self
    }

    /// "Settings…" (⌘, in the app menu on macOS) / "Preferences…" elsewhere.
    pub fn preferences(mut self, action: impl FnMut(&mut S, &mut Ui<S>) + 'static) -> Self {
        let label = if cfg!(target_os = "macos") { "Settings…" } else { "Preferences…" };
        self.push_item(label, Some(Shortcut::cmd(',')), Some(Box::new(action)), Some(Role::Preferences));
        self
    }

    /// Undo, Redo, Cut, Copy, Paste, Select All with the platform's shortcuts.
    pub fn standard_edit(mut self) -> Self {
        self.push_item("Undo", Some(Shortcut::cmd('Z')), None, Some(Role::Undo));
        self.push_item("Redo", Some(if cfg!(target_os = "macos") { Shortcut::cmd_shift('Z') } else { Shortcut::cmd('Y') }), None, Some(Role::Redo));
        self.entries.push(MenuEntry::Separator);
        self.push_item("Cut", Some(Shortcut::cmd('X')), None, Some(Role::Cut));
        self.push_item("Copy", Some(Shortcut::cmd('C')), None, Some(Role::Copy));
        self.push_item("Paste", Some(Shortcut::cmd('V')), None, Some(Role::Paste));
        self.push_item("Select All", Some(Shortcut::cmd('A')), None, Some(Role::SelectAll));
        self
    }

    /// Minimize and Close Window (used for a "Window" menu).
    pub fn standard_window(mut self) -> Self {
        self.push_item("Minimize", Some(Shortcut::cmd('M')), None, Some(Role::Minimize));
        self.push_item("Close Window", Some(Shortcut::cmd('W')), None, Some(Role::CloseWindow));
        self
    }
}

pub struct MenuBar<S: 'static> {
    pub menus: Vec<Menu<S>>,
    next_id: MenuItemId,
}

impl<S: 'static> Default for MenuBar<S> {
    fn default() -> Self {
        Self::new()
    }
}

impl<S: 'static> MenuBar<S> {
    pub fn new() -> Self {
        Self { menus: Vec::new(), next_id: 1 }
    }

    /// Add a top-level menu built by `f`.
    pub fn menu(mut self, title: impl Into<String>, f: impl FnOnce(Menu<S>) -> Menu<S>) -> Self {
        let mut menu = f(Menu::new(title));
        for e in &mut menu.entries {
            if let MenuEntry::Item { id, .. } = e {
                *id = self.next_id;
                self.next_id += 1;
            }
        }
        self.menus.push(menu);
        self
    }

    pub(crate) fn find_shortcut(&self, key: &Key, mods: &Modifiers) -> Option<MenuItemId> {
        for m in &self.menus {
            for e in &m.entries {
                if let MenuEntry::Item { id, shortcut: Some(sc), enabled: true, .. } = e {
                    if sc.matches(key, mods) {
                        return Some(*id);
                    }
                }
            }
        }
        None
    }

    /// Run the item's action; returns the role for roles the app handles itself (Quit etc.).
    pub(crate) fn activate(&mut self, item: MenuItemId, state: &mut S, ui: &mut Ui<S>) -> Option<Role> {
        for m in &mut self.menus {
            for e in &mut m.entries {
                if let MenuEntry::Item { id, action, role, .. } = e {
                    if *id == item {
                        if let Some(a) = action {
                            a(state, ui);
                        }
                        return *role;
                    }
                }
            }
        }
        None
    }
}
