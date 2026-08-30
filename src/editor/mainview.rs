//! Row 2 of the middle column: the main editing surface. Composites and
//! draws the current document state, and turns pointer input into edits
//! (paint, marquee, pan, zoom, select) against the shared app state.

use std::any::Any;

use rustle_core::{
    EditorMode, FrameId, LevelAccessory, LevelBackground, LevelTile, OnionSide, Project, Selection,
    SpriteCanvas, Tool,
};
use rustle_ui::prelude::*;
use rustle_ui::widgets::ViewportContent;

use super::render::{composite_canvas, composite_frame_with_base};
use super::theme::*;
use super::Editor;

enum Drag {
    None,
    Pan { start_pan: (f32, f32), start_mouse: (f32, f32) },
    Marquee { start: (f32, f32) },
    Paint { last: (f32, f32) },
    /// Line / Rectangle rubber-band; committed on release.
    Shape { start: (f32, f32) },
    /// Level: dragging the selected item; `grab` is cursor-minus-origin.
    MoveItem { grab: (f32, f32) },
}

pub struct MainViewport {
    editor: Editor,
    kind: EditorMode,
    cache: Option<ImageData>,
    cache_rev: u64,
    canvas: (u32, u32),
    drag: Drag,
    shape_end: Option<(f32, f32)>,
    /// Texel origin of an in-progress Text-tool edit.
    text_origin: Option<(f32, f32)>,
    /// Onion-skin ghost cache: (signature, image) for prev / next.
    onion: [Option<(u64, ImageData, (u32, u32))>; 2],
}

impl MainViewport {
    pub fn new(editor: Editor, kind: EditorMode) -> Self {
        Self {
            editor,
            kind,
            cache: None,
            cache_rev: u64::MAX,
            canvas: (0, 0),
            drag: Drag::None,
            shape_end: None,
            text_origin: None,
            onion: [None, None],
        }
    }

    fn commit_text(&mut self) {
        let Some(origin) = self.text_origin.take() else { return };
        self.editor.text_editing.set(false);
        let font = self.editor.text_font.clone();
        let params = self.editor.session(|s| {
            (
                s.tools.text.font.clone(),
                s.tools.text.text.clone(),
                s.tools.text.size.max(4) as f32,
                s.tools.text.char_spacing as f32,
                s.tools.text.line_spacing as f32,
                s.colors.foreground,
            )
        });
        let Some((fname, text, size, cs, ls, col)) = params else { return };
        if text.trim().is_empty() || !font.available() {
            return;
        }
        self.editor.bump_generation();
        let (ox, oy) = (origin.0.floor() as i32, origin.1.floor() as i32);
        self.with_layer(move |w, h, px| {
            font.blit(px, w, h, &fname, &text, ox, oy, size, cs, ls, col)
        });
        self.editor.edit_session(|s| s.tools.text.text.clear());
    }

    fn cancel_text(&mut self) {
        self.text_origin = None;
        self.editor.text_editing.set(false);
        self.editor.edit_session(|s| s.tools.text.text.clear());
    }

    fn view(&self) -> (f32, f32, f32) {
        self.editor
            .session(|s| (s.main_view.zoom, s.main_view.pan_x, s.main_view.pan_y))
            .unwrap_or((1.0, 0.0, 0.0))
    }

    fn set_view(&self, zoom: f32, pan_x: f32, pan_y: f32) {
        self.editor.edit_session(|s| {
            s.main_view.zoom = zoom.clamp(0.05, 64.0);
            s.main_view.pan_x = pan_x;
            s.main_view.pan_y = pan_y;
        });
    }

    fn texel(&self, mx: f32, my: f32) -> (f32, f32) {
        let (z, px, py) = self.view();
        ((mx - px) / z, (my - py) / z)
    }

    fn ensure_composite(&mut self) {
        let rev = self.editor.revision.get();
        if self.cache.is_some() && rev == self.cache_rev {
            return;
        }
        self.cache_rev = rev;
        self.cache = None;
        if let Some(Some((w, h, buf))) = self.editor.with_project(composite_canvas) {
            self.canvas = (w, h);
            self.cache = Some(ImageData::from_rgba(w, h, buf));
        }
    }

    fn zoom_about(&self, px: f32, py: f32, factor: f32) {
        let (z, panx, pany) = self.view();
        let nz = (z * factor).clamp(0.05, 64.0);
        let nx = px - (px - panx) / z * nz;
        let ny = py - (py - pany) / z * nz;
        self.set_view(nz, nx, ny);
    }

    /// Foreground colour with the pencil opacity folded into alpha.
    fn fg(&self) -> [u8; 4] {
        self.editor
            .session(|s| {
                let c = s.colors.foreground;
                let op = s.tools.pencil.opacity as u32;
                [c[0], c[1], c[2], ((c[3] as u32 * op) / 255) as u8]
            })
            .unwrap_or([0, 0, 0, 255])
    }

