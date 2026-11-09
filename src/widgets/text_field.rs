use crate::event::{Event, Key, MouseButton};
use crate::geometry::{Rect, Size};
use crate::render::{Align, Painter};
use crate::theme::Platform;
use crate::ui::{Ui, WidgetCmd, WidgetCmdKind};
use crate::widget::{EventCx, LayoutCx, PaintCx, Uid, ValueCallback, Widget};

/// Single-line text input.
pub struct TextField<S: 'static> {
    uid: Uid,
    id: Option<String>,
    text: String,
    placeholder: String,
    /// Caret position in characters.
    caret: usize,
    bind: Option<Box<dyn Fn(&mut S) -> &mut String>>,
    on_change: Option<ValueCallback<S, String>>,
    on_submit: Option<ValueCallback<S, String>>,
    enabled: bool,
    hovered: bool,
    min_width: f32,
    rect: Rect,
}

impl<S: 'static> TextField<S> {
    pub fn new() -> Self {
        Self {
            uid: Uid::next(),
            id: None,
            text: String::new(),
            placeholder: String::new(),
            caret: 0,
            bind: None,
            on_change: None,
            on_submit: None,
            enabled: true,
            hovered: false,
            min_width: 120.0,
            rect: Rect::default(),
        }
    }

    pub fn id(mut self, id: &str) -> Self {
        self.id = Some(id.to_string());
        self
    }

    pub fn text(mut self, text: impl Into<String>) -> Self {
        self.text = text.into();
        self.caret = self.text.chars().count();
        self
    }

    pub fn placeholder(mut self, text: impl Into<String>) -> Self {
        self.placeholder = text.into();
        self
    }

    pub fn min_width(mut self, w: f32) -> Self {
        self.min_width = w;
        self
    }

    /// Two-way bind to a `String` in the state.
    pub fn bind(mut self, f: impl Fn(&mut S) -> &mut String + 'static) -> Self {
        self.bind = Some(Box::new(f));
        self
    }

    /// Called with `(text, state, ui)` after every edit.
    pub fn on_change(mut self, f: impl FnMut(String, &mut S, &mut Ui<S>) + 'static) -> Self {
        self.on_change = Some(Box::new(f));
        self
    }

    /// Called with `(text, state, ui)` when Enter is pressed.
    pub fn on_submit(mut self, f: impl FnMut(String, &mut S, &mut Ui<S>) + 'static) -> Self {
        self.on_submit = Some(Box::new(f));
        self
    }

    fn char_len(&self) -> usize {
        self.text.chars().count()
    }

    fn byte_index(&self, char_idx: usize) -> usize {
        self.text.char_indices().nth(char_idx).map(|(i, _)| i).unwrap_or(self.text.len())
    }

    fn changed(&mut self, cx: &mut EventCx<S>) {
        if let Some(b) = &self.bind {
            *b(cx.state) = self.text.clone();
        }
        if let Some(cb) = &mut self.on_change {
            cb(self.text.clone(), cx.state, cx.ui);
        }
        cx.request_redraw();
    }

    fn insert(&mut self, s: &str, cx: &mut EventCx<S>) {
        let at = self.byte_index(self.caret);
        self.text.insert_str(at, s);
        self.caret += s.chars().count();
        self.changed(cx);
    }

    fn padding(theme: &crate::theme::Theme) -> f32 {
        match theme.platform {
            Platform::MacOs => 5.0,
            _ => 10.0,
        }
    }
}

impl<S: 'static> Default for TextField<S> {
    fn default() -> Self {
        Self::new()
    }
}

