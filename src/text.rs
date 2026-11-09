//! Text shaping, layout and rasterisation via cosmic-text. One `TextSystem`
//! lives for the lifetime of the app and owns the system font database.

use cosmic_text::{Attrs, Buffer, Family, FamilyOwned, FontSystem, Metrics, Shaping, SwashCache, SwashContent};

use crate::theme::Color;

pub struct TextSystem {
    pub(crate) font_system: FontSystem,
    pub(crate) cache: SwashCache,
    family: FamilyOwned,
}

impl TextSystem {
    pub fn new(preferred_families: &[&str]) -> Self {
        let font_system = FontSystem::new();
        let family = pick_family(&font_system, preferred_families);
        Self { font_system, cache: SwashCache::new(), family }
    }

    pub fn family_name(&self) -> String {
        match &self.family {
            FamilyOwned::Name(n) => n.clone(),
            other => format!("{other:?}"),
        }
    }

    /// Lay out `text` at `font_size` logical px. `max_width == None` means a single
    /// unbounded line.
    pub fn layout(&mut self, text: &str, font_size: f32, max_width: Option<f32>) -> Buffer {
        let line_height = (font_size * 1.3).round();
        let mut buffer = Buffer::new(&mut self.font_system, Metrics::new(font_size, line_height));
        buffer.set_size(&mut self.font_system, max_width, None);
        let attrs = Attrs::new().family(as_family(&self.family));
        buffer.set_text(&mut self.font_system, text, attrs, Shaping::Advanced);
        buffer.shape_until_scroll(&mut self.font_system, true);
        buffer
    }

    /// Width and height in logical px of `text` laid out on one line.
    pub fn measure(&mut self, text: &str, font_size: f32) -> (f32, f32) {
        let buffer = self.layout(text, font_size, None);
        measure_buffer(&buffer)
    }

    /// Horizontal advance (logical px) of the first `char_count` characters of `text`.
    /// Used for caret placement in single-line fields.
    pub fn prefix_width(&mut self, text: &str, char_count: usize, font_size: f32) -> f32 {
        let prefix: String = text.chars().take(char_count).collect();
        if prefix.is_empty() {
            return 0.0;
        }
        self.measure(&prefix, font_size).0
    }

    /// Draw `buffer` with its top-left corner at logical (`x`, `y`) into a premultiplied
    /// RGBA8 pixmap of `pm_w`×`pm_h` physical pixels, scaled by `scale`.
    /// `clip` is a physical-pixel rect (x0, y0, x1, y1).
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &mut self,
        pixels: &mut [u8],
        pm_w: u32,
        pm_h: u32,
        clip: (i32, i32, i32, i32),
        buffer: &Buffer,
        x: f32,
        y: f32,
        scale: f32,
        color: Color,
    ) {
        for run in buffer.layout_runs() {
            let line_y_px = (run.line_y + y) * scale;
            for glyph in run.glyphs.iter() {
                let physical = glyph.physical((x * scale, 0.0), scale);
                let Some(image) = self.cache.get_image(&mut self.font_system, physical.cache_key) else {
                    continue;
                };
                let gx = physical.x + image.placement.left;
                let gy = line_y_px.round() as i32 + physical.y - image.placement.top;
                let w = image.placement.width as i32;
                let h = image.placement.height as i32;
                match image.content {
                    SwashContent::Mask => {
                        for row in 0..h {
                            for col in 0..w {
                                let a = image.data[(row * w + col) as usize] as f32 / 255.0;
                                if a <= 0.0 {
                                    continue;
                                }
                                blend(pixels, pm_w, pm_h, clip, gx + col, gy + row, color, a);
                            }
                        }
                    }
                    SwashContent::SubpixelMask | SwashContent::Color => {
                        for row in 0..h {
                            for col in 0..w {
                                let i = ((row * w + col) * 4) as usize;
                                let a = image.data[i + 3] as f32 / 255.0;
                                if a <= 0.0 {
                                    continue;
                                }
                                let c = if matches!(image.content, SwashContent::Color) {
                                    Color::rgb(
                                        image.data[i] as f32 / 255.0,
                                        image.data[i + 1] as f32 / 255.0,
                                        image.data[i + 2] as f32 / 255.0,
                                    )
                                } else {
                                    color
                                };
                                blend(pixels, pm_w, pm_h, clip, gx + col, gy + row, c, a);
                            }
                        }
                    }
                }
            }
        }
    }
}

pub fn measure_buffer(buffer: &Buffer) -> (f32, f32) {
    let mut w: f32 = 0.0;
    let mut h: f32 = 0.0;
    for run in buffer.layout_runs() {
        w = w.max(run.line_w);
        h = h.max(run.line_top + run.line_height);
    }
    (w.ceil(), h.ceil())
}

#[allow(clippy::too_many_arguments)]
#[inline]
fn blend(pixels: &mut [u8], pm_w: u32, pm_h: u32, clip: (i32, i32, i32, i32), x: i32, y: i32, c: Color, cov: f32) {
    if x < clip.0 || y < clip.1 || x >= clip.2 || y >= clip.3 {
        return;
    }
    if x < 0 || y < 0 || x >= pm_w as i32 || y >= pm_h as i32 {
        return;
    }
    let i = ((y as u32 * pm_w + x as u32) * 4) as usize;
    let a = (c.a * cov).clamp(0.0, 1.0);
    let inv = 1.0 - a;
    // destination is premultiplied
    let dr = pixels[i] as f32 / 255.0;
    let dg = pixels[i + 1] as f32 / 255.0;
    let db = pixels[i + 2] as f32 / 255.0;
    let da = pixels[i + 3] as f32 / 255.0;
    pixels[i] = ((c.r * a + dr * inv) * 255.0).round() as u8;
    pixels[i + 1] = ((c.g * a + dg * inv) * 255.0).round() as u8;
    pixels[i + 2] = ((c.b * a + db * inv) * 255.0).round() as u8;
    pixels[i + 3] = ((a + da * inv) * 255.0).round() as u8;
}

fn pick_family(fs: &FontSystem, preferred: &[&str]) -> FamilyOwned {
    let db = fs.db();
    for name in preferred {
        let found = db
            .faces()
            .any(|face| face.families.iter().any(|(n, _)| n.eq_ignore_ascii_case(name)));
        if found {
            return FamilyOwned::Name((*name).to_string());
        }
    }
    FamilyOwned::SansSerif
}

fn as_family(f: &FamilyOwned) -> Family<'_> {
    {
        match f {
            FamilyOwned::Name(n) => Family::Name(n.as_str()),
            FamilyOwned::Serif => Family::Serif,
            FamilyOwned::SansSerif => Family::SansSerif,
            FamilyOwned::Cursive => Family::Cursive,
            FamilyOwned::Fantasy => Family::Fantasy,
            FamilyOwned::Monospace => Family::Monospace,
        }
    }
}