    fn brush_size(&self) -> i64 {
        self.editor.session(|s| s.tools.pencil.size.max(1) as i64).unwrap_or(1)
    }

    fn with_layer(&self, f: impl FnOnce(u32, u32, &mut [u8])) {
        self.editor.edit(|p| {
            if let Some(l) = p.session.active.layer.and_then(|k| p.layers.get_mut(k)) {
                f(l.width, l.height, &mut l.pixels);
            }
        });
    }

    fn marquee_clip(&self) -> Clip {
        match self.editor.main_marquee.get() {
            Some((x, y, w, h)) if w >= 0.5 && h >= 0.5 => Clip([
                x.floor() as i64,
                y.floor() as i64,
                (x + w).ceil() as i64 - 1,
                (y + h).ceil() as i64 - 1,
            ]),
            _ => Clip::ALL,
        }
    }

    fn paint(&self, tx: f32, ty: f32) {
        let (fg, size) = (self.fg(), self.brush_size());
        let clip = self.marquee_clip();
        self.with_layer(|w, h, px| {
            stamp(px, w, h, tx.floor() as i64, ty.floor() as i64, size, fg, clip)
        });
    }

    fn erase(&self, tx: f32, ty: f32) {
        let size = self.editor.session(|s| s.tools.eraser.size.max(1) as i64).unwrap_or(1);
        let clip = self.marquee_clip();
        self.with_layer(|w, h, px| {
            erase_stamp(px, w, h, tx.floor() as i64, ty.floor() as i64, size, clip)
        });
    }

    fn erase_stroke(&self, a: (f32, f32), b: (f32, f32)) {
        let size = self.editor.session(|s| s.tools.eraser.size.max(1) as i64).unwrap_or(1);
        let clip = self.marquee_clip();
        self.with_layer(|w, h, px| {
            let (mut x0, mut y0) = (a.0.floor() as i64, a.1.floor() as i64);
            let (x1, y1) = (b.0.floor() as i64, b.1.floor() as i64);
            let dx = (x1 - x0).abs();
            let dy = -(y1 - y0).abs();
            let sx = if x0 < x1 { 1 } else { -1 };
            let sy = if y0 < y1 { 1 } else { -1 };
            let mut err = dx + dy;
            loop {
                erase_stamp(px, w, h, x0, y0, size, clip);
                if x0 == x1 && y0 == y1 {
                    break;
                }
                let e2 = 2 * err;
                if e2 >= dy {
                    err += dy;
                    x0 += sx;
                }
                if e2 <= dx {
                    err += dx;
                    y0 += sy;
                }
            }
        });
    }

    fn paint_stroke(&self, a: (f32, f32), b: (f32, f32)) {
        let (fg, size) = (self.fg(), self.brush_size());
        let clip = self.marquee_clip();
        self.with_layer(|w, h, px| {
            line(
                px,
                w,
                h,
                a.0.floor() as i64,
                a.1.floor() as i64,
                b.0.floor() as i64,
                b.1.floor() as i64,
                size,
                fg,
                clip,
            )
        });
    }

    fn sample(&mut self, tx: f32, ty: f32) {
        self.ensure_composite();
        let Some(img) = &self.cache else { return };
        let (x, y) = (tx.floor() as i64, ty.floor() as i64);
        if x < 0 || y < 0 || x >= img.width() as i64 || y >= img.height() as i64 {
            return;
        }
        let i = ((y * img.width() as i64 + x) * 4) as usize;
        let d = img.rgba();
        let c = [d[i], d[i + 1], d[i + 2], d[i + 3]];
        self.editor.edit_session(move |s| s.colors.foreground = c);
    }

    fn commit_shape(&self, a: (f32, f32), b: (f32, f32)) {
        let fg = self.fg();
        let tool = self.editor.tool();
        let clip = self.marquee_clip();
        let (ax, ay) = (a.0.floor() as i64, a.1.floor() as i64);
        let (bx, by) = (b.0.floor() as i64, b.1.floor() as i64);
        match tool {
            Tool::Line => {
                let width = self.editor.session(|s| s.tools.line.width.max(1) as i64).unwrap_or(1);
                self.with_layer(|w, h, px| line(px, w, h, ax, ay, bx, by, width, fg, clip));
            }
            Tool::Rectangle => {
                let (filled, stroke) = self
                    .editor
                    .session(|s| (s.tools.rectangle.filled, s.tools.rectangle.stroke.max(1) as i64))
                    .unwrap_or((false, 1));
                let (x0, x1) = (ax.min(bx), ax.max(bx));
                let (y0, y1) = (ay.min(by), ay.max(by));
                self.with_layer(|w, h, px| {
                    if filled {
                        for y in y0..=y1 {
                            for x in x0..=x1 {
                                put(px, w, h, x, y, fg, clip);
                            }
                        }
                    } else {
                        for s in 0..stroke {
                            line(px, w, h, x0, y0 + s, x1, y0 + s, 1, fg, clip);
                            line(px, w, h, x0, y1 - s, x1, y1 - s, 1, fg, clip);
                            line(px, w, h, x0 + s, y0, x0 + s, y1, 1, fg, clip);
                            line(px, w, h, x1 - s, y0, x1 - s, y1, 1, fg, clip);
                        }
                    }
                });
            }
            _ => {}
        }
    }

