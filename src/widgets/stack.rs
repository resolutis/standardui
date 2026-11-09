use crate::event::Event;
use crate::geometry::{Insets, Rect, Size};
use crate::render::Painter;
use crate::ui::WidgetCmd;
use crate::widget::{BoxedWidget, EventCx, IntoWidget, LayoutCx, PaintCx, Uid, Widget};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CrossAlign {
    Start,
    Center,
    End,
    Stretch,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Axis {
    Vertical,
    Horizontal,
}

/// Linear layout shared by `Column` and `Row`.
pub struct Stack<S: 'static> {
    uid: Uid,
    axis: Axis,
    children: Vec<BoxedWidget<S>>,
    sizes: Vec<Size>,
    spacing: Option<f32>,
    padding: Insets,
    cross: CrossAlign,
    flex: f32,
    rect: Rect,
    /// The `avail` size `sizes` was measured against.
    measured_for: Option<Size>,
    /// Last overflow amount reported, so the warning isn't repeated every frame.
    overflow_reported: f32,
}

impl<S: 'static> Stack<S> {
    fn with_axis(axis: Axis) -> Self {
        Self {
            uid: Uid::next(),
            axis,
            children: Vec::new(),
            sizes: Vec::new(),
            spacing: None,
            padding: Insets::all(0.0),
            cross: match axis {
                Axis::Vertical => CrossAlign::Stretch,
                Axis::Horizontal => CrossAlign::Center,
            },
            flex: 0.0,
            rect: Rect::default(),
            measured_for: None,
            overflow_reported: 0.0,
        }
    }

    /// Lay children out top-to-bottom. Children stretch to the full width by default.
    pub fn column() -> Self {
        Self::with_axis(Axis::Vertical)
    }

    /// Lay children out left-to-right, vertically centred by default.
    pub fn row() -> Self {
        Self::with_axis(Axis::Horizontal)
    }

    pub fn add(mut self, child: impl IntoWidget<S>) -> Self {
        self.children.push(child.into_widget());
        self
    }

    /// Gap between children (defaults to the theme spacing).
    pub fn spacing(mut self, spacing: f32) -> Self {
        self.spacing = Some(spacing);
        self
    }

    pub fn padding(mut self, padding: f32) -> Self {
        self.padding = Insets::all(padding);
        self
    }

    pub fn padding_insets(mut self, padding: Insets) -> Self {
        self.padding = padding;
        self
    }

    pub fn align(mut self, cross: CrossAlign) -> Self {
        self.cross = cross;
        self
    }

    /// Make this stack take a share of leftover space in its parent.
    pub fn flex(mut self, flex: f32) -> Self {
        self.flex = flex;
        self
    }

    fn gap(&self, cx: &LayoutCx) -> f32 {
        self.spacing.unwrap_or(cx.theme.spacing)
    }

    /// In debug builds, warn once (per distinct amount) when the fixed-size children
    /// don't fit: anything past the edge would otherwise just silently not appear.
    fn report_overflow(&mut self, overflow: f32, needed: f32, have: f32) {
        if overflow <= 0.5 {
            self.overflow_reported = 0.0;
            return;
        }
        if (overflow - self.overflow_reported).abs() > 0.5 {
            self.overflow_reported = overflow;
            if cfg!(debug_assertions) {
                eprintln!(
                    "standardui: {} overflowed by {:.0}px (children need {:.0}px, have {:.0}px); \
                     use .flex() on a child to let it take the remaining space instead",
                    self.axis.name(),
                    overflow,
                    needed,
                    have
                );
            }
        }
    }
}

impl Axis {
    fn main(self, s: Size) -> f32 {
        match self {
            Axis::Vertical => s.height,
            Axis::Horizontal => s.width,
        }
    }
    fn cross(self, s: Size) -> f32 {
        match self {
            Axis::Vertical => s.width,
            Axis::Horizontal => s.height,
        }
    }
    /// `s` with its main-axis length replaced by `main`.
    fn with_main(self, s: Size, main: f32) -> Size {
        match self {
            Axis::Vertical => Size::new(s.width, main),
            Axis::Horizontal => Size::new(main, s.height),
        }
    }
    fn name(self) -> &'static str {
        match self {
            Axis::Vertical => "Column",
            Axis::Horizontal => "Row",
        }
    }
}

