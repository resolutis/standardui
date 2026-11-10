//! A settings window: `ComboBox` drop-down lists bound to the state, with a
//! label that follows the choices.
//!
//!     cargo run --example settings

use standardui::prelude::*;

const THEMES: [&str; 3] = ["Match system", "Light", "Dark"];
const SIZES: [&str; 5] = ["Small", "Medium", "Large", "Extra large", "Huge"];

struct State {
    theme: usize,
    size: usize,
    changes: u32,
}

fn main() {
    // `STANDARDUI_SNAPSHOT=out_dir cargo run --example settings` renders PNGs instead of opening a window.
    if let Ok(dir) = std::env::var("STANDARDUI_SNAPSHOT") {
        snapshots(&dir);
        return;
    }
    build().run();
}

fn build() -> App<State> {
    App::new("Settings", State { theme: 0, size: 1, changes: 0 })
        .menu(MenuBar::new().menu("File", |m| m.quit()))
        .window(
            Window::new("Settings").size(380.0, 340.0).min_size(300.0, 220.0).content(
                Column::new()
                    .add(Label::new("Appearance").title())
                    .add(
                        Row::new()
                            .add(Label::new("Theme"))
                            .add(Spacer::new())
                            .add(ComboBox::new(THEMES).bind(|s: &mut State| &mut s.theme).on_select(|_i, s: &mut State, _ui| s.changes += 1)),
                    )
                    .add(
                        Row::new()
                            .add(Label::new("Text size"))
                            .add(Spacer::new())
                            .add(ComboBox::new(SIZES).id("size").bind(|s: &mut State| &mut s.size).on_select(|_i, s: &mut State, _ui| s.changes += 1)),
                    )
                    .add(Spacer::new())
                    .add(
                        Label::new("")
                            .bind(|s: &State| format!("{} theme, {} text. Changed {} times.", THEMES[s.theme], SIZES[s.size].to_lowercase(), s.changes))
                            .secondary(),
                    )
                    .add(Row::new().add(Spacer::new()).add(Button::new("Reset").on_click(|s: &mut State, _ui| {
                        s.theme = 0;
                        s.size = 1;
                    }))),
            ),
        )
}

/// Headless renders: the initial window, then the second list opened with the keyboard.
fn snapshots(dir: &str) {
    std::fs::create_dir_all(dir).unwrap();
    let key = |key: Key| Event::KeyDown { key, mods: Modifiers::default(), text: None };
    // Tab to the second combo box, open it, and move down one item.
    let events = [key(Key::Tab), key(Key::Tab), key(Key::Space), key(Key::Down)];

    for (platform, tag) in [(Platform::Linux, "linux"), (Platform::MacOs, "macos"), (Platform::Windows, "windows")] {
        for dark in [false, true] {
            let mode = if dark { "dark" } else { "light" };
            let theme = Theme::for_platform(platform, dark);
            build().snapshot(theme.clone(), 2.0).save_png(format!("{dir}/settings-{tag}-{mode}.png")).unwrap();
            build().snapshot_with(theme, 2.0, &events).save_png(format!("{dir}/settings-{tag}-{mode}-open.png")).unwrap();
        }
    }
    println!("wrote screenshots to {dir}");
}
