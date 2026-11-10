//! `App` is the whole program: name, icon, menu bar, one window, and the event
//! loop that drives them.

use std::num::NonZeroU32;
use std::path::PathBuf;
use std::sync::Arc;

use tiny_skia::Pixmap;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window as WinitWindow, WindowId};

use crate::event::{Event, Key, Modifiers, MouseButton};
use crate::geometry::{Point, Rect};
use crate::menu::{MenuBar, MenuItemId, Role};
use crate::menubar_drawn::DrawnMenuBar;
use crate::render::Painter;
use crate::text::TextSystem;
use crate::theme::{Platform, Theme};
use crate::ui::{Cmd, Message, Ui};
use crate::widget::{BoxedWidget, EventCx, IntoWidget, LayoutCx, PaintCx, Uid};

/// Description of the main window.
pub struct Window<S: 'static> {
    title: String,
    size: (f32, f32),
    min_size: Option<(f32, f32)>,
    padding: Option<f32>,
    content: Option<BoxedWidget<S>>,
}

impl<S: 'static> Window<S> {
    pub fn new(title: impl Into<String>) -> Self {
        Self { title: title.into(), size: (640.0, 480.0), min_size: None, padding: None, content: None }
    }
    /// Initial size in logical pixels.
    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.size = (width, height);
        self
    }
    pub fn min_size(mut self, width: f32, height: f32) -> Self {
        self.min_size = Some((width, height));
        self
    }
    /// Space between the window edge and the content (defaults per OS).
    pub fn padding(mut self, padding: f32) -> Self {
        self.padding = Some(padding);
        self
    }
    pub fn content(mut self, widget: impl IntoWidget<S>) -> Self {
        self.content = Some(widget.into_widget());
        self
    }
}

pub struct App<S: 'static> {
    name: String,
    state: S,
    icon: Option<PathBuf>,
    menu: Option<MenuBar<S>>,
    window: Option<Window<S>>,
}

impl<S: 'static> App<S> {
    /// `name` is the application name (shown in the macOS app menu and used as
    /// the Linux app id); `state` is whatever your callbacks need.
    pub fn new(name: impl Into<String>, state: S) -> Self {
        Self { name: name.into(), state, icon: None, menu: None, window: None }
    }

    /// A PNG used as the window/taskbar icon on Linux and Windows. On macOS the
    /// icon comes from the .app bundle — see `tools/bundle-macos.sh`.
    pub fn icon(mut self, path: impl Into<PathBuf>) -> Self {
        self.icon = Some(path.into());
        self
    }

    pub fn menu(mut self, menu: MenuBar<S>) -> Self {
        self.menu = Some(menu);
        self
    }

    pub fn window(mut self, window: Window<S>) -> Self {
        self.window = Some(window);
        self
    }

    /// Run the app. Returns when the last window closes or `ui.quit()` is called.
    pub fn run(self) {
        let event_loop = EventLoop::<Message<S>>::with_user_event().build().expect("create event loop");
        event_loop.set_control_flow(ControlFlow::Wait);
        let proxy = event_loop.create_proxy();
        let mut runner = Runner::new(self, Some(proxy));
        event_loop.run_app(&mut runner).expect("run event loop");
    }

    /// Render the window off-screen (no display needed) at the given theme and
    /// scale. Useful for screenshots and tests.
    pub fn snapshot(self, theme: Theme, scale: f32) -> Pixmap {
        let mut core = Core::new(self, None, theme);
        let (w, h) = core.window_size;
        let mut pixmap = Pixmap::new((w * scale) as u32, (h * scale) as u32).expect("pixmap");
        core.render(&mut pixmap, scale);
        pixmap
    }

    /// Like `snapshot`, after feeding a sequence of input events (so screenshots can
    /// show hover, focus or an open menu).
    pub fn snapshot_with(self, theme: Theme, scale: f32, events: &[Event]) -> Pixmap {
        let mut core = Core::new(self, None, theme);
        let (w, h) = core.window_size;
        let mut pixmap = Pixmap::new((w * scale) as u32, (h * scale) as u32).expect("pixmap");
        core.prepare_layout(w);
        for ev in events {
            let out = core.dispatch(ev);
            core.apply_cmds(out.cmds);
        }
        core.render(&mut pixmap, scale);
        pixmap
    }
}

// ---------------------------------------------------------------------------