impl<S: 'static> Widget<S> for TextField<S> {
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
            let v = b(state);
            if *v != self.text {
                self.text = v.clone();
                self.caret = self.caret.min(self.char_len());
            }
        }
    }
    fn measure(&mut self, cx: &mut LayoutCx, avail: Size) -> Size {
        Size::new(avail.width.max(self.min_width), cx.theme.control_height)
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
        let r = self.rect;
        let focused = cx.focused == Some(self.uid);
        let radius = match t.platform {
            Platform::MacOs => 5.0,
            _ => t.corner_radius,
        };

        p.fill_round_rect(r, radius, pal.field_bg);
        match t.platform {
            Platform::MacOs => {
                p.stroke_round_rect(r, radius, 1.0, pal.control_border);
                if focused {
                    p.stroke_round_rect(r.inset(-2.5), radius + 2.5, 3.5, pal.focus_ring);
                }
            }
            Platform::Windows => {
                p.stroke_round_rect(r, radius, 1.0, pal.control_border);
                let c = if focused { pal.accent } else { pal.text_secondary };
                let w = if focused { 2.0 } else { 1.0 };
                p.line(r.x + radius, r.bottom() - w / 2.0, r.right() - radius, r.bottom() - w / 2.0, w, c);
            }
            Platform::Linux => {
                if focused {
                    p.stroke_round_rect(r, radius, 2.0, pal.accent);
                }
            }
        }

        let pad = Self::padding(t);
        let inner = Rect::new(r.x + pad, r.y, r.width - 2.0 * pad, r.height);
        let fg = if self.enabled { pal.text } else { pal.text_disabled };
        let caret_x = if focused {
            Some(inner.x + p.text.prefix_width(&self.text, self.caret, t.font_size))
        } else {
            None
        };
        p.clipped(inner, |p| {
            if self.text.is_empty() {
                p.text_in(&self.placeholder, inner, Align::Start, pal.text_disabled);
            } else {
                p.text_in(&self.text, inner, Align::Start, fg);
            }
            if let Some(cx_) = caret_x {
                let ch = (t.font_size * 1.3).round();
                let cy = inner.y + (inner.height - ch) / 2.0;
                p.fill_rect(Rect::new(cx_.round(), cy, 1.0, ch), pal.text);
            }
        });
    }
    fn event(&mut self, ev: &Event, cx: &mut EventCx<S>) -> bool {
        match ev {
            Event::MouseMove(pos) => {
                self.hovered = self.rect.contains(*pos);
                false
            }
            Event::MouseDown { pos, button: MouseButton::Left } if self.rect.contains(*pos) && self.enabled => {
                cx.request_focus(self.uid);
                // Place caret at nearest character boundary.
                let pad = Self::padding(cx.theme);
                let rel = pos.x - (self.rect.x + pad);
                let n = self.char_len();
                let mut best = n;
                let mut best_d = f32::MAX;
                for i in 0..=n {
                    let w = cx.text.prefix_width(&self.text, i, cx.theme.font_size);
                    let d = (w - rel).abs();
                    if d < best_d {
                        best_d = d;
                        best = i;
                    }
                }
                self.caret = best;
                cx.request_redraw();
                true
            }
            Event::KeyDown { key, mods, text } if cx.is_focused(self.uid) && self.enabled => {
                match key {
                    Key::Left => {
                        if mods.command() || (cfg!(target_os = "macos") && mods.meta) {
                            self.caret = 0;
                        } else {
                            self.caret = self.caret.saturating_sub(1);
                        }
                    }
                    Key::Right => {
                        if mods.command() {
                            self.caret = self.char_len();
                        } else {
                            self.caret = (self.caret + 1).min(self.char_len());
                        }
                    }
                    Key::Home => self.caret = 0,
                    Key::End => self.caret = self.char_len(),
                    Key::Backspace => {
                        if self.caret > 0 {
                            let end = self.byte_index(self.caret);
                            self.caret -= 1;
                            let start = self.byte_index(self.caret);
                            self.text.replace_range(start..end, "");
                            self.changed(cx);
                        }
                    }
                    Key::Delete => {
                        if self.caret < self.char_len() {
                            let start = self.byte_index(self.caret);
                            let end = self.byte_index(self.caret + 1);
                            self.text.replace_range(start..end, "");
                            self.changed(cx);
                        }
                    }
                    Key::Enter => {
                        if let Some(cb) = &mut self.on_submit {
                            cb(self.text.clone(), cx.state, cx.ui);
                        }
                    }
                    Key::Tab | Key::Escape => return false,
                    _ => {
                        if mods.command() || mods.alt || mods.ctrl {
                            return false;
                        }
                        if let Some(s) = text {
                            let s: String = s.chars().filter(|c| !c.is_control()).collect();
                            if !s.is_empty() {
                                self.insert(&s, cx);
                            }
                        }
                    }
                }
                cx.request_redraw();
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
            WidgetCmdKind::SetText(t) => {
                self.text = t.clone();
                self.caret = self.caret.min(self.char_len());
            }
            WidgetCmdKind::SetEnabled(e) => self.enabled = *e,
            WidgetCmdKind::SetChecked(_) => {}
        }
        true
    }
}
