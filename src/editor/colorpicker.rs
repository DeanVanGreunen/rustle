//! The "Swatches & Preview" tab body: a full HSV colour picker
//! (saturation/value square + hue bar + alpha bar), two hex readouts for
//! the foreground / background colour, and the swatch row.

use std::cell::{Cell, RefCell};

use rustle_ui::prelude::*;

use super::render::{hsv_to_rgb, rgb_to_hsv};
use super::theme::*;
use super::Editor;

const PAD: f32 = 14.0;
const BAR_H: f32 = 16.0;
const SV_RES: u32 = 128;

pub struct ColorPicker {
    editor: Editor,
    /// Editing foreground (false) or background (true).
    target_bg: Cell<bool>,
    hue: Cell<f32>,
    sv_cache: RefCell<Option<(u32, ImageData)>>,
    hue_img: RefCell<Option<ImageData>>,
    drag: Cell<u8>, // 0 none, 1 sv, 2 hue, 3 alpha
    // geometry captured at render for hit-testing
    geom: Cell<Geom>,
    /// Hex being typed: `(is_background, digits without '#')`.
    editing: Option<(bool, String)>,
}

fn parse_hex6(s: &str) -> Option<[u8; 3]> {
    if s.len() != 6 {
        return None;
    }
    let v = u32::from_str_radix(s, 16).ok()?;
    Some([(v >> 16) as u8, (v >> 8) as u8, v as u8])
}

#[derive(Clone, Copy, Default)]
struct Geom {
    sv: Rect,
    hue: Rect,
    alpha: Rect,
    fg_row: Rect,
    bg_row: Rect,
    add: Rect,
    swatch0_x: f32,
    swatch_y: f32,
    swatch_n: usize,
}

impl ColorPicker {
    pub fn new(editor: Editor) -> Self {
        let hue = editor
            .session(|s| rgb_to_hsv(s.colors.foreground).0)
            .unwrap_or(0.0);
        Self {
            editor,
            target_bg: Cell::new(false),
            hue: Cell::new(hue),
            sv_cache: RefCell::new(None),
            hue_img: RefCell::new(None),
            drag: Cell::new(0),
            geom: Cell::new(Geom::default()),
            editing: None,
        }
    }

    fn active(&self) -> [u8; 4] {
        let bg = self.target_bg.get();
        self.editor
            .session(|s| if bg { s.colors.background } else { s.colors.foreground })
            .unwrap_or([255, 255, 255, 255])
    }

    fn set_active(&self, c: [u8; 4]) {
        let bg = self.target_bg.get();
        self.editor.edit_session(move |s| {
            if bg {
                s.colors.background = c;
            } else {
                s.colors.foreground = c;
            }
        });
    }

    fn sv_image(&self) -> std::cell::Ref<'_, Option<(u32, ImageData)>> {
        let key = (self.hue.get() * 1000.0) as u32;
        let stale = !matches!(&*self.sv_cache.borrow(), Some((k, _)) if *k == key);
        if stale {
            let mut buf = vec![0u8; (SV_RES * SV_RES * 4) as usize];
            for y in 0..SV_RES {
                for x in 0..SV_RES {
                    let s = x as f32 / (SV_RES - 1) as f32;
                    let v = 1.0 - y as f32 / (SV_RES - 1) as f32;
                    let [r, g, b] = hsv_to_rgb(self.hue.get(), s, v);
                    let i = ((y * SV_RES + x) * 4) as usize;
                    buf[i..i + 4].copy_from_slice(&[r, g, b, 255]);
                }
            }
            *self.sv_cache.borrow_mut() = Some((key, ImageData::from_rgba(SV_RES, SV_RES, buf)));
        }
        self.sv_cache.borrow()
    }

    fn hue_image(&self) -> std::cell::Ref<'_, Option<ImageData>> {
        if self.hue_img.borrow().is_none() {
            let w = 256u32;
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
}

