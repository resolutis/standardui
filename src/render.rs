//! The software renderer. Widgets paint through `Painter` in logical pixels;
//! it rasterises into a tiny-skia pixmap at the window's physical size.

use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Stroke, Transform};

use crate::geometry::Rect;
use crate::text::{measure_buffer, TextSystem};
use crate::theme::{Color, Theme};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Start,
    Center,
    End,
}

pub struct Painter<'a> {
    pub pixmap: &'a mut Pixmap,
    pub text: &'a mut TextSystem,
    pub theme: &'a Theme,
    pub scale: f32,
    clip_stack: Vec<(i32, i32, i32, i32)>,
}

impl<'a> Painter<'a> {
    pub fn new(pixmap: &'a mut Pixmap, text: &'a mut TextSystem, theme: &'a Theme, scale: f32) -> Self {
        let full = (0, 0, pixmap.width() as i32, pixmap.height() as i32);
        Self { pixmap, text, theme, scale, clip_stack: vec![full] }
    }

    fn clip(&self) -> (i32, i32, i32, i32) {
        *self.clip_stack.last().unwrap()
    }

    fn skia_mask(&self) -> Option<tiny_skia::Mask> {
        let c = self.clip();
        let full = (0, 0, self.pixmap.width() as i32, self.pixmap.height() as i32);
        if c == full {
            return None;
        }
        let mut mask = tiny_skia::Mask::new(self.pixmap.width(), self.pixmap.height())?;
        let r = tiny_skia::Rect::from_ltrb(c.0 as f32, c.1 as f32, c.2 as f32, c.3 as f32)?;
        mask.fill_path(&PathBuilder::from_rect(r), FillRule::Winding, true, Transform::identity());
        Some(mask)
    }

    /// Run `f` with painting clipped to `rect` (logical).
    pub fn clipped(&mut self, rect: Rect, f: impl FnOnce(&mut Painter)) {
        let s = self.scale;
        let cur = self.clip();
        let new = (
            cur.0.max((rect.x * s).floor() as i32),
            cur.1.max((rect.y * s).floor() as i32),
            cur.2.min((rect.right() * s).ceil() as i32),
            cur.3.min((rect.bottom() * s).ceil() as i32),
        );
        self.clip_stack.push(new);
        f(self);
        self.clip_stack.pop();
    }

