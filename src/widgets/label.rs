use crate::geometry::{Rect, Size};
use crate::render::{Align, Painter};
use crate::ui::{WidgetCmd, WidgetCmdKind};
use crate::widget::{EventCx, LayoutCx, PaintCx, Uid, Widget};
use crate::event::Event;

/// Static or state-bound text.
pub struct Label<S: 'static> {
    uid: Uid,
    id: Option<String>,
    text: String,
    bind: Option<Box<dyn Fn(&S) -> String>>,
    align: Align,
    secondary: bool,
    wrap: bool,
    rect: Rect,
    font_size: Option<f32>,
}

impl<S: 'static> Label<S> {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            uid: Uid::next(),
            id: None,
            text: text.into(),
            bind: None,
            align: Align::Start,
            secondary: false,
            wrap: false,
            rect: Rect::default(),
            font_size: None,
        }
    }

    pub fn id(mut self, id: &str) -> Self {
        self.id = Some(id.to_string());
        self
    }

    /// Recompute the text from state before every frame.
    pub fn bind(mut self, f: impl Fn(&S) -> String + 'static) -> Self {
        self.bind = Some(Box::new(f));
        self
    }

    pub fn align(mut self, align: Align) -> Self {
        self.align = align;
        self
    }

    /// Dimmer "secondary" text colour.
    pub fn secondary(mut self) -> Self {
        self.secondary = true;
        self
    }

    /// Wrap onto multiple lines at the available width.
    pub fn wrap(mut self) -> Self {
        self.wrap = true;
        self
    }

    /// Larger title text.
    pub fn title(mut self) -> Self {
        self.font_size = Some(20.0);
        self
    }

    pub fn font_size(mut self, size: f32) -> Self {
        self.font_size = Some(size);
        self
    }
}

impl<S: 'static> Widget<S> for Label<S> {
    fn uid(&self) -> Uid {
        self.uid
    }
    fn id(&self) -> Option<&str> {
        self.id.as_deref()
    }
    fn sync(&mut self, state: &mut S) {
        if let Some(b) = &self.bind {
            self.text = b(state);
        }
    }
    fn measure(&mut self, cx: &mut LayoutCx, avail: Size) -> Size {
        let fs = self.font_size.unwrap_or(cx.theme.font_size);
        if self.wrap {
            let buffer = cx.text.layout(&self.text, fs, Some(avail.width));
            let (w, h) = crate::text::measure_buffer(&buffer);
            Size::new(w.min(avail.width), h)
        } else {
            let (w, h) = cx.text.measure(&self.text, fs);
            Size::new(w, h.max(cx.theme.control_height.min(22.0)))
        }
    }
    fn layout(&mut self, _cx: &mut LayoutCx, rect: Rect) {
        self.rect = rect;
    }
    fn rect(&self) -> Rect {
        self.rect
    }
    fn paint(&mut self, p: &mut Painter, _cx: &PaintCx) {
        let color = if self.secondary { p.theme.palette.text_secondary } else { p.theme.palette.text };
        let fs = self.font_size.unwrap_or(p.theme.font_size);
        if self.wrap {
            p.text_wrapped(&self.text, self.rect, color);
        } else {
            p.text_in_sized(&self.text, self.rect, self.align, color, fs);
        }
    }
    fn event(&mut self, _ev: &Event, _cx: &mut EventCx<S>) -> bool {
        false
    }
    fn apply(&mut self, cmd: &WidgetCmd) -> bool {
        if self.id.as_deref() != Some(cmd.target.as_str()) {
            return false;
        }
        if let WidgetCmdKind::SetText(t) = &cmd.kind {
            self.text = t.clone();
        }
        true
    }
}