impl<S: 'static> Widget<S> for Stack<S> {
    fn uid(&self) -> Uid {
        self.uid
    }
    fn flex(&self) -> f32 {
        self.flex
    }
    fn sync(&mut self, state: &mut S) {
        for c in &mut self.children {
            c.sync(state);
        }
    }
    fn measure(&mut self, cx: &mut LayoutCx, avail: Size) -> Size {
        let gap = self.gap(cx);
        let inner = Size::new(
            (avail.width - self.padding.horizontal()).max(0.0),
            (avail.height - self.padding.vertical()).max(0.0),
        );
        let axis = self.axis;
        let n = self.children.len();
        self.sizes.clear();
        self.sizes.resize(n, Size::ZERO);
        let main_avail = axis.main(inner);

        // Pass 1: fixed children, each offered only what earlier children left over.
        let mut used = gap * n.saturating_sub(1) as f32;
        let mut total_flex = 0.0;
        for (i, c) in self.children.iter_mut().enumerate() {
            let f = c.flex();
            if f > 0.0 {
                total_flex += f;
                continue;
            }
            let remaining = (main_avail - used).max(0.0);
            let s = c.measure(cx, axis.with_main(inner, remaining));
            used += axis.main(s);
            self.sizes[i] = s;
        }

        // Pass 2: flex children share whatever is left, by weight.
        let flex_space = (main_avail - used).max(0.0);
        let mut main_total = used;
        if total_flex > 0.0 {
            for (i, c) in self.children.iter_mut().enumerate() {
                let f = c.flex();
                if f > 0.0 {
                    let share = flex_space * f / total_flex;
                    let s = c.measure(cx, axis.with_main(inner, share));
                    main_total += axis.main(s);
                    self.sizes[i] = s;
                }
            }
        }

        // Report natural size; stretching to fill happens in `layout`.
        let cross_max = self.sizes.iter().map(|s| axis.cross(*s)).fold(0.0, f32::max);
        self.measured_for = Some(avail);
        let (w, h) = match axis {
            Axis::Vertical => (cross_max, main_total),
            Axis::Horizontal => (main_total, cross_max),
        };
        Size::new(w + self.padding.horizontal(), h + self.padding.vertical())
    }
    fn layout(&mut self, cx: &mut LayoutCx, rect: Rect) {
        self.rect = rect;
        let gap = self.gap(cx);
        let inner = self.padding.shrink(rect);
        // Re-measure if the parent gave us a different size from the one we measured
        // against (e.g. we were stretched or given a flex share).
        if self.sizes.len() != self.children.len() || self.measured_for != Some(rect.size()) {
            self.measure(cx, rect.size());
        }
        let axis = self.axis;
        let main_avail = axis.main(inner.size());
        let total_flex: f32 = self.children.iter().map(|c| c.flex()).sum();
        let fixed: f32 = self
            .children
            .iter()
            .zip(&self.sizes)
            .filter(|(c, _)| c.flex() <= 0.0)
            .map(|(_, s)| axis.main(*s))
            .sum::<f32>()
            + gap * self.children.len().saturating_sub(1) as f32;
        let flex_space = (main_avail - fixed).max(0.0);

        self.report_overflow(fixed - main_avail, fixed, main_avail);

        let cross_avail = axis.cross(inner.size());
        let mut pos = 0.0;
        for (i, c) in self.children.iter_mut().enumerate() {
            let s = self.sizes[i];
            let f = c.flex();
            let main_len = if f > 0.0 && total_flex > 0.0 {
                flex_space * f / total_flex
            } else {
                axis.main(s)
            };
            let cross_len = match self.cross {
                CrossAlign::Stretch => cross_avail,
                _ => axis.cross(s).min(cross_avail),
            };
            let cross_off = match self.cross {
                CrossAlign::Start | CrossAlign::Stretch => 0.0,
                CrossAlign::Center => (cross_avail - cross_len) / 2.0,
                CrossAlign::End => cross_avail - cross_len,
            };
            let r = match axis {
                Axis::Vertical => Rect::new(inner.x + cross_off, inner.y + pos, cross_len, main_len),
                Axis::Horizontal => Rect::new(inner.x + pos, inner.y + cross_off, main_len, cross_len),
            };
            c.layout(cx, r);
            pos += main_len + gap;
        }
    }
    fn rect(&self) -> Rect {
        self.rect
    }
    fn paint(&mut self, p: &mut Painter, cx: &PaintCx) {
        for c in &mut self.children {
            c.paint(p, cx);
        }
    }
    fn event(&mut self, ev: &Event, cx: &mut EventCx<S>) -> bool {
        // Mouse moves/leaves go to everyone (hover bookkeeping); other events stop at the first taker.
        let broadcast = matches!(ev, Event::MouseMove(_) | Event::MouseLeave);
        let mut handled = false;
        for c in &mut self.children {
            if c.event(ev, cx) {
                handled = true;
                if !broadcast {
                    break;
                }
            }
        }
        handled
    }
    fn apply(&mut self, cmd: &WidgetCmd) -> bool {
        self.children.iter_mut().any(|c| c.apply(cmd))
    }
    fn collect_focusable(&self, out: &mut Vec<Uid>) {
        for c in &self.children {
            c.collect_focusable(out);
        }
    }
    fn find_uid(&self, id: &str) -> Option<Uid> {
        self.children.iter().find_map(|c| c.find_uid(id))
    }
}

