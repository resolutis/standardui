use crate::event::{Event, Key, MouseButton};
use crate::geometry::{Point, Rect, Size};
use crate::render::{Align, Painter};
use crate::text::TextSystem;
use crate::theme::{Platform, Theme};
use crate::ui::{Ui, WidgetCmd, WidgetCmdKind};
use crate::widget::{EventCx, LayoutCx, PaintCx, Uid, ValueCallback, Widget};

/// Space above the first and below the last item inside the popup.
const POPUP_PAD: f32 = 5.0;
/// Gap between the field and the popup.
const POPUP_GAP: f32 = 4.0;
/// Closest the popup gets to the window edge.
const POPUP_MARGIN: f32 = 4.0;
/// Width inside an item taken by the check mark column and the side padding.
const ITEM_CHROME: f32 = 38.0;

/// A drop-down list: shows the selected item, and opens a list of all the
/// items to pick one from.
///
/// ```no_run
/// # use standardui::prelude::*;
/// # struct State { size: usize }
/// # let _: ComboBox<State> =
/// ComboBox::new(["Small", "Medium", "Large"])
///     .bind(|s: &mut State| &mut s.size)
///     .on_select(|index, _s: &mut State, _ui| println!("picked {index}"))
/// # ;
/// ```
pub struct ComboBox<S: 'static> {
    uid: Uid,
    id: Option<String>,
    items: Vec<String>,
    selected: Option<usize>,
    placeholder: String,
    bind: Option<Box<dyn Fn(&mut S) -> &mut usize>>,
    on_select: Option<ValueCallback<S, usize>>,
    enabled: bool,
    hovered: bool,
    min_width: f32,
    flex: f32,
    rect: Rect,
    /// Whether the list is showing.
    open: bool,
    /// The item under the mouse or reached with the arrow keys while open.
    highlight: Option<usize>,
    /// Where the open list is drawn.
    popup: Rect,
    /// Vertical scroll offset of the list, in logical px, when it's too long to fit.
    scroll: f32,
    max_scroll: f32,
    /// The window area the popup may cover, as of the last layout.
    bounds: Rect,
    /// Cached (font size, width of the widest item or the placeholder).
    widest: Option<(f32, f32)>,
}