    fn flood_fill(&mut self, tx: f32, ty: f32) {
        // The region is chosen from the merged image the user actually
        // sees; the fill is written to the active layer only.
        self.ensure_composite();
        let fg = self.editor.session(|s| s.colors.foreground).unwrap_or([0, 0, 0, 255]);
        let tol = self.editor.session(|s| s.tools.fill.tolerance as i32).unwrap_or(0);
        let contiguous = self.editor.session(|s| s.tools.fill.contiguous).unwrap_or(true);
        let clip = self.marquee_clip();

        let Some((sw, sh, sbuf)) = self
            .cache
            .as_ref()
            .map(|img| (img.width() as i64, img.height() as i64, img.rgba().to_vec()))
        else {
            return;
        };
        let (sx, sy) = (tx.floor() as i64, ty.floor() as i64);
        if sx < 0 || sy < 0 || sx >= sw || sy >= sh {
            return;
        }
        let at = |x: i64, y: i64| {
            let i = ((y * sw + x) * 4) as usize;
            [sbuf[i], sbuf[i + 1], sbuf[i + 2], sbuf[i + 3]]
        };
        let target = at(sx, sy);
        let matches = |c: [u8; 4]| (0..4).all(|k| (c[k] as i32 - target[k] as i32).abs() <= tol);

        // Mask of pixels sharing the clicked colour (within tolerance).
        let mut mask = vec![false; (sw * sh) as usize];
        if contiguous {
            let mut stack = vec![(sx, sy)];
            while let Some((x, y)) = stack.pop() {
                if x < 0 || y < 0 || x >= sw || y >= sh {
                    continue;
                }
                let idx = (y * sw + x) as usize;
                if mask[idx] || !clip.allows(x, y) || !matches(at(x, y)) {
                    continue;
                }
                mask[idx] = true;
                stack.extend([(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)]);
            }
        } else {
            for y in 0..sh {
                for x in 0..sw {
                    if clip.allows(x, y) && matches(at(x, y)) {
                        mask[(y * sw + x) as usize] = true;
                    }
                }
            }
        }

        self.with_layer(|w, h, px| {
            let (w, h) = (w as i64, h as i64);
            for y in 0..h.min(sh) {
                for x in 0..w.min(sw) {
                    if mask[(y * sw + x) as usize] {
                        put(px, w as u32, h as u32, x, y, fg, clip);
                    }
                }
            }
        });
    }

    /// Level mode: stamp the selected tile/background/accessory definition
    /// at `(tx, ty)` and select the new placement.
    fn place_def(&self, tx: f32, ty: f32) {
        let sel = self.editor.selection();
        let (x, y) = (tx.floor(), ty.floor());
        self.editor.edit(|p| {
            let Some(lvl) = p.session.active.level else { return };
            if !p.levels.contains_key(lvl) {
                return;
            }
            let placed = match sel {
                Selection::Tile(tk) => {
                    let (w, h) = p.tiles.get(tk).map(|t| (t.width.max(1), t.height.max(1))).unwrap_or((16, 16));
                    let l = &mut p.levels[lvl];
                    l.tiles.push(LevelTile { tile: tk, x, y, width: w, height: h });
                    Selection::LevelTile { level: lvl, index: l.tiles.len() - 1 }
                }
                Selection::Background(bk) => {
                    let (w, h) = p.backgrounds.get(bk).map(|b| (b.width.max(1), b.height.max(1))).unwrap_or((64, 64));
                    let l = &mut p.levels[lvl];
                    l.backgrounds.push(LevelBackground { background: bk, x, y, width: w, height: h });
                    Selection::LevelBackground { level: lvl, index: l.backgrounds.len() - 1 }
                }
                Selection::Accessory(ak) => {
                    let (w, h) = p.accessories.get(ak).map(|a| (a.width.max(1), a.height.max(1))).unwrap_or((16, 16));
                    let l = &mut p.levels[lvl];
                    l.accessories.push(LevelAccessory { accessory: ak, x, y, width: w, height: h });
                    Selection::LevelAccessory { level: lvl, index: l.accessories.len() - 1 }
                }
                _ => return,
            };
            p.session.selection = placed;
        });
    }

