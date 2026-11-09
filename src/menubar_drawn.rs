//! In-window menu bar for Linux and Windows (macOS uses the native one).

use crate::event::{Event, Key, MouseButton};
use crate::geometry::{Point, Rect};
use crate::menu::{MenuBar, MenuEntry, MenuItemId};
use crate::render::{Align, Painter};
use crate::text::TextSystem;
use crate::theme::{Platform, Theme};

#[derive(Default)]
pub struct DrawnMenuBar {
    pub rect: Rect,
    title_rects: Vec<Rect>,
    open: Option<usize>,
    hover_title: Option<usize>,
    hover_item: Option<usize>,
    keyboard_item: Option<usize>,
    dropdown: Rect,
    item_rects: Vec<Option<Rect>>, // None for separators
}

pub struct MenuResponse {
    pub consumed: bool,
    pub redraw: bool,
    pub activate: Option<MenuItemId>,
}

impl DrawnMenuBar {
    #[allow(dead_code)]
    pub fn is_open(&self) -> bool {
        self.open.is_some()
    }

    fn item_height(theme: &Theme) -> f32 {
        match theme.platform {
            Platform::Windows => 34.0,
            _ => 30.0,
        }
    }

    pub fn layout<S>(&mut self, bar: &MenuBar<S>, text: &mut TextSystem, theme: &Theme, width: f32) {
        self.rect = Rect::new(0.0, 0.0, width, theme.menubar_height);
        self.title_rects.clear();
        let mut x = 6.0;
        for m in &bar.menus {
            let (w, _) = text.measure(&m.title, theme.font_size);
            let r = Rect::new(x, 2.0, w + 20.0, theme.menubar_height - 4.0);
            self.title_rects.push(r);
            x += r.width + 2.0;
        }
        if let Some(i) = self.open {
            self.layout_dropdown(bar, i, text, theme);
        }
    }

    fn layout_dropdown<S>(&mut self, bar: &MenuBar<S>, idx: usize, text: &mut TextSystem, theme: &Theme) {
        let menu = &bar.menus[idx];
        let pad = 12.0;
        let mut label_w: f32 = 0.0;
        let mut sc_w: f32 = 0.0;
        for e in &menu.entries {
            if let MenuEntry::Item { label, shortcut, .. } = e {
                label_w = label_w.max(text.measure(label, theme.font_size).0);
                if let Some(sc) = shortcut {
                    sc_w = sc_w.max(text.measure(&sc.display(), theme.font_size).0);
                }
            }
        }
        let width = (pad * 2.0 + label_w + if sc_w > 0.0 { 32.0 + sc_w } else { 0.0 }).max(180.0);
        let ih = Self::item_height(theme);
        let title = self.title_rects[idx];
        let mut y = title.bottom() + 4.0 + 6.0;
        self.item_rects.clear();
        for e in &menu.entries {
            match e {
                MenuEntry::Item { .. } => {
                    self.item_rects.push(Some(Rect::new(title.x + 6.0, y, width - 12.0, ih)));
                    y += ih;
                }
                MenuEntry::Separator => {
                    self.item_rects.push(None);
                    y += 9.0;
                }
            }
        }
        self.dropdown = Rect::new(title.x, title.bottom() + 4.0, width, y - (title.bottom() + 4.0) + 6.0);
    }

    pub fn paint<S>(&mut self, p: &mut Painter, bar: &MenuBar<S>) {
        let t = p.theme;
        let pal = &t.palette;
        p.fill_rect(self.rect, pal.menubar_bg);
        p.line(0.0, self.rect.bottom() - 0.5, self.rect.width, self.rect.bottom() - 0.5, 1.0, pal.separator);
        for (i, m) in bar.menus.iter().enumerate() {
            let r = self.title_rects[i];
            if self.open == Some(i) {
                p.fill_round_rect(r, 5.0, pal.control_bg_pressed);
            } else if self.hover_title == Some(i) {
                p.fill_round_rect(r, 5.0, pal.control_bg_hover);
            }
            p.text_in(&m.title, r, Align::Center, pal.text);
        }
    }

    /// Painted after the content so it floats above it.
    pub fn paint_dropdown<S>(&mut self, p: &mut Painter, bar: &MenuBar<S>) {
        let Some(idx) = self.open else { return };
        let t = p.theme;
        let pal = &t.palette;
        let radius = if t.platform == Platform::Windows { 8.0 } else { 10.0 };
        p.shadow(self.dropdown, radius, pal.shadow);
        p.fill_round_rect(self.dropdown, radius, pal.menu_bg);
        p.stroke_round_rect(self.dropdown, radius, 1.0, pal.menu_border);
        let menu = &bar.menus[idx];
        let hl = self.hover_item.or(self.keyboard_item);
        for (i, e) in menu.entries.iter().enumerate() {
            match (e, self.item_rects[i]) {
                (MenuEntry::Item { label, shortcut, enabled, .. }, Some(r)) => {
                    if hl == Some(i) && *enabled {
                        p.fill_round_rect(r, 6.0, pal.control_bg_hover);
                    }
                    let fg = if *enabled { pal.text } else { pal.text_disabled };
                    let inner = Rect::new(r.x + 10.0, r.y, r.width - 20.0, r.height);
                    p.text_in(label, inner, Align::Start, fg);
                    if let Some(sc) = shortcut {
                        p.text_in(&sc.display(), inner, Align::End, if *enabled { pal.text_secondary } else { pal.text_disabled });
                    }
                }
                (MenuEntry::Separator, _) => {
                    // find y: separators have no rect; derive from neighbours
                    let y = self.separator_y(i);
                    p.line(self.dropdown.x + 8.0, y, self.dropdown.right() - 8.0, y, 1.0, pal.separator);
                }
                _ => {}
            }
        }
    }

