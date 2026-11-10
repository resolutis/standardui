//! `Ui` is the handle every callback receives alongside `&mut S`. Nothing it
//! does happens immediately: it queues commands which the app applies once the
//! callback returns. That is what keeps callbacks re-entrancy-safe and lets a
//! button safely clear a text field it never captured.

use std::fmt;

use crate::widget::Uid;

/// Work posted from another thread, run on the UI thread with the state.
pub type Posted<S> = Box<dyn FnOnce(&mut S, &mut Ui<S>) + Send + 'static>;

/// Messages delivered through the window event loop.
pub enum Message<S: 'static> {
    Post(Posted<S>),
    /// A native menu item was clicked (macOS); carries the native item id.
    NativeMenu(String),
}

impl<S: 'static> fmt::Debug for Message<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Message::Post(_) => f.write_str("Message::Post(..)"),
            Message::NativeMenu(id) => write!(f, "Message::NativeMenu({id})"),
        }
    }
}

pub(crate) enum Cmd {
    Quit,
    Alert { title: String, message: String },
    SetTitle(String),
    Widget(WidgetCmd),
    Focus(String),
}

#[derive(Clone, Debug)]
pub struct WidgetCmd {
    pub target: String,
    pub kind: WidgetCmdKind,
}

#[derive(Clone, Debug)]
pub enum WidgetCmdKind {
    SetText(String),
    SetEnabled(bool),
    SetChecked(bool),
    /// Select an item of a `ComboBox` by index, or clear the selection.
    SetSelected(Option<usize>),
    /// Replace the items of a `ComboBox`.
    SetItems(Vec<String>),
}

pub struct Ui<S: 'static> {
    pub(crate) cmds: Vec<Cmd>,
    pub(crate) proxy: Option<winit::event_loop::EventLoopProxy<Message<S>>>,
    pub(crate) focus: Option<Uid>,
}

impl<S: 'static> Ui<S> {
    pub(crate) fn new(proxy: Option<winit::event_loop::EventLoopProxy<Message<S>>>) -> Self {
        Self { cmds: Vec::new(), proxy, focus: None }
    }

    /// Quit the application after this callback returns.
    pub fn quit(&mut self) {
        self.cmds.push(Cmd::Quit);
    }

    /// Show a native message box.
    pub fn alert(&mut self, title: impl Into<String>, message: impl Into<String>) {
        self.cmds.push(Cmd::Alert { title: title.into(), message: message.into() });
    }

    /// Change the window title.
    pub fn set_title(&mut self, title: impl Into<String>) {
        self.cmds.push(Cmd::SetTitle(title.into()));
    }

    /// Set the text of the widget with this `id` (Label, Button, TextField).
    pub fn set_text(&mut self, id: &str, text: impl Into<String>) {
        self.widget(id, WidgetCmdKind::SetText(text.into()));
    }

    pub fn set_enabled(&mut self, id: &str, enabled: bool) {
        self.widget(id, WidgetCmdKind::SetEnabled(enabled));
    }

    pub fn set_checked(&mut self, id: &str, checked: bool) {
        self.widget(id, WidgetCmdKind::SetChecked(checked));
    }

    /// Select item `index` of the `ComboBox` with this `id`, or pass `None` to
    /// clear the selection. `on_select` is not called. If the combo box is bound
    /// to the state, change the state instead.
    pub fn set_selected(&mut self, id: &str, index: impl Into<Option<usize>>) {
        self.widget(id, WidgetCmdKind::SetSelected(index.into()));
    }

    /// Replace the items of the `ComboBox` with this `id`. The selection is kept
    /// if that index still exists, and cleared otherwise.
    pub fn set_items<T: Into<String>>(&mut self, id: &str, items: impl IntoIterator<Item = T>) {
        self.widget(id, WidgetCmdKind::SetItems(items.into_iter().map(Into::into).collect()));
    }

    /// Move keyboard focus to the widget with this `id`.
    pub fn focus(&mut self, id: &str) {
        self.cmds.push(Cmd::Focus(id.to_string()));
    }

    fn widget(&mut self, id: &str, kind: WidgetCmdKind) {
        self.cmds.push(Cmd::Widget(WidgetCmd { target: id.to_string(), kind }));
    }

    /// A `Send` handle for posting work back onto the UI thread from a worker
    /// thread (the equivalent of Swing's `invokeLater`).
    pub fn handle(&self) -> UiHandle<S> {
        UiHandle { proxy: self.proxy.clone() }
    }

    /// Run `work` on a background thread, then `then` back on the UI thread with
    /// the result and the state.
    pub fn spawn<R, W, T>(&self, work: W, then: T)
    where
        R: Send + 'static,
        W: FnOnce() -> R + Send + 'static,
        T: FnOnce(R, &mut S, &mut Ui<S>) + Send + 'static,
    {
        let handle = self.handle();
        std::thread::spawn(move || {
            let r = work();
            handle.post(move |s, ui| then(r, s, ui));
        });
    }
}

pub struct UiHandle<S: 'static> {
    proxy: Option<winit::event_loop::EventLoopProxy<Message<S>>>,
}

impl<S: 'static> Clone for UiHandle<S> {
    fn clone(&self) -> Self {
        Self { proxy: self.proxy.clone() }
    }
}

impl<S: 'static> UiHandle<S> {
    /// Queue `f` to run on the UI thread with access to the state.
    pub fn post(&self, f: impl FnOnce(&mut S, &mut Ui<S>) + Send + 'static) {
        if let Some(p) = &self.proxy {
            let _ = p.send_event(Message::Post(Box::new(f)));
        }
    }
}