    fn move_selected(&self, tx: f32, ty: f32, grab: (f32, f32)) {
        let (grid_on, gw, gh) = self
            .editor
            .session(|s| (s.grid.enabled, s.grid.width.max(1) as f32, s.grid.height.max(1) as f32))
            .unwrap_or((false, 1.0, 1.0));
        let tool_snap = self.editor.session(|s| s.tools.move_.snap).unwrap_or(false);
        let mut nx = tx - grab.0;
        let mut ny = ty - grab.1;
        if grid_on || tool_snap {
            nx = (nx / gw).round() * gw;
            ny = (ny / gh).round() * gh;
        }
        let sel = self.editor.selection();
        self.editor.edit(|p| match sel {
            Selection::LevelTile { level, index } => {
                if let Some(t) = p.levels.get_mut(level).and_then(|l| l.tiles.get_mut(index)) {
                    t.x = nx;
                    t.y = ny;
                }
            }
            Selection::LevelBackground { level, index } => {
                if let Some(b) = p.levels.get_mut(level).and_then(|l| l.backgrounds.get_mut(index)) {
                    b.x = nx;
                    b.y = ny;
                }
            }
            Selection::LevelAccessory { level, index } => {
                if let Some(a) = p.levels.get_mut(level).and_then(|l| l.accessories.get_mut(index)) {
                    a.x = nx;
                    a.y = ny;
                }
            }
            _ => {}
        });
    }

    /// When a sprite / canvas has just been opened, recentre the view and
    /// zoom so the artwork fills the viewport with an 8px margin.
    fn maybe_fit(&mut self, bounds: Rect) {
        if !self.editor.fit_request.get() {
            return;
        }
        self.ensure_composite();
        let (cw, ch) = self.canvas;
        if self.cache.is_none() || cw == 0 || ch == 0 {
            return;
        }
        self.editor.fit_request.set(false);
        let margin = 8.0;
        let avail_w = (bounds.width - margin * 2.0).max(1.0);
        let avail_h = (bounds.height - margin * 2.0).max(1.0);
        let z = (avail_w / cw as f32).min(avail_h / ch as f32).clamp(0.05, 64.0);
        let pan_x = (bounds.x + (bounds.width - cw as f32 * z) * 0.5).round();
        let pan_y = (bounds.y + (bounds.height - ch as f32 * z) * 0.5).round();
        self.editor.edit_session(|s| {
            s.main_view.zoom = z;
            s.main_view.pan_x = pan_x;
            s.main_view.pan_y = pan_y;
        });
    }

    fn draw_pixels(&mut self, r: &mut Renderer, bounds: Rect) {
        self.ensure_composite();
        let (z, px, py) = self.view();
        // Dark surround; the checkerboard only shows through the artwork.
        r.fill_rect(bounds, Color::hex(0x3F3F3F));

        let (cw, ch) = self.canvas;
        let have_img = self.cache.is_some();
        // Snap to the screen pixel grid so every texel is a crisp square
        // of `z` device pixels (nearest-neighbour sampling in the backend).
        let ox = px.round();
        let oy = py.round();
        let dw = if have_img { (cw as f32 * z).round() } else { 0.0 };
        let dh = if have_img { (ch as f32 * z).round() } else { 0.0 };
        let dst = Rect::new(ox, oy, dw, dh);

        if have_img {
            r.push_clip(dst);
            checkerboard(r, dst, 8.0);
            r.pop_clip();
        }

        self.ensure_onion();

        // Ghosts that draw under the current frame.
        self.draw_onion(r, ox, oy, z, OnionSide::Below);

        if let Some(img) = &self.cache {
            r.image(dst, img);
            for (x, y, w, h) in [
                (dst.x, dst.y, dst.width, 1.0),
                (dst.x, dst.y + dst.height - 1.0, dst.width, 1.0),
                (dst.x, dst.y, 1.0, dst.height),
                (dst.x + dst.width - 1.0, dst.y, 1.0, dst.height),
            ] {
                r.fill_rect(Rect::new(x, y, w, h), FAINT);
            }
        } else {
            text(r, "Nothing to display", 16.0, 16.0, 12.0, DIM);
        }

        // Ghosts that draw over the current frame.
        self.draw_onion(r, ox, oy, z, OnionSide::Above);

        self.draw_grid(r, bounds, px, py, z);
    }

    /// Prev / next animation frames around the active one.
    fn onion_neighbours(&self) -> (Option<FrameId>, Option<FrameId>) {
        self.editor
            .with_project(|p| {
                let SpriteCanvas::Frame(cur) = p.session.active.canvas else {
                    return (None, None);
                };
                let Some(anim) = p.session.active.animation.and_then(|k| p.animations.get(k)) else {
                    return (None, None);
                };
                let frames: Vec<FrameId> = anim.frames.iter().copied().collect();
                let Some(i) = frames.iter().position(|&f| f == cur) else {
                    return (None, None);
                };
                (
                    i.checked_sub(1).and_then(|j| frames.get(j).copied()),
                    frames.get(i + 1).copied(),
                )
            })
            .unwrap_or((None, None))
    }

