use cosmic_text::Buffer;

use crate::event::{Event, Key, MouseButton};
use crate::geometry::{Point, Rect, Size};
use crate::render::Painter;
use crate::text::{measure_buffer, TextSystem};
use crate::theme::{Platform, Theme};
use crate::ui::{Ui, WidgetCmd, WidgetCmdKind};
use crate::widget::{EventCx, LayoutCx, PaintCx, Uid, ValueCallback, Widget};

use super::text_field::paint_field_frame;

/// Multi-line text input. Text wraps at the widget's width; when it's taller than
/// the widget, it scrolls (mouse wheel, or by moving the caret).
pub struct TextArea<S: 'static> {
    uid: Uid,
    id: Option<String>,
    text: String,
    placeholder: String,
    /// Caret position in characters.
    caret: usize,
    /// Horizontal position to aim for when moving up/down through lines of
    /// different lengths; cleared by any other caret movement.
    goal_x: Option<f32>,
    bind: Option<Box<dyn Fn(&mut S) -> &mut String>>,
    on_change: Option<ValueCallback<S, String>>,
    enabled: bool,
    rows: usize,
    min_width: f32,
    flex: f32,
    rect: Rect,
    /// Vertical scroll offset of the content, in logical px.
    scroll: f32,
    /// Largest valid `scroll`, as of the last paint.
    max_scroll: f32,
    /// Scroll the caret into view on the next paint.
    reveal: bool,
}

/// Where the caret sits in the laid-out text.
struct CaretLine {
    /// Paragraph (text between newlines) the caret is in.
    para: usize,
    /// Byte range of the caret's visual line within that paragraph.
    start: usize,
    end: usize,
    x: f32,
    top: f32,
    height: f32,
}

