//! # standardui
//!
//! Easy, native-looking desktop UIs in Rust.
//!
//! ```no_run
//! use standardui::prelude::*;
//!
//! struct State { name: String }
//!
//! fn main() {
//!     App::new("Hello", State { name: String::new() })
//!         .menu(MenuBar::new()
//!             .menu("File", |m| m.quit()))
//!         .window(Window::new("Hello")
//!             .content(Column::new()
//!                 .add(TextField::new().bind(|s: &mut State| &mut s.name))
//!                 .add(Button::new("Greet").on_click(|s: &mut State, ui| {
//!                     ui.alert("Hello", format!("Hi {}!", s.name));
//!                 }))))
//!         .run();
//! }
//! ```

pub mod app;
pub mod event;
pub mod geometry;
pub mod icon;
pub mod menu;
mod menubar_drawn;
pub mod platform;
pub mod render;
pub mod text;
pub mod theme;
pub mod ui;
pub mod widget;
pub mod widgets;

pub mod prelude {
    pub use crate::app::{App, Window};
    pub use crate::event::{Event, Key, Modifiers, MouseButton};
    pub use crate::geometry::{Insets, Point, Rect, Size};
    pub use crate::menu::{MenuBar, Shortcut, ShortcutKey};
    pub use crate::render::Align;
    pub use crate::theme::{Color, Platform, Theme};
    pub use crate::ui::{Ui, UiHandle};
    pub use crate::widget::Widget;
    pub use crate::widgets::*;
}