impl Behavior for ColorPicker {
    fn render(&mut self, ctx: &mut RenderContext) {
        let (w, h) = ctx.size();
        let r = &mut *ctx.renderer;
        r.fill_rect(Rect::new(0.0, 0.0, w, h), PANEL_BG_ALT);

        // Keep the SV square's hue in sync with the active colour (e.g.
        // after an eyedropper pick) unless the user is mid-interaction.
        if self.drag.get() == 0 && self.editing.is_none() {
            let (hh, s, _) = rgb_to_hsv(self.active());
            if s > 0.02 {
                self.hue.set(hh);
            }
        }

        let inner = w - PAD * 2.0;
        let mut y = PAD;

        // SV square
        let side = inner.min(h - 210.0).max(80.0);
        let sv = Rect::new(PAD, y, side, side);
        if let Some((_, img)) = &*self.sv_image() {
            r.image(sv, img);
        }
        let c = self.active();
        let (_, s, v) = rgb_to_hsv(c);
        let mx = sv.x + s * sv.width;
        let my = sv.y + (1.0 - v) * sv.height;
        ring(r, mx, my);
        y += side + 12.0;

        // hue bar
        let hue = Rect::new(PAD, y, inner, BAR_H);
        if let Some(img) = &*self.hue_image() {
            r.image(hue, img);
        }
        marker_v(r, hue, self.hue.get());
        y += BAR_H + 10.0;

        // alpha bar
        let alpha = Rect::new(PAD, y, inner, BAR_H);
        checkerboard(r, alpha, 8.0);
        let slices = alpha.width as i32;
        for i in 0..slices {
            let a = i as f32 / slices as f32;
            r.fill_rect(
                Rect::new(alpha.x + i as f32, alpha.y, 1.0, alpha.height),
                Color::rgba(c[0] as f32 / 255.0, c[1] as f32 / 255.0, c[2] as f32 / 255.0, a),
            );
        }
        marker_v(r, alpha, c[3] as f32 / 255.0);
        y += BAR_H + 16.0;

        // fg / bg rows
        let fg_row = Rect::new(PAD, y, inner, 34.0);
        let bg_row = Rect::new(PAD, y + 40.0, inner, 34.0);
        let fg_edit = match &self.editing {
            Some((false, buf)) => Some(buf.as_str()),
            _ => None,
        };
        let bg_edit = match &self.editing {
            Some((true, buf)) => Some(buf.as_str()),
            _ => None,
        };
        color_row(r, fg_row, "Foreground Color", self.editor.session(|s| s.colors.foreground).unwrap_or_default(), !self.target_bg.get(), fg_edit);
        color_row(r, bg_row, "Background Color", self.editor.session(|s| s.colors.background).unwrap_or_default(), self.target_bg.get(), bg_edit);
        y += 84.0;

        // swatches
        let hy = section_header(r, "Color Swatches", PAD, y, inner);
        let add = Rect::new(w - PAD - 20.0, y - 2.0, 20.0, 18.0);
        r.fill_rounded_rect(add, 4.0, FIELD_BG);
        text(r, "+", add.x + 6.0, y + 1.0, 13.0, INK);

        let swatches = self.editor.session(|s| s.colors.swatches.clone()).unwrap_or_default();
        let sw = 22.0;
        let sy = hy + 2.0;
        for (i, col) in swatches.iter().enumerate() {
            let sx = PAD + i as f32 * (sw + 6.0);
            let rect = Rect::new(sx, sy, sw, sw);
            checkerboard(r, rect, 6.0);
            r.fill_rect(rect, rgba(*col));
            r.fill_rect(Rect::new(rect.x, rect.y, rect.width, 1.0), BORDER);
        }

        self.geom.set(Geom {
            sv,
            hue,
            alpha,
            fg_row,
            bg_row,
            add,
            swatch0_x: PAD,
            swatch_y: sy,
            swatch_n: swatches.len(),
        });
    }

    fn pointer_event(&mut self, ctx: &mut EventContext, event: PointerEvent) {
        let b = ctx.ui.absolute_box(ctx.node);
        let g = self.geom.get();
        let to_local = |x: f32, y: f32| (x - b.x, y - b.y);

        let apply_sv = |this: &Self, lx: f32, ly: f32| {
            let s = ((lx - g.sv.x) / g.sv.width).clamp(0.0, 1.0);
            let v = (1.0 - (ly - g.sv.y) / g.sv.height).clamp(0.0, 1.0);
            let a = this.active()[3];
            let [r, gg, bb] = hsv_to_rgb(this.hue.get(), s, v);
            this.set_active([r, gg, bb, a]);
        };
        let apply_hue = |this: &Self, lx: f32| {
            let hh = ((lx - g.hue.x) / g.hue.width).clamp(0.0, 1.0);
            this.hue.set(hh);
            let c = this.active();
            let (_, s, v) = rgb_to_hsv(c);
            let [r, gg, bb] = hsv_to_rgb(hh, s.max(0.0001), v.max(0.0001));
            this.set_active([r, gg, bb, c[3]]);
        };
        let apply_alpha = |this: &Self, lx: f32| {
            let a = (((lx - g.alpha.x) / g.alpha.width).clamp(0.0, 1.0) * 255.0) as u8;
            let mut c = this.active();
            c[3] = a;
            this.set_active(c);
        };

        match event {
            PointerEvent::Down { button: MouseButton::Left, x, y } => {
                let (lx, ly) = to_local(x, y);
                self.editing = None;
                if g.sv.contains(lx, ly) {
                    self.drag.set(1);
                    apply_sv(self, lx, ly);
                } else if g.hue.contains(lx, ly) {
                    self.drag.set(2);
                    apply_hue(self, lx);
                } else if g.alpha.contains(lx, ly) {
                    self.drag.set(3);
                    apply_alpha(self, lx);
                } else if g.fg_row.contains(lx, ly) {
                    self.target_bg.set(false);
                    ctx.focus();
                    let cur = self.editor.session(|s| hex_string(s.colors.foreground)).unwrap_or_default();
                    self.editing = Some((false, cur.trim_start_matches('#').to_string()));
                } else if g.bg_row.contains(lx, ly) {
                    self.target_bg.set(true);
                    ctx.focus();
                    let cur = self.editor.session(|s| hex_string(s.colors.background)).unwrap_or_default();
                    self.editing = Some((true, cur.trim_start_matches('#').to_string()));
                } else if g.add.contains(lx, ly) {
                    let c = self.active();
                    self.editor.edit_session(move |s| {
                        if !s.colors.swatches.contains(&c) {
                            s.colors.swatches.push(c);
                        }
                    });
                } else {
                    let sw = 22.0;
                    for i in 0..g.swatch_n {
                        let sx = g.swatch0_x + i as f32 * (sw + 6.0);
                        if lx >= sx && lx <= sx + sw && ly >= g.swatch_y && ly <= g.swatch_y + sw {
                            if let Some(Some(col)) =
                                self.editor.session(move |s| s.colors.swatches.get(i).copied())
                            {
                                self.hue.set(rgb_to_hsv(col).0);
                                self.set_active(col);
                            }
                        }
                    }
                }
                ctx.stop_propagation();
            }
            PointerEvent::Move { x, y } => {
                let (lx, ly) = to_local(x, y);
                match self.drag.get() {
                    1 => apply_sv(self, lx, ly),
                    2 => apply_hue(self, lx),
                    3 => apply_alpha(self, lx),
                    _ => {}
                }
            }
            PointerEvent::Up { .. } => self.drag.set(0),
            _ => {}
        }
    }