struct DispatchOut {
    redraw: bool,
    cmds: Vec<Cmd>,
    menu_activate: Option<MenuItemId>,
}

/// Everything about the app that doesn't need a real OS window.
struct Core<S: 'static> {
    name: String,
    state: S,
    ui: Ui<S>,
    root: Option<BoxedWidget<S>>,
    menubar: Option<MenuBar<S>>,
    drawn_menu: DrawnMenuBar,
    text: TextSystem,
    theme: Theme,
    padding: f32,
    window_size: (f32, f32),
    title: String,
    min_size: Option<(f32, f32)>,
    icon: Option<PathBuf>,
    mouse: Point,
    mods: Modifiers,
    quit: bool,
    set_title: Option<String>,
}

impl<S: 'static> Core<S> {
    fn new(app: App<S>, proxy: Option<winit::event_loop::EventLoopProxy<Message<S>>>, theme: Theme) -> Self {
        let window = app.window.unwrap_or_else(|| Window::new(&app.name));
        let padding = window.padding.unwrap_or(match theme.platform {
            Platform::MacOs => 20.0,
            _ => 16.0,
        });
        let text = TextSystem::new(&theme.font_families);
        Self {
            name: app.name,
            state: app.state,
            ui: Ui::new(proxy),
            root: window.content,
            menubar: app.menu,
            drawn_menu: DrawnMenuBar::default(),
            text,
            theme,
            padding,
            window_size: window.size,
            title: window.title,
            min_size: window.min_size,
            icon: app.icon,
            mouse: Point::default(),
            mods: Modifiers::default(),
            quit: false,
            set_title: None,
        }
    }

    fn uses_drawn_menu(&self) -> bool {
        self.menubar.is_some() && self.theme.platform != Platform::MacOs
    }

    fn set_theme(&mut self, theme: Theme) {
        if theme.font_families != self.theme.font_families {
            self.text = TextSystem::new(&theme.font_families);
        }
        self.theme = theme;
    }

    fn menubar_height(&self) -> f32 {
        if self.uses_drawn_menu() {
            self.theme.menubar_height
        } else {
            0.0
        }
    }

    fn content_rect(&self, width: f32, height: f32) -> Rect {
        let top = self.menubar_height();
        Rect::new(self.padding, top + self.padding, (width - 2.0 * self.padding).max(0.0), (height - top - 2.0 * self.padding).max(0.0))
    }

    /// Sync bound values and lay the tree out for a window of `width` × current height.
    fn prepare_layout(&mut self, width: f32) {
        let height = self.window_size.1;
        if let Some(bar) = &self.menubar {
            if self.theme.platform != Platform::MacOs {
                self.drawn_menu.layout(bar, &mut self.text, &self.theme, width);
            }
        }
        let rect = self.content_rect(width, height);
        let top = self.menubar_height();
        if let Some(root) = &mut self.root {
            root.sync(&mut self.state);
            let window = Rect::new(0.0, top, width, (height - top).max(0.0));
            let mut cx = LayoutCx { text: &mut self.text, theme: &self.theme, window };
            let _ = root.measure(&mut cx, rect.size());
            root.layout(&mut cx, rect);
        }
    }

    fn render(&mut self, pixmap: &mut Pixmap, scale: f32) {
        let width = pixmap.width() as f32 / scale;
        let height = pixmap.height() as f32 / scale;
        self.window_size = (width, height);
        self.prepare_layout(width);

        let focused = self.ui.focus;
        let rect = self.content_rect(width, height);
        let mut p = Painter::new(pixmap, &mut self.text, &self.theme, scale);
        p.clear(self.theme.palette.window_bg);
        if let Some(root) = &mut self.root {
            p.clipped(rect, |p| root.paint(p, &PaintCx { focused }));
            // Popups (an open ComboBox list) float above the content, unclipped.
            root.paint_overlay(&mut p, &PaintCx { focused });
        }
        if let Some(bar) = &self.menubar {
            if self.theme.platform != Platform::MacOs {
                self.drawn_menu.paint(&mut p, bar);
                self.drawn_menu.paint_dropdown(&mut p, bar);
            }
        }
    }

    fn dispatch(&mut self, ev: &Event) -> DispatchOut {
        let mut out = DispatchOut { redraw: false, cmds: Vec::new(), menu_activate: None };

        // A widget with an open popup (a ComboBox list) gets first look, so it can
        // take input anywhere in the window and close when you click away.
        if let Some(root) = &mut self.root {
            let mut cx = EventCx { state: &mut self.state, ui: &mut self.ui, theme: &self.theme, text: &mut self.text, redraw: false };
            let consumed = root.popup_event(ev, &mut cx);
            out.redraw |= cx.redraw;
            out.cmds.append(&mut self.ui.cmds);
            if consumed {
                // The popup has the mouse; let the widgets under it drop hover.
                if matches!(ev, Event::MouseMove(_)) {
                    self.forward(&Event::MouseLeave, &mut out);
                }
                return out;
            }
        }

        // In-window menu bar gets next look.
        if self.uses_drawn_menu() {
            let bar = self.menubar.as_ref().unwrap();
            let r = self.drawn_menu.event(ev, bar, &mut self.text, &self.theme);
            out.redraw |= r.redraw;
            out.menu_activate = r.activate;
            if r.consumed {
                // Still let widgets drop hover when the menu takes the mouse.
                if matches!(ev, Event::MouseMove(_)) {
                    self.forward(&Event::MouseLeave, &mut out);
                }
                return out;
            }
        }

        // Keyboard shortcuts (macOS handles these natively through the menu bar).
        if let Event::KeyDown { key, mods, .. } = ev {
            if self.theme.platform != Platform::MacOs {
                if let Some(id) = self.menubar.as_ref().and_then(|b| b.find_shortcut(key, mods)) {
                    out.menu_activate = Some(id);
                    return out;
                }
            }
            if *key == Key::Tab {
                self.cycle_focus(!mods.shift);
                out.redraw = true;
                return out;
            }
        }

        self.forward(ev, &mut out);
        out
    }

    fn forward(&mut self, ev: &Event, out: &mut DispatchOut) {
        if let Some(root) = &mut self.root {
            let mut cx = EventCx { state: &mut self.state, ui: &mut self.ui, theme: &self.theme, text: &mut self.text, redraw: false };
            root.event(ev, &mut cx);
            out.redraw |= cx.redraw;
        }
        out.cmds.append(&mut self.ui.cmds);
    }

    fn cycle_focus(&mut self, forward: bool) {
        let Some(root) = &self.root else { return };
        let mut order: Vec<Uid> = Vec::new();
        root.collect_focusable(&mut order);
        if order.is_empty() {
            return;
        }
        let cur = self.ui.focus.and_then(|f| order.iter().position(|u| *u == f));
        let next = match (cur, forward) {
            (None, true) => 0,
            (None, false) => order.len() - 1,
            (Some(i), true) => (i + 1) % order.len(),
            (Some(i), false) => (i + order.len() - 1) % order.len(),
        };
        self.ui.focus = Some(order[next]);
    }

    fn activate_menu(&mut self, id: MenuItemId) -> Vec<Cmd> {
        let role = self.menubar.as_mut().map(|b| b.activate(id, &mut self.state, &mut self.ui)).unwrap_or(None);
        match role {
            Some(Role::Quit) | Some(Role::CloseWindow) => self.ui.cmds.push(Cmd::Quit),
            _ => {}
        }
        std::mem::take(&mut self.ui.cmds)
    }

    /// Apply queued commands. Returns true if a redraw is needed.
    fn apply_cmds(&mut self, cmds: Vec<Cmd>) -> bool {
        let mut redraw = false;
        for cmd in cmds {
            match cmd {
                Cmd::Quit => self.quit = true,
                Cmd::Alert { title, message } => show_alert(&title, &message),
                Cmd::SetTitle(t) => self.set_title = Some(t),
                Cmd::Widget(wc) => {
                    if let Some(root) = &mut self.root {
                        root.apply(&wc);
                    }
                    redraw = true;
                }
                Cmd::Focus(id) => {
                    if let Some(uid) = self.root.as_ref().and_then(|r| r.find_uid(&id)) {
                        self.ui.focus = Some(uid);
                        redraw = true;
                    }
                }
            }
        }
        redraw
    }
}