    fn ensure_onion(&mut self) {
        let Some((on, prev_on, next_on, pc, nc, opacity)) = self.editor.session(|s| {
            let o = s.onion;
            (o.enabled, o.prev_enabled, o.next_enabled, o.prev_color, o.next_color, o.opacity)
        }) else {
            self.onion = [None, None];
            return;
        };
        if !on || self.kind == EditorMode::Level {
            self.onion = [None, None];
            return;
        }
        let (prev, next) = self.onion_neighbours();
        let rev = self.editor.revision.get();
        let a = (opacity.clamp(0.0, 1.0) * 255.0) as u8;

        for (slot, frame, col, want) in [
            (0usize, prev, pc, prev_on),
            (1usize, next, nc, next_on),
        ] {
            if frame.is_none() || !want {
                self.onion[slot] = None;
                continue;
            }
            let sig = rev
                ^ (slot as u64)
                ^ ((col[0] as u64) << 24 | (col[1] as u64) << 16 | (col[2] as u64) << 8 | a as u64);
            if matches!(&self.onion[slot], Some((cs, ..)) if *cs == sig) {
                continue;
            }
            let ghost = frame.and_then(|f| {
                self.editor
                    .with_project(|p| composite_frame_with_base(p, f, p.session.active.sprite))
                    .flatten()
            });
            self.onion[slot] = ghost.map(|(w, h, buf)| {
                let mut tinted = vec![0u8; buf.len()];
                for i in (0..buf.len()).step_by(4) {
                    if buf[i + 3] > 0 {
                        tinted[i] = col[0];
                        tinted[i + 1] = col[1];
                        tinted[i + 2] = col[2];
                        tinted[i + 3] = a;
                    }
                }
                (sig, ImageData::from_rgba(w, h, tinted), (w, h))
            });
        }
    }

    fn draw_onion(&self, r: &mut Renderer, ox: f32, oy: f32, z: f32, side: OnionSide) {
        let sides = self.editor.session(|s| (s.onion.prev_side, s.onion.next_side)).unwrap_or((OnionSide::Below, OnionSide::Below));
        for (slot, want_side) in [(0usize, sides.0), (1usize, sides.1)] {
            if want_side != side {
                continue;
            }
            if let Some((_, img, (w, h))) = &self.onion[slot] {
                r.image(Rect::new(ox, oy, (*w as f32 * z).round(), (*h as f32 * z).round()), img);
            }
        }
    }

    fn draw_grid(&self, r: &mut Renderer, bounds: Rect, px: f32, py: f32, z: f32) {
        let Some((on, gw, gh)) = self.editor.session(|s| (s.grid.enabled, s.grid.width.max(1), s.grid.height.max(1))) else {
            return;
        };
        if !on {
            return;
        }
        let sx = gw as f32 * z;
        let sy = gh as f32 * z;
        let col = Color::rgba(0.5, 0.5, 0.5, 0.35);
        if sx >= 4.0 {
            let mut gx = px.rem_euclid(sx);
            while gx < bounds.width {
                r.fill_rect(Rect::new(gx.round(), 0.0, 1.0, bounds.height), col);
                gx += sx;
            }
        }
        if sy >= 4.0 {
            let mut gy = py.rem_euclid(sy);
            while gy < bounds.height {
                r.fill_rect(Rect::new(0.0, gy.round(), bounds.width, 1.0), col);
                gy += sy;
            }
        }
    }

    fn draw_level(&self, r: &mut Renderer, bounds: Rect) {
        let (z, px, py) = self.view();
        r.fill_rect(bounds, Color::hex(0x3a3a3a));

        self.draw_grid(r, bounds, px, py, z);

        let sel = self.editor.selection();
        self.editor.with_project(|p| {
            let Some(level) = p.session.active.level.and_then(|k| p.levels.get(k)) else {
                text(r, "No level", 16.0, 16.0, 12.0, Color::hex(0xcccccc));
                return;
            };
            let lvl_key = p.session.active.level.unwrap();
            let world = |x: f32, y: f32, w: u32, h: u32| {
                Rect::new(x * z + px, y * z + py, w as f32 * z, h as f32 * z)
            };
            for (i, b) in level.backgrounds.iter().enumerate() {
                let rect = world(b.x, b.y, b.width, b.height);
                r.fill_rect(rect, Color::rgba(0.35, 0.5, 0.7, 0.5));
                if sel == (Selection::LevelBackground { level: lvl_key, index: i }) {
                    stroke(r, rect, ACCENT);
                }
            }
            for (i, t) in level.tiles.iter().enumerate() {
                let rect = world(t.x, t.y, t.width, t.height);
                let c = tile_color(i);
                r.fill_rect(rect, c);
                if let Some(tile) = p.tiles.get(t.tile) {
                    text(r, &tile.name, rect.x + 3.0, rect.y + 2.0, 10.0, Color::WHITE);
                }
                if sel == (Selection::LevelTile { level: lvl_key, index: i }) {
                    stroke(r, rect, ACCENT);
                }
            }
            for (i, a) in level.accessories.iter().enumerate() {
                let rect = world(a.x, a.y, a.width, a.height);
                r.fill_rect(rect, Color::rgba(0.8, 0.6, 0.2, 0.6));
                if let Some(acc) = p.accessories.get(a.accessory) {
                    text(r, &acc.name, rect.x + 3.0, rect.y + 2.0, 10.0, Color::WHITE);
                }
                if sel == (Selection::LevelAccessory { level: lvl_key, index: i }) {
                    stroke(r, rect, ACCENT);
                }
            }
        });
    }

