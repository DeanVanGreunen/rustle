//! "Properties" tab body: a form reflecting whatever is selected. For
//! level items it also offers a grouped tile/background/accessory picker
//! (headers shown but disabled) drawn in the overlay pass.

use std::cell::{Cell, RefCell};

use rustle_core::{BlendMode, EditorMode, GroupId, Project, Selection, SpriteCanvas, TileId};
use rustle_ui::prelude::*;

use super::form::{Form, FormHit};
use super::numinput::NumState;
use super::theme::*;
use super::Editor;

pub struct PropertiesForm {
    editor: Editor,
    #[allow(dead_code)]
    kind: EditorMode,
    dd_open: Cell<bool>,
    dd_btn: Cell<Rect>,
    abs: Cell<(f32, f32)>,
    nums: RefCell<NumState>,
}

impl PropertiesForm {
    pub fn new(editor: Editor, kind: EditorMode) -> Self {
        Self {
            editor,
            kind,
            dd_open: Cell::new(false),
            dd_btn: Cell::new(Rect::new(0.0, 0.0, 0.0, 0.0)),
            abs: Cell::new((0.0, 0.0)),
            nums: RefCell::new(NumState::default()),
        }
    }

    fn build(&self, width: f32) -> Form {
        let inner = (width - 28.0).max(80.0);
        let edit = {
            let n = self.nums.borrow();
            n.editing_key().map(|k| (k, n.buffer_for(k).unwrap_or("").to_string()))
        };
        let mut f = Form::new(14.0, 12.0, inner).editing(edit);
        let Some(sel) = self.editor.session(|s| s.selection) else {
            return f;
        };

        self.editor.with_project(|p| match sel {
            Selection::None => f.heading("Nothing selected"),

            Selection::Layer(k) => {
                if let Some(l) = p.layers.get(k) {
                    f.heading("Layer");
                    f.readonly("UUID", l.id.to_string());
                    f.readonly("Size", format!("{} \u{00d7} {}", l.width, l.height));
                    let mode = l.blend_mode;
                    f.cycle("Blend", mode.label().into(), BlendMode::ALL.len(), blend_idx(mode), move |p, i| {
                        if let Some(l) = p.layers.get_mut(k) {
                            l.blend_mode = BlendMode::ALL[i];
                        }
                    });
                    let vis = l.visible;
                    f.toggle("Visible", vis, move |p, on| {
                        if let Some(l) = p.layers.get_mut(k) {
                            l.visible = on;
                        }
                    });
                }
            }

            Selection::Group(k) => {
                if let Some(g) = p.groups.get(k) {
                    f.heading("Group");
                    f.readonly("UUID", g.id.to_string());
                    let (tl, tg) = group_totals(p, k);
                    f.readonly("Layers (total)", tl.to_string());
                    f.readonly("Groups (total)", tg.to_string());
                    let mode = g.blend_mode;
                    f.cycle("Blend", mode.label().into(), BlendMode::ALL.len(), blend_idx(mode), move |p, i| {
                        if let Some(g) = p.groups.get_mut(k) {
                            g.blend_mode = BlendMode::ALL[i];
                        }
                    });
                    f.stepper("Width", g.width as i64, 0, 8192, 1, move |p, v| {
                        if let Some(g) = p.groups.get_mut(k) { g.width = v as u32; }
                    });
                    f.stepper("Height", g.height as i64, 0, 8192, 1, move |p, v| {
                        if let Some(g) = p.groups.get_mut(k) { g.height = v as u32; }
                    });
                    let vis = g.visible;
                    f.toggle("Visible", vis, move |p, on| {
                        if let Some(g) = p.groups.get_mut(k) {
                            g.visible = on;
                        }
                    });
                }
            }

            Selection::Frame(k) => {
                if let Some(fr) = p.frames.get(k) {
                    f.heading("Frame");
                    f.readonly("UUID", fr.id.to_string());
                    f.stepper("Delay (ms)", fr.delay_ms as i64, 1, 60000, 10, move |p, v| {
                        if let Some(fr) = p.frames.get_mut(k) {
                            fr.delay_ms = v as u64;
                        }
                    });
                    f.readonly("Layers", fr.layers.len().to_string());
                    f.readonly("Groups", fr.groups.len().to_string());
                }
            }

            Selection::Animation(k) => {
                if let Some(a) = p.animations.get(k) {
                    f.heading("Animation");
                    f.readonly("Name", if a.name.is_empty() { "\u{2014}".into() } else { a.name.clone() });
                    f.readonly("Frames", a.frames.len().to_string());
                    if let SpriteCanvas::Frame(fk) = p.session.active.canvas {
                        if let Some(fr) = p.frames.get(fk) {
                            f.heading("Current Frame");
                            f.readonly("Frame UUID", fr.id.to_string());
                            f.stepper("Delay (ms)", fr.delay_ms as i64, 1, 60000, 10, move |p, v| {
                                if let Some(fr) = p.frames.get_mut(fk) {
                                    fr.delay_ms = v as u64;
                                }
                            });
                        }
                    }
                }
            }

            Selection::Tile(k) => {
                if let Some(t) = p.tiles.get(k) {
                    f.heading("Tile");
                    f.readonly("UUID", t.id.to_string());
                    f.readonly("Name", if t.name.is_empty() { "—".into() } else { t.name.clone() });
                    f.stepper("Width", t.width as i64, 1, 4096, 1, move |p, v| set_tile(p, k, |t| t.width = v as u32));
                    f.stepper("Height", t.height as i64, 1, 4096, 1, move |p, v| set_tile(p, k, |t| t.height = v as u32));
                    f.stepper("Origin X", t.origin.x as i64, -4096, 4096, 1, move |p, v| set_tile(p, k, |t| t.origin.x = v as f32));
                    f.stepper("Origin Y", t.origin.y as i64, -4096, 4096, 1, move |p, v| set_tile(p, k, |t| t.origin.y = v as f32));
                    f.readonly("Animations", t.animations.len().to_string());
                    f.readonly("Base frame", if t.base_frame.is_some() { "set".into() } else { "—".into() });
                }
            }

            Selection::Background(k) => {
                if let Some(b) = p.backgrounds.get(k) {
                    f.heading("Background");
                    f.readonly("UUID", b.id.to_string());
                    f.readonly("Name", if b.name.is_empty() { "—".into() } else { b.name.clone() });
                    f.stepper("Width", b.width as i64, 1, 8192, 1, move |p, v| {
                        if let Some(b) = p.backgrounds.get_mut(k) { b.width = v as u32; }
                    });
                    f.stepper("Height", b.height as i64, 1, 8192, 1, move |p, v| {
                        if let Some(b) = p.backgrounds.get_mut(k) { b.height = v as u32; }
                    });
                    let par = b.parallax;
                    f.toggle("Parallax", par, move |p, on| {
                        if let Some(b) = p.backgrounds.get_mut(k) { b.parallax = on; }
                    });
                    f.stepper("Z-Index", b.z_index as i64, -999, 999, 1, move |p, v| {
                        if let Some(b) = p.backgrounds.get_mut(k) { b.z_index = v as i32; }
                    });
                }
            }

            Selection::Accessory(k) => {
                if let Some(a) = p.accessories.get(k) {
                    f.heading("Accessory");
                    f.readonly("UUID", a.id.to_string());
                    f.readonly("Name", if a.name.is_empty() { "—".into() } else { a.name.clone() });
                    f.stepper("Width", a.width as i64, 1, 8192, 1, move |p, v| {
                        if let Some(a) = p.accessories.get_mut(k) { a.width = v as u32; }
                    });
                    f.stepper("Height", a.height as i64, 1, 8192, 1, move |p, v| {
                        if let Some(a) = p.accessories.get_mut(k) { a.height = v as u32; }
                    });
                }
            }

            Selection::Level(k) => {
                if let Some(l) = p.levels.get(k) {
                    f.heading("Level");
                    f.readonly("Name", if l.name.is_empty() { "—".into() } else { l.name.clone() });
                    f.readonly("ID", l.id.to_string());
                    f.readonly("Tiles", l.tiles.len().to_string());
                    f.readonly("Backgrounds", l.backgrounds.len().to_string());
                    f.readonly("Accessories", l.accessories.len().to_string());
                    for (tid, count) in tile_usage(l) {
                        let name = p.tiles.get(tid).map(|t| t.name.clone()).unwrap_or_default();
                        f.readonly(
                            &format!("  {}", if name.is_empty() { "tile".into() } else { name }),
                            format!("×{count}"),
                        );
                    }
                }
            }

            Selection::LevelTile { level, index } => {
                if let Some(l) = p.levels.get(level) {
                    if let Some(t) = l.tiles.get(index) {
                        f.heading("Placed Tile");
                        let name = p.tiles.get(t.tile).map(|x| x.name.clone()).unwrap_or_default();
                        f.readonly("Tile", if name.is_empty() { "(pick below)".into() } else { name });
                        let (tx, ty, tw, th) = (t.x, t.y, t.width, t.height);
                        f.stepper("X", tx as i64, -100000, 100000, 1, move |p, v| set_lt(p, level, index, |t| t.x = v as f32));
                        f.stepper("Y", ty as i64, -100000, 100000, 1, move |p, v| set_lt(p, level, index, |t| t.y = v as f32));
                        f.stepper("Width", tw as i64, 1, 8192, 1, move |p, v| set_lt(p, level, index, |t| t.width = v as u32));
                        f.stepper("Height", th as i64, 1, 8192, 1, move |p, v| set_lt(p, level, index, |t| t.height = v as u32));
                        let uses = l.tiles.iter().filter(|x| x.tile == t.tile).count();
                        f.readonly("Uses in level", uses.to_string());
                    }
                }
            }

            Selection::LevelBackground { level, index } => {
                if let Some(l) = p.levels.get(level) {
                    if let Some(b) = l.backgrounds.get(index) {
                        f.heading("Placed Background");
                        let name = p.backgrounds.get(b.background).map(|x| x.name.clone()).unwrap_or_default();
                        f.readonly("Background", if name.is_empty() { "-".into() } else { name });
                        let (bx, by, bw, bh) = (b.x, b.y, b.width, b.height);
                        f.stepper("X", bx as i64, -100000, 100000, 1, move |p, v| set_lb(p, level, index, |b| b.x = v as f32));
                        f.stepper("Y", by as i64, -100000, 100000, 1, move |p, v| set_lb(p, level, index, |b| b.y = v as f32));
                        f.stepper("Width", bw as i64, 1, 8192, 1, move |p, v| set_lb(p, level, index, |b| b.width = v as u32));
                        f.stepper("Height", bh as i64, 1, 8192, 1, move |p, v| set_lb(p, level, index, |b| b.height = v as u32));
                    }
                }
            }

            Selection::LevelAccessory { level, index } => {
                if let Some(l) = p.levels.get(level) {
                    if let Some(a) = l.accessories.get(index) {
                        f.heading("Placed Accessory");
                        let name = p.accessories.get(a.accessory).map(|x| x.name.clone()).unwrap_or_default();
                        f.readonly("Accessory", if name.is_empty() { "-".into() } else { name });
                        let (ax, ay, aw, ah) = (a.x, a.y, a.width, a.height);
                        f.stepper("X", ax as i64, -100000, 100000, 1, move |p, v| set_la(p, level, index, |a| a.x = v as f32));
                        f.stepper("Y", ay as i64, -100000, 100000, 1, move |p, v| set_la(p, level, index, |a| a.y = v as f32));
                        f.stepper("Width", aw as i64, 1, 8192, 1, move |p, v| set_la(p, level, index, |a| a.width = v as u32));
                        f.stepper("Height", ah as i64, 1, 8192, 1, move |p, v| set_la(p, level, index, |a| a.height = v as u32));
                    }
                }
            }
        });

        f
    }

