//! Shared colours and tiny draw helpers for the editor panels.

use rustle_ui::prelude::*;

pub const PANEL_BG: Color = Color::hex(0xFAFAFA);
pub const PANEL_BG_ALT: Color = Color::hex(0xFFFFFF);
pub const BORDER: Color = Color::hex(0xE4E4E4);
pub const INK: Color = Color::hex(0x2F2F2F);
pub const DIM: Color = Color::hex(0x8A8A8A);
pub const FAINT: Color = Color::hex(0xB4B4B4);
pub const ACCENT: Color = Color::hex(0x2F7FD8);
pub const ACCENT_BG: Color = Color::hex(0xE7F0FB);
pub const FIELD_BG: Color = Color::hex(0xF0F0F0);
pub const CHECKER_A: Color = Color::hex(0xC8C8C8);
pub const CHECKER_B: Color = Color::hex(0xEDEDED);

/// Left-aligned text; `y` is the visual top of the line.
pub fn text(r: &mut Renderer, s: &str, x: f32, y: f32, size: f32, color: Color) {
    r.text_styled(
        s,
        Vec2 { x, y: y + size * 0.82 },
        TextStyle { size, color, font: FontId::DEFAULT },
    );
}

/// Right-aligned text ending at `right`.
pub fn text_right(r: &mut Renderer, s: &str, right: f32, y: f32, size: f32, color: Color) {
    let w = r.measure(s, &TextStyle { size, color, font: FontId::DEFAULT });
    text(r, s, right - w, y, size, color);
}

/// A dim section header with a hairline under it. Returns the y below it.
pub fn section_header(r: &mut Renderer, s: &str, x: f32, y: f32, width: f32) -> f32 {
    text(r, &s.to_uppercase(), x, y + 3.0, 10.5, DIM);
    r.fill_rect(Rect::new(x, y + 20.0, width, 1.0), BORDER);
    y + 27.0
}

pub fn checkerboard(r: &mut Renderer, area: Rect, cell: f32) {
    r.fill_rect(area, CHECKER_B);
    let cols = (area.width / cell).ceil() as i32;
    let rows = (area.height / cell).ceil() as i32;
    for gy in 0..rows {
        for gx in 0..cols {
            if (gx + gy) % 2 == 0 {
                let x = area.x + gx as f32 * cell;
                let y = area.y + gy as f32 * cell;
                let w = (area.x + area.width - x).min(cell);
                let h = (area.y + area.height - y).min(cell);
                if w > 0.0 && h > 0.0 {
                    r.fill_rect(Rect::new(x, y, w, h), CHECKER_A);
                }
            }
        }
    }
}

pub fn rgba(c: [u8; 4]) -> Color {
    Color::rgba(
        c[0] as f32 / 255.0,
        c[1] as f32 / 255.0,
        c[2] as f32 / 255.0,
        c[3] as f32 / 255.0,
    )
}

pub fn hex_string(c: [u8; 4]) -> String {
    format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2])
}
