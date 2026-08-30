//! A compact reusable HSV colour picker: saturation/value square, hue
//! bar, and a typeable hex field. Stateless w.r.t. the colour itself —
//! the host owns the `[u8; 4]` and applies whatever this returns.

use std::cell::{Cell, RefCell};

use rustle_ui::prelude::*;

use super::render::{hsv_to_rgb, rgb_to_hsv};
use super::theme::*;

const SV: u32 = 96;
const BAR_H: f32 = 14.0;
const HEX_H: f32 = 22.0;

#[derive(Clone, Copy, Default)]
struct Geom {
    sv: Rect,
    hue: Rect,
    hex: Rect,
}

pub struct HsvPanel {
    hue: Cell<f32>,
    drag: Cell<u8>, // 0 none, 1 sv, 2 hue
    sv_cache: RefCell<Option<(u32, ImageData)>>,
    hue_img: RefCell<Option<ImageData>>,
    hex_edit: RefCell<Option<String>>,
    geom: Cell<Geom>,
}

impl HsvPanel {
    pub fn new() -> Self {
        Self {
            hue: Cell::new(0.0),
            drag: Cell::new(0),
            sv_cache: RefCell::new(None),
            hue_img: RefCell::new(None),
            hex_edit: RefCell::new(None),
            geom: Cell::new(Geom::default()),
        }
    }

    pub fn height(&self, w: f32) -> f32 {
        let side = w.min(160.0);
        side + BAR_H + HEX_H + 22.0
    }