    fn dd_options(&self) -> Vec<(String, Option<TileId>)> {
        // Grouped: a disabled header row (None) then each tile (Some).
        let mut out = vec![("TILES".to_string(), None)];
        self.editor.with_project(|p| {
            for (k, t) in p.tiles.iter() {
                out.push((
                    format!("  {}", if t.name.is_empty() { "tile".into() } else { t.name.clone() }),
                    Some(k),
                ));
            }
        });
        out
    }
}

fn blend_idx(m: BlendMode) -> usize {
    BlendMode::ALL.iter().position(|x| *x == m).unwrap_or(0)
}

/// Recursive layer / group totals for a group (counts descendants).
fn group_totals(p: &Project, g: GroupId) -> (usize, usize) {
    let Some(gr) = p.groups.get(g) else { return (0, 0) };
    let mut layers = gr.layers.len();
    let mut groups = gr.groups.len();
    for &c in &gr.groups {
        let (l, gg) = group_totals(p, c);
        layers += l;
        groups += gg;
    }
    (layers, groups)
}

fn set_tile(p: &mut Project, k: TileId, f: impl FnOnce(&mut rustle_core::Tile)) {
    if let Some(t) = p.tiles.get_mut(k) {
        f(t);
    }
}
fn set_lt(p: &mut Project, level: rustle_core::LevelId, i: usize, f: impl FnOnce(&mut rustle_core::LevelTile)) {
    if let Some(l) = p.levels.get_mut(level) {
        if let Some(t) = l.tiles.get_mut(i) {
            f(t);
        }
    }
}
fn set_lb(p: &mut Project, level: rustle_core::LevelId, i: usize, f: impl FnOnce(&mut rustle_core::LevelBackground)) {
    if let Some(l) = p.levels.get_mut(level) {
        if let Some(b) = l.backgrounds.get_mut(i) {
            f(b);
        }
    }
}

