//! Column 1: the outline tree. Contents depend on the workspace and are
//! rebuilt from the app state every frame; the widget draws its own rows
//! and tracks expand / scroll locally.

use std::cell::{Cell, RefCell};
use std::collections::HashSet;

use rustle_core::{AnimationId, EditorMode, GroupId, LevelId, Project, Selection};
use rustle_ui::prelude::*;

use super::theme::*;
use super::Editor;

const ROW_H: f32 = 22.0;
const INDENT: f32 = 14.0;
const BASE_X: f32 = 10.0;
const DEL_W: f32 = 20.0;

#[derive(Hash, Eq, PartialEq, Clone, Copy)]
enum Expand {
    Group(GroupId),
    Anim(AnimationId),
    Level(LevelId),
}

#[derive(Clone, Copy)]
enum Act {
    None,
    Select(Selection),
    AddLevel,
    AddLayer,
    AddGroup,
    AddFrame,
    AddAnimation,
    NewTile,
    NewBackground,
    NewAccessory,
}

struct OutRow {
    depth: u8,
    label: String,
    header: bool,
    add: bool,
    twisty: Option<(Expand, bool)>,
    act: Act,
    selected: bool,
    deletable: Option<Selection>,
}

impl OutRow {
    fn plain(depth: u8, label: String, act: Act, selected: bool) -> Self {
        Self { depth, label, header: false, add: false, twisty: None, act, selected, deletable: None }
    }
    fn del(mut self, sel: Selection) -> Self {
        self.deletable = Some(sel);
        self
    }
}

pub struct OutlineTreeView {
    editor: Editor,
    kind: EditorMode,
    expanded: RefCell<HashSet<Expand>>,
    scroll: Cell<f32>,
}

impl OutlineTreeView {
    pub fn new(editor: Editor, kind: EditorMode) -> Self {
        Self {
            editor,
            kind,
            expanded: RefCell::new(HashSet::new()),
            scroll: Cell::new(0.0),
        }
    }

    fn is_open(&self, e: Expand) -> bool {
        self.expanded.borrow().contains(&e)
    }

    fn rows(&self) -> Vec<OutRow> {
        let mut rows = Vec::new();
        self.editor.with_project(|p| match self.kind {
            EditorMode::Sprite => self.sprite_rows(p, &mut rows),
            EditorMode::Animation => self.animation_rows(p, &mut rows),
            EditorMode::Level => self.level_rows(p, &mut rows),
        });
        rows
    }

    fn sprite_rows(&self, p: &Project, rows: &mut Vec<OutRow>) {
        header(rows, "Frame Layers");
        let Some(frame) = p.session.active.frame.and_then(|k| p.frames.get(k)) else {
            add_row(rows, "+ Add Frame", Act::AddFrame);
            return;
        };
        let sel = p.session.selection;
        for &g in &frame.groups {
            self.group_rows(p, g, 0, sel, rows);
        }
        for (i, &l) in frame.layers.iter().enumerate() {
            let vis = p.layers.get(l).map(|x| x.visible).unwrap_or(true);
            rows.push(
                OutRow::plain(
                    0,
                    format!("{}Layer {}", if vis { "" } else { "• " }, i + 1),
                    Act::Select(Selection::Layer(l)),
                    sel == Selection::Layer(l),
                )
                .del(Selection::Layer(l)),
            );
        }
        add_row(rows, "+ Add Layer", Act::AddLayer);
        add_row(rows, "+ Add Group", Act::AddGroup);
    }

