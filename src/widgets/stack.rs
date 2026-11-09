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
        self.sizes.clear();
        let mut main_total = 0.0;
        let mut cross_max: f32 = 0.0;
        let axis = self.axis;
        for c in &mut self.children {
            let s = c.measure(cx, inner);
            main_total += axis.main(s);
            cross_max = cross_max.max(axis.cross(s));
            self.sizes.push(s);
        }
        if !self.children.is_empty() {
            main_total += gap * (self.children.len() - 1) as f32;
        }
        let (w, h) = match self.axis {
            Axis::Vertical => (if self.cross == CrossAlign::Stretch { inner.width } else { cross_max }, main_total),
            Axis::Horizontal => (main_total, cross_max),
        };
        Size::new(w + self.padding.horizontal(), h + self.padding.vertical())
    }
    fn layout(&mut self, cx: &mut LayoutCx, rect: Rect) {
        self.rect = rect;
        let gap = self.gap(cx);
        let inner = self.padding.shrink(rect);
        if self.sizes.len() != self.children.len() {
            self.measure(cx, rect.size());
        }
        let axis = self.axis;
        let main_avail = axis.main(inner.size());
        let fixed: f32 = self.sizes.iter().map(|s| axis.main(*s)).sum::<f32>()
            + gap * self.children.len().saturating_sub(1) as f32;
        let total_flex: f32 = self.children.iter().map(|c| c.flex()).sum();
        let leftover = (main_avail - fixed).max(0.0);

        let mut pos = 0.0;
        for (i, c) in self.children.iter_mut().enumerate() {
            let s = self.sizes[i];
            let extra = if total_flex > 0.0 { leftover * c.flex() / total_flex } else { 0.0 };
            let main_len = match self.axis {
                Axis::Vertical => s.height,
                Axis::Horizontal => s.width,
            } + extra;
            let cross_avail = match self.axis {
                Axis::Vertical => inner.width,
                Axis::Horizontal => inner.height,
            };
            let cross_len = match self.cross {
                CrossAlign::Stretch => cross_avail,
                _ => match self.axis {
                    Axis::Vertical => s.width.min(cross_avail),
                    Axis::Horizontal => s.height.min(cross_avail),
                },
            };
            let cross_off = match self.cross {
                CrossAlign::Start | CrossAlign::Stretch => 0.0,
                CrossAlign::Center => (cross_avail - cross_len) / 2.0,
                CrossAlign::End => cross_avail - cross_len,
            };
            let r = match self.axis {
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
