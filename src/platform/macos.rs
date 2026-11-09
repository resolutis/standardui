//! macOS: the menu bar is the real global one, built with muda.

use std::collections::HashMap;

use muda::accelerator::Accelerator;
use muda::{AboutMetadata, Menu as NMenu, MenuEvent, MenuItem as NItem, PredefinedMenuItem, Submenu};

use crate::menu::{MenuBar, MenuEntry, MenuItemId, Role};

pub struct NativeMenu {
    _menu: NMenu,
    ids: HashMap<String, MenuItemId>,
}

impl NativeMenu {
    /// Build and install the menu bar. `notify` is called (from the main thread)
    /// with the native item id whenever an item is clicked.
    pub fn build<S: 'static>(app_name: &str, bar: &MenuBar<S>, notify: impl Fn(String) + Send + Sync + 'static) -> NativeMenu {
        MenuEvent::set_event_handler(Some(move |ev: MenuEvent| notify(ev.id.0)));
        let menu = NMenu::new();
        let mut ids = HashMap::new();

        // --- App menu: About / Settings… / Services / Hide / Quit ---
        let app_menu = Submenu::new(app_name, true);
        let mut about: Option<&MenuEntry<S>> = None;
        let mut prefs: Option<&MenuEntry<S>> = None;
        let mut quit: Option<&MenuEntry<S>> = None;
        for m in &bar.menus {
            for e in &m.entries {
                if let MenuEntry::Item { role: Some(r), .. } = e {
                    match r {
                        Role::About => about = Some(e),
                        Role::Preferences => prefs = Some(e),
                        Role::Quit => quit = Some(e),
                        _ => {}
                    }
                }
            }
        }
        match about {
            Some(MenuEntry::Item { id, action: Some(_), .. }) => {
                let item = NItem::new(format!("About {app_name}"), true, None);
                ids.insert(item.id().0.clone(), *id);
                let _ = app_menu.append(&item);
            }
            _ => {
                let meta = AboutMetadata { name: Some(app_name.to_string()), ..Default::default() };
                let _ = app_menu.append(&PredefinedMenuItem::about(None, Some(meta)));
            }
        }
        if let Some(MenuEntry::Item { id, label, shortcut, .. }) = prefs {
            let _ = app_menu.append(&PredefinedMenuItem::separator());
            let item = NItem::new(label, true, shortcut.as_ref().and_then(accel));
            ids.insert(item.id().0.clone(), *id);
            let _ = app_menu.append(&item);
        }
        let _ = app_menu.append(&PredefinedMenuItem::separator());
        let _ = app_menu.append(&PredefinedMenuItem::services(None));
        let _ = app_menu.append(&PredefinedMenuItem::separator());
        let _ = app_menu.append(&PredefinedMenuItem::hide(None));
        let _ = app_menu.append(&PredefinedMenuItem::hide_others(None));
        let _ = app_menu.append(&PredefinedMenuItem::show_all(None));
        let _ = app_menu.append(&PredefinedMenuItem::separator());
        if let Some(MenuEntry::Item { id, .. }) = quit {
            let item = NItem::new(format!("Quit {app_name}"), true, "CmdOrCtrl+Q".parse().ok());
            ids.insert(item.id().0.clone(), *id);
            let _ = app_menu.append(&item);
        } else {
            let _ = app_menu.append(&PredefinedMenuItem::quit(None));
        }
        let _ = menu.append(&app_menu);

        // --- The app's own menus ---
        for m in &bar.menus {
            let sub = Submenu::new(&m.title, true);
            let mut pending_sep = false;
            let mut count = 0;
            for e in &m.entries {
                match e {
                    MenuEntry::Separator => pending_sep = count > 0,
                    MenuEntry::Item { id, label, shortcut, enabled, role, .. } => {
                        // Roles that live in the app menu on macOS are skipped here.
                        if matches!(role, Some(Role::About | Role::Preferences | Role::Quit)) {
                            continue;
                        }
                        if pending_sep {
                            let _ = sub.append(&PredefinedMenuItem::separator());
                            pending_sep = false;
                        }
                        let predefined = match role {
                            Some(Role::Undo) => Some(PredefinedMenuItem::undo(None)),
                            Some(Role::Redo) => Some(PredefinedMenuItem::redo(None)),
                            Some(Role::Cut) => Some(PredefinedMenuItem::cut(None)),
                            Some(Role::Copy) => Some(PredefinedMenuItem::copy(None)),
                            Some(Role::Paste) => Some(PredefinedMenuItem::paste(None)),
                            Some(Role::SelectAll) => Some(PredefinedMenuItem::select_all(None)),
                            Some(Role::Minimize) => Some(PredefinedMenuItem::minimize(None)),
                            Some(Role::CloseWindow) => Some(PredefinedMenuItem::close_window(None)),
                            _ => None,
                        };
                        if let Some(p) = predefined {
                            let _ = sub.append(&p);
                        } else {
                            let item = NItem::new(label, *enabled, shortcut.as_ref().and_then(accel));
                            ids.insert(item.id().0.clone(), *id);
                            let _ = sub.append(&item);
                        }
                        count += 1;
                    }
                }
            }
            if count == 0 {
                continue;
            }
            if m.title == "Window" {
                let _ = sub.set_as_windows_menu_for_nsapp();
            } else if m.title == "Help" {
                let _ = sub.set_as_help_menu_for_nsapp();
            }
            let _ = menu.append(&sub);
        }

        menu.init_for_nsapp();
        NativeMenu { _menu: menu, ids }
    }

    /// Map a native item id back to the menu model's item id.
    pub fn resolve(&self, native_id: &str) -> Option<MenuItemId> {
        self.ids.get(native_id).copied()
    }
}

fn accel(sc: &crate::menu::Shortcut) -> Option<Accelerator> {
    sc.accelerator_string().parse().ok()
}
