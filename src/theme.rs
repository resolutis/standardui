//! Per-OS look. The theme is chosen automatically from the OS the app runs on
//! and from the system light/dark setting; app code never touches it unless it
//! wants to.

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const fn rgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }
    pub const fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b, a: 1.0 }
    }
    /// From 0xRRGGBB.
    pub const fn hex(v: u32) -> Self {
        Self::rgb(
            ((v >> 16) & 0xff) as f32 / 255.0,
            ((v >> 8) & 0xff) as f32 / 255.0,
            (v & 0xff) as f32 / 255.0,
        )
    }
    pub const fn with_alpha(self, a: f32) -> Self {
        Self { a, ..self }
    }
    pub const TRANSPARENT: Color = Color::rgba(0.0, 0.0, 0.0, 0.0);
    pub const WHITE: Color = Color::rgb(1.0, 1.0, 1.0);
    pub const BLACK: Color = Color::rgb(0.0, 0.0, 0.0);

    pub(crate) fn to_skia(self) -> tiny_skia::Color {
        tiny_skia::Color::from_rgba(
            self.r.clamp(0.0, 1.0),
            self.g.clamp(0.0, 1.0),
            self.b.clamp(0.0, 1.0),
            self.a.clamp(0.0, 1.0),
        )
        .unwrap()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Platform {
    MacOs,
    Windows,
    Linux,
}

impl Platform {
    pub fn current() -> Platform {
        if cfg!(target_os = "macos") {
            Platform::MacOs
        } else if cfg!(target_os = "windows") {
            Platform::Windows
        } else {
            Platform::Linux
        }
    }
}

#[derive(Clone, Debug)]
pub struct Palette {
    pub window_bg: Color,
    pub text: Color,
    pub text_secondary: Color,
    pub text_disabled: Color,
    pub control_bg: Color,
    pub control_bg_hover: Color,
    pub control_bg_pressed: Color,
    pub control_border: Color,
    pub field_bg: Color,
    pub accent: Color,
    pub accent_text: Color,
    pub focus_ring: Color,
    pub menubar_bg: Color,
    pub menu_bg: Color,
    pub menu_border: Color,
    pub separator: Color,
    pub shadow: Color,
}

#[derive(Clone, Debug)]
pub struct Theme {
    pub platform: Platform,
    pub dark: bool,
    pub palette: Palette,
    /// Preferred UI font families, tried in order; the text system falls back to the
    /// generic sans-serif if none are installed.
    pub font_families: Vec<&'static str>,
    pub font_size: f32,
    pub control_height: f32,
    pub corner_radius: f32,
    pub menubar_height: f32,
    pub spacing: f32,
}

impl Theme {
    pub fn for_platform(platform: Platform, dark: bool) -> Theme {
        match platform {
            Platform::MacOs => macos(dark),
            Platform::Windows => windows(dark),
            Platform::Linux => linux(dark),
        }
    }

    pub fn current(dark: bool) -> Theme {
        Theme::for_platform(Platform::current(), dark)
    }
}

fn macos(dark: bool) -> Theme {
    let accent = Color::hex(0x007AFF);
    let palette = if dark {
        Palette {
            window_bg: Color::hex(0x282828),
            text: Color::rgba(1.0, 1.0, 1.0, 0.85),
            text_secondary: Color::rgba(1.0, 1.0, 1.0, 0.55),
            text_disabled: Color::rgba(1.0, 1.0, 1.0, 0.25),
            control_bg: Color::hex(0x656565),
            control_bg_hover: Color::hex(0x6E6E6E),
            control_bg_pressed: Color::hex(0x7A7A7A),
            control_border: Color::rgba(0.0, 0.0, 0.0, 0.3),
            field_bg: Color::hex(0x1E1E1E),
            accent,
            accent_text: Color::WHITE,
            focus_ring: accent.with_alpha(0.6),
            menubar_bg: Color::hex(0x2B2B2B),
            menu_bg: Color::hex(0x323232),
            menu_border: Color::rgba(1.0, 1.0, 1.0, 0.15),
            separator: Color::rgba(1.0, 1.0, 1.0, 0.15),
            shadow: Color::rgba(0.0, 0.0, 0.0, 0.5),
        }
    } else {
        Palette {
            window_bg: Color::hex(0xECECEC),
            text: Color::rgba(0.0, 0.0, 0.0, 0.85),
            text_secondary: Color::rgba(0.0, 0.0, 0.0, 0.5),
            text_disabled: Color::rgba(0.0, 0.0, 0.0, 0.25),
            control_bg: Color::WHITE,
            control_bg_hover: Color::hex(0xF7F7F7),
            control_bg_pressed: Color::hex(0xE0E0E0),
            control_border: Color::rgba(0.0, 0.0, 0.0, 0.18),
            field_bg: Color::WHITE,
            accent,
            accent_text: Color::WHITE,
            focus_ring: accent.with_alpha(0.5),
            menubar_bg: Color::hex(0xE8E8E8),
            menu_bg: Color::hex(0xF6F6F6),
            menu_border: Color::rgba(0.0, 0.0, 0.0, 0.15),
            separator: Color::rgba(0.0, 0.0, 0.0, 0.12),
            shadow: Color::rgba(0.0, 0.0, 0.0, 0.25),
        }
    };
    Theme {
        platform: Platform::MacOs,
        dark,
        palette,
        font_families: vec![".AppleSystemUIFont", "SF Pro Text", "Helvetica Neue", "Helvetica"],
        font_size: 13.0,
        control_height: 22.0,
        corner_radius: 6.0,
        menubar_height: 0.0, // native global menu bar
        spacing: 8.0,
    }
}

fn windows(dark: bool) -> Theme {
    let accent = Color::hex(0x0067C0);
    let palette = if dark {
        Palette {
            window_bg: Color::hex(0x202020),
            text: Color::WHITE,
            text_secondary: Color::rgba(1.0, 1.0, 1.0, 0.6),
            text_disabled: Color::rgba(1.0, 1.0, 1.0, 0.36),
            control_bg: Color::rgba(1.0, 1.0, 1.0, 0.06),
            control_bg_hover: Color::rgba(1.0, 1.0, 1.0, 0.08),
            control_bg_pressed: Color::rgba(1.0, 1.0, 1.0, 0.03),
            control_border: Color::rgba(1.0, 1.0, 1.0, 0.09),
            field_bg: Color::rgba(1.0, 1.0, 1.0, 0.06),
            accent: Color::hex(0x4CC2FF),
            accent_text: Color::BLACK,
            focus_ring: Color::WHITE,
            menubar_bg: Color::hex(0x202020),
            menu_bg: Color::hex(0x2C2C2C),
            menu_border: Color::rgba(1.0, 1.0, 1.0, 0.1),
            separator: Color::rgba(1.0, 1.0, 1.0, 0.1),
            shadow: Color::rgba(0.0, 0.0, 0.0, 0.5),
        }
    } else {
        Palette {
            window_bg: Color::hex(0xF3F3F3),
            text: Color::rgba(0.0, 0.0, 0.0, 0.9),
            text_secondary: Color::rgba(0.0, 0.0, 0.0, 0.6),
            text_disabled: Color::rgba(0.0, 0.0, 0.0, 0.36),
            control_bg: Color::rgba(1.0, 1.0, 1.0, 0.7),
            control_bg_hover: Color::rgba(0.976, 0.976, 0.976, 0.5),
            control_bg_pressed: Color::rgba(0.976, 0.976, 0.976, 0.3),
            control_border: Color::rgba(0.0, 0.0, 0.0, 0.06),
            field_bg: Color::rgba(1.0, 1.0, 1.0, 0.7),
            accent,
            accent_text: Color::WHITE,
            focus_ring: Color::BLACK,
            menubar_bg: Color::hex(0xF3F3F3),
            menu_bg: Color::hex(0xF9F9F9),
            menu_border: Color::rgba(0.0, 0.0, 0.0, 0.08),
            separator: Color::rgba(0.0, 0.0, 0.0, 0.08),
            shadow: Color::rgba(0.0, 0.0, 0.0, 0.2),
        }
    };
    Theme {
        platform: Platform::Windows,
        dark,
        palette,
        font_families: vec!["Segoe UI Variable Text", "Segoe UI"],
        font_size: 14.0,
        control_height: 32.0,
        corner_radius: 4.0,
        menubar_height: 32.0,
        spacing: 8.0,
    }
}

fn linux(dark: bool) -> Theme {
    // Adwaita (GNOME) colours.
    let accent = Color::hex(0x3584E4);
    let palette = if dark {
        Palette {
            window_bg: Color::hex(0x242424),
            text: Color::WHITE,
            text_secondary: Color::rgba(1.0, 1.0, 1.0, 0.7),
            text_disabled: Color::rgba(1.0, 1.0, 1.0, 0.5),
            control_bg: Color::rgba(1.0, 1.0, 1.0, 0.1),
            control_bg_hover: Color::rgba(1.0, 1.0, 1.0, 0.15),
            control_bg_pressed: Color::rgba(1.0, 1.0, 1.0, 0.3),
            control_border: Color::TRANSPARENT,
            field_bg: Color::rgba(1.0, 1.0, 1.0, 0.08),
            accent,
            accent_text: Color::WHITE,
            focus_ring: accent.with_alpha(0.5),
            menubar_bg: Color::hex(0x303030),
            menu_bg: Color::hex(0x383838),
            menu_border: Color::rgba(1.0, 1.0, 1.0, 0.1),
            separator: Color::rgba(1.0, 1.0, 1.0, 0.1),
            shadow: Color::rgba(0.0, 0.0, 0.0, 0.5),
        }
    } else {
        Palette {
            window_bg: Color::hex(0xFAFAFA),
            text: Color::rgba(0.0, 0.0, 0.0, 0.8),
            text_secondary: Color::rgba(0.0, 0.0, 0.0, 0.55),
            text_disabled: Color::rgba(0.0, 0.0, 0.0, 0.4),
            control_bg: Color::rgba(0.0, 0.0, 0.0, 0.08),
            control_bg_hover: Color::rgba(0.0, 0.0, 0.0, 0.12),
            control_bg_pressed: Color::rgba(0.0, 0.0, 0.0, 0.24),
            control_border: Color::TRANSPARENT,
            field_bg: Color::rgba(0.0, 0.0, 0.0, 0.05),
            accent,
            accent_text: Color::WHITE,
            focus_ring: accent.with_alpha(0.5),
            menubar_bg: Color::hex(0xEBEBEB),
            menu_bg: Color::WHITE,
            menu_border: Color::rgba(0.0, 0.0, 0.0, 0.12),
            separator: Color::rgba(0.0, 0.0, 0.0, 0.1),
            shadow: Color::rgba(0.0, 0.0, 0.0, 0.2),
        }
    };
    Theme {
        platform: Platform::Linux,
        dark,
        palette,
        font_families: vec!["Cantarell", "Adwaita Sans", "Inter", "Noto Sans", "DejaVu Sans"],
        font_size: 14.7, // 11pt at 96 dpi
        control_height: 34.0,
        corner_radius: 6.0,
        menubar_height: 30.0,
        spacing: 8.0,
    }
}
