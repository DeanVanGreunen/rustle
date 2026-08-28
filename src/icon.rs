//! Runtime window icon (title bar / taskbar while the app is running).
//! The `.exe` file icon is embedded separately by `build.rs`.

use macroquad::miniquad::conf::Icon;
use macroquad::prelude::*;

const LOGO_PNG: &[u8] = include_bytes!("../assets/logo.png");

/// Area-average downscale of a square RGBA image to `size` x `size`.
/// `src` must be a square whose side is a multiple of `size`.
fn downscale(src: &Image, size: usize) -> Vec<u8> {
    let sw = src.width as usize;
    let sh = src.height as usize;
    let bytes = src.get_image_data();
    let bx = (sw / size).max(1);
    let by = (sh / size).max(1);
    let mut out = vec![0u8; size * size * 4];

    for y in 0..size {
        for x in 0..size {
            let (mut r, mut g, mut b, mut a) = (0u32, 0u32, 0u32, 0u32);
            for oy in 0..by {
                for ox in 0..bx {
                    let sx = (x * bx + ox).min(sw - 1);
                    let sy = (y * by + oy).min(sh - 1);
                    let p = bytes[sy * sw + sx];
                    r += p[0] as u32;
                    g += p[1] as u32;
                    b += p[2] as u32;
                    a += p[3] as u32;
                }
            }
            let n = (bx * by) as u32;
            let o = (y * size + x) * 4;
            out[o] = (r / n) as u8;
            out[o + 1] = (g / n) as u8;
            out[o + 2] = (b / n) as u8;
            out[o + 3] = (a / n) as u8;
        }
    }
    out
}

/// Decode `assets/logo.png` and build the 16/32/64 icon set macroquad
/// wants. Returns `None` if the PNG can't be decoded.
pub fn window_icon() -> Option<Icon> {
    let img = Image::from_file_with_format(LOGO_PNG, Some(ImageFormat::Png)).ok()?;

    let mut small = [0u8; 16 * 16 * 4];
    let mut medium = [0u8; 32 * 32 * 4];
    let mut big = [0u8; 64 * 64 * 4];
    small.copy_from_slice(&downscale(&img, 16));
    medium.copy_from_slice(&downscale(&img, 32));
    big.copy_from_slice(&downscale(&img, 64));

    Some(Icon { small, medium, big })
}
