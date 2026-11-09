//! Application icon helpers.

use std::path::Path;

/// Load a PNG as a winit window icon (used on Linux and Windows).
pub(crate) fn load_window_icon(path: &Path) -> Option<winit::window::Icon> {
    let img = image::open(path).ok()?.into_rgba8();
    let (w, h) = img.dimensions();
    winit::window::Icon::from_rgba(img.into_raw(), w, h).ok()
}

/// Reverse-DNS-ish app id derived from the app name, e.g. "Hello StandardUI" → "hello-standardui".
/// On Linux this is the WM_CLASS / Wayland app_id, which must match the
/// `.desktop` file name for the icon to show in docks and task switchers.
pub fn app_id(name: &str) -> String {
    let mut s: String = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
        .collect();
    while s.contains("--") {
        s = s.replace("--", "-");
    }
    s.trim_matches('-').to_string()
}