    fn paint_for(color: Color) -> Paint<'static> {
        let mut p = Paint::default();
        p.set_color(color.to_skia());
        p.anti_alias = true;
        p
    }

    fn transform(&self) -> Transform {
        Transform::from_scale(self.scale, self.scale)
    }

    pub fn clear(&mut self, color: Color) {
        self.pixmap.fill(color.to_skia());
    }

    pub fn fill_rect(&mut self, rect: Rect, color: Color) {
        self.fill_round_rect(rect, 0.0, color);
    }

    pub fn fill_round_rect(&mut self, rect: Rect, radius: f32, color: Color) {
        if color.a <= 0.0 || rect.width <= 0.0 || rect.height <= 0.0 {
            return;
        }
        let path = round_rect_path(rect, radius);
        let mask = self.skia_mask();
        self.pixmap.fill_path(&path, &Self::paint_for(color), FillRule::Winding, self.transform(), mask.as_ref());
    }

    pub fn stroke_round_rect(&mut self, rect: Rect, radius: f32, width: f32, color: Color) {
        if color.a <= 0.0 || width <= 0.0 {
            return;
        }
        // Stroke centred on the inset rect so it stays inside `rect`.
        let path = round_rect_path(rect.inset(width / 2.0), (radius - width / 2.0).max(0.0));
        let stroke = Stroke { width, ..Stroke::default() };
        let mask = self.skia_mask();
        self.pixmap.stroke_path(&path, &Self::paint_for(color), &stroke, self.transform(), mask.as_ref());
    }

    pub fn line(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, width: f32, color: Color) {
        let mut pb = PathBuilder::new();
        pb.move_to(x0, y0);
        pb.line_to(x1, y1);
        let Some(path) = pb.finish() else { return };
        let stroke = Stroke { width, ..Stroke::default() };
        let mask = self.skia_mask();
        self.pixmap.stroke_path(&path, &Self::paint_for(color), &stroke, self.transform(), mask.as_ref());
    }

    /// Connected line segments through `points`, with round caps and joins
    /// (used for chevrons).
    pub fn polyline(&mut self, points: &[(f32, f32)], width: f32, color: Color) {
        let Some((first, rest)) = points.split_first() else { return };
        let mut pb = PathBuilder::new();
        pb.move_to(first.0, first.1);
        for (x, y) in rest {
            pb.line_to(*x, *y);
        }
        let Some(path) = pb.finish() else { return };
        let stroke = Stroke {
            width,
            line_cap: tiny_skia::LineCap::Round,
            line_join: tiny_skia::LineJoin::Round,
            ..Stroke::default()
        };
        let mask = self.skia_mask();
        self.pixmap.stroke_path(&path, &Self::paint_for(color), &stroke, self.transform(), mask.as_ref());
    }

    /// A simple check mark inside `rect`.
    pub fn check_mark(&mut self, rect: Rect, width: f32, color: Color) {
        let mut pb = PathBuilder::new();
        pb.move_to(rect.x + rect.width * 0.22, rect.y + rect.height * 0.52);
        pb.line_to(rect.x + rect.width * 0.42, rect.y + rect.height * 0.72);
        pb.line_to(rect.x + rect.width * 0.78, rect.y + rect.height * 0.30);
        let Some(path) = pb.finish() else { return };
        let stroke = Stroke {
            width,
            line_cap: tiny_skia::LineCap::Round,
            line_join: tiny_skia::LineJoin::Round,
            ..Stroke::default()
        };
        let mask = self.skia_mask();
        self.pixmap.stroke_path(&path, &Self::paint_for(color), &stroke, self.transform(), mask.as_ref());
    }

    /// Soft drop shadow under `rect` (used for menus).
    pub fn shadow(&mut self, rect: Rect, radius: f32, color: Color) {
        for i in (1..=6).rev() {
            let d = i as f32;
            let c = color.with_alpha(color.a * 0.12 / d.sqrt());
            self.fill_round_rect(Rect::new(rect.x - d, rect.y - d + 2.0, rect.width + 2.0 * d, rect.height + 2.0 * d), radius + d, c);
        }
    }

    /// Measure text at the theme font size.
    pub fn measure_text(&mut self, text: &str) -> (f32, f32) {
        self.text.measure(text, self.theme.font_size)
    }

    /// Draw single-line text inside `rect`, horizontally aligned and vertically centred.
    pub fn text_in(&mut self, text: &str, rect: Rect, align: Align, color: Color) {
        self.text_in_sized(text, rect, align, color, self.theme.font_size);
    }

    pub fn text_in_sized(&mut self, text: &str, rect: Rect, align: Align, color: Color, font_size: f32) {
        if text.is_empty() {
            return;
        }
        let buffer = self.text.layout(text, font_size, None);
        let (w, h) = measure_buffer(&buffer);
        let x = match align {
            Align::Start => rect.x,
            Align::Center => rect.x + (rect.width - w) / 2.0,
            Align::End => rect.right() - w,
        };
        let y = rect.y + (rect.height - h) / 2.0;
        let clip = self.clip();
        let (pw, ph) = (self.pixmap.width(), self.pixmap.height());
        self.text.draw(self.pixmap.data_mut(), pw, ph, clip, &buffer, x.round(), y.round(), self.scale, color);
    }

    /// Draw wrapped multi-line text with its top-left at `rect` origin, wrapping at `rect.width`.
    /// Returns the laid-out height.
    pub fn text_wrapped(&mut self, text: &str, rect: Rect, color: Color) -> f32 {
        if text.is_empty() {
            return 0.0;
        }
        let buffer = self.text.layout(text, self.theme.font_size, Some(rect.width));
        let (_, h) = measure_buffer(&buffer);
        let clip = self.clip();
        let (pw, ph) = (self.pixmap.width(), self.pixmap.height());
        self.text.draw(self.pixmap.data_mut(), pw, ph, clip, &buffer, rect.x, rect.y, self.scale, color);
        h
    }
}

fn round_rect_path(rect: Rect, radius: f32) -> tiny_skia::Path {
    let r = radius.min(rect.width / 2.0).min(rect.height / 2.0).max(0.0);
    let (x, y, w, h) = (rect.x, rect.y, rect.width, rect.height);
    let mut pb = PathBuilder::new();
    if r <= 0.0 {
        pb.push_rect(tiny_skia::Rect::from_xywh(x, y, w, h).unwrap_or(tiny_skia::Rect::from_xywh(0.0, 0.0, 1.0, 1.0).unwrap()));
        return pb.finish().unwrap();
    }
    // Quarter-circle approximations with cubic Béziers.
    let k = 0.5523 * r;
    pb.move_to(x + r, y);
    pb.line_to(x + w - r, y);
    pb.cubic_to(x + w - r + k, y, x + w, y + r - k, x + w, y + r);
    pb.line_to(x + w, y + h - r);
    pb.cubic_to(x + w, y + h - r + k, x + w - r + k, y + h, x + w - r, y + h);
    pb.line_to(x + r, y + h);
    pb.cubic_to(x + r - k, y + h, x, y + h - r + k, x, y + h - r);
    pb.line_to(x, y + r);
    pb.cubic_to(x, y + r - k, x + r - k, y, x + r, y);
    pb.close();
    pb.finish().unwrap()
}