    fn draw_shape_preview(&self, r: &mut Renderer) {
        let (Drag::Shape { start }, Some(end)) = (&self.drag, self.shape_end) else {
            return;
        };
        let (z, px, py) = self.view();
        let sp = |t: (f32, f32)| (t.0 * z + px, t.1 * z + py);
        let (ax, ay) = sp(*start);
        let (bx, by) = sp(end);
        match self.editor.tool() {
            Tool::Rectangle => {
                let rect = Rect::new(ax.min(bx), ay.min(by), (bx - ax).abs(), (by - ay).abs());
                stroke(r, rect, ACCENT);
            }
            Tool::Line => {
                // thin diagonal preview via short segments
                let steps = ((bx - ax).hypot(by - ay) / 3.0).max(1.0) as i32;
                for i in 0..=steps {
                    let t = i as f32 / steps as f32;
                    r.fill_rect(
                        Rect::new(ax + (bx - ax) * t - 1.0, ay + (by - ay) * t - 1.0, 2.0, 2.0),
                        ACCENT,
                    );
                }
            }
            _ => {}
        }
    }

    fn draw_text_caret(&self, r: &mut Renderer) {
        let Some((tx, ty)) = self.text_origin else { return };
        let (z, px, py) = self.view();
        let (sx, sy) = (tx * z + px, ty * z + py);
        let buf = self.editor.session(|s| s.tools.text.text.clone()).unwrap_or_default();
        let size = self.editor.session(|s| s.tools.text.size.max(4) as f32).unwrap_or(16.0);
        let col = self.editor.session(|s| s.colors.foreground).unwrap_or([0, 0, 0, 255]);
        let tc = Color::rgba(col[0] as f32 / 255.0, col[1] as f32 / 255.0, col[2] as f32 / 255.0, 1.0);
        let draw_size = (size * z).clamp(9.0, 96.0);
        if !buf.is_empty() {
            r.text_styled(
                &buf,
                Vec2 { x: sx, y: sy + draw_size * 0.82 },
                TextStyle { size: draw_size, color: tc, font: FontId::DEFAULT },
            );
        }
        let cw = r.measure(&buf, &TextStyle { size: draw_size, color: tc, font: FontId::DEFAULT });
        r.fill_rect(Rect::new(sx + cw + 1.0, sy, 1.5, draw_size), tc);
    }

    fn draw_marquee(&self, r: &mut Renderer) {
        let Some((x, y, w, h)) = self.editor.main_marquee.get() else {
            return;
        };
        if w < 0.5 || h < 0.5 {
            return;
        }
        let (z, px, py) = self.view();
        let rect = Rect::new(x * z + px, y * z + py, w * z, h * z);
        r.fill_rect(rect, Color::rgba(0.18, 0.5, 0.85, 0.15));
        stroke(r, rect, ACCENT);
    }
}

fn stroke(r: &mut Renderer, rect: Rect, c: Color) {
    r.fill_rect(Rect::new(rect.x, rect.y, rect.width, 1.0), c);
    r.fill_rect(Rect::new(rect.x, rect.y + rect.height - 1.0, rect.width, 1.0), c);
    r.fill_rect(Rect::new(rect.x, rect.y, 1.0, rect.height), c);
    r.fill_rect(Rect::new(rect.x + rect.width - 1.0, rect.y, 1.0, rect.height), c);
}

fn selected_origin(p: &Project) -> Option<(f32, f32)> {
    match p.session.selection {
        Selection::LevelTile { level, index } => {
            p.levels.get(level)?.tiles.get(index).map(|t| (t.x, t.y))
        }
        Selection::LevelBackground { level, index } => {
            p.levels.get(level)?.backgrounds.get(index).map(|b| (b.x, b.y))
        }
        Selection::LevelAccessory { level, index } => {
            p.levels.get(level)?.accessories.get(index).map(|a| (a.x, a.y))
        }
        _ => None,
    }
}

/// Optional texel-space write mask (inclusive), e.g. from an active
/// marquee. `Clip::ALL` lets everything through.
#[derive(Clone, Copy)]
struct Clip([i64; 4]);

impl Clip {
    const ALL: Clip = Clip([i64::MIN, i64::MIN, i64::MAX, i64::MAX]);
    fn allows(&self, x: i64, y: i64) -> bool {
        x >= self.0[0] && y >= self.0[1] && x <= self.0[2] && y <= self.0[3]
    }
}

