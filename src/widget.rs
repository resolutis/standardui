//! The widget contract. Widgets are owned by the tree; callbacks never capture
//! other widgets — they get `&mut S` (the app state) and `&mut Ui<S>` instead.

use std::sync::atomic::{AtomicU64, Ordering};

use crate::event::Event;
use crate::geometry::{Rect, Size};
use crate::render::Painter;
use crate::text::TextSystem;
use crate::theme::Theme;
use crate::ui::{Ui, WidgetCmd};

/// Unique per-widget identity, assigned at construction.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Uid(u64);

static NEXT_UID: AtomicU64 = AtomicU64::new(1);

impl Uid {
    pub fn next() -> Uid {
        Uid(NEXT_UID.fetch_add(1, Ordering::Relaxed))
    }
}

/// Click / action callback: `(state, ui)`.
pub type Callback<S> = Box<dyn FnMut(&mut S, &mut Ui<S>)>;
/// Value-change callback: `(new_value, state, ui)`.
pub type ValueCallback<S, V> = Box<dyn FnMut(V, &mut S, &mut Ui<S>)>;

pub struct LayoutCx<'a> {
    pub text: &'a mut TextSystem,
    pub theme: &'a Theme,
}

pub struct PaintCx {
    pub focused: Option<Uid>,
}

pub struct EventCx<'a, S: 'static> {
    pub state: &'a mut S,
    pub ui: &'a mut Ui<S>,
    pub theme: &'a Theme,
    pub text: &'a mut TextSystem,
    pub(crate) redraw: bool,
}

impl<'a, S: 'static> EventCx<'a, S> {
    pub fn focused(&self) -> Option<Uid> {
        self.ui.focus
    }
    pub fn is_focused(&self, uid: Uid) -> bool {
        self.ui.focus == Some(uid)
    }
    pub fn request_focus(&mut self, uid: Uid) {
        if self.ui.focus != Some(uid) {
            self.ui.focus = Some(uid);
            self.redraw = true;
        }
    }
    pub fn request_redraw(&mut self) {
        self.redraw = true;
    }
}

pub trait Widget<S: 'static> {
    fn uid(&self) -> Uid;

    /// The author-assigned id (for `ui.set_text("name", ..)` etc.).
    fn id(&self) -> Option<&str> {
        None
    }

    /// Pull bound values out of the state before layout/paint.
    fn sync(&mut self, _state: &mut S) {}

    /// Desired size given the available space.
    fn measure(&mut self, cx: &mut LayoutCx, avail: Size) -> Size;

    /// Assign the final rect (and lay out children).
    fn layout(&mut self, cx: &mut LayoutCx, rect: Rect);

    fn rect(&self) -> Rect;

    fn paint(&mut self, p: &mut Painter, cx: &PaintCx);

    /// Returns true if the event was consumed.
    fn event(&mut self, ev: &Event, cx: &mut EventCx<S>) -> bool;

    /// Apply a queued command if `cmd.target` is this widget's id. Containers
    /// should forward to children. Returns true when handled.
    fn apply(&mut self, _cmd: &WidgetCmd) -> bool {
        false
    }

    fn focusable(&self) -> bool {
        false
    }

    /// Append focusable uids in tab order.
    fn collect_focusable(&self, out: &mut Vec<Uid>) {
        if self.focusable() {
            out.push(self.uid());
        }
    }

    /// Find the uid of the widget with author id `id`.
    fn find_uid(&self, id: &str) -> Option<Uid> {
        if self.id() == Some(id) {
            Some(self.uid())
        } else {
            None
        }
    }

    /// Share of leftover space in a Row/Column (0 = fixed size).
    fn flex(&self) -> f32 {
        0.0
    }
}

pub type BoxedWidget<S> = Box<dyn Widget<S>>;

/// Convenience so `Column::add(Button::new(..))` and `Column::add(Box::new(..))` both work.
pub trait IntoWidget<S: 'static> {
    fn into_widget(self) -> BoxedWidget<S>;
}

impl<S: 'static, W: Widget<S> + 'static> IntoWidget<S> for W {
    fn into_widget(self) -> BoxedWidget<S> {
        Box::new(self)
    }
}
