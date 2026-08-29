//! Software compositing helpers shared by the main viewport and the
//! preview.

use rustle_core::{FrameId, GroupId, LayerId, Project, SpriteCanvas};

fn collect_group(p: &Project, g: GroupId, out: &mut Vec<LayerId>) {
    let Some(group) = p.groups.get(g) else { return };
    if !group.visible {
        return;
    }
    out.extend(group.layers.iter().copied());
    for &child in &group.groups {
        collect_group(p, child, out);
    }
}

/// Composite an explicit list of layers plus groups, bottom-first.
pub fn composite_layers(
    p: &Project,
    layers: &[LayerId],
    groups: &[GroupId],
) -> Option<(u32, u32, Vec<u8>)> {
    let mut ids: Vec<LayerId> = layers.to_vec();
    for &g in groups {
        collect_group(p, g, &mut ids);
    }
    composite_ids(p, &ids)
}

/// Composite the visible layers of `frame`. `None` if there is nothing.
pub fn composite_frame(p: &Project, frame: FrameId) -> Option<(u32, u32, Vec<u8>)> {
    let f = p.frames.get(frame)?;
    composite_layers(p, &f.layers, &f.groups)
}

/// Composite whatever the sprite workspace currently has open.
pub fn composite_canvas(p: &Project) -> Option<(u32, u32, Vec<u8>)> {
    match p.session.active.canvas {
        SpriteCanvas::Base(k) => {
            let b = p.base_frame.get(k)?;
            composite_layers(p, &b.layers, &b.groups)
        }
        SpriteCanvas::Frame(k) => composite_frame(p, k),
        SpriteCanvas::None => None,
    }
}

fn composite_ids(p: &Project, ids: &[LayerId]) -> Option<(u32, u32, Vec<u8>)> {
    let mut cw = 0u32;
    let mut ch = 0u32;
    for &id in ids {
        if let Some(l) = p.layers.get(id) {
            cw = cw.max(l.width);
            ch = ch.max(l.height);
        }
    }
    if cw == 0 || ch == 0 {
        return None;
    }

    let mut dst = vec![0u8; (cw as usize) * (ch as usize) * 4];
    for &id in ids {
        let Some(l) = p.layers.get(id) else { continue };
        if !l.visible || l.width == 0 || l.height == 0 {
            continue;
        }
        let lw = l.width.min(cw) as usize;
        let lh = l.height.min(ch) as usize;
        for y in 0..lh {
            for x in 0..lw {
                let si = (y * l.width as usize + x) * 4;
                let sa = l.pixels[si + 3] as f32 / 255.0;
                if sa <= 0.0 {
                    continue;
                }
                let di = (y * cw as usize + x) * 4;
                over(&mut dst[di..di + 4], &l.pixels[si..si + 4], sa);
            }
        }
    }
    Some((cw, ch, dst))
}

fn over(dst: &mut [u8], src: &[u8], sa: f32) {
    let da = dst[3] as f32 / 255.0;
    let out_a = sa + da * (1.0 - sa);
    if out_a <= 0.0 {
        dst.iter_mut().for_each(|b| *b = 0);
        return;
    }
    for c in 0..3 {
        let s = src[c] as f32 / 255.0;
        let d = dst[c] as f32 / 255.0;
        let v = (s * sa + d * da * (1.0 - sa)) / out_a;
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