    fn group_rows(&self, p: &Project, g: GroupId, depth: u8, sel: Selection, rows: &mut Vec<OutRow>) {
        let Some(group) = p.groups.get(g) else { return };
        let open = self.is_open(Expand::Group(g));
        let mut row = OutRow::plain(
            depth,
            format!("Group ({} layers)", group.layers.len()),
            Act::Select(Selection::Group(g)),
            sel == Selection::Group(g),
        )
        .del(Selection::Group(g));
        row.twisty = Some((Expand::Group(g), open));
        rows.push(row);
        if open {
            for &child in &group.groups {
                self.group_rows(p, child, depth + 1, sel, rows);
            }
            for (i, &l) in group.layers.iter().enumerate() {
                rows.push(
                    OutRow::plain(
                        depth + 1,
                        format!("Layer {}", i + 1),
                        Act::Select(Selection::Layer(l)),
                        sel == Selection::Layer(l),
                    )
                    .del(Selection::Layer(l)),
                );
            }
        }
    }

    fn animation_rows(&self, p: &Project, rows: &mut Vec<OutRow>) {
        let sel = p.session.selection;
        header(rows, "Animations");
        for (i, (k, anim)) in p.animations.iter().enumerate() {
            let open = self.is_open(Expand::Anim(k));
            let mut row = OutRow::plain(
                0,
                format!("Animation {} ({} frames)", i + 1, anim.frames.len()),
                Act::Select(Selection::Animation(k)),
                sel == Selection::Animation(k),
            )
            .del(Selection::Animation(k));
            row.twisty = Some((Expand::Anim(k), open));
            rows.push(row);
            if open {
                for (j, &f) in anim.frames.iter().enumerate() {
                    rows.push(OutRow::plain(
                        1,
                        format!("Frame {}", j + 1),
                        Act::Select(Selection::Frame(f)),
                        sel == Selection::Frame(f),
                    ));
                }
            }
        }
        add_row(rows, "+ Add Animation", Act::AddAnimation);

        header(rows, "All Frames");
        for (i, (k, fr)) in p.frames.iter().enumerate() {
            rows.push(
                OutRow::plain(
                    0,
                    format!("Frame {} — {}ms", i + 1, fr.delay_ms),
                    Act::Select(Selection::Frame(k)),
                    sel == Selection::Frame(k),
                )
                .del(Selection::Frame(k)),
            );
        }
        add_row(rows, "+ Add Frame", Act::AddFrame);
    }

    fn level_rows(&self, p: &Project, rows: &mut Vec<OutRow>) {
        let sel = p.session.selection;

        header(rows, "Levels");
        for (k, level) in p.levels.iter() {
            let open = self.is_open(Expand::Level(k));
            let mut row = OutRow::plain(
                0,
                if level.name.is_empty() { "Level".into() } else { level.name.clone() },
                Act::Select(Selection::Level(k)),
                sel == Selection::Level(k),
            )
            .del(Selection::Level(k));
            row.twisty = Some((Expand::Level(k), open));
            rows.push(row);
            if !open {
                continue;
            }
            sub_header(rows, &format!("Tiles ({})", level.tiles.len()));
            for (i, t) in level.tiles.iter().enumerate() {
                let name = p.tiles.get(t.tile).map(|x| x.name.clone()).unwrap_or_default();
                let s = Selection::LevelTile { level: k, index: i };
                rows.push(
                    OutRow::plain(
                        2,
                        format!("{}  @ {},{}", if name.is_empty() { "tile".into() } else { name }, t.x as i64, t.y as i64),
                        Act::Select(s),
                        sel == s,
                    )
                    .del(s),
                );
            }
            sub_header(rows, &format!("Backgrounds ({})", level.backgrounds.len()));
            for (i, b) in level.backgrounds.iter().enumerate() {
                let s = Selection::LevelBackground { level: k, index: i };
                rows.push(
                    OutRow::plain(2, format!("bg  @ {},{}", b.x as i64, b.y as i64), Act::Select(s), sel == s)
                        .del(s),
                );
            }
        }
        add_row(rows, "+ Add Level", Act::AddLevel);

        header(rows, "Tile Library");
        for (k, t) in p.tiles.iter() {
            rows.push(
                OutRow::plain(
                    0,
                    if t.name.is_empty() { "tile".into() } else { t.name.clone() },
                    Act::Select(Selection::Tile(k)),
                    sel == Selection::Tile(k),
                )
                .del(Selection::Tile(k)),
            );
        }
        add_row(rows, "+ New Tile", Act::NewTile);

        header(rows, "Backgrounds");
        for (k, bg) in p.backgrounds.iter() {
            rows.push(
                OutRow::plain(
                    0,
                    if bg.name.is_empty() { "background".into() } else { bg.name.clone() },
                    Act::Select(Selection::Background(k)),
                    sel == Selection::Background(k),
                )
                .del(Selection::Background(k)),
            );
        }
        add_row(rows, "+ New Background", Act::NewBackground);

        header(rows, "Accessories");
        for (k, a) in p.accessories.iter() {
            rows.push(
                OutRow::plain(
                    0,
                    if a.name.is_empty() { "accessory".into() } else { a.name.clone() },
                    Act::Select(Selection::Accessory(k)),
                    sel == Selection::Accessory(k),
                )
                .del(Selection::Accessory(k)),
            );
        }
        add_row(rows, "+ New Accessory", Act::NewAccessory);
    }

