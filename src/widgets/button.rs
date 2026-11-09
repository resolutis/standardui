use crate::event::{Event, Key, MouseButton};
use crate::geometry::{Rect, Size};
use crate::render::{Align, Painter};
use crate::theme::Platform;
use crate::ui::{WidgetCmd, WidgetCmdKind};
use crate::widget::{Callback, EventCx, LayoutCx, PaintCx, Uid, Widget};

pub struct Button<S: 'static> {
    uid: Uid,
    id: Option<String>,
    label: String,
    on_click: Option<Callback<S>>,
    enabled: bool,
    is_default: bool,
    hovered: bool,
    pressed: bool,
    rect: Rect,
}

impl<S: 'static> Button<S> {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            uid: Uid::next(),
            id: None,
            label: label.into(),
            on_click: None,
            enabled: true,
            is_default: false,
            hovered: false,
            pressed: false,
            rect: Rect::default(),
        }
    }

    pub fn id(mut self, id: &str) -> Self {
        self.id = Some(id.to_string());
        self
    }

    /// Called with `(state, ui)` when the button is clicked or activated with the keyboard.
    pub fn on_click(mut self, f: impl FnMut(&mut S, &mut crate::ui::Ui<S>) + 'static) -> Self {
        self.on_click = Some(Box::new(f));
        self
    }

    /// Style as the window's default (accent-filled) button.
    pub fn primary(mut self) -> Self {
        self.is_default = true;
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    fn fire(&mut self, cx: &mut EventCx<S>) {
        if !self.enabled {
            return;
        }
        if let Some(cb) = &mut self.on_click {
            cb(cx.state, cx.ui);
        }
        cx.request_redraw();
    }
}

impl<S: 'static> Widget<S> for Button<S> {
    fn uid(&self) -> Uid {
        self.uid
    }
    fn id(&self) -> Option<&str> {
        self.id.as_deref()
    }
    fn focusable(&self) -> bool {
        self.enabled
    }
    fn measure(&mut self, cx: &mut LayoutCx, _avail: Size) -> Size {
        let (w, _) = cx.text.measure(&self.label, cx.theme.font_size);
        let pad = match cx.theme.platform {
            Platform::MacOs => 14.0,
            _ => 16.0,
        };
        Size::new((w + 2.0 * pad).max(cx.theme.control_height * 2.8), cx.theme.control_height)
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
        let focused = cx.focused == Some(self.uid);
        let r = self.rect;
        let radius = t.corner_radius;

        let (bg, fg) = if !self.enabled {
            (pal.control_bg.with_alpha(pal.control_bg.a * 0.5), pal.text_disabled)
        } else if self.is_default {
            let a = if self.pressed { 0.8 } else if self.hovered && t.platform != Platform::MacOs { 0.9 } else { 1.0 };
            (pal.accent.with_alpha(a), pal.accent_text)
        } else if self.pressed {
            (pal.control_bg_pressed, pal.text)
        } else if self.hovered {
            (pal.control_bg_hover, pal.text)
        } else {
            (pal.control_bg, pal.text)
        };

        match t.platform {
            Platform::MacOs => {
                // Subtle drop shadow + border, like AppKit push buttons.
                if self.enabled && !self.is_default {
                    p.fill_round_rect(r.translate(0.0, 0.5), radius, pal.shadow.with_alpha(0.12));
                }
                p.fill_round_rect(r, radius, bg);
                if !self.is_default {
                    p.stroke_round_rect(r, radius, 1.0, pal.control_border);
                }
            }
            Platform::Windows => {
                p.fill_round_rect(r, radius, bg);
                p.stroke_round_rect(r, radius, 1.0, pal.control_border);
                // Fluent: darker bottom edge
                p.line(r.x + radius, r.bottom() - 0.5, r.right() - radius, r.bottom() - 0.5, 1.0, pal.control_border.with_alpha(pal.control_border.a * 2.5));
            }
            Platform::Linux => {
                p.fill_round_rect(r, radius, bg);
            }
        }

        if focused {
            match t.platform {
                Platform::Windows => p.stroke_round_rect(r.inset(-1.0), radius + 1.0, 2.0, pal.focus_ring),
                _ => p.stroke_round_rect(r.inset(-2.0), radius + 2.0, 3.0, pal.focus_ring),
            }
        }

        p.text_in(&self.label, r, Align::Center, fg);
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
                if self.hovered || self.pressed {
                    self.hovered = false;
                    self.pressed = false;
                    cx.request_redraw();
                }
                false
            }
            Event::MouseDown { pos, button: MouseButton::Left } if self.rect.contains(*pos) && self.enabled => {
                self.pressed = true;
                cx.request_focus(self.uid);
                cx.request_redraw();
                true
            }
            Event::MouseUp { pos, button: MouseButton::Left } if self.pressed => {
                self.pressed = false;
                if self.rect.contains(*pos) {
                    self.fire(cx);
                }
                cx.request_redraw();
                true
            }
            Event::KeyDown { key, .. } if cx.is_focused(self.uid) => match key {
                Key::Space | Key::Enter => {
                    self.fire(cx);
                    true
                }
                _ => false,
            },
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
            WidgetCmdKind::SetChecked(_) => {}
        }
        true
    }
}