fn set_la(p: &mut Project, level: rustle_core::LevelId, i: usize, f: impl FnOnce(&mut rustle_core::LevelAccessory)) {
    if let Some(l) = p.levels.get_mut(level) {
        if let Some(a) = l.accessories.get_mut(i) {
            f(a);
        }
    }
}

fn tile_usage(l: &rustle_core::Level) -> Vec<(TileId, usize)> {
    let mut acc: Vec<(TileId, usize)> = Vec::new();
    for t in &l.tiles {
        if let Some(e) = acc.iter_mut().find(|(k, _)| *k == t.tile) {
            e.1 += 1;
        } else {
            acc.push((t.tile, 1));
        }
    }
    acc
}

impl Behavior for PropertiesForm {
    fn update(&mut self, ctx: &mut UpdateContext) {
        let b = ctx.ui.absolute_box(ctx.node);
        self.abs.set((b.x, b.y));
    }

    fn render(&mut self, ctx: &mut RenderContext) {
        let (w, h) = ctx.size();
        ctx.renderer.fill_rect(Rect::new(0.0, 0.0, w, h), PANEL_BG_ALT);
        if !self.editor.has_project() {
            text(ctx.renderer, "No project", 14.0, 14.0, 12.0, DIM);
            return;
        }

        let f = self.build(w);
        f.render(ctx.renderer);

        // Grouped picker button for placed level items.
        if matches!(
            self.editor.selection(),
            Selection::LevelTile { .. } | Selection::LevelBackground { .. }
        ) {
            let y = f.height() + 6.0;
            let btn = Rect::new(14.0, y, w - 28.0, 24.0);
            self.dd_btn.set(btn);
            ctx.renderer.fill_rounded_rect(btn, 4.0, FIELD_BG);
            text(ctx.renderer, "Change type / instance", btn.x + 8.0, btn.y + 5.0, 11.0, INK);
            text_right(ctx.renderer, "v", btn.x + btn.width - 10.0, btn.y + 5.0, 11.0, DIM);
        } else {
            self.dd_open.set(false);
        }
    }