    fn apply(&self, act: Act) {
        match act {
            Act::AddLevel => self.editor.new_level_open.set(true),
            Act::AddLayer => {
                self.editor.edit(|p| {
                    if let Some(f) = p.session.active.frame {
                        if let Some(l) = p.add_layer_to_frame(f) {
                            p.session.active.layer = Some(l);
                            p.session.selection = Selection::Layer(l);
                        }
                    }
                });
            }
            Act::AddGroup => {
                self.editor.edit(|p| {
                    if let Some(f) = p.session.active.frame {
                        if let Some(g) = p.add_group_to_frame(f) {
                            p.session.selection = Selection::Group(g);
                        }
                    }
                });
            }
            Act::AddFrame => {
                self.editor.edit(|p| {
                    let f = p.add_frame_seeded();
                    p.session.active.frame = Some(f);
                    p.session.selection = Selection::Frame(f);
                });
            }
            Act::AddAnimation => {
                self.editor.edit(|p| {
                    let a = p.add_animation(Default::default());
                    p.session.active.animation = Some(a);
                    p.session.selection = Selection::Animation(a);
                });
            }
            Act::NewTile => {
                self.editor.edit(|p| {
                    let t = p.new_tile();
                    p.session.selection = Selection::Tile(t);
                });
            }
            Act::NewBackground => {
                self.editor.edit(|p| {
                    let b = p.new_background();
                    p.session.selection = Selection::Background(b);
                });
            }
            Act::NewAccessory => {
                self.editor.edit(|p| {
                    let a = p.new_accessory();
                    p.session.selection = Selection::Accessory(a);
                });
            }
            Act::Select(sel) => {
                self.editor.edit(|p| {
                    p.session.selection = sel;
                    match sel {
                        Selection::Layer(k) => p.session.active.layer = Some(k),
                        Selection::Frame(k) => p.session.active.frame = Some(k),
                        Selection::Animation(k) => p.session.active.animation = Some(k),
                        Selection::Level(k)
                        | Selection::LevelTile { level: k, .. }
                        | Selection::LevelBackground { level: k, .. } => {
                            p.session.active.level = Some(k)
                        }
                        _ => {}
                    }
                });
            }
            Act::None => {}
        }
    }
}

fn header(rows: &mut Vec<OutRow>, label: &str) {
    rows.push(OutRow {
        depth: 0,
        label: label.to_uppercase(),
        header: true,
        add: false,
        twisty: None,
        act: Act::None,
        selected: false,
        deletable: None,
    });
}

fn sub_header(rows: &mut Vec<OutRow>, label: &str) {
    rows.push(OutRow {
        depth: 1,
        label: label.to_string(),
        header: true,
        add: false,
        twisty: None,
        act: Act::None,
        selected: false,
        deletable: None,
    });
}

