//! CPU text rasterisation for the Text tool. Loads whatever system
//! TrueType fonts it can find (fontdue) and bakes glyph coverage into a
//! layer.

use std::rc::Rc;

use fontdue::{Font, FontSettings};

/// Candidate font files to try, in order. Windows first, then common
/// Linux / macOS locations.
const CANDIDATES: &[&str] = &[
    "C:/Windows/Fonts/segoeui.ttf",
    "C:/Windows/Fonts/arial.ttf",
    "C:/Windows/Fonts/tahoma.ttf",
    "C:/Windows/Fonts/consola.ttf",
    "C:/Windows/Fonts/times.ttf",
    "C:/Windows/Fonts/comic.ttf",
    "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
    "/System/Library/Fonts/Supplemental/Arial.ttf",
    "/Library/Fonts/Arial.ttf",
];

fn stem(path: &str) -> String {
    path.rsplit(['/', '\\'])
        .next()
        .unwrap_or(path)
        .trim_end_matches(".ttf")
        .trim_end_matches(".TTF")
        .to_string()
}

#[derive(Clone)]
pub struct TextFont(Rc<Vec<(String, Font)>>);

impl TextFont {
    pub fn load() -> Self {
        let mut fonts = Vec::new();
        for path in CANDIDATES {
            if let Ok(bytes) = std::fs::read(path) {
                if let Ok(font) = Font::from_bytes(bytes, FontSettings::default()) {
                    fonts.push((stem(path), font));
                }
            }
        }
        Self(Rc::new(fonts))
    }

    pub fn available(&self) -> bool {
        !self.0.is_empty()
    }

    pub fn names(&self) -> Vec<String> {
        self.0.iter().map(|(n, _)| n.clone()).collect()
    }

    fn pick(&self, name: &str) -> Option<&Font> {
        if name.is_empty() {
            return self.0.first().map(|(_, f)| f);
        }
        self.0
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .or_else(|| self.0.first())
            .map(|(_, f)| f)
    }

    /// Rasterise `text` and blit it onto `pixels` (RGBA, `lw` x `lh`) with
    /// its top-left at `(ox, oy)`, in `color`.
    #[allow(clippy::too_many_arguments)]
    pub fn blit(
        &self,
        pixels: &mut [u8],
        lw: u32,
        lh: u32,
        font_name: &str,
        text: &str,
        ox: i32,
        oy: i32,
        size: f32,
        char_spacing: f32,
        line_spacing: f32,
        color: [u8; 4],
    ) {
        let Some(font) = self.pick(font_name) else { return };
        let line_h = size + line_spacing;
        let mut pen_y = oy as f32 + size;
        let mut pen_x = ox as f32;

        for ch in text.chars() {
            if ch == '\n' {
                pen_x = ox as f32;
                pen_y += line_h;
                continue;
            }
            let (metrics, bitmap) = font.rasterize(ch, size);
            let gx = pen_x + metrics.xmin as f32;
            let gy = pen_y - metrics.height as f32 - metrics.ymin as f32;

            for row in 0..metrics.height {
                for col in 0..metrics.width {
                    let cov = bitmap[row * metrics.width + col] as f32 / 255.0;
                    if cov <= 0.0 {
                        continue;
                    }
                    let px = (gx as i32) + col as i32;
                    let py = (gy as i32) + row as i32;
                    if px < 0 || py < 0 || px >= lw as i32 || py >= lh as i32 {
                        continue;
                    }
                    let i = ((py as u32 * lw + px as u32) * 4) as usize;
                    let sa = cov * (color[3] as f32 / 255.0);
                    let da = pixels[i + 3] as f32 / 255.0;
                    let oa = sa + da * (1.0 - sa);
                    if oa <= 0.0 {
                        continue;
                    }
                    for k in 0..3 {
                        let s = color[k] as f32 / 255.0;
                        let d = pixels[i + k] as f32 / 255.0;
                        pixels[i + k] =
                            (((s * sa + d * da * (1.0 - sa)) / oa).clamp(0.0, 1.0) * 255.0) as u8;
                    }
                    pixels[i + 3] = (oa.clamp(0.0, 1.0) * 255.0) as u8;
                }
            }
            pen_x += metrics.advance_width + char_spacing;
        }
    }
}