impl<S: 'static> TextArea<S> {
    pub fn new() -> Self {
        Self {
            uid: Uid::next(),
            id: None,
            text: String::new(),
            placeholder: String::new(),
            caret: 0,
            goal_x: None,
            bind: None,
            on_change: None,
            enabled: true,
            rows: 4,
            min_width: 200.0,
            flex: 0.0,
            rect: Rect::default(),
            scroll: 0.0,
            max_scroll: 0.0,
            reveal: false,
        }
    }

    pub fn id(mut self, id: &str) -> Self {
        self.id = Some(id.to_string());
        self
    }

    pub fn text(mut self, text: impl Into<String>) -> Self {
        self.text = normalize_newlines(&text.into());
        self.caret = self.char_len();
        self
    }

    pub fn placeholder(mut self, text: impl Into<String>) -> Self {
        self.placeholder = text.into();
        self
    }

    /// Height in lines of text (default 4). Use `.flex()` in a `Column` to fill
    /// the available height instead.
    pub fn rows(mut self, rows: usize) -> Self {
        self.rows = rows.max(1);
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

    // --- text and caret helpers -------------------------------------------

    fn char_len(&self) -> usize {
        self.text.chars().count()
    }

    fn byte_index(&self, char_idx: usize) -> usize {
        self.text.char_indices().nth(char_idx).map(|(i, _)| i).unwrap_or(self.text.len())
    }

    /// Caret as (paragraph, byte offset within that paragraph).
    fn para_pos(&self) -> (usize, usize) {
        let bi = self.byte_index(self.caret);
        let before = &self.text[..bi];
        let para = before.matches('\n').count();
        let start = before.rfind('\n').map(|i| i + 1).unwrap_or(0);
        (para, bi - start)
    }

    /// Character index of (paragraph, byte offset within that paragraph).
    fn char_at(&self, para: usize, byte: usize) -> usize {
        let mut start = 0;
        for _ in 0..para {
            match self.text[start..].find('\n') {
                Some(i) => start += i + 1,
                None => return self.char_len(),
            }
        }
        let end = self.text[start..].find('\n').map(|i| start + i).unwrap_or(self.text.len());
        let mut g = (start + byte).min(end);
        while !self.text.is_char_boundary(g) {
            g -= 1;
        }
        self.text[..g].chars().count()
    }

    fn padding(theme: &Theme) -> (f32, f32) {
        match theme.platform {
            Platform::MacOs => (6.0, 4.0),
            _ => (10.0, 7.0),
        }
    }

    fn line_height(theme: &Theme) -> f32 {
        (theme.font_size * 1.3).round()
    }

    /// The text area inside the frame and padding.
    fn inner(&self, theme: &Theme) -> Rect {
        let (px, py) = Self::padding(theme);
        Rect::new(self.rect.x + px, self.rect.y + py, (self.rect.width - 2.0 * px).max(1.0), (self.rect.height - 2.0 * py).max(0.0))
    }

    fn layout_text(&self, text: &mut TextSystem, theme: &Theme) -> Buffer {
        text.layout(&self.text, theme.font_size, Some(self.inner(theme).width))
    }

    /// Locate the caret in laid-out text (positions are relative to the text's top-left).
    fn caret_line(&self, buf: &Buffer, line_height: f32) -> CaretLine {
        let (para, byte) = self.para_pos();
        let mut prev_bottom: f32 = 0.0;
        let runs: Vec<_> = buf.layout_runs().filter(|r| r.line_i == para).collect();
        for r in buf.layout_runs().filter(|r| r.line_i < para) {
            prev_bottom = prev_bottom.max(r.line_top + r.line_height);
        }
        let n = runs.len();
        for (k, run) in runs.into_iter().enumerate() {
            let start = run.glyphs.first().map(|g| g.start).unwrap_or(0);
            let end = run.glyphs.last().map(|g| g.end).unwrap_or(0);
            // A caret on a wrap boundary belongs at the start of the next line.
            if byte < end || k + 1 == n {
                let x = match run.glyphs.iter().find(|g| g.end > byte) {
                    Some(g) => g.x,
                    None => run.glyphs.last().map(|g| g.x + g.w).unwrap_or(0.0),
                };
                let end = if k + 1 == n { usize::MAX } else { end };
                return CaretLine { para, start, end, x, top: run.line_top, height: run.line_height };
            }
        }
        // Paragraph not laid out (e.g. an empty last line): start of a new line.
        CaretLine { para, start: 0, end: usize::MAX, x: 0.0, top: prev_bottom, height: line_height }
    }

    /// Character index nearest to `p`, given relative to the text's top-left.
    fn hit(&self, buf: &Buffer, p: Point) -> usize {
        if p.y < 0.0 {
            return 0;
        }
        match buf.hit(p.x.max(0.0), p.y) {
            Some(c) => self.char_at(c.line, c.index),
            None => self.char_len(),
        }
    }

    fn content_height(&self, buf: &Buffer, line_height: f32) -> f32 {
        let (_, h) = measure_buffer(buf);
        let c = self.caret_line(buf, line_height);
        h.max(c.top + c.height).max(line_height)
    }

    // --- editing -----------------------------------------------------------

    fn changed(&mut self, cx: &mut EventCx<S>) {
        if let Some(b) = &self.bind {
            *b(cx.state) = self.text.clone();
        }
        if let Some(cb) = &mut self.on_change {
            cb(self.text.clone(), cx.state, cx.ui);
        }
    }

    fn insert(&mut self, s: &str, cx: &mut EventCx<S>) {
        let at = self.byte_index(self.caret);
        self.text.insert_str(at, s);
        self.caret += s.chars().count();
        self.changed(cx);
    }

    /// Move up (`-1`) or down (`+1`) one visual line, keeping the horizontal position.
    fn move_vertical(&mut self, dir: f32, cx: &mut EventCx<S>) {
        let buf = self.layout_text(cx.text, cx.theme);
        let lh = Self::line_height(cx.theme);
        let line = self.caret_line(&buf, lh);
        let x = self.goal_x.unwrap_or(line.x);
        let y = if dir < 0.0 { line.top - line.height / 2.0 } else { line.top + line.height * 1.5 };
        self.caret = if y < 0.0 {
            0
        } else if y >= self.content_height(&buf, lh) {
            self.char_len()
        } else {
            self.hit(&buf, Point::new(x, y))
        };
        self.goal_x = Some(x);
    }

    /// Move to the start or end of the caret's visual line.
    fn move_line_edge(&mut self, to_end: bool, cx: &mut EventCx<S>) {
        let buf = self.layout_text(cx.text, cx.theme);
        let line = self.caret_line(&buf, Self::line_height(cx.theme));
        self.caret = self.char_at(line.para, if to_end { line.end } else { line.start });
    }
}