fn add_row(rows: &mut Vec<OutRow>, label: &str, act: Act) {
    rows.push(OutRow {
        depth: 0,
        label: label.to_string(),
        header: false,
        add: true,
        twisty: None,
        act,
        selected: false,
        deletable: None,
    });
}

impl Behavior for OutlineTreeView {
    fn render(&mut self, ctx: &mut RenderContext) {
        let (w, h) = ctx.size();
        let rows = self.rows();
        let r = &mut *ctx.renderer;
        r.fill_rect(Rect::new(0.0, 0.0, w, h), PANEL_BG);
        r.fill_rect(Rect::new(w - 1.0, 0.0, 1.0, h), BORDER);

        if !self.editor.has_project() {
            text(r, "No project", 12.0, 12.0, 12.0, DIM);
            return;
        }

        r.push_clip(Rect::new(0.0, 0.0, w, h));
        let mut y = 6.0 - self.scroll.get();
        for row in &rows {
            if y + ROW_H > 0.0 && y < h {
                let x = BASE_X + row.depth as f32 * INDENT;
                if row.selected {
                    r.fill_rect(Rect::new(0.0, y, w, ROW_H), ACCENT_BG);
                }
                if let Some((_, open)) = row.twisty {
                    let cx = x - 2.0;
                    let cy = y + ROW_H * 0.5;
                    if open {
                        r.fill_rect(Rect::new(cx - 3.0, cy - 2.0, 7.0, 2.0), DIM);
                    } else {
                        r.fill_rect(Rect::new(cx, cy - 3.0, 2.0, 8.0), DIM);
                        r.fill_rect(Rect::new(cx - 3.0, cy, 8.0, 2.0), DIM);
                    }
                }
                let (size, color) = if row.header {
                    (10.5, DIM)
                } else if row.add {
                    (12.0, ACCENT)
                } else {
                    (12.0, INK)
                };
                let tx = if row.twisty.is_some() { x + 10.0 } else { x };
                text(r, &row.label, tx, y + 5.0, size, if row.selected { ACCENT } else { color });

                if row.deletable.is_some() && row.selected {
                    text(r, "x", w - DEL_W + 4.0, y + 5.0, 12.0, DIM);
                }
            }
            y += ROW_H;
        }
        r.pop_clip();
    }

    fn pointer_event(&mut self, ctx: &mut EventContext, event: PointerEvent) {
        let b = ctx.ui.absolute_box(ctx.node);
        match event {
            PointerEvent::Wheel { delta, .. } => {
                let rows = self.rows().len() as f32;
                let max = (rows * ROW_H - b.height + 12.0).max(0.0);
                self.scroll.set((self.scroll.get() - delta * 40.0).clamp(0.0, max));
                ctx.stop_propagation();
            }
            PointerEvent::Down { button: MouseButton::Left, x, y } => {
                let ly = y - b.y - 6.0 + self.scroll.get();
                if ly < 0.0 {
                    return;
                }
                let idx = (ly / ROW_H) as usize;
                let rows = self.rows();
                let Some(row) = rows.get(idx) else { return };
                let lx = x - b.x;

                if let (Some(sel), true) = (row.deletable, lx > b.width - DEL_W) {
                    if row.selected {
                        self.editor.delete_selection();
                        ctx.stop_propagation();
                        return;
                    }
                    let _ = sel;
                }

                let tx = BASE_X + row.depth as f32 * INDENT;
                if let Some((key, open)) = row.twisty {
                    if lx < tx + 8.0 {
                        let mut e = self.expanded.borrow_mut();
                        if open {
                            e.remove(&key);
                        } else {
                            e.insert(key);
                        }
                        ctx.stop_propagation();
                        return;
                    }
                }
                self.apply(row.act);
                ctx.stop_propagation();
            }
            _ => {}
        }
    }
}