fn put(px: &mut [u8], w: u32, h: u32, x: i64, y: i64, c: [u8; 4], clip: Clip) {
    if x < 0 || y < 0 || x >= w as i64 || y >= h as i64 || !clip.allows(x, y) {
        return;
    }
    let i = ((y * w as i64 + x) * 4) as usize;
    if c[3] == 255 {
        px[i..i + 4].copy_from_slice(&c);
        return;
    }
    if c[3] == 0 {
        return;
    }
    // straight-alpha src-over
    let sa = c[3] as f32 / 255.0;
    let da = px[i + 3] as f32 / 255.0;
    let oa = sa + da * (1.0 - sa);
    if oa <= 0.0 {
        px[i..i + 4].copy_from_slice(&[0, 0, 0, 0]);
        return;
    }
    for k in 0..3 {
        let s = c[k] as f32 / 255.0;
        let d = px[i + k] as f32 / 255.0;
        px[i + k] = (((s * sa + d * da * (1.0 - sa)) / oa).clamp(0.0, 1.0) * 255.0).round() as u8;
    }
    px[i + 3] = (oa.clamp(0.0, 1.0) * 255.0).round() as u8;
}

fn stamp(px: &mut [u8], w: u32, h: u32, cx: i64, cy: i64, size: i64, c: [u8; 4], clip: Clip) {
    let half = size / 2;
    for y in cy - half..cy - half + size {
        for x in cx - half..cx - half + size {
            put(px, w, h, x, y, c, clip);
        }
    }
}

/// Hard-clear a square brush footprint to fully transparent.
fn erase_stamp(px: &mut [u8], w: u32, h: u32, cx: i64, cy: i64, size: i64, clip: Clip) {
    let half = size / 2;
    for y in cy - half..cy - half + size {
        for x in cx - half..cx - half + size {
            if x < 0 || y < 0 || x >= w as i64 || y >= h as i64 || !clip.allows(x, y) {
                continue;
            }
            let i = ((y * w as i64 + x) * 4) as usize;
            px[i..i + 4].copy_from_slice(&[0, 0, 0, 0]);
        }
    }
}

fn line(
    px: &mut [u8],
    w: u32,
    h: u32,
    x0: i64,
    y0: i64,
    x1: i64,
    y1: i64,
    width: i64,
    c: [u8; 4],
    clip: Clip,
) {
    let (mut x0, mut y0) = (x0, y0);
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    loop {
        stamp(px, w, h, x0, y0, width.max(1), c, clip);
        if x0 == x1 && y0 == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x0 += sx;
        }
        if e2 <= dx {
            err += dx;
            y0 += sy;
        }
    }
}

fn tile_color(i: usize) -> Color {
    let hues = [0.02, 0.12, 0.32, 0.55, 0.72, 0.85];
    let h = hues[i % hues.len()];
    let [r, g, b] = super::render::hsv_to_rgb(h, 0.55, 0.85);
    Color::rgba(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 0.85)
}

impl ViewportContent for MainViewport {
    fn draw(&mut self, renderer: &mut Renderer, bounds: Rect) {
        if !self.editor.has_project() {
            renderer.fill_rect(bounds, Color::hex(0x2f2f2f));
            return;
        }
        if self.kind == EditorMode::Level {
            self.draw_level(renderer, bounds);
        } else {
            self.maybe_fit(bounds);
            self.draw_pixels(renderer, bounds);
        }
        self.draw_marquee(renderer);
        self.draw_shape_preview(renderer);
        self.draw_text_caret(renderer);
    }

    fn keyboard_event(&mut self, event: KeyboardEvent) -> bool {
        if self.text_origin.is_none() {
            return false;
        }
        match event {
            KeyboardEvent::TextInput(ch) if !ch.is_control() => {
                self.editor.edit_session(|s| s.tools.text.text.push(ch));
                true
            }
            KeyboardEvent::KeyDown { key: Key::Backspace, .. } => {
                self.editor.edit_session(|s| {
                    s.tools.text.text.pop();
                });
                true
            }
            KeyboardEvent::KeyDown { key: Key::Enter, .. } => {
                self.commit_text();
                true
            }
            KeyboardEvent::KeyDown { key: Key::Escape, .. } => {
                self.cancel_text();
                true
            }
            _ => false,
        }
    }