    fn overlay(&mut self, ctx: &mut RenderContext) {
        if !self.dd_open.get() {
            return;
        }
        let (ax, ay) = self.abs.get();
        let o = Vec2 { x: -ax, y: -ay };
        let btn = self.dd_btn.get();
        let opts = self.dd_options();
        let row_h = 22.0;
        let list = Rect::new(
            btn.x + o.x,
            btn.y + btn.height + 2.0 + o.y,
            btn.width,
            opts.len() as f32 * row_h + 6.0,
        );
        ctx.renderer.fill_rounded_rect(list, 5.0, PANEL_BG_ALT);
        for (i, (label, id)) in opts.iter().enumerate() {
            let ry = list.y + 3.0 + i as f32 * row_h;
            let disabled = id.is_none();
            text(
                ctx.renderer,
                label,
                list.x + 8.0,
                ry + 4.0,
                11.0,
                if disabled { FAINT } else { INK },
            );
        }
    }

    fn focusable(&self) -> bool {
        true
    }

    fn keyboard_event(&mut self, ctx: &mut EventContext, event: KeyboardEvent) {
        if self.nums.borrow_mut().on_key(&event, &self.editor) {
            ctx.stop_propagation();
        }
    }

    fn pointer_event(&mut self, ctx: &mut EventContext, event: PointerEvent) {
        let b = ctx.ui.absolute_box(ctx.node);

        match event {
            PointerEvent::Move { x, .. } => {
                if self.nums.borrow_mut().on_move(x, &self.editor) {
                    ctx.stop_propagation();
                }
                return;
            }
            PointerEvent::Up { .. } => {
                self.nums.borrow_mut().on_up();
                return;
            }
            _ => {}
        }

        if let PointerEvent::Down { button: MouseButton::Left, x, y } = event {
            ctx.focus();
            let (lx, ly) = (x - b.x, y - b.y);
            self.nums.borrow_mut().commit(&self.editor);

            if self.dd_open.get() {
                let btn = self.dd_btn.get();
                let row_h = 22.0;
                let list_y = btn.y + btn.height + 2.0 + 3.0;
                let opts = self.dd_options();
                let idx = ((ly - list_y) / row_h).floor();
                if idx >= 0.0 && (idx as usize) < opts.len() {
                    if let (Some(tid), Selection::LevelTile { level, index }) =
                        (opts[idx as usize].1, self.editor.selection())
                    {
                        self.editor.edit(|p| set_lt(p, level, index, |t| t.tile = tid));
                    }
                }
                self.dd_open.set(false);
                ctx.stop_propagation();
                return;
            }

            let btn = self.dd_btn.get();
            if btn.width > 0.0 && lx >= btn.x && lx <= btn.x + btn.width && ly >= btn.y && ly <= btn.y + btn.height {
                self.dd_open.set(true);
                ctx.stop_propagation();
                return;
            }

            match self.build(b.width).hit(lx, ly) {
                Some(FormHit::Mut(m)) => {
                    self.editor.edit(|p| m(p));
                    ctx.stop_propagation();
                }
                Some(FormHit::Number(nh)) => {
                    self.nums.borrow_mut().begin(nh, x, &self.editor);
                    ctx.stop_propagation();
                }
                Some(FormHit::Signal(_)) | None => {}
            }
        }
    }
}