impl<S: 'static> Default for TextArea<S> {
    fn default() -> Self {
        Self::new()
    }
}

fn normalize_newlines(s: &str) -> String {
    s.replace("\r\n", "\n").replace('\r', "\n")
}

impl<S: 'static> Widget<S> for TextArea<S> {
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
                self.text = normalize_newlines(v);
                self.caret = self.caret.min(self.char_len());
            }
        }
    }
    fn measure(&mut self, cx: &mut LayoutCx, _avail: Size) -> Size {
        let (_, py) = Self::padding(cx.theme);
        Size::new(self.min_width, self.rows as f32 * Self::line_height(cx.theme) + 2.0 * py)
    }
    fn flex(&self) -> f32 {
        self.flex
    }
    fn layout(&mut self, _cx: &mut LayoutCx, rect: Rect) {
        if rect.size() != self.rect.size() {
            self.reveal = true;
        }
        self.rect = rect;
    }
    fn rect(&self) -> Rect {
        self.rect
    }
    fn paint(&mut self, p: &mut Painter, cx: &PaintCx) {
        let t = p.theme;
        let pal = &t.palette;
        let focused = cx.focused == Some(self.uid);
        paint_field_frame(p, self.rect, focused);

        let inner = self.inner(t);
        let lh = Self::line_height(t);
        let buf = self.layout_text(p.text, t);
        let caret = self.caret_line(&buf, lh);
        let content_h = self.content_height(&buf, lh);

        // Keep the scroll offset valid, and the caret in view after it moves.
        self.max_scroll = (content_h - inner.height).max(0.0);
        if self.reveal && focused {
            if caret.top < self.scroll {
                self.scroll = caret.top;
            } else if caret.top + caret.height > self.scroll + inner.height {
                self.scroll = caret.top + caret.height - inner.height;
            }
        }
        self.reveal = false;
        self.scroll = self.scroll.clamp(0.0, self.max_scroll).round();

        let fg = if self.enabled { pal.text } else { pal.text_disabled };
        let top = inner.y - self.scroll;
        let scroll = self.scroll;
        let max_scroll = self.max_scroll;
        let (text, placeholder) = (&self.text, &self.placeholder);
        p.clipped(inner, |p| {
            if text.is_empty() {
                p.text_wrapped(placeholder, inner, pal.text_disabled);
            } else {
                p.text_wrapped(text, Rect::new(inner.x, top, inner.width, content_h), fg);
            }
            if focused {
                p.fill_rect(Rect::new((inner.x + caret.x).round(), top + caret.top, 1.0, caret.height), pal.text);
            }
        });

        // Thin scroll indicator along the right edge when the text overflows.
        if max_scroll > 0.0 {
            let track = Rect::new(self.rect.right() - 6.0, inner.y, 3.0, inner.height);
            let thumb_h = (inner.height / content_h * track.height).max(16.0).min(track.height);
            let thumb_y = track.y + scroll / max_scroll * (track.height - thumb_h);
            p.fill_round_rect(Rect::new(track.x, thumb_y, track.width, thumb_h), 1.5, pal.text.with_alpha(0.3));
        }
    }
    fn event(&mut self, ev: &Event, cx: &mut EventCx<S>) -> bool {
        match ev {
            Event::Scroll { pos, delta } if self.rect.contains(*pos) && self.max_scroll > 0.0 => {
                self.scroll = (self.scroll - delta.y).clamp(0.0, self.max_scroll);
                cx.request_redraw();
                true
            }
            Event::MouseDown { pos, button: MouseButton::Left } if self.rect.contains(*pos) && self.enabled => {
                cx.request_focus(self.uid);
                let inner = self.inner(cx.theme);
                let buf = self.layout_text(cx.text, cx.theme);
                self.caret = self.hit(&buf, Point::new(pos.x - inner.x, pos.y - inner.y + self.scroll));
                self.goal_x = None;
                self.reveal = true;
                cx.request_redraw();
                true
            }
            Event::KeyDown { key, mods, text } if cx.is_focused(self.uid) && self.enabled => {
                let mac = cfg!(target_os = "macos");
                let vertical = matches!(key, Key::Up | Key::Down);
                match key {
                    // ⌘↑/⌘↓ on macOS, Ctrl+Home/End elsewhere: start/end of the text.
                    Key::Up if mac && mods.meta => self.caret = 0,
                    Key::Down if mac && mods.meta => self.caret = self.char_len(),
                    Key::Home if mods.ctrl => self.caret = 0,
                    Key::End if mods.ctrl => self.caret = self.char_len(),
                    Key::Up => self.move_vertical(-1.0, cx),
                    Key::Down => self.move_vertical(1.0, cx),
                    // ⌘←/⌘→ on macOS, Home/End elsewhere: start/end of the line.
                    Key::Left if mac && mods.meta => self.move_line_edge(false, cx),
                    Key::Right if mac && mods.meta => self.move_line_edge(true, cx),
                    Key::Home => self.move_line_edge(false, cx),
                    Key::End => self.move_line_edge(true, cx),
                    Key::Left => self.caret = self.caret.saturating_sub(1),
                    Key::Right => self.caret = (self.caret + 1).min(self.char_len()),
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
                    Key::Enter => self.insert("\n", cx),
                    Key::Tab | Key::Escape => return false,
                    _ => {
                        // Leave shortcuts (⌘S, Ctrl+Q, ...) for the menu bar.
                        if mods.command() || mods.ctrl || (mods.alt && text.is_none()) {
                            return false;
                        }
                        let Some(s) = text else { return false };
                        let s: String = normalize_newlines(s).chars().filter(|c| *c == '\n' || !c.is_control()).collect();
                        if s.is_empty() {
                            return false;
                        }
                        self.insert(&s, cx);
                    }
                }
                if !vertical {
                    self.goal_x = None;
                }
                self.reveal = true;
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
                self.text = normalize_newlines(t);
                self.caret = self.caret.min(self.char_len());
            }
            WidgetCmdKind::SetEnabled(e) => self.enabled = *e,
            WidgetCmdKind::SetChecked(_) => {}
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::Modifiers;
    use crate::theme::Platform;

    struct Harness {
        area: TextArea<String>,
        state: String,
        ui: Ui<String>,
        theme: Theme,
        text: TextSystem,
    }

    impl Harness {
        /// A focused text area bound to the state, laid out `width` × `rows` lines.
        fn new(initial: &str, width: f32, rows: usize) -> Self {
            let theme = Theme::for_platform(Platform::Linux, false);
            let mut text = TextSystem::new(&theme.font_families);
            let mut area = TextArea::new().rows(rows).bind(|s: &mut String| s);
            let mut state = initial.to_string();
            area.sync(&mut state);
            let mut lcx = LayoutCx { text: &mut text, theme: &theme };
            let size = area.measure(&mut lcx, Size::new(width, 1000.0));
            area.layout(&mut lcx, Rect::new(0.0, 0.0, width, size.height));
            let mut ui = Ui::new(None);
            ui.focus = Some(area.uid);
            Harness { area, state, ui, theme, text }
        }

        fn send(&mut self, ev: Event) -> bool {
            let mut cx = EventCx { state: &mut self.state, ui: &mut self.ui, theme: &self.theme, text: &mut self.text, redraw: false };
            self.area.event(&ev, &mut cx)
        }

        fn key(&mut self, key: Key) {
            self.send(Event::KeyDown { key, mods: Modifiers::default(), text: None });
        }

        fn typed(&mut self, s: &str) {
            for c in s.chars() {
                self.send(Event::KeyDown { key: Key::Character(c.to_string()), mods: Modifiers::default(), text: Some(c.to_string()) });
            }
        }

        fn caret_line(&mut self) -> CaretLine {
            let buf = self.area.layout_text(&mut self.text, &self.theme);
            self.area.caret_line(&buf, TextArea::<String>::line_height(&self.theme))
        }

        fn visual_lines(&mut self) -> usize {
            let buf = self.area.layout_text(&mut self.text, &self.theme);
            buf.layout_runs().count()
        }
    }

    #[test]
    fn para_and_char_positions_round_trip() {
        let mut h = Harness::new("ab\ncdé\n\nf", 300.0, 4);
        for i in 0..=h.area.char_len() {
            h.area.caret = i;
            let (para, byte) = h.area.para_pos();
            assert_eq!(h.area.char_at(para, byte), i, "caret {i}");
        }
    }

    #[test]
    fn typing_and_enter_update_the_bound_state() {
        let mut h = Harness::new("", 300.0, 4);
        h.typed("one");
        h.key(Key::Enter);
        h.typed("two");
        assert_eq!(h.state, "one\ntwo");
        h.key(Key::Backspace);
        h.key(Key::Backspace);
        h.key(Key::Backspace);
        h.key(Key::Backspace);
        assert_eq!(h.state, "one");
    }

    #[test]
    fn caret_after_trailing_newline_is_on_a_new_line() {
        let mut h = Harness::new("", 300.0, 4);
        h.typed("one");
        let first = h.caret_line();
        h.key(Key::Enter);
        let second = h.caret_line();
        assert!(second.top > first.top, "caret should drop to the next line");
        assert_eq!(second.x, 0.0);
    }

    #[test]
    fn up_and_down_move_between_lines_and_keep_the_column() {
        let mut h = Harness::new("", 300.0, 4);
        h.typed("long first line\nab\nlong third line");
        // End of line 3 -> up lands at the end of the short line 2 -> up again
        // returns to the original column on line 1.
        h.key(Key::Up);
        assert_eq!(h.area.caret, "long first line\nab".chars().count());
        h.key(Key::Up);
        assert_eq!(h.area.caret, "long first line".chars().count());
        h.key(Key::Up);
        assert_eq!(h.area.caret, 0, "up on the first line goes to the start");
        h.key(Key::Down);
        h.key(Key::Down);
        h.key(Key::Down);
        assert_eq!(h.area.caret, h.area.char_len(), "down on the last line goes to the end");
    }

    #[test]
    fn home_and_end_use_the_visual_line_when_wrapped() {
        let words = "alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu";
        let mut h = Harness::new(words, 160.0, 4);
        assert!(h.visual_lines() > 1, "test text should wrap");
        h.area.caret = 0;
        h.key(Key::End);
        let end_of_first = h.area.caret;
        assert!(end_of_first > 0 && end_of_first < h.area.char_len(), "End stays on the first visual line");
        h.key(Key::Right);
        let second = h.caret_line();
        h.key(Key::Home);
        assert_eq!(h.caret_line().top, second.top, "Home stays on the second visual line");
    }

    #[test]
    fn scroll_is_ignored_until_text_overflows() {
        let mut h = Harness::new("short", 300.0, 2);
        let pos = Point::new(10.0, 10.0);
        assert!(!h.send(Event::Scroll { pos, delta: Point::new(0.0, -40.0) }));
        h.area.max_scroll = 100.0;
        assert!(h.send(Event::Scroll { pos, delta: Point::new(0.0, -40.0) }));
        assert_eq!(h.area.scroll, 40.0);
        h.send(Event::Scroll { pos, delta: Point::new(0.0, 500.0) });
        assert_eq!(h.area.scroll, 0.0);
    }
}