    fn pointer_event(&mut self, event: PointerEvent, _bounds: Rect) -> bool {
        let tool = self.editor.tool();
        let shift = self.editor.input.borrow().shift;

        match event {
            PointerEvent::Wheel { delta, x, y } => {
                let f = if delta > 0.0 { 1.1 } else { 1.0 / 1.1 };
                self.zoom_about(x, y, f);
                true
            }
            // Middle-button drag pans the view.
            PointerEvent::Down { button: MouseButton::Middle, x, y } => {
                let (_, px, py) = self.view();
                self.drag = Drag::Pan { start_pan: (px, py), start_mouse: (x, y) };
                true
            }
            PointerEvent::Down { button: MouseButton::Left, x, y } => {
                let (tx, ty) = self.texel(x, y);
                self.editor.main_cursor.set((tx, ty));

                let pixels = self.kind != EditorMode::Level;

                if self.text_origin.is_some() {
                    self.commit_text();
                }

                match tool {
                    Tool::Text if pixels => {
                        self.editor.edit_session(|s| s.tools.text.text.clear());
                        self.text_origin = Some((tx, ty));
                        self.editor.text_editing.set(true);
                    }
                    Tool::Zoom => {
                        let step = self.editor.session(|s| s.tools.zoom.step).unwrap_or(2.0);
                        self.zoom_about(x, y, if shift { 1.0 / step } else { step });
                    }
                    Tool::Pencil if pixels => {
                        self.paint(tx, ty);
                        self.drag = Drag::Paint { last: (tx, ty) };
                    }
                    Tool::Pencil => self.place_def(tx, ty),
                    Tool::Eraser if pixels => {
                        self.erase(tx, ty);
                        self.drag = Drag::Paint { last: (tx, ty) };
                    }
                    Tool::Eyedropper if pixels => self.sample(tx, ty),
                    Tool::Fill if pixels => self.flood_fill(tx, ty),
                    Tool::Line | Tool::Rectangle if pixels => {
                        self.shape_end = Some((tx, ty));
                        self.drag = Drag::Shape { start: (tx, ty) };
                    }
                    Tool::Marquee => {
                        self.editor.main_marquee.set(Some((tx, ty, 0.0, 0.0)));
                        self.drag = Drag::Marquee { start: (tx, ty) };
                    }
                    Tool::Select => self.pick(tx, ty),
                    Tool::Move if self.kind == EditorMode::Level => {
                        self.pick(tx, ty);
                        let origin = self.editor.with_project(|p| selected_origin(p)).flatten();
                        if let Some((ox, oy)) = origin {
                            self.drag = Drag::MoveItem { grab: (tx - ox, ty - oy) };
                        }
                    }
                    _ => {
                        let (_, px, py) = self.view();
                        self.drag = Drag::Pan { start_pan: (px, py), start_mouse: (x, y) };
                    }
                }
                true
            }
            PointerEvent::Move { x, y } => {
                let (tx, ty) = self.texel(x, y);
                self.editor.main_cursor.set((tx, ty));
                match &self.drag {
                    Drag::Pan { start_pan, start_mouse } => {
                        let (z, _, _) = self.view();
                        self.set_view(
                            z,
                            start_pan.0 + (x - start_mouse.0),
                            start_pan.1 + (y - start_mouse.1),
                        );
                        true
                    }
                    Drag::Marquee { start } => {
                        let (sx, sy) = *start;
                        self.editor.main_marquee.set(Some((
                            sx.min(tx),
                            sy.min(ty),
                            (tx - sx).abs(),
                            (ty - sy).abs(),
                        )));
                        true
                    }
                    Drag::Paint { last } => {
                        let l = *last;
                        if self.editor.tool() == Tool::Eraser {
                            self.erase_stroke(l, (tx, ty));
                        } else {
                            self.paint_stroke(l, (tx, ty));
                        }
                        self.drag = Drag::Paint { last: (tx, ty) };
                        true
                    }
                    Drag::Shape { .. } => {
                        self.shape_end = Some((tx, ty));
                        true
                    }
                    Drag::MoveItem { grab } => {
                        let grab = *grab;
                        self.move_selected(tx, ty, grab);
                        true
                    }
                    Drag::None => false,
                }
            }
            PointerEvent::Up { .. } => {
                if let Drag::Shape { start } = self.drag {
                    if let Some(end) = self.shape_end.take() {
                        self.commit_shape(start, end);
                    }
                }
                self.shape_end = None;
                self.drag = Drag::None;
                true
            }
            _ => false,
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl MainViewport {
    fn pick(&self, tx: f32, ty: f32) {
        match self.kind {
            EditorMode::Level => {
                let sel = self.editor.with_project(|p| {
                    let lvl_key = p.session.active.level?;
                    let level = p.levels.get(lvl_key)?;
                    for (i, a) in level.accessories.iter().enumerate().rev() {
                        if tx >= a.x && ty >= a.y && tx < a.x + a.width as f32 && ty < a.y + a.height as f32 {
                            return Some(Selection::LevelAccessory { level: lvl_key, index: i });
                        }
                    }
                    for (i, t) in level.tiles.iter().enumerate().rev() {
                        if tx >= t.x && ty >= t.y && tx < t.x + t.width as f32 && ty < t.y + t.height as f32 {
                            return Some(Selection::LevelTile { level: lvl_key, index: i });
                        }
                    }
                    for (i, b) in level.backgrounds.iter().enumerate().rev() {
                        if tx >= b.x && ty >= b.y && tx < b.x + b.width as f32 && ty < b.y + b.height as f32 {
                            return Some(Selection::LevelBackground { level: lvl_key, index: i });
                        }
                    }
                    None
                });
                if let Some(Some(s)) = sel {
                    self.editor.select(s);
                }
            }
            _ => {
                if let Some(Some(l)) = self.editor.session(|s| s.active.layer) {
                    self.editor.select(Selection::Layer(l));
                }
            }
        }
    }
}
