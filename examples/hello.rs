//! Hello world: a menu bar, a text field, a checkbox, buttons with callbacks,
//! and a label that follows the state.
//!
//!     cargo run --example hello

use standardui::prelude::*;

/// Whatever your callbacks need lives here. Every callback gets `&mut State`.
struct State {
    name: String,
    shout: bool,
    greetings: u32,
}

fn main() {
    let app = build();
    // `STANDARDUI_SNAPSHOT=out_dir cargo run --example hello` renders PNGs instead of opening a window.
    if let Ok(dir) = std::env::var("STANDARDUI_SNAPSHOT") {
        snapshots(&dir);
        return;
    }
    app.run();
}

fn build() -> App<State> {
    App::new("Hello StandardUI", State { name: String::new(), shout: false, greetings: 0 })
        .icon("assets/icon.png")
        .menu(
            MenuBar::new()
                .menu("File", |m| {
                    m.item("New Greeting", Shortcut::cmd('N'), |s: &mut State, ui| {
                        s.name.clear();
                        ui.focus("name");
                    })
                    .separator()
                    .preferences(|_s, ui| ui.alert("Settings", "Nothing to configure yet."))
                    .separator()
                    .quit()
                })
                .menu("Edit", |m| m.standard_edit())
                .menu("Help", |m| {
                    m.item("Say Hello", Shortcut::cmd('G'), |s: &mut State, ui| greet(s, ui))
                        .separator()
                        .about(|_s, ui| ui.alert("Hello StandardUI", "A tiny app built with standardui."))
                }),
        )
        .window(
            Window::new("Hello StandardUI")
                .size(460.0, 300.0)
                .min_size(360.0, 240.0)
                .content(
                    Column::new()
                        .add(Label::new("Hello, world!").title())
                        .add(Label::new("Type your name and press Greet (or Enter).").secondary())
                        .add(Spacer::fixed(4.0))
                        .add(
                            TextField::new()
                                .id("name")
                                .placeholder("Your name")
                                .bind(|s: &mut State| &mut s.name)
                                .on_submit(|_text, s: &mut State, ui| greet(s, ui)),
                        )
                        .add(Checkbox::new("Shout it").bind(|s: &mut State| &mut s.shout))
                        .add(Spacer::new())
                        .add(
                            Label::new("")
                                .bind(|s: &State| match s.greetings {
                                    0 => "No greetings yet.".to_string(),
                                    1 => "Greeted once.".to_string(),
                                    n => format!("Greeted {n} times."),
                                })
                                .secondary(),
                        )
                        .add(
                            Row::new()
                                .add(Spacer::new())
                                .add(Button::new("Clear").on_click(|s: &mut State, _ui| s.name.clear()))
                                .add(Button::new("Greet").primary().on_click(|s: &mut State, ui| greet(s, ui))),
                        ),
                ),
        )
}

/// Headless renders in every theme, with some interaction applied so the
/// screenshots show focus, hover and an open menu.
fn snapshots(dir: &str) {
    std::fs::create_dir_all(dir).unwrap();
    let typing: Vec<Event> = "Ada"
        .chars()
        .map(|c| Event::KeyDown { key: Key::Character(c.to_string()), mods: Modifiers::default(), text: Some(c.to_string()) })
        .collect();
    let mut events = vec![Event::MouseDown { pos: Point::new(200.0, 140.0), button: MouseButton::Left }];
    events.push(Event::MouseUp { pos: Point::new(200.0, 140.0), button: MouseButton::Left });
    events.extend(typing);
    events.push(Event::MouseMove(Point::new(400.0, 262.0))); // hover the Greet button
    let menu_open = [Event::MouseDown { pos: Point::new(20.0, 15.0), button: MouseButton::Left }, Event::MouseMove(Point::new(60.0, 60.0))];

    for (platform, tag) in [(Platform::Linux, "linux"), (Platform::MacOs, "macos"), (Platform::Windows, "windows")] {
        for dark in [false, true] {
            let theme = Theme::for_platform(platform, dark);
            let mode = if dark { "dark" } else { "light" };
            // Field focused + text typed + button hovered
            let mut evs = events.clone();
            if platform == Platform::MacOs {
                // no in-window menu bar on macOS, content sits 30px higher
                for e in &mut evs {
                    shift(e, -30.0);
                }
            }
            let px = build().snapshot_with(theme.clone(), 2.0, &evs);
            px.save_png(format!("{dir}/hello-{tag}-{mode}.png")).unwrap();
            // Menu open (in-window menus only)
            if platform != Platform::MacOs {
                let px = build().snapshot_with(theme, 2.0, &menu_open);
                px.save_png(format!("{dir}/hello-{tag}-{mode}-menu.png")).unwrap();
            }
        }
    }
    println!("wrote screenshots to {dir}");
}

fn shift(e: &mut Event, dy: f32) {
    match e {
        Event::MouseMove(p) | Event::MouseDown { pos: p, .. } | Event::MouseUp { pos: p, .. } => p.y += dy,
        _ => {}
    }
}

fn greet(s: &mut State, ui: &mut Ui<State>) {
    let who = if s.name.trim().is_empty() { "world" } else { s.name.trim() };
    let mut msg = format!("Hello, {who}!");
    if s.shout {
        msg = msg.to_uppercase();
    }
    s.greetings += 1;
    ui.alert("Greeting", msg);
}