fn show_alert(title: &str, message: &str) {
    rfd::MessageDialog::new()
        .set_title(title)
        .set_description(message)
        .set_level(rfd::MessageLevel::Info)
        .set_buttons(rfd::MessageButtons::Ok)
        .show();
}

// ---------------------------------------------------------------------------

struct Runner<S: 'static> {
    core: Core<S>,
    window: Option<Arc<WinitWindow>>,
    surface: Option<softbuffer::Surface<Arc<WinitWindow>, Arc<WinitWindow>>>,
    pixmap: Option<Pixmap>,
    #[cfg(target_os = "macos")]
    native_menu: Option<crate::platform::macos::NativeMenu>,
}

impl<S: 'static> Runner<S> {
    fn new(app: App<S>, proxy: Option<winit::event_loop::EventLoopProxy<Message<S>>>) -> Self {
        let theme = Theme::current(false);
        Self {
            core: Core::new(app, proxy, theme),
            window: None,
            surface: None,
            pixmap: None,
            #[cfg(target_os = "macos")]
            native_menu: None,
        }
    }

    fn scale(&self) -> f32 {
        self.window.as_ref().map(|w| w.scale_factor() as f32).unwrap_or(1.0)
    }

    fn finish(&mut self, out: DispatchOut, el: &ActiveEventLoop) {
        let mut redraw = out.redraw;
        let mut cmds = out.cmds;
        if let Some(id) = out.menu_activate {
            cmds.extend(self.core.activate_menu(id));
            redraw = true;
        }
        redraw |= self.core.apply_cmds(cmds);
        self.after_cmds(redraw, el);
    }

    fn after_cmds(&mut self, redraw: bool, el: &ActiveEventLoop) {
        if self.core.quit {
            el.exit();
            return;
        }
        if let Some(t) = self.core.set_title.take() {
            if let Some(w) = &self.window {
                w.set_title(&t);
            }
        }
        if redraw {
            if let Some(w) = &self.window {
                w.request_redraw();
            }
        }
    }

    fn redraw(&mut self) {
        let (Some(window), Some(surface)) = (&self.window, &mut self.surface) else { return };
        let size = window.inner_size();
        let (Some(w), Some(h)) = (NonZeroU32::new(size.width), NonZeroU32::new(size.height)) else { return };
        if surface.resize(w, h).is_err() {
            return;
        }
        let scale = window.scale_factor() as f32;
        let need_new = self.pixmap.as_ref().map(|p| p.width() != size.width || p.height() != size.height).unwrap_or(true);
        if need_new {
            self.pixmap = Pixmap::new(size.width, size.height);
        }
        let Some(pixmap) = &mut self.pixmap else { return };
        self.core.render(pixmap, scale);

        let Ok(mut buffer) = surface.buffer_mut() else { return };
        let src = pixmap.data();
        for (i, px) in buffer.iter_mut().enumerate() {
            let j = i * 4;
            let (r, g, b) = (src[j] as u32, src[j + 1] as u32, src[j + 2] as u32);
            *px = (r << 16) | (g << 8) | b;
        }
        let _ = buffer.present();
    }
}