    fn keyboard_event(&mut self, ctx: &mut EventContext, event: KeyboardEvent) {
        let Some((is_bg, mut buf)) = self.editing.take() else { return };
        let mut keep = true;
        match event {
            KeyboardEvent::TextInput(c) if c.is_ascii_hexdigit() && buf.len() < 6 => {
                buf.push(c.to_ascii_uppercase());
            }
            KeyboardEvent::KeyDown { key: Key::Backspace, .. } => {
                buf.pop();
            }
            KeyboardEvent::KeyDown { key: Key::Enter, .. } => {
                if let Some(rgb) = parse_hex6(&buf) {
                    self.target_bg.set(is_bg);
                    let a = self.active()[3];
                    self.set_active([rgb[0], rgb[1], rgb[2], a]);
                    self.hue.set(rgb_to_hsv([rgb[0], rgb[1], rgb[2], 255]).0);
                }
                keep = false;
            }
            KeyboardEvent::KeyDown { key: Key::Escape, .. } => keep = false,
            _ => {}
        }
        if keep {
            self.editing = Some((is_bg, buf));
        }
        ctx.stop_propagation();
    }

    fn focusable(&self) -> bool {
        true
    }
}

fn ring(r: &mut Renderer, x: f32, y: f32) {
    for (dx, dy, w, h) in [(-5.0, -5.0, 10.0, 1.0), (-5.0, 4.0, 10.0, 1.0), (-5.0, -5.0, 1.0, 10.0), (4.0, -5.0, 1.0, 11.0)] {
        r.fill_rect(Rect::new(x + dx, y + dy, w, h), Color::WHITE);
    }
}

fn marker_v(r: &mut Renderer, bar: Rect, t: f32) {
    let x = bar.x + t.clamp(0.0, 1.0) * bar.width;
    r.fill_rect(Rect::new(x - 2.0, bar.y - 2.0, 4.0, bar.height + 4.0), Color::BLACK);
    r.fill_rect(Rect::new(x - 1.0, bar.y - 1.0, 2.0, bar.height + 2.0), Color::WHITE);
}

fn color_row(
    r: &mut Renderer,
    rect: Rect,
    label: &str,
    col: [u8; 4],
    active: bool,
    editing: Option<&str>,
) {
    text(r, label, rect.x, rect.y, 11.0, DIM);
    let chip = Rect::new(rect.x, rect.y + 14.0, rect.width, 18.0);
    checkerboard(r, chip, 6.0);
    r.fill_rect(chip, rgba(col));
    if active {
        r.fill_rect(Rect::new(chip.x, chip.y, chip.width, 2.0), ACCENT);
    }
    let fg = contrast(col);
    match editing {
        Some(buf) => {
            let shown = format!("#{buf}");
            text(r, &shown, chip.x + 6.0, chip.y + 3.0, 11.0, fg);
            let cw = r.measure(&shown, &TextStyle { size: 11.0, color: fg, font: FontId::DEFAULT });
            r.fill_rect(Rect::new(chip.x + 6.0 + cw + 1.0, chip.y + 3.0, 1.0, 12.0), fg);
        }
        None => text(r, &hex_string(col), chip.x + 6.0, chip.y + 3.0, 11.0, fg),
    }
}

fn contrast(c: [u8; 4]) -> Color {
    let l = 0.299 * c[0] as f32 + 0.587 * c[1] as f32 + 0.114 * c[2] as f32;
    if l > 140.0 { Color::BLACK } else { Color::WHITE }
}
