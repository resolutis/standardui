use standardui::prelude::*;

/// Whatever your callbacks need lives here. Every callback gets `&mut State`.
struct State {
    name: String,
    shout: bool,
    greetings: u32,
}

fn main() {
    let app = build();
    app.run();
}

fn build() -> App<State> {
    App::new(
        "Hello",
        State {
            name: String::new(),
            shout: false,
            greetings: 1,
        },
    )
    .icon("assets/icon.png")
    .menu(
        MenuBar::new()
            .menu("File", |m| {
                m.item("New", Shortcut::cmd('N'), |s: &mut State, ui| {
                    s.name.clear();
                    ui.focus("name");
                })
                .separator()
                .quit()
            })
            .menu("Edit", |m| m.standard_edit()),
    )
    .window(
        Window::new("Hello").size(420.0, 240.0).content(
            Column::new()
                .add(
                    TextField::new()
                        .id("name")
                        .placeholder("Your name")
                        .bind(|s: &mut State| &mut s.name),
                )
                .add(
                    Button::new("Greet")
                        .primary()
                        .on_click(|s: &mut State, ui| {
                            ui.alert("Hi", format!("Hello, {}!", s.name))
                        }),
                ),
        ),
    )
}