    fn separator_y(&self, i: usize) -> f32 {
        // the separator sits between the previous item's bottom and the next item's top
        let prev = self.item_rects[..i].iter().rev().find_map(|r| *r).map(|r| r.bottom());
        let next = self.item_rects[i + 1..].iter().find_map(|r| *r).map(|r| r.y);
        match (prev, next) {
            (Some(a), Some(b)) => ((a + b) / 2.0).round() + 0.5,
            (Some(a), None) => a + 4.5,
            (None, Some(b)) => b - 4.5,
            _ => self.dropdown.y,
        }
    }

    fn title_at(&self, pos: Point) -> Option<usize> {
        self.title_rects.iter().position(|r| r.contains(pos))
    }

    fn item_at(&self, pos: Point) -> Option<usize> {
        self.item_rects.iter().position(|r| r.map(|r| r.contains(pos)).unwrap_or(false))
    }

    fn enabled_item_id<S>(bar: &MenuBar<S>, menu: usize, i: usize) -> Option<MenuItemId> {
        match &bar.menus[menu].entries[i] {
            MenuEntry::Item { id, enabled: true, .. } => Some(*id),
            _ => None,
        }
    }

    pub fn event<S>(&mut self, ev: &Event, bar: &MenuBar<S>, text: &mut TextSystem, theme: &Theme) -> MenuResponse {
        let mut resp = MenuResponse { consumed: false, redraw: false, activate: None };
        match ev {
            Event::MouseMove(pos) => {
                let ht = self.title_at(*pos);
                if ht != self.hover_title {
                    self.hover_title = ht;
                    resp.redraw = true;
                }
                if let Some(open) = self.open {
                    if let Some(t) = ht {
                        if t != open {
                            self.open = Some(t);
                            self.hover_item = None;
                            self.keyboard_item = None;
                            self.layout_dropdown(bar, t, text, theme);
                            resp.redraw = true;
                        }
                    }
                    let hi = self.item_at(*pos);
                    if hi != self.hover_item {
                        self.hover_item = hi;
                        if hi.is_some() {
                            self.keyboard_item = None;
                        }
                        resp.redraw = true;
                    }
                    resp.consumed = true;
                }
            }
            Event::MouseLeave => {
                if self.hover_title.take().is_some() | self.hover_item.take().is_some() {
                    resp.redraw = true;
                }
            }
            Event::MouseDown { pos, button: MouseButton::Left } => {
                if let Some(t) = self.title_at(*pos) {
                    if self.open == Some(t) {
                        self.close();
                    } else {
                        self.open = Some(t);
                        self.hover_item = None;
                        self.keyboard_item = None;
                        self.layout_dropdown(bar, t, text, theme);
                    }
                    resp.consumed = true;
                    resp.redraw = true;
                } else if self.open.is_some() {
                    if self.item_at(*pos).is_none() && !self.dropdown.contains(*pos) {
                        self.close();
                        resp.redraw = true;
                    }
                    resp.consumed = true;
                }
            }
            Event::MouseUp { pos, button: MouseButton::Left } => {
                if let Some(open) = self.open {
                    if let Some(i) = self.item_at(*pos) {
                        if let Some(id) = Self::enabled_item_id(bar, open, i) {
                            resp.activate = Some(id);
                            self.close();
                            resp.redraw = true;
                        }
                    }
                    resp.consumed = true;
                }
            }
            Event::KeyDown { key, .. } if self.open.is_some() => {
                let open = self.open.unwrap();
                let n = bar.menus[open].entries.len();
                let selectable = |i: usize| Self::enabled_item_id(bar, open, i).is_some();
                match key {
                    Key::Escape => self.close(),
                    Key::Down => {
                        let start = self.keyboard_item.or(self.hover_item).map(|i| i + 1).unwrap_or(0);
                        self.keyboard_item = (start..n).chain(0..start).find(|&i| selectable(i));
                        self.hover_item = None;
                    }
                    Key::Up => {
                        let start = self.keyboard_item.or(self.hover_item).unwrap_or(0);
                        self.keyboard_item = (0..start).rev().chain((start..n).rev()).find(|&i| selectable(i));
                        self.hover_item = None;
                    }
                    Key::Left => {
                        let t = (open + bar.menus.len() - 1) % bar.menus.len();
                        self.open = Some(t);
                        self.keyboard_item = None;
                        self.layout_dropdown(bar, t, text, theme);
                    }
                    Key::Right => {
                        let t = (open + 1) % bar.menus.len();
                        self.open = Some(t);
                        self.keyboard_item = None;
                        self.layout_dropdown(bar, t, text, theme);
                    }
                    Key::Enter | Key::Space => {
                        if let Some(i) = self.keyboard_item.or(self.hover_item) {
                            resp.activate = Self::enabled_item_id(bar, open, i);
                            self.close();
                        }
                    }
                    _ => {}
                }
                resp.consumed = true;
                resp.redraw = true;
            }
            _ => {}
        }
        resp
    }

    fn close(&mut self) {
        self.open = None;
        self.hover_item = None;
        self.keyboard_item = None;
    }
}
