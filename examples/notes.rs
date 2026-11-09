//! A notes window: a multi-line `TextArea` bound to the state, with a live
//! character count.
//!
//!     cargo run --example notes

use standardui::prelude::*;

struct State {
    title: String,
    notes: String,
}

fn main() {
    // `STANDARDUI_SNAPSHOT=out_dir cargo run --example notes` renders PNGs instead of opening a window.
    if let Ok(dir) = std::env::var("STANDARDUI_SNAPSHOT") {
        snapshots(&dir);
        return;
    }
    build().run();
}

fn build() -> App<State> {
    App::new(
        "Notes",
        State {
            title: String::new(),
            notes: "Shopping list:\n- Milk\n- Bread\n\nText wraps at the edge of the box, and once there's more than fits, it scrolls.".to_string(),
        },
    )
    .menu(MenuBar::new().menu("File", |m| m.quit()).menu("Edit", |m| m.standard_edit()))
    .window(
        Window::new("Notes").size(420.0, 320.0).min_size(300.0, 220.0).content(
            Column::new()
                .add(TextField::new().placeholder("Title").bind(|s: &mut State| &mut s.title))
                .add(
                    TextArea::new()
                        .id("notes")
                        .placeholder("Write something…")
                        .flex(1.0)
                        .bind(|s: &mut State| &mut s.notes),
                )
                .add(
                    Row::new()
                        .add(Label::new("").bind(|s: &State| format!("{} characters", s.notes.chars().count())).secondary())
                        .add(Spacer::new())
                        .add(Button::new("Clear").on_click(|s: &mut State, ui| {
                            s.notes.clear();
                            ui.focus("notes");
                        })),
                ),
        ),
    )
}

/// Headless renders: the initial window, then the text area focused and typed into.
fn snapshots(dir: &str) {
    std::fs::create_dir_all(dir).unwrap();
    let key = |key: Key| Event::KeyDown { key, mods: Modifiers::default(), text: None };
    let typed = |s: &str| -> Vec<Event> {
        s.chars()
            .map(|c| Event::KeyDown { key: Key::Character(c.to_string()), mods: Modifiers::default(), text: Some(c.to_string()) })
            .collect()
    };
    // Click at the end of the text area, add enough lines to overflow it, then move up a line.
    let mut events = vec![
        Event::MouseDown { pos: Point::new(380.0, 230.0), button: MouseButton::Left },
        Event::MouseUp { pos: Point::new(380.0, 230.0), button: MouseButton::Left },
    ];
    for line in ["One more line", "And another", "Still typing", "Last one"] {
        events.push(key(Key::Enter));
        events.extend(typed(line));
    }
    events.push(key(Key::Up));

    for (platform, tag) in [(Platform::Linux, "linux"), (Platform::MacOs, "macos"), (Platform::Windows, "windows")] {
        for dark in [false, true] {
            let mode = if dark { "dark" } else { "light" };
            let theme = Theme::for_platform(platform, dark);
            build().snapshot(theme.clone(), 2.0).save_png(format!("{dir}/notes-{tag}-{mode}.png")).unwrap();
            build().snapshot_with(theme, 2.0, &events).save_png(format!("{dir}/notes-{tag}-{mode}-typed.png")).unwrap();
        }
    }
    println!("wrote screenshots to {dir}");
}
