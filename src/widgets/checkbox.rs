use crate::event::{Event, Key, MouseButton};
use crate::geometry::{Rect, Size};
use crate::render::{Align, Painter};
use crate::theme::Platform;
use crate::ui::{Ui, WidgetCmd, WidgetCmdKind};
use crate::widget::{EventCx, LayoutCx, PaintCx, Uid, ValueCallback, Widget};

pub struct Checkbox<S: 'static> {
    uid: Uid,
    id: Option<String>,
    label: String,
    checked: bool,
    bind: Option<Box<dyn Fn(&mut S) -> &mut bool>>,
    on_toggle: Option<ValueCallback<S, bool>>,
    enabled: bool,
    hovered: bool,
    pressed: bool,
    rect: Rect,
}

impl<S: 'static> Checkbox<S> {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            uid: Uid::next(),
            id: None,
            label: label.into(),
            checked: false,
            bind: None,
            on_toggle: None,
            enabled: true,
            hovered: false,
            pressed: false,
            rect: Rect::default(),
        }
    }

    pub fn id(mut self, id: &str) -> Self {
        self.id = Some(id.to_string());
        self
    }

    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }

    /// Two-way bind to a `bool` in the state.
    pub fn bind(mut self, f: impl Fn(&mut S) -> &mut bool + 'static) -> Self {
        self.bind = Some(Box::new(f));
        self
    }

    /// Called with `(checked, state, ui)` after every toggle.
    pub fn on_toggle(mut self, f: impl FnMut(bool, &mut S, &mut Ui<S>) + 'static) -> Self {
        self.on_toggle = Some(Box::new(f));
        self
    }

    fn toggle(&mut self, cx: &mut EventCx<S>) {
        if !self.enabled {
            return;
        }
        self.checked = !self.checked;
        if let Some(b) = &self.bind {
            *b(cx.state) = self.checked;
        }
        if let Some(cb) = &mut self.on_toggle {
            cb(self.checked, cx.state, cx.ui);
        }
        cx.request_redraw();
    }

    fn box_size(theme: &crate::theme::Theme) -> f32 {
        match theme.platform {
            Platform::MacOs => 14.0,
            Platform::Windows => 20.0,
            Platform::Linux => 16.0,
        }
    }
}

impl<S: 'static> Widget<S> for Checkbox<S> {
    fn uid(&self) -> Uid {
        self.uid
    }
    fn id(&self) -> Option<&str> {
        self.id.as_deref()
    }
    fn focusable(&self) -> bool {
        self.enabled
    }
    fn sync(&mut self, state: &mut S) {
        if let Some(b) = &self.bind {
            self.checked = *b(state);
        }
    }
    fn measure(&mut self, cx: &mut LayoutCx, _avail: Size) -> Size {
        let (w, h) = cx.text.measure(&self.label, cx.theme.font_size);
        let b = Self::box_size(cx.theme);
        Size::new(b + 8.0 + w, h.max(b).max(cx.theme.control_height.min(24.0)))
    }
    fn layout(&mut self, _cx: &mut LayoutCx, rect: Rect) {
        self.rect = rect;
    }
    fn rect(&self) -> Rect {
        self.rect
    }
    fn paint(&mut self, p: &mut Painter, cx: &PaintCx) {
        let t = p.theme;
        let pal = &t.palette;
        let b = Self::box_size(t);
        let bx = Rect::new(self.rect.x, self.rect.y + (self.rect.height - b) / 2.0, b, b);
        let radius = match t.platform {
            Platform::MacOs => 3.5,
            Platform::Windows => 4.0,
            Platform::Linux => 4.0,
        };
        let focused = cx.focused == Some(self.uid);

        let (bg, border) = if self.checked && self.enabled {
            (pal.accent, pal.accent)
        } else if self.pressed {
            (pal.control_bg_pressed, pal.control_border)
        } else if self.hovered {
            (pal.control_bg_hover, pal.control_border)
        } else {
            (pal.control_bg, pal.control_border)
        };
        if t.platform == Platform::MacOs && !self.checked {
            p.fill_round_rect(bx.translate(0.0, 0.5), radius, pal.shadow.with_alpha(0.12));
        }
        p.fill_round_rect(bx, radius, bg);
        let border = if t.platform == Platform::Linux && !self.checked {
            pal.text.with_alpha(0.3)
        } else {
            border
        };
        p.stroke_round_rect(bx, radius, 1.0, border);
        if self.checked {
            p.check_mark(bx, if b > 16.0 { 2.0 } else { 1.8 }, pal.accent_text);
        }
        if focused {
            p.stroke_round_rect(bx.inset(-2.0), radius + 2.0, 2.5, pal.focus_ring);
        }

        let fg = if self.enabled { pal.text } else { pal.text_disabled };
        let text_rect = Rect::new(bx.right() + 8.0, self.rect.y, self.rect.width - b - 8.0, self.rect.height);
        p.text_in(&self.label, text_rect, Align::Start, fg);
    }
    fn event(&mut self, ev: &Event, cx: &mut EventCx<S>) -> bool {
        match ev {
            Event::MouseMove(pos) => {
                let h = self.rect.contains(*pos);
                if h != self.hovered {
                    self.hovered = h;
                    cx.request_redraw();
                }
                false
            }
            Event::MouseLeave => {
                self.hovered = false;
                self.pressed = false;
                false
            }
            Event::MouseDown { pos, button: MouseButton::Left } if self.rect.contains(*pos) && self.enabled => {
                self.pressed = true;
                cx.request_focus(self.uid);
                true
            }
            Event::MouseUp { pos, button: MouseButton::Left } if self.pressed => {
                self.pressed = false;
                if self.rect.contains(*pos) {
                    self.toggle(cx);
                }
                cx.request_redraw();
                true
            }
            Event::KeyDown { key: Key::Space, .. } if cx.is_focused(self.uid) => {
                self.toggle(cx);
                true
            }
            _ => false,
        }
    }
    fn apply(&mut self, cmd: &WidgetCmd) -> bool {
        if self.id.as_deref() != Some(cmd.target.as_str()) {
            return false;
        }
        match &cmd.kind {
            WidgetCmdKind::SetText(t) => self.label = t.clone(),
            WidgetCmdKind::SetEnabled(e) => self.enabled = *e,
            WidgetCmdKind::SetChecked(c) => self.checked = *c,
        }
        true
    }
}