impl<S: 'static> ComboBox<S> {
    /// A combo box with these items. Nothing is selected until you call
    /// `.selected(..)` or `.bind(..)`.
    pub fn new<T: Into<String>>(items: impl IntoIterator<Item = T>) -> Self {
        Self {
            uid: Uid::next(),
            id: None,
            items: items.into_iter().map(Into::into).collect(),
            selected: None,
            placeholder: String::new(),
            bind: None,
            on_select: None,
            enabled: true,
            hovered: false,
            min_width: 80.0,
            flex: 0.0,
            rect: Rect::default(),
            open: false,
            highlight: None,
            popup: Rect::default(),
            scroll: 0.0,
            max_scroll: 0.0,
            bounds: Rect::default(),
            widest: None,
        }
    }

    pub fn id(mut self, id: &str) -> Self {
        self.id = Some(id.to_string());
        self
    }

    /// The item selected to start with, by index.
    pub fn selected(mut self, index: usize) -> Self {
        self.selected = (index < self.items.len()).then_some(index);
        self
    }

    /// Text shown while nothing is selected.
    pub fn placeholder(mut self, text: impl Into<String>) -> Self {
        self.placeholder = text.into();
        self
    }

    pub fn min_width(mut self, w: f32) -> Self {
        self.min_width = w;
        self
    }

    /// Take a share of leftover space in a `Row`/`Column`.
    pub fn flex(mut self, flex: f32) -> Self {
        self.flex = flex;
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Two-way bind to the selected index in the state. An index past the last
    /// item shows as nothing selected.
    pub fn bind(mut self, f: impl Fn(&mut S) -> &mut usize + 'static) -> Self {
        self.bind = Some(Box::new(f));
        self
    }

    /// Called with `(index, state, ui)` when the user picks a different item.
    pub fn on_select(mut self, f: impl FnMut(usize, &mut S, &mut Ui<S>) + 'static) -> Self {
        self.on_select = Some(Box::new(f));
        self
    }

    // --- metrics -----------------------------------------------------------

    fn item_height(theme: &Theme) -> f32 {
        match theme.platform {
            Platform::MacOs => 22.0,
            Platform::Windows => 34.0,
            Platform::Linux => 30.0,
        }
    }

    /// Space between the field's left edge and its text.
    fn text_pad(theme: &Theme) -> f32 {
        match theme.platform {
            Platform::MacOs => 9.0,
            _ => 12.0,
        }
    }

    /// Width of the area on the right that holds the arrow.
    fn arrow_zone(theme: &Theme) -> f32 {
        match theme.platform {
            Platform::MacOs => theme.control_height,
            _ => 32.0,
        }
    }

    /// Width of the widest item (or the placeholder), measured once per font size.
    fn widest_item(&mut self, text: &mut TextSystem, font_size: f32) -> f32 {
        if let Some((size, w)) = self.widest {
            if size == font_size {
                return w;
            }
        }
        let w = self
            .items
            .iter()
            .chain(std::iter::once(&self.placeholder))
            .map(|s| text.measure(s, font_size).0)
            .fold(0.0, f32::max);
        self.widest = Some((font_size, w));
        w
    }

    // --- popup -------------------------------------------------------------

    /// Position the list: below the field if it fits, otherwise on whichever side
    /// has more room, shortened to whole rows (it then scrolls).
    fn place_popup(&mut self, theme: &Theme) {
        let ih = Self::item_height(theme);
        let full = self.items.len() as f32 * ih + 2.0 * POPUP_PAD;
        let b = self.bounds;
        let below = b.bottom() - POPUP_MARGIN - (self.rect.bottom() + POPUP_GAP);
        let above = self.rect.y - POPUP_GAP - (b.y + POPUP_MARGIN);
        let (mut height, up) = if full <= below {
            (full, false)
        } else if above > below {
            (full.min(above), true)
        } else {
            (full.min(below), false)
        };
        if height < full {
            let rows = ((height - 2.0 * POPUP_PAD) / ih).floor().max(1.0);
            height = rows * ih + 2.0 * POPUP_PAD;
        }
        let widest = self.widest.map(|(_, w)| w).unwrap_or(0.0);
        let width = self.rect.width.max(widest + ITEM_CHROME + 2.0 * POPUP_PAD);
        let x = self.rect.x.min(b.right() - POPUP_MARGIN - width).max(b.x + POPUP_MARGIN);
        let y = if up { self.rect.y - POPUP_GAP - height } else { self.rect.bottom() + POPUP_GAP };
        self.popup = Rect::new(x, y, width, height);
        self.max_scroll = (full - height).max(0.0);
        self.scroll = self.scroll.clamp(0.0, self.max_scroll);
    }

    /// Where item `i` is drawn (it may be scrolled out of the popup).
    fn item_rect(&self, i: usize, ih: f32) -> Rect {
        Rect::new(
            self.popup.x + POPUP_PAD,
            self.popup.y + POPUP_PAD + i as f32 * ih - self.scroll,
            self.popup.width - 2.0 * POPUP_PAD,
            ih,
        )
    }

    fn item_at(&self, pos: Point, theme: &Theme) -> Option<usize> {
        let list = Rect::new(self.popup.x, self.popup.y + POPUP_PAD, self.popup.width, self.popup.height - 2.0 * POPUP_PAD);
        if !list.contains(pos) {
            return None;
        }
        let i = ((pos.y - list.y + self.scroll) / Self::item_height(theme)).floor() as usize;
        (i < self.items.len()).then_some(i)
    }

    /// Scroll the list so item `i` is fully visible.
    fn reveal(&mut self, i: usize, theme: &Theme) {
        let ih = Self::item_height(theme);
        let top = i as f32 * ih;
        let view = self.popup.height - 2.0 * POPUP_PAD;
        if top < self.scroll {
            self.scroll = top;
        } else if top + ih > self.scroll + view {
            self.scroll = top + ih - view;
        }
        self.scroll = self.scroll.clamp(0.0, self.max_scroll);
    }

    fn open_popup(&mut self, cx: &mut EventCx<S>) {
        if self.items.is_empty() {
            return;
        }
        self.widest_item(cx.text, cx.theme.font_size);
        self.open = true;
        self.highlight = self.selected;
        self.scroll = 0.0;
        self.place_popup(cx.theme);
        if let Some(i) = self.highlight {
            self.reveal(i, cx.theme);
        }
        cx.request_redraw();
    }

    fn close_popup(&mut self, cx: &mut EventCx<S>) {
        self.open = false;
        self.highlight = None;
        cx.request_redraw();
    }

    /// Move the highlight to item `i` and keep it in view.
    fn highlight_item(&mut self, i: usize, cx: &mut EventCx<S>) {
        self.highlight = Some(i);
        self.reveal(i, cx.theme);
        cx.request_redraw();
    }

    // --- selection ---------------------------------------------------------

    fn select(&mut self, i: usize, cx: &mut EventCx<S>) {
        if i >= self.items.len() || self.selected == Some(i) {
            return;
        }
        self.selected = Some(i);
        if let Some(b) = &self.bind {
            *b(cx.state) = i;
        }
        if let Some(cb) = &mut self.on_select {
            cb(i, cx.state, cx.ui);
        }
        cx.request_redraw();
    }

    /// The next item after `from` (wrapping round) that starts with `typed`,
    /// ignoring case.
    fn find_prefix(&self, typed: &str, from: Option<usize>) -> Option<usize> {
        let typed = typed.to_lowercase();
        if typed.trim().is_empty() {
            return None;
        }
        let n = self.items.len();
        let start = from.map(|i| i + 1).unwrap_or(0);
        (start..n).chain(0..start.min(n)).find(|&i| self.items[i].to_lowercase().starts_with(&typed))
    }

    // --- painting ----------------------------------------------------------

    fn paint_arrow(&self, p: &mut Painter) {
        let t = p.theme;
        let pal = &t.palette;
        let r = self.rect;
        let zone = Self::arrow_zone(t);
        let cy = r.y + r.height / 2.0;
        match t.platform {
            Platform::MacOs => {
                // Accent-filled cap with up and down chevrons, like an AppKit pop-up button.
                let side = r.height - 6.0;
                let cap = Rect::new(r.right() - 3.0 - side, r.y + 3.0, side, side);
                let (bg, fg) = if self.enabled { (pal.accent, pal.accent_text) } else { (pal.control_bg_pressed, pal.text_disabled) };
                p.fill_round_rect(cap, 4.5, bg);
                let cx = cap.center().x;
                p.polyline(&[(cx - 2.8, cy - 1.8), (cx, cy - 4.4), (cx + 2.8, cy - 1.8)], 1.5, fg);
                p.polyline(&[(cx - 2.8, cy + 1.8), (cx, cy + 4.4), (cx + 2.8, cy + 1.8)], 1.5, fg);
            }
            _ => {
                let cx = r.right() - zone / 2.0;
                let fg = if self.enabled { pal.text_secondary } else { pal.text_disabled };
                p.polyline(&[(cx - 4.0, cy - 2.0), (cx, cy + 2.0), (cx + 4.0, cy - 2.0)], 1.5, fg);
            }
        }
    }
}

