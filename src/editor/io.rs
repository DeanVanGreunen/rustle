//! Import / export: PNG in and out, driven by the nav menu actions.

use rustle_core::{BaseFrame, EditorMode, Frame, Layer, Selection, Tile, FILE_EXT};

use super::render::composite_frame;
use super::Editor;

/// Prompt for a new `.rustle` path, repoint the project, and save.
/// Returns `(project_name, new_path)` on success so the caller can
/// update the recent list and nav bar.
pub fn save_as(editor: &Editor) -> Option<(String, String)> {
    let (name, dir_name) = editor.with_project(|p| {
        (
            p.project_name.clone(),
            if p.project_name.is_empty() { "Untitled".into() } else { p.project_name.clone() },
        )
    })?;
    let path = rfd::FileDialog::new()
        .add_filter("Rustle Project", &[FILE_EXT])
        .set_file_name(format!("{dir_name}.{FILE_EXT}"))
        .save_file()?;
    let path = path.to_string_lossy().into_owned();
    let saved = editor.edit(|p| {
        p.file_path = path.clone();
        p.save()
    });
    match saved {
        Some(Ok(())) => {
            editor.dirty.set(false);
            Some((name, path))
        }
        Some(Err(e)) => {
            eprintln!("save as: {e}");
            None
        }
        None => None,
    }
}

/// Open a native picker and bring an image into the project. In pixel
/// workspaces it becomes a new layer of the active frame; in the level
/// workspace it becomes a new tile definition.
pub fn import_image(editor: &Editor) {
    let Some(path) = rfd::FileDialog::new()
        .add_filter("Image", &["png", "jpg", "jpeg", "bmp"])
        .pick_file()
    else {
        return;
    };
    let dyn_img = match image::open(&path) {
        Ok(i) => i.to_rgba8(),
        Err(e) => {
            eprintln!("import: {e}");
            return;
        }
    };
    let (w, h) = (dyn_img.width(), dyn_img.height());
    let pixels = dyn_img.into_raw();
    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("import")
        .to_string();

    editor.bump_generation();
    editor.edit(|p| {
        let layer_id = p.add_layer(Layer { width: w, height: h, pixels, visible: true, ..Default::default() });

        if p.session.mode == EditorMode::Level {
            let bf = p.add_base_frame(BaseFrame { width: w, height: h, layers: vec![layer_id], ..Default::default() });
            let tile = p.add_tile(Tile {
                name,
                width: w,
                height: h,
                base_frame: Some(bf),
                ..Default::default()
            });
            p.session.selection = Selection::Tile(tile);
        } else if let Some(frame) = p.session.active.frame {
            if let Some(f) = p.frames.get_mut(frame) {
                f.layers.push(layer_id);
            }
            p.session.active.layer = Some(layer_id);
            p.session.selection = Selection::Layer(layer_id);
        } else {
            let frame = p.add_frame(Frame { layers: vec![layer_id], delay_ms: 100, ..Default::default() });
            p.session.active.frame = Some(frame);
            p.session.active.layer = Some(layer_id);
            p.session.selection = Selection::Layer(layer_id);
        }
    });
}

/// Open a native picker and write the current view to a PNG. Sprite:
/// the active frame. Animation: a horizontal spritesheet of the active
/// animation's frames. Level: the level's colour-block render.
pub fn export_image(editor: &Editor) {
    let default_name = editor
        .with_project(|p| {
            let base = if p.project_name.is_empty() { "export" } else { &p.project_name };
            format!("{base}.png")
        })
        .unwrap_or_else(|| "export.png".into());

    let Some(path) = rfd::FileDialog::new()
        .add_filter("PNG", &["png"])
        .set_file_name(default_name)
        .save_file()
    else {
        return;
    };

    let image = editor.with_project(|p| match p.session.mode {
        EditorMode::Animation => export_animation(p),
        EditorMode::Level => export_level(p),
        EditorMode::Sprite => p
            .session
            .active
            .frame
            .and_then(|f| composite_frame(p, f)),
    });

    let Some(Some((w, h, buf))) = image else {
        eprintln!("export: nothing to write");
        return;
    };
    if let Err(e) = image::save_buffer(&path, &buf, w, h, image::ColorType::Rgba8) {
        eprintln!("export: {e}");
    }
}

fn export_animation(p: &rustle_core::Project) -> Option<(u32, u32, Vec<u8>)> {
    let anim = p.session.active.animation.and_then(|k| p.animations.get(k))?;
    let frames: Vec<_> = anim
        .frames
        .iter()
        .filter_map(|f| composite_frame(p, *f))
        .collect();
    if frames.is_empty() {
        return None;
    }
    let sheet_h = frames.iter().map(|(_, h, _)| *h).max().unwrap_or(0);
    let sheet_w: u32 = frames.iter().map(|(w, _, _)| *w).sum();
    let mut buf = vec![0u8; (sheet_w * sheet_h * 4) as usize];
    let mut ox = 0u32;
    for (fw, fh, fb) in &frames {
        for y in 0..*fh {
            for x in 0..*fw {
                let si = ((y * fw + x) * 4) as usize;
                let di = ((y * sheet_w + ox + x) * 4) as usize;
                buf[di..di + 4].copy_from_slice(&fb[si..si + 4]);
            }
        }
        ox += fw;
    }
    Some((sheet_w, sheet_h, buf))
}

fn export_level(p: &rustle_core::Project) -> Option<(u32, u32, Vec<u8>)> {
    let level = p.session.active.level.and_then(|k| p.levels.get(k))?;
    let mut max_x = 1.0f32;
    let mut max_y = 1.0f32;
    for t in &level.tiles {
        max_x = max_x.max(t.x + t.width as f32);
        max_y = max_y.max(t.y + t.height as f32);
    }
    for b in &level.backgrounds {
        max_x = max_x.max(b.x + b.width as f32);
        max_y = max_y.max(b.y + b.height as f32);
    }
    let (w, h) = (max_x.ceil().max(1.0) as u32, max_y.ceil().max(1.0) as u32);
    let mut buf = vec![0u8; (w * h * 4) as usize];

    let mut fill = |x0: f32, y0: f32, rw: u32, rh: u32, c: [u8; 4]| {
        for y in y0.max(0.0) as u32..((y0 as u32 + rh).min(h)) {
            for x in x0.max(0.0) as u32..((x0 as u32 + rw).min(w)) {
                let i = ((y * w + x) * 4) as usize;
                buf[i..i + 4].copy_from_slice(&c);
            }
        }
    };
    for b in &level.backgrounds {
        fill(b.x, b.y, b.width, b.height, [90, 128, 178, 128]);
    }
    for (i, t) in level.tiles.iter().enumerate() {
        let hue = [0.02f32, 0.12, 0.32, 0.55, 0.72, 0.85][i % 6];
        let [r, g, bl] = super::render::hsv_to_rgb(hue, 0.55, 0.85);
        fill(t.x, t.y, t.width, t.height, [r, g, bl, 230]);
    }
    Some((w, h, buf))
}