impl<S: 'static> ApplicationHandler<Message<S>> for Runner<S> {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let (w, h) = self.core.window_size;
        let mut attrs = WinitWindow::default_attributes()
            .with_title(&self.core.title)
            .with_inner_size(LogicalSize::new(w as f64, h as f64));
        if let Some((mw, mh)) = self.core.min_size {
            attrs = attrs.with_min_inner_size(LogicalSize::new(mw as f64, mh as f64));
        }
        if let Some(icon) = self.core.icon.as_ref().and_then(|p| crate::icon::load_window_icon(p)) {
            attrs = attrs.with_window_icon(Some(icon));
        }
        #[cfg(target_os = "linux")]
        {
            let app_id = crate::icon::app_id(&self.core.name);
            {
                use winit::platform::wayland::WindowAttributesExtWayland;
                attrs = WindowAttributesExtWayland::with_name(attrs, &app_id, &app_id);
            }
            {
                use winit::platform::x11::WindowAttributesExtX11;
                attrs = WindowAttributesExtX11::with_name(attrs, &app_id, &app_id);
            }
        }

        let window = Arc::new(el.create_window(attrs).expect("create window"));
        let dark = matches!(window.theme(), Some(winit::window::Theme::Dark));
        self.core.set_theme(Theme::current(dark));

        let context = softbuffer::Context::new(window.clone()).expect("softbuffer context");
        let surface = softbuffer::Surface::new(&context, window.clone()).expect("softbuffer surface");
        self.surface = Some(surface);

        #[cfg(target_os = "macos")]
        if let Some(bar) = &self.core.menubar {
            let proxy = self.core.ui.proxy.clone();
            let notify = move |id: String| {
                if let Some(p) = &proxy {
                    let _ = p.send_event(Message::NativeMenu(id));
                }
            };
            self.native_menu = Some(crate::platform::macos::NativeMenu::build(&self.core.name, bar, notify));
        }

        window.request_redraw();
        self.window = Some(window);
    }

    fn user_event(&mut self, el: &ActiveEventLoop, event: Message<S>) {
        match event {
            Message::Post(f) => {
                f(&mut self.core.state, &mut self.core.ui);
                let cmds = std::mem::take(&mut self.core.ui.cmds);
                self.core.apply_cmds(cmds);
                self.after_cmds(true, el);
            }
            #[allow(unused_variables)]
            Message::NativeMenu(native_id) => {
                #[cfg(target_os = "macos")]
                if let Some(id) = self.native_menu.as_ref().and_then(|m| m.resolve(&native_id)) {
                    let out = DispatchOut { redraw: true, cmds: Vec::new(), menu_activate: Some(id) };
                    self.finish(out, el);
                }
            }
        }
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => el.exit(),
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }
            WindowEvent::ThemeChanged(t) => {
                self.core.set_theme(Theme::current(t == winit::window::Theme::Dark));
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }
            WindowEvent::RedrawRequested => self.redraw(),
            WindowEvent::ModifiersChanged(m) => {
                let s = m.state();
                self.core.mods = Modifiers { shift: s.shift_key(), ctrl: s.control_key(), alt: s.alt_key(), meta: s.super_key() };
            }
            WindowEvent::CursorMoved { position, .. } => {
                let scale = self.scale();
                self.core.mouse = Point::new(position.x as f32 / scale, position.y as f32 / scale);
                let out = self.core.dispatch(&Event::MouseMove(self.core.mouse));
                self.finish(out, el);
            }
            WindowEvent::CursorLeft { .. } => {
                let out = self.core.dispatch(&Event::MouseLeave);
                self.finish(out, el);
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let button = match button {
                    winit::event::MouseButton::Left => MouseButton::Left,
                    winit::event::MouseButton::Right => MouseButton::Right,
                    winit::event::MouseButton::Middle => MouseButton::Middle,
                    _ => return,
                };
                let pos = self.core.mouse;
                let ev = match state {
                    ElementState::Pressed => Event::MouseDown { pos, button },
                    ElementState::Released => Event::MouseUp { pos, button },
                };
                let out = self.core.dispatch(&ev);
                self.finish(out, el);
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let delta = match delta {
                    // One wheel notch scrolls about three lines.
                    winit::event::MouseScrollDelta::LineDelta(x, y) => Point::new(x * 48.0, y * 48.0),
                    winit::event::MouseScrollDelta::PixelDelta(p) => {
                        let s = self.scale();
                        Point::new(p.x as f32 / s, p.y as f32 / s)
                    }
                };
                let out = self.core.dispatch(&Event::Scroll { pos: self.core.mouse, delta });
                self.finish(out, el);
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let key = map_key(&event.logical_key);
                let mods = self.core.mods;
                let ev = match event.state {
                    ElementState::Pressed => Event::KeyDown { key, mods, text: event.text.as_ref().map(|t| t.to_string()) },
                    ElementState::Released => Event::KeyUp { key, mods },
                };
                let out = self.core.dispatch(&ev);
                self.finish(out, el);
            }
            _ => {}
        }
    }
}

fn map_key(k: &winit::keyboard::Key) -> Key {
    use winit::keyboard::{Key as WKey, NamedKey};
    match k {
        WKey::Character(s) => Key::Character(s.to_string()),
        WKey::Named(n) => match n {
            NamedKey::Enter => Key::Enter,
            NamedKey::Tab => Key::Tab,
            NamedKey::Escape => Key::Escape,
            NamedKey::Backspace => Key::Backspace,
            NamedKey::Delete => Key::Delete,
            NamedKey::Space => Key::Space,
            NamedKey::ArrowLeft => Key::Left,
            NamedKey::ArrowRight => Key::Right,
            NamedKey::ArrowUp => Key::Up,
            NamedKey::ArrowDown => Key::Down,
            NamedKey::Home => Key::Home,
            NamedKey::End => Key::End,
            _ => Key::Other,
        },
        _ => Key::Other,
    }
}