/// Flexible empty space.
pub struct Spacer {
    uid: Uid,
    flex: f32,
    fixed: f32,
    rect: Rect,
}

impl Spacer {
    /// Grows to fill leftover space.
    pub fn new() -> Self {
        Self { uid: Uid::next(), flex: 1.0, fixed: 0.0, rect: Rect::default() }
    }
    /// Fixed-size gap.
    pub fn fixed(size: f32) -> Self {
        Self { uid: Uid::next(), flex: 0.0, fixed: size, rect: Rect::default() }
    }
}

impl Default for Spacer {
    fn default() -> Self {
        Self::new()
    }
}

impl<S: 'static> Widget<S> for Spacer {
    fn uid(&self) -> Uid {
        self.uid
    }
    fn flex(&self) -> f32 {
        self.flex
    }
    fn measure(&mut self, _cx: &mut LayoutCx, _avail: Size) -> Size {
        Size::new(self.fixed, self.fixed)
    }
    fn layout(&mut self, _cx: &mut LayoutCx, rect: Rect) {
        self.rect = rect;
    }
    fn rect(&self) -> Rect {
        self.rect
    }
    fn paint(&mut self, _p: &mut Painter, _cx: &PaintCx) {}
    fn event(&mut self, _ev: &Event, _cx: &mut EventCx<S>) -> bool {
        false
    }
}

macro_rules! stack_wrapper {
    ($name:ident, $ctor:ident, $doc:expr) => {
        #[doc = $doc]
        pub struct $name<S: 'static>(Stack<S>);

        impl<S: 'static> $name<S> {
            #[allow(clippy::new_without_default)]
            pub fn new() -> Self {
                Self(Stack::$ctor())
            }
            pub fn add(mut self, child: impl IntoWidget<S>) -> Self {
                self.0 = self.0.add(child);
                self
            }
            /// Gap between children (defaults to the theme spacing).
            pub fn spacing(mut self, spacing: f32) -> Self {
                self.0 = self.0.spacing(spacing);
                self
            }
            pub fn padding(mut self, padding: f32) -> Self {
                self.0 = self.0.padding(padding);
                self
            }
            pub fn padding_insets(mut self, padding: Insets) -> Self {
                self.0 = self.0.padding_insets(padding);
                self
            }
            /// Cross-axis alignment of children.
            pub fn align(mut self, cross: CrossAlign) -> Self {
                self.0 = self.0.align(cross);
                self
            }
            /// Take a share of leftover space in the parent.
            pub fn flex(mut self, flex: f32) -> Self {
                self.0 = self.0.flex(flex);
                self
            }
        }

        impl<S: 'static> Widget<S> for $name<S> {
            fn uid(&self) -> Uid { self.0.uid() }
            fn flex(&self) -> f32 { Widget::flex(&self.0) }
            fn sync(&mut self, state: &mut S) { self.0.sync(state) }
            fn measure(&mut self, cx: &mut LayoutCx, avail: Size) -> Size { self.0.measure(cx, avail) }
            fn layout(&mut self, cx: &mut LayoutCx, rect: Rect) { self.0.layout(cx, rect) }
            fn rect(&self) -> Rect { self.0.rect() }
            fn paint(&mut self, p: &mut Painter, cx: &PaintCx) { self.0.paint(p, cx) }
            fn event(&mut self, ev: &Event, cx: &mut EventCx<S>) -> bool { self.0.event(ev, cx) }
            fn apply(&mut self, cmd: &WidgetCmd) -> bool { self.0.apply(cmd) }
            fn collect_focusable(&self, out: &mut Vec<Uid>) { self.0.collect_focusable(out) }
            fn find_uid(&self, id: &str) -> Option<Uid> { self.0.find_uid(id) }
        }
    };
}

stack_wrapper!(Column, column, "Vertical stack of widgets; children stretch to the full width by default.");
stack_wrapper!(Row, row, "Horizontal stack of widgets; children are vertically centred by default.");
