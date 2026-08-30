//! Software compositing helpers shared by the main viewport and the
//! preview.

use rustle_core::{BlendMode, FrameId, GroupId, LayerId, Project, SpriteCanvas, SpriteEntity};

/// An RGBA8 accumulation buffer being composited into.
pub struct Canvas {
    pub w: u32,
    pub h: u32,
    pub buf: Vec<u8>,
}

impl Canvas {
    fn new(w: u32, h: u32) -> Self {
        Self { w, h, buf: vec![0u8; (w as usize) * (h as usize) * 4] }
    }

    /// Composite an ordered set of layers then groups onto this canvas,
    /// honouring per-layer and per-group blend modes.
    pub fn draw_node(&mut self, p: &Project, layers: &[LayerId], groups: &[GroupId]) {
        for &lk in layers {
            let Some(l) = p.layers.get(lk) else { continue };
            if !l.visible || l.width == 0 || l.height == 0 {
                continue;
            }
            self.blend_pixels(&l.pixels, l.width, l.height, l.blend_mode);
        }
        for &gk in groups {
            let Some(g) = p.groups.get(gk) else { continue };
            if !g.visible {
                continue;
            }
            let mut sub = Canvas::new(self.w, self.h);
            sub.draw_node(p, &g.layers, &g.groups);
            self.blend_pixels(&sub.buf, sub.w, sub.h, g.blend_mode);
        }
    }

    fn blend_pixels(&mut self, src: &[u8], sw: u32, sh: u32, mode: BlendMode) {
        let cw = self.w as usize;
        let w = (sw.min(self.w)) as usize;
        let h = (sh.min(self.h)) as usize;
        for y in 0..h {
            for x in 0..w {
                let si = (y * sw as usize + x) * 4;
                let sa = src[si + 3] as f32 / 255.0;
                if sa <= 0.0 {
                    continue;
                }
                let di = (y * cw + x) * 4;
                blend(&mut self.buf[di..di + 4], &src[si..si + 4], sa, mode);
            }
        }
    }
}

fn node_size(p: &Project, layers: &[LayerId], groups: &[GroupId]) -> (u32, u32) {
    let mut w = 0;
    let mut h = 0;
    for &lk in layers {
        if let Some(l) = p.layers.get(lk) {
            w = w.max(l.width);
            h = h.max(l.height);
        }
    }
    for &gk in groups {
        if let Some(g) = p.groups.get(gk) {
            let (gw, gh) = node_size(p, &g.layers, &g.groups);
            w = w.max(gw);
            h = h.max(gh);
        }
    }
    (w, h)
}

/// Composite an explicit list of layers plus groups.
pub fn composite_layers(
    p: &Project,
    layers: &[LayerId],
    groups: &[GroupId],
) -> Option<(u32, u32, Vec<u8>)> {
    let (w, h) = node_size(p, layers, groups);
    if w == 0 || h == 0 {
        return None;
    }
    let mut c = Canvas::new(w, h);
    c.draw_node(p, layers, groups);
    Some((c.w, c.h, c.buf))
}

/// Composite one animation frame, with the owning entity's base image
/// underneath (base first, then the frame's own layers on top).
pub fn composite_frame_with_base(p: &Project, frame: FrameId, base: Option<SpriteEntity>) -> Option<(u32, u32, Vec<u8>)> {
    let f = p.frames.get(frame)?;

    let base_lg = base
        .and_then(|e| p.entity_base_frame(e))
        .and_then(|k| p.base_frame.get(k))
        .map(|b| (b.layers.clone(), b.groups.clone()));

    let (bw, bh) = base_lg
        .as_ref()
        .map(|(l, g)| node_size(p, l, g))
        .unwrap_or((0, 0));
    let (fw, fh) = node_size(p, &f.layers, &f.groups);
    let (w, h) = (bw.max(fw), bh.max(fh));
    if w == 0 || h == 0 {
        return None;
    }

    let mut c = Canvas::new(w, h);
    if let Some((bl, bg)) = base_lg {
        c.draw_node(p, &bl, &bg);
    }
    c.draw_node(p, &f.layers, &f.groups);
    Some((c.w, c.h, c.buf))
}

/// Composite whatever the sprite workspace currently has open.
pub fn composite_canvas(p: &Project) -> Option<(u32, u32, Vec<u8>)> {
    match p.session.active.canvas {
        SpriteCanvas::Base(k) => {
            let b = p.base_frame.get(k)?;
            composite_layers(p, &b.layers, &b.groups)
        }
        SpriteCanvas::Frame(k) => composite_frame_with_base(p, k, p.session.active.sprite),
        SpriteCanvas::None => None,
    }
}

fn blend_fn(mode: BlendMode, cb: f32, cs: f32) -> f32 {
    match mode {
        BlendMode::Normal => cs,
        BlendMode::Multiply => cb * cs,
        BlendMode::Screen => 1.0 - (1.0 - cb) * (1.0 - cs),
        BlendMode::Overlay => {
            if cb <= 0.5 {
                2.0 * cb * cs
            } else {
                1.0 - 2.0 * (1.0 - cb) * (1.0 - cs)
            }
        }
        BlendMode::Add => (cb + cs).min(1.0),
        BlendMode::Subtract => (cb - cs).max(0.0),
    }
}

fn blend(dst: &mut [u8], src: &[u8], sa: f32, mode: BlendMode) {
    let da = dst[3] as f32 / 255.0;
    let out_a = sa + da * (1.0 - sa);
    if out_a <= 0.0 {
        dst.iter_mut().for_each(|b| *b = 0);
        return;
    }
    for c in 0..3 {
        let s = src[c] as f32 / 255.0;
        let d = dst[c] as f32 / 255.0;
        // W3C blend: mix straight source with the blend of source over
        // the backdrop, weighted by the backdrop alpha, then src-over.
        let blended = (1.0 - da) * s + da * blend_fn(mode, d, s);
        let v = (blended * sa + d * da * (1.0 - sa)) / out_a;
        dst[c] = (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    }
    dst[3] = (out_a.clamp(0.0, 1.0) * 255.0).round() as u8;
}

/// HSV (all 0..1) to straight RGB bytes.
pub fn hsv_to_rgb(h: f32, s: f32, v: f32) -> [u8; 3] {
    let h = (h.fract() + 1.0).fract() * 6.0;
    let c = v * s;
    let x = c * (1.0 - (h % 2.0 - 1.0).abs());
    let m = v - c;
    let (r, g, b) = match h as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    [
        ((r + m) * 255.0).round() as u8,
        ((g + m) * 255.0).round() as u8,
        ((b + m) * 255.0).round() as u8,
    ]
}

/// RGB bytes to HSV (all 0..1).
pub fn rgb_to_hsv(c: [u8; 4]) -> (f32, f32, f32) {
    let r = c[0] as f32 / 255.0;
    let g = c[1] as f32 / 255.0;
    let b = c[2] as f32 / 255.0;
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let d = max - min;
    let h = if d == 0.0 {
        0.0
    } else if max == r {
        ((g - b) / d).rem_euclid(6.0) / 6.0
    } else if max == g {
        ((b - r) / d + 2.0) / 6.0
    } else {
        ((r - g) / d + 4.0) / 6.0
    };
    let s = if max == 0.0 { 0.0 } else { d / max };
    (h, s, max)
}