    fn sv_image(&self) -> std::cell::Ref<'_, Option<(u32, ImageData)>> {
        let key = (self.hue.get() * 1000.0) as u32;
        if !matches!(&*self.sv_cache.borrow(), Some((k, _)) if *k == key) {
            let mut buf = vec![0u8; (SV * SV * 4) as usize];
            for y in 0..SV {
                for x in 0..SV {
                    let s = x as f32 / (SV - 1) as f32;
                    let v = 1.0 - y as f32 / (SV - 1) as f32;
                    let [r, g, b] = hsv_to_rgb(self.hue.get(), s, v);
                    let i = ((y * SV + x) * 4) as usize;
                    buf[i..i + 4].copy_from_slice(&[r, g, b, 255]);
                }
            }
            *self.sv_cache.borrow_mut() = Some((key, ImageData::from_rgba(SV, SV, buf)));
        }
        self.sv_cache.borrow()
    }

    fn hue_image(&self) -> std::cell::Ref<'_, Option<ImageData>> {
        if self.hue_img.borrow().is_none() {
            let w = 240u32;
            let mut buf = vec![0u8; (w * 4) as usize];
            for x in 0..w {
                let [r, g, b] = hsv_to_rgb(x as f32 / w as f32, 1.0, 1.0);
                let i = (x * 4) as usize;
                buf[i..i + 4].copy_from_slice(&[r, g, b, 255]);
            }
            *self.hue_img.borrow_mut() = Some(ImageData::from_rgba(w, 1, buf));
        }
        self.hue_img.borrow()
    }

    /// Draw at `(ox, oy)` for width `w`, showing `color`.
    pub fn draw(&self, r: &mut Renderer, ox: f32, oy: f32, w: f32, color: [u8; 4]) {
        if self.drag.get() == 0 && self.hex_edit.borrow().is_none() {
            let (h, s, _) = rgb_to_hsv(color);
            if s > 0.02 {
                self.hue.set(h);
            }
        }

        let side = w.min(160.0);
        let sv = Rect::new(ox, oy, side, side);
        if let Some((_, img)) = &*self.sv_image() {
            r.image(sv, img);
        }
        let (_, s, v) = rgb_to_hsv(color);
        ring(r, sv.x + s * sv.width, sv.y + (1.0 - v) * sv.height);

        let hue = Rect::new(ox, oy + side + 6.0, w, BAR_H);
        if let Some(img) = &*self.hue_image() {
            r.image(hue, img);
        }
        let hx = hue.x + self.hue.get().clamp(0.0, 1.0) * hue.width;
        r.fill_rect(Rect::new(hx - 1.0, hue.y - 2.0, 2.0, hue.height + 4.0), Color::WHITE);
        r.fill_rect(Rect::new(hx - 2.0, hue.y - 2.0, 1.0, hue.height + 4.0), Color::BLACK);
        r.fill_rect(Rect::new(hx + 1.0, hue.y - 2.0, 1.0, hue.height + 4.0), Color::BLACK);

        let hex = Rect::new(ox, hue.y + BAR_H + 8.0, w, HEX_H);
        r.fill_rounded_rect(hex, 4.0, FIELD_BG);
        let shown = match &*self.hex_edit.borrow() {
            Some(buf) => format!("#{buf}"),
            None => hex_string(color),
        };
        text(r, &shown, hex.x + 8.0, hex.y + 5.0, 12.0, INK);
        if self.hex_edit.borrow().is_some() {
            let cw = r.measure(&shown, &TextStyle { size: 12.0, color: INK, font: FontId::DEFAULT });
            r.fill_rect(Rect::new(hex.x + 8.0 + cw + 1.0, hex.y + 4.0, 1.0, HEX_H - 8.0), ACCENT);
        }
        // colour chip on the right of the hex row
        let chip = Rect::new(hex.x + hex.width - 26.0, hex.y + 3.0, 22.0, HEX_H - 6.0);
        r.fill_rect(chip, rgba(color));

        self.geom.set(Geom { sv, hue, hex });
    }

    fn apply_sv(&self, lx: f32, ly: f32, g: Geom, alpha: u8) -> [u8; 4] {
        let s = ((lx - g.sv.x) / g.sv.width).clamp(0.0, 1.0);
        let v = (1.0 - (ly - g.sv.y) / g.sv.height).clamp(0.0, 1.0);
        let [r, gg, b] = hsv_to_rgb(self.hue.get(), s, v);
        [r, gg, b, alpha]
    }

    pub fn on_down(&self, lx: f32, ly: f32, color: [u8; 4]) -> Option<[u8; 4]> {
        let g = self.geom.get();
        *self.hex_edit.borrow_mut() = None;
        if g.sv.contains(lx, ly) {
            self.drag.set(1);
            return Some(self.apply_sv(lx, ly, g, color[3]));
        }
        if g.hue.contains(lx, ly) {
            self.drag.set(2);
            let hh = ((lx - g.hue.x) / g.hue.width).clamp(0.0, 1.0);
            self.hue.set(hh);
            let (_, s, v) = rgb_to_hsv(color);
            let [r, gg, b] = hsv_to_rgb(hh, s.max(0.001), v.max(0.001));
            return Some([r, gg, b, color[3]]);
        }
        if g.hex.contains(lx, ly) {
            *self.hex_edit.borrow_mut() = Some(hex_string(color).trim_start_matches('#').to_string());
        }
        None
    }

    pub fn on_move(&self, lx: f32, ly: f32, color: [u8; 4]) -> Option<[u8; 4]> {
        let g = self.geom.get();
        match self.drag.get() {
            1 => Some(self.apply_sv(lx, ly, g, color[3])),
            2 => {
                let hh = ((lx - g.hue.x) / g.hue.width).clamp(0.0, 1.0);
                self.hue.set(hh);
                let (_, s, v) = rgb_to_hsv(color);
                let [r, gg, b] = hsv_to_rgb(hh, s.max(0.001), v.max(0.001));
                Some([r, gg, b, color[3]])
            }
            _ => None,
        }
    }

    pub fn on_up(&self) {
        self.drag.set(0);
    }


    pub fn on_key(&self, event: &KeyboardEvent, color: [u8; 4]) -> Option<[u8; 4]> {
        let mut edit = self.hex_edit.borrow_mut();
        let Some(buf) = edit.as_mut() else { return None };
        match event {
            KeyboardEvent::TextInput(c) if c.is_ascii_hexdigit() && buf.len() < 6 => {
                buf.push(c.to_ascii_uppercase());
                None
            }
            KeyboardEvent::KeyDown { key: Key::Backspace, .. } => {
                buf.pop();
                None
            }
            KeyboardEvent::KeyDown { key: Key::Enter, .. } => {
                let parsed = parse_hex6(buf);
                drop(edit);
                *self.hex_edit.borrow_mut() = None;
                parsed.map(|[r, g, b]| {
                    self.hue.set(rgb_to_hsv([r, g, b, 255]).0);
                    [r, g, b, color[3]]
                })
            }
            KeyboardEvent::KeyDown { key: Key::Escape, .. } => {
                drop(edit);
                *self.hex_edit.borrow_mut() = None;
                None
            }
            _ => None,
        }
    }
}

fn parse_hex6(s: &str) -> Option<[u8; 3]> {
    if s.len() != 6 {
        return None;
    }
    let v = u32::from_str_radix(s, 16).ok()?;
    Some([(v >> 16) as u8, (v >> 8) as u8, v as u8])
}

fn ring(r: &mut Renderer, x: f32, y: f32) {
    for (dx, dy, w, h) in [(-5.0, -5.0, 10.0, 1.0), (-5.0, 4.0, 10.0, 1.0), (-5.0, -5.0, 1.0, 10.0), (4.0, -5.0, 1.0, 11.0)] {
        r.fill_rect(Rect::new(x + dx, y + dy, w, h), Color::WHITE);
    }
}