impl<S: 'static> Widget<S> for ComboBox<S> {
    fn uid(&self) -> Uid {
        self.uid
    }
    fn id(&self) -> Option<&str> {
        self.id.as_deref()
    }
    fn focusable(&self) -> bool {
        self.enabled
    }
    fn flex(&self) -> f32 {
        self.flex
    }
    fn sync(&mut self, state: &mut S) {
        if let Some(b) = &self.bind {
            let v = *b(state);
            self.selected = (v < self.items.len()).then_some(v);
        }
    }
    fn measure(&mut self, cx: &mut LayoutCx, _avail: Size) -> Size {
        // Wide enough for the longest item, so the field doesn't change size as
        // the selection changes.
        let widest = self.widest_item(cx.text, cx.theme.font_size);
        let w = Self::text_pad(cx.theme) + widest + 6.0 + Self::arrow_zone(cx.theme);
        Size::new(w.max(self.min_width), cx.theme.control_height)
    }
    fn layout(&mut self, cx: &mut LayoutCx, rect: Rect) {
        self.rect = rect;
        self.bounds = cx.window;
        if self.open {
            self.place_popup(cx.theme);
        }
    }
    fn rect(&self) -> Rect {
        self.rect
    }
    fn paint(&mut self, p: &mut Painter, cx: &PaintCx) {
        let t = p.theme;
        let pal = &t.palette;
        let r = self.rect;
        let radius = t.corner_radius;
        let focused = cx.focused == Some(self.uid);

        let bg = if !self.enabled {
            pal.control_bg.with_alpha(pal.control_bg.a * 0.5)
        } else if self.open && t.platform != Platform::MacOs {
            pal.control_bg_pressed
        } else if self.hovered {
            pal.control_bg_hover
        } else {
            pal.control_bg
        };
        match t.platform {
            Platform::MacOs => {
                if self.enabled {
                    p.fill_round_rect(r.translate(0.0, 0.5), radius, pal.shadow.with_alpha(0.12));
                }
                p.fill_round_rect(r, radius, bg);
                p.stroke_round_rect(r, radius, 1.0, pal.control_border);
            }
            Platform::Windows => {
                p.fill_round_rect(r, radius, bg);
                p.stroke_round_rect(r, radius, 1.0, pal.control_border);
                p.line(r.x + radius, r.bottom() - 0.5, r.right() - radius, r.bottom() - 0.5, 1.0, pal.control_border.with_alpha(pal.control_border.a * 2.5));
            }
            Platform::Linux => p.fill_round_rect(r, radius, bg),
        }
        if focused {
            match t.platform {
                Platform::Windows => p.stroke_round_rect(r.inset(-1.0), radius + 1.0, 2.0, pal.focus_ring),
                _ => p.stroke_round_rect(r.inset(-2.0), radius + 2.0, 3.0, pal.focus_ring),
            }
        }

        let pad = Self::text_pad(t);
        let inner = Rect::new(r.x + pad, r.y, (r.width - pad - Self::arrow_zone(t)).max(0.0), r.height);
        let (label, fg) = match self.selected.and_then(|i| self.items.get(i)) {
            Some(item) if self.enabled => (item.as_str(), pal.text),
            Some(item) => (item.as_str(), pal.text_disabled),
            None => (self.placeholder.as_str(), pal.text_disabled),
        };
        p.clipped(inner, |p| p.text_in(label, inner, Align::Start, fg));
        self.paint_arrow(p);
    }
    fn paint_overlay(&mut self, p: &mut Painter, _cx: &PaintCx) {
        if !self.open {
            return;
        }
        let t = p.theme;
        let pal = &t.palette;
        let radius = match t.platform {
            Platform::MacOs => 6.0,
            Platform::Windows => 8.0,
            Platform::Linux => 10.0,
        };
        p.shadow(self.popup, radius, pal.shadow);
        p.fill_round_rect(self.popup, radius, pal.menu_bg);
        p.stroke_round_rect(self.popup, radius, 1.0, pal.menu_border);

        let ih = Self::item_height(t);
        let list = Rect::new(self.popup.x, self.popup.y + POPUP_PAD, self.popup.width, self.popup.height - 2.0 * POPUP_PAD);
        p.clipped(list, |p| {
            for (i, item) in self.items.iter().enumerate() {
                let r = self.item_rect(i, ih);
                if r.bottom() <= list.y || r.y >= list.bottom() {
                    continue;
                }
                let mut fg = pal.text;
                if self.highlight == Some(i) {
                    if t.platform == Platform::MacOs {
                        p.fill_round_rect(r, 4.0, pal.accent);
                        fg = pal.accent_text;
                    } else {
                        // The theme's hover colour is too faint over the menu background on Windows.
                        p.fill_round_rect(r, 6.0, pal.text.with_alpha(0.1));
                    }
                }
                if self.selected == Some(i) {
                    p.check_mark(Rect::new(r.x + 7.0, r.y + (ih - 12.0) / 2.0, 12.0, 12.0), 1.6, fg);
                }
                let text = Rect::new(r.x + 26.0, r.y, (r.width - ITEM_CHROME).max(0.0), ih);
                p.text_in(item, text, Align::Start, fg);
            }
        });

        // A thin thumb on the right when the list is longer than the popup.
        if self.max_scroll > 0.0 {
            let content = self.items.len() as f32 * ih;
            let thumb = (list.height * list.height / content).max(16.0);
            let y = list.y + (list.height - thumb) * (self.scroll / self.max_scroll);
            p.fill_round_rect(Rect::new(self.popup.right() - 5.0, y, 3.0, thumb), 1.5, pal.text.with_alpha(0.3));
        }
    }
    fn popup_event(&mut self, ev: &Event, cx: &mut EventCx<S>) -> bool {
        if !self.open {
            return false;
        }
        match ev {
            Event::MouseMove(pos) => {
                // Moving off the list keeps the last highlight, so the keyboard can carry on from it.
                if let Some(i) = self.item_at(*pos, cx.theme) {
                    if self.highlight != Some(i) {
                        self.highlight = Some(i);
                        cx.request_redraw();
                    }
                }
                true
            }
            Event::MouseLeave => false,
            Event::MouseDown { pos, .. } => {
                if self.popup.contains(*pos) {
                    true
                } else if self.rect.contains(*pos) {
                    // Clicking the field again closes the list.
                    self.close_popup(cx);
                    true
                } else {
                    // Clicking away closes the list; the click still reaches what's under it.
                    self.close_popup(cx);
                    false
                }
            }
            Event::MouseUp { pos, button } => {
                // Covers both click-then-click and press-drag-release.
                if *button == MouseButton::Left {
                    if let Some(i) = self.item_at(*pos, cx.theme) {
                        self.close_popup(cx);
                        self.select(i, cx);
                    }
                }
                true
            }
            Event::Scroll { delta, .. } => {
                if self.max_scroll > 0.0 {
                    self.scroll = (self.scroll - delta.y).clamp(0.0, self.max_scroll);
                    cx.request_redraw();
                }
                true
            }
            Event::KeyDown { key, mods, text } => {
                let last = self.items.len().saturating_sub(1);
                match key {
                    Key::Escape => self.close_popup(cx),
                    Key::Tab => {
                        // Close, and let focus move on as usual.
                        self.close_popup(cx);
                        return false;
                    }
                    Key::Down => {
                        let i = self.highlight.map(|i| (i + 1).min(last)).unwrap_or(0);
                        self.highlight_item(i, cx);
                    }
                    Key::Up => {
                        let i = self.highlight.map(|i| i.saturating_sub(1)).unwrap_or(last);
                        self.highlight_item(i, cx);
                    }
                    Key::Home => self.highlight_item(0, cx),
                    Key::End => self.highlight_item(last, cx),
                    Key::Enter | Key::Space => {
                        let pick = self.highlight;
                        self.close_popup(cx);
                        if let Some(i) = pick {
                            self.select(i, cx);
                        }
                    }
                    _ => {
                        if !(mods.command() || mods.alt || mods.ctrl) {
                            if let Some(i) = text.as_deref().and_then(|s| self.find_prefix(s, self.highlight)) {
                                self.highlight_item(i, cx);
                            }
                        }
                    }
                }
                true
            }
            Event::KeyUp { .. } => true,
        }
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
                if self.hovered {
                    self.hovered = false;
                    cx.request_redraw();
                }
                false
            }
            Event::MouseDown { pos, button: MouseButton::Left } if self.rect.contains(*pos) && self.enabled => {
                cx.request_focus(self.uid);
                self.open_popup(cx);
                true
            }
            Event::KeyDown { key, mods, text } if cx.is_focused(self.uid) && self.enabled => match key {
                Key::Space | Key::Enter | Key::Up | Key::Down => {
                    self.open_popup(cx);
                    true
                }
                Key::Tab | Key::Escape => false,
                _ => {
                    if mods.command() || mods.alt || mods.ctrl {
                        return false;
                    }
                    // Typing a letter picks the next item that starts with it.
                    match text.as_deref().and_then(|s| self.find_prefix(s, self.selected)) {
                        Some(i) => {
                            self.select(i, cx);
                            true
                        }
                        None => false,
                    }
                }
            },
            _ => false,
        }
    }
    fn apply(&mut self, cmd: &WidgetCmd) -> bool {
        if self.id.as_deref() != Some(cmd.target.as_str()) {
            return false;
        }
        match &cmd.kind {
            WidgetCmdKind::SetEnabled(e) => {
                self.enabled = *e;
                if !self.enabled {
                    self.open = false;
                }
            }
            WidgetCmdKind::SetSelected(i) => self.selected = i.filter(|i| *i < self.items.len()),
            WidgetCmdKind::SetItems(items) => {
                self.items = items.clone();
                self.selected = self.selected.filter(|i| *i < self.items.len());
                self.widest = None;
                self.open = false;
                self.highlight = None;
            }
            _ => {}
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::Modifiers;

    struct Harness {
        combo: ComboBox<usize>,
        state: usize,
        ui: Ui<usize>,
        theme: Theme,
        text: TextSystem,
    }

    impl Harness {
        /// A focused combo box bound to the state, at `y` in a 300 × `window_h` window.
        fn new(items: &[&str], y: f32, window_h: f32) -> Self {
            let theme = Theme::for_platform(Platform::Linux, false);
            let mut text = TextSystem::new(&theme.font_families);
            let mut combo = ComboBox::new(items.iter().copied()).bind(|s: &mut usize| s);
            let mut state = 0;
            combo.sync(&mut state);
            let mut lcx = LayoutCx { text: &mut text, theme: &theme, window: Rect::new(0.0, 0.0, 300.0, window_h) };
            let size = combo.measure(&mut lcx, Size::new(300.0, window_h));
            combo.layout(&mut lcx, Rect::new(10.0, y, size.width, size.height));
            let mut ui = Ui::new(None);
            ui.focus = Some(combo.uid);
            Harness { combo, state, ui, theme, text }
        }

        /// Route an event the way the app does: the open popup first, then the widget.
        fn send(&mut self, ev: Event) -> bool {
            let mut cx = EventCx { state: &mut self.state, ui: &mut self.ui, theme: &self.theme, text: &mut self.text, redraw: false };
            self.combo.popup_event(&ev, &mut cx) || self.combo.event(&ev, &mut cx)
        }

        fn key(&mut self, key: Key) -> bool {
            self.send(Event::KeyDown { key, mods: Modifiers::default(), text: None })
        }

        fn typed(&mut self, c: char) -> bool {
            self.send(Event::KeyDown { key: Key::Character(c.to_string()), mods: Modifiers::default(), text: Some(c.to_string()) })
        }

        fn click(&mut self, pos: Point) -> bool {
            let down = self.send(Event::MouseDown { pos, button: MouseButton::Left });
            let up = self.send(Event::MouseUp { pos, button: MouseButton::Left });
            down || up
        }

        fn item_center(&self, i: usize) -> Point {
            self.combo.item_rect(i, ComboBox::<usize>::item_height(&self.theme)).center()
        }
    }

    const SIZES: [&str; 3] = ["Small", "Medium", "Large"];

    #[test]
    fn clicking_an_item_selects_it_and_updates_the_bound_state() {
        let mut h = Harness::new(&SIZES, 10.0, 400.0);
        h.click(h.combo.rect.center());
        assert!(h.combo.open, "clicking the field opens the list");
        h.click(h.item_center(2));
        assert!(!h.combo.open);
        assert_eq!(h.state, 2);
        assert_eq!(h.combo.selected, Some(2));
    }

    #[test]
    fn press_drag_release_selects() {
        let mut h = Harness::new(&SIZES, 10.0, 400.0);
        h.send(Event::MouseDown { pos: h.combo.rect.center(), button: MouseButton::Left });
        let over = h.item_center(1);
        h.send(Event::MouseMove(over));
        assert_eq!(h.combo.highlight, Some(1));
        h.send(Event::MouseUp { pos: over, button: MouseButton::Left });
        assert_eq!(h.state, 1);
    }

    #[test]
    fn keyboard_opens_moves_and_selects() {
        let mut h = Harness::new(&SIZES, 10.0, 400.0);
        h.key(Key::Down);
        assert!(h.combo.open);
        assert_eq!(h.combo.highlight, Some(0), "opens on the selected item");
        h.key(Key::Down);
        h.key(Key::Down);
        h.key(Key::Down);
        assert_eq!(h.combo.highlight, Some(2), "stops at the last item");
        assert_eq!(h.state, 0, "moving the highlight doesn't select");
        h.key(Key::Enter);
        assert!(!h.combo.open);
        assert_eq!(h.state, 2);
    }

    #[test]
    fn escape_and_clicking_away_close_without_selecting() {
        let mut h = Harness::new(&SIZES, 10.0, 400.0);
        h.key(Key::Space);
        h.key(Key::Down);
        assert!(h.key(Key::Escape));
        assert!(!h.combo.open);
        assert_eq!(h.state, 0);

        h.key(Key::Space);
        let away = Point::new(290.0, 390.0);
        assert!(!h.send(Event::MouseDown { pos: away, button: MouseButton::Left }), "the click passes through");
        assert!(!h.combo.open);
        assert_eq!(h.state, 0);
    }

    #[test]
    fn tab_closes_the_list_and_is_not_consumed() {
        let mut h = Harness::new(&SIZES, 10.0, 400.0);
        h.key(Key::Space);
        assert!(!h.key(Key::Tab));
        assert!(!h.combo.open);
    }

    #[test]
    fn typing_a_letter_jumps_to_the_next_match() {
        let mut h = Harness::new(&["Apple", "Banana", "Blueberry", "Cherry"], 10.0, 400.0);
        assert!(h.typed('b'));
        assert_eq!(h.state, 1);
        h.typed('B');
        assert_eq!(h.state, 2);
        h.typed('b');
        assert_eq!(h.state, 1, "wraps round");
        assert!(!h.typed('z'), "no match is not consumed");
        assert_eq!(h.state, 1);
    }

    #[test]
    fn on_select_fires_only_when_the_selection_changes() {
        let mut h = Harness::new(&SIZES, 10.0, 400.0);
        h.combo = ComboBox::new(SIZES).selected(0).on_select(|i, s: &mut usize, _ui| *s += 10 + i);
        h.ui.focus = Some(h.combo.uid);
        h.key(Key::Space);
        h.key(Key::Enter);
        assert_eq!(h.state, 0, "re-picking the selected item is not a change");
        h.key(Key::Space);
        h.key(Key::Down);
        h.key(Key::Enter);
        assert_eq!(h.state, 11);
    }

    #[test]
    fn list_opens_upwards_and_scrolls_when_there_is_no_room_below() {
        let items: Vec<String> = (0..40).map(|i| format!("Item {i}")).collect();
        let items: Vec<&str> = items.iter().map(String::as_str).collect();
        let mut h = Harness::new(&items, 250.0, 300.0);
        h.state = 39;
        h.combo.sync(&mut h.state);
        h.key(Key::Space);
        let popup = h.combo.popup;
        assert!(popup.bottom() <= h.combo.rect.y, "opens above the field");
        assert!(popup.y >= 0.0, "stays inside the window");
        assert!(h.combo.max_scroll > 0.0);
        assert_eq!(h.combo.scroll, h.combo.max_scroll, "the selected item is scrolled into view");
        h.key(Key::Home);
        assert_eq!(h.combo.scroll, 0.0);
        h.send(Event::Scroll { pos: popup.center(), delta: Point::new(0.0, -60.0) });
        assert_eq!(h.combo.scroll, 60.0);
    }

    #[test]
    fn set_items_keeps_a_valid_selection_and_clears_an_invalid_one() {
        let mut combo: ComboBox<()> = ComboBox::new(SIZES).id("size").selected(2);
        let cmd = |kind| WidgetCmd { target: "size".to_string(), kind };
        combo.apply(&cmd(WidgetCmdKind::SetItems(vec!["A".into(), "B".into(), "C".into(), "D".into()])));
        assert_eq!(combo.selected, Some(2));
        combo.apply(&cmd(WidgetCmdKind::SetItems(vec!["A".into()])));
        assert_eq!(combo.selected, None);
        combo.apply(&cmd(WidgetCmdKind::SetSelected(Some(0))));
        assert_eq!(combo.selected, Some(0));
        combo.apply(&cmd(WidgetCmdKind::SetSelected(Some(9))));
        assert_eq!(combo.selected, None);
    }
}
