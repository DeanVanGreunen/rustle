//! Column 1: the outline tree. Rebuilt from the app state every frame;
//! the widget draws its own rows plus per-row action buttons and tracks
//! expand / scroll locally.

use std::cell::{Cell, RefCell};
use std::collections::HashSet;

use rustle_core::{
    AnimationId, EditorMode, FrameId, GroupId, LayerId, LevelId, Project, Selection, SpriteCanvas,
    SpriteEntity,
};
use rustle_ui::prelude::*;

use super::theme::*;
use super::Editor;

const ROW_H: f32 = 22.0;
const INDENT: f32 = 13.0;
const BASE_X: f32 = 10.0;
const BTN_H: f32 = 15.0;
const BTN_GAP: f32 = 5.0;

#[derive(Hash, Eq, PartialEq, Clone, Copy)]
enum Expand {
    Entity(SpriteEntity),
    Base(SpriteEntity),
    Anims(SpriteEntity),
    Anim(AnimationId),
    Frame(FrameId),
    Group(GroupId),
    Level(LevelId),
}

#[derive(Clone, Copy)]
enum Act {
    None,
    OpenEntity(SpriteEntity),
    DeleteEntity(SpriteEntity),
    ShowBase(SpriteEntity),
    ShowFrame(FrameId),
    OpenAnimation(SpriteEntity, AnimationId),
    NewAnimation(SpriteEntity),
    NewFrame(AnimationId),
    AddLayerBase(SpriteEntity),
    AddGroupBase(SpriteEntity),
    AddLayerFrame(FrameId),
    AddGroupFrame(FrameId),
    AddLayerGroup(GroupId),
    AddGroupGroup(GroupId),
    SelectLayer(LayerId),
    SelectGroup(GroupId),
    DeleteLayer(LayerId),
    DeleteGroup(GroupId),
    DeleteFrame(FrameId),
    DeleteAnimation(AnimationId),
    // level workspace
    SelectLevelItem(Selection),
    AddLevel,
    NewSprite,
    NewTile,
    NewBackground,
    NewAccessory,
}

#[derive(Clone, Copy, PartialEq)]
enum Btn {
    Open,
    Delete,
    New,
    AddFrame,
    AddLayer,
    AddGroup,
}

impl Btn {
    fn label(self) -> &'static str {
        match self {
            Btn::Open => "Open",
            Btn::Delete => "Delete",
            Btn::New => "New",
            Btn::AddFrame => "+ Frame",
            Btn::AddLayer => "+ Layer",
            Btn::AddGroup => "+ Group",
        }
    }
}

struct Row {
    depth: u8,
    label: String,
    header: bool,
    add: bool,
    bold: bool,
    twisty: Option<(Expand, bool)>,
    click: Act,
    buttons: Vec<(Btn, Act)>,
    selected: bool,
}

impl Row {
    fn item(depth: u8, label: impl Into<String>, click: Act, selected: bool) -> Self {
        Self {
            depth,
            label: label.into(),
            header: false,
            add: false,
            bold: false,
            twisty: None,
            click,
            buttons: Vec::new(),
            selected,
        }
    }
    fn twisty(mut self, e: Expand, open: bool) -> Self {
        self.twisty = Some((e, open));
        self
    }
    fn btn(mut self, b: Btn, a: Act) -> Self {
        self.buttons.push((b, a));
        self
    }
    fn bold(mut self) -> Self {
        self.bold = true;
        self
    }
}

/// Geometry captured at render for the following frame's hit-testing.
#[derive(Default)]
struct HitRow {
    twisty: Option<Expand>,
    twisty_x: f32,
    click: Act,
    buttons: Vec<(f32, f32, Act)>, // x, w, act
}

impl Default for Act {
    fn default() -> Self {
        Act::None
    }
}

pub struct OutlineTreeView {
    editor: Editor,
    kind: EditorMode,
    expanded: RefCell<HashSet<Expand>>,
    scroll: Cell<f32>,
    hits: RefCell<Vec<HitRow>>,
    last_sprite: Cell<Option<SpriteEntity>>,
}

impl OutlineTreeView {
    pub fn new(editor: Editor, kind: EditorMode) -> Self {
        Self {
            editor,
            kind,
            expanded: RefCell::new(HashSet::new()),
            scroll: Cell::new(0.0),
            hits: RefCell::new(Vec::new()),
            last_sprite: Cell::new(None),
        }
    }

    fn open(&self, e: Expand) -> bool {
        self.expanded.borrow().contains(&e)
    }

    fn toggle(&self, e: Expand) {
        let mut s = self.expanded.borrow_mut();
        if !s.remove(&e) {
            s.insert(e);
        }
    }

    fn rows(&self) -> Vec<Row> {
        let mut rows = Vec::new();
        self.editor.with_project(|p| match self.kind {
            EditorMode::Sprite => self.sprite_rows(p, &mut rows),
            EditorMode::Level => self.level_rows(p, &mut rows),
        });
        rows
    }

    // --- sprite / animation workspace --------------------------------

    fn sprite_rows(&self, p: &Project, rows: &mut Vec<Row>) {
        let sel = p.session.selection;
        let active_layer = p.session.active.layer;
        let active_anim = p.session.active.animation;

        // When a different sprite becomes active, collapse everything else
        // and open just that one's subtree.
        let cur = p.session.active.sprite;
        if self.last_sprite.get() != cur {
            self.last_sprite.set(cur);
            let mut ex = self.expanded.borrow_mut();
            ex.clear();
            if let Some(e) = cur {
                ex.insert(Expand::Entity(e));
                ex.insert(Expand::Base(e));
            }
        }

        rows.push(Row::item(0, "+ New Sprite", Act::NewSprite, false).mark_add());

        let cats: [(&str, Vec<SpriteEntity>); 3] = [
            ("Backgrounds", p.backgrounds.keys().map(SpriteEntity::Background).collect()),
            ("Tiles", p.tiles.keys().map(SpriteEntity::Tile).collect()),
            ("Accessories", p.accessories.keys().map(SpriteEntity::Accessory).collect()),
        ];

        for (title, entities) in cats {
            header(rows, title);
            for e in entities {
                let name = p.entity_name(e);
                let e_sel = matches!(
                    (sel, e),
                    (Selection::Background(a), SpriteEntity::Background(b)) if a == b
                ) || matches!((sel, e), (Selection::Tile(a), SpriteEntity::Tile(b)) if a == b)
                    || matches!((sel, e), (Selection::Accessory(a), SpriteEntity::Accessory(b)) if a == b);
                let open = self.open(Expand::Entity(e));
                rows.push(
                    Row::item(1, if name.is_empty() { "unnamed".into() } else { name }, Act::OpenEntity(e), e_sel)
                        .twisty(Expand::Entity(e), open)
                        .btn(Btn::Open, Act::OpenEntity(e))
                        .btn(Btn::Delete, Act::DeleteEntity(e)),
                );
                if !open {
                    continue;
                }

                // Base image.
                let base_open = self.open(Expand::Base(e));
                rows.push(
                    Row::item(2, "Base Image", Act::ShowBase(e), false)
                        .twisty(Expand::Base(e), base_open)
                        .btn(Btn::AddLayer, Act::AddLayerBase(e))
                        .btn(Btn::AddGroup, Act::AddGroupBase(e)),
                );
                if base_open {
                    if let Some(bf) = p.entity_base_frame(e).and_then(|k| p.base_frame.get(k)) {
                        for &g in &bf.groups {
                            self.group_subtree(p, g, 3, sel, active_layer, rows);
                        }
                        for (i, &l) in bf.layers.iter().enumerate() {
                            self.layer_row(p, l, i, 3, sel, active_layer, rows);
                        }
                    }
                }

                // Animations.
                let anims_open = self.open(Expand::Anims(e));
                rows.push(
                    Row::item(2, "Animations", Act::None, false)
                        .twisty(Expand::Anims(e), anims_open)
                        .btn(Btn::New, Act::NewAnimation(e)),
                );
                if anims_open {
                    for (n, a) in p.entity_animations(e).into_iter().enumerate() {
                        let a_open = self.open(Expand::Anim(a));
                        let a_sel = sel == Selection::Animation(a) || active_anim == Some(a);
                        let name = p.animations.get(a).map(|x| x.name.clone()).unwrap_or_default();
                        let count = p.animations.get(a).map(|x| x.frames.len()).unwrap_or(0);
                        let label = if name.trim().is_empty() {
                            format!("Animation {} ({count})", n + 1)
                        } else {
                            format!("{name} ({count})")
                        };
                        rows.push(
                            Row::item(3, label, Act::OpenAnimation(e, a), a_sel)
                                .twisty(Expand::Anim(a), a_open)
                                .btn(Btn::Open, Act::OpenAnimation(e, a))
                                .btn(Btn::AddFrame, Act::NewFrame(a))
                                .btn(Btn::Delete, Act::DeleteAnimation(a)),
                        );
                        if !a_open {
                            continue;
                        }
                        let frames: Vec<FrameId> = p
                            .animations
                            .get(a)
                            .map(|x| x.frames.iter().copied().collect())
                            .unwrap_or_default();
                        for (fi, f) in frames.iter().copied().enumerate() {
                            let f_open = self.open(Expand::Frame(f));
                            let f_sel = matches!(p.session.active.canvas, SpriteCanvas::Frame(x) if x == f);
                            rows.push(
                                Row::item(4, format!("Frame {}", fi + 1), Act::ShowFrame(f), f_sel)
                                    .twisty(Expand::Frame(f), f_open)
                                    .btn(Btn::Open, Act::ShowFrame(f))
                                    .btn(Btn::AddLayer, Act::AddLayerFrame(f))
                                    .btn(Btn::AddGroup, Act::AddGroupFrame(f))
                                    .btn(Btn::Delete, Act::DeleteFrame(f)),
                            );
                            if f_open {
                                if let Some(fr) = p.frames.get(f) {
                                    for &g in &fr.groups {
                                        self.group_subtree(p, g, 5, sel, active_layer, rows);
                                    }
                                    for (i, &l) in fr.layers.iter().enumerate() {
                                        self.layer_row(p, l, i, 5, sel, active_layer, rows);
                                    }
                                }
                            }
                        }
                        rows.push(Row::item(4, "+ Frame", Act::NewFrame(a), false).mark_add());
                    }
                }
            }
        }
    }

    fn group_subtree(
        &self,
        p: &Project,
        g: GroupId,
        depth: u8,
        sel: Selection,
        active_layer: Option<LayerId>,
        rows: &mut Vec<Row>,
    ) {
        let Some(group) = p.groups.get(g) else { return };
        let open = self.open(Expand::Group(g));
        rows.push(
            Row::item(
                depth,
                {
                    let base = if group.name.trim().is_empty() { "Group" } else { group.name.trim() };
                    format!("{base} ({} / {})", group.layers.len(), group.groups.len())
                },
                Act::SelectGroup(g),
                sel == Selection::Group(g),
            )
            .twisty(Expand::Group(g), open)
            .btn(Btn::Open, Act::SelectGroup(g))
            .btn(Btn::AddLayer, Act::AddLayerGroup(g))
            .btn(Btn::AddGroup, Act::AddGroupGroup(g))
            .btn(Btn::Delete, Act::DeleteGroup(g)),
        );
        if open {
            for &child in &group.groups {
                self.group_subtree(p, child, depth + 1, sel, active_layer, rows);
            }
            for (i, &l) in group.layers.iter().enumerate() {
                self.layer_row(p, l, i, depth + 1, sel, active_layer, rows);
            }
        }
    }

    fn layer_row(
        &self,
        p: &Project,
        l: LayerId,
        i: usize,
        depth: u8,
        sel: Selection,
        active_layer: Option<LayerId>,
        rows: &mut Vec<Row>,
    ) {
        let vis = p.layers.get(l).map(|x| x.visible).unwrap_or(true);
        let label = format!("{}Layer {}", if vis { "" } else { "(hidden) " }, i + 1);
        let active = active_layer == Some(l);
        let mut row = Row::item(
            depth,
            label,
            Act::SelectLayer(l),
            active || sel == Selection::Layer(l),
        )
        .btn(Btn::Open, Act::SelectLayer(l))
        .btn(Btn::Delete, Act::DeleteLayer(l));
        if active {
            row = row.bold();
        }
        rows.push(row);
    }

    // --- level workspace -------------------------------------------

    fn level_rows(&self, p: &Project, rows: &mut Vec<Row>) {
        let sel = p.session.selection;

        header(rows, "Levels");
        for (k, level) in p.levels.iter() {
            let open = self.open(Expand::Level(k));
            rows.push(
                Row::item(
                    1,
                    if level.name.is_empty() { "Level".into() } else { level.name.clone() },
                    Act::SelectLevelItem(Selection::Level(k)),
                    sel == Selection::Level(k),
                )
                .twisty(Expand::Level(k), open)
                .btn(Btn::Delete, Act::SelectLevelItem(Selection::Level(k))),
            );
            if !open {
                continue;
            }
            sub_header(rows, &format!("Backgrounds ({})", level.backgrounds.len()));
            for (i, b) in level.backgrounds.iter().enumerate() {
                let name = p.backgrounds.get(b.background).map(|x| x.name.clone()).unwrap_or_default();
                let s = Selection::LevelBackground { level: k, index: i };
                rows.push(Row::item(3, disp(name, "background"), Act::SelectLevelItem(s), sel == s));
            }
            sub_header(rows, &format!("Tiles ({})", level.tiles.len()));
            for (i, t) in level.tiles.iter().enumerate() {
                let name = p.tiles.get(t.tile).map(|x| x.name.clone()).unwrap_or_default();
                let s = Selection::LevelTile { level: k, index: i };
                rows.push(Row::item(3, disp(name, "tile"), Act::SelectLevelItem(s), sel == s));
            }
            sub_header(rows, &format!("Accessories ({})", level.accessories.len()));
            for (i, a) in level.accessories.iter().enumerate() {
                let name = p.accessories.get(a.accessory).map(|x| x.name.clone()).unwrap_or_default();
                let s = Selection::LevelAccessory { level: k, index: i };
                rows.push(Row::item(3, disp(name, "accessory"), Act::SelectLevelItem(s), sel == s));
            }
        }
        rows.push(Row::item(0, "+ Add Level", Act::AddLevel, false).mark_add());

        header(rows, "Tile Library");
        for (k, t) in p.tiles.iter() {
            rows.push(Row::item(
                1,
                disp(t.name.clone(), "tile"),
                Act::SelectLevelItem(Selection::Tile(k)),
                sel == Selection::Tile(k),
            ));
        }
        rows.push(Row::item(0, "+ New Tile", Act::NewTile, false).mark_add());

        header(rows, "Background Library");
        for (k, b) in p.backgrounds.iter() {
            rows.push(Row::item(
                1,
                disp(b.name.clone(), "background"),
                Act::SelectLevelItem(Selection::Background(k)),
                sel == Selection::Background(k),
            ));
        }
        rows.push(Row::item(0, "+ New Background", Act::NewBackground, false).mark_add());

        header(rows, "Accessory Library");
        for (k, a) in p.accessories.iter() {
            rows.push(Row::item(
                1,
                disp(a.name.clone(), "accessory"),
                Act::SelectLevelItem(Selection::Accessory(k)),
                sel == Selection::Accessory(k),
            ));
        }
        rows.push(Row::item(0, "+ New Accessory", Act::NewAccessory, false).mark_add());
    }

    // --- act -----------------------------------------------------

    fn apply(&self, act: Act) {
        let ed = &self.editor;
        match act {
            Act::None => {}
            Act::OpenEntity(e) => ed.open_entity(e),
            Act::ShowFrame(f) => ed.show_canvas(SpriteCanvas::Frame(f)),
            Act::SelectGroup(g) => ed.select(Selection::Group(g)),
            Act::AddLevel => ed.new_level_open.set(true),
            Act::DeleteEntity(e) => {
                ed.edit(|p| match e {
                    SpriteEntity::Tile(k) => p.remove_tile(k),
                    SpriteEntity::Background(k) => p.remove_background(k),
                    SpriteEntity::Accessory(k) => p.remove_accessory(k),
                });
            }
            Act::ShowBase(e) => {
                ed.edit(|p| {
                    let bf = p.ensure_base_frame(e);
                    p.session.active.sprite = Some(e);
                    p.session.active.animation = None;
                    p.session.active.canvas = SpriteCanvas::Base(bf);
                });
            }
            Act::OpenAnimation(e, a) => {
                ed.edit(|p| {
                    p.session.active.sprite = Some(e);
                    p.session.active.animation = Some(a);
                    p.session.selection = Selection::Animation(a);
                    if let Some(f) = p.animations.get(a).and_then(|x| x.frames.front().copied()) {
                        p.session.active.canvas = SpriteCanvas::Frame(f);
                        p.session.active.frame = Some(f);
                        p.restore_frame_layer(f);
                    }
                });
            }
            Act::NewSprite => ed.new_sprite_open.set(true),
            Act::NewAnimation(e) => ed.add_anim_open.set(Some(e)),
            Act::NewFrame(a) => ed.add_frame_open.set(Some(a)),
            Act::AddLayerBase(e) => {
                ed.edit(|p| {
                    let l = p.add_layer_to_base(e);
                    p.session.active.layer = Some(l);
                    p.session.selection = Selection::Layer(l);
                });
            }
            Act::AddGroupBase(e) => {
                ed.add_group_open.set(Some(super::GroupTarget::Base(e)));
            }
            Act::AddLayerFrame(f) => {
                ed.edit(|p| {
                    if let Some(l) = p.add_layer_to_frame(f) {
                        p.session.active.layer = Some(l);
                        p.session.selection = Selection::Layer(l);
                        p.remember_frame_layer();
                    }
                });
            }
            Act::AddGroupFrame(f) => {
                ed.add_group_open.set(Some(super::GroupTarget::Frame(f)));
            }
            Act::AddLayerGroup(g) => {
                ed.edit(|p| {
                    if let Some(l) = p.add_layer_to_group(g) {
                        p.session.active.layer = Some(l);
                        p.session.selection = Selection::Layer(l);
                        p.remember_frame_layer();
                    }
                });
            }
            Act::AddGroupGroup(g) => {
                ed.add_group_open.set(Some(super::GroupTarget::Group(g)));
            }
            Act::SelectLayer(l) => {
                ed.edit(|p| {
                    p.session.active.layer = Some(l);
                    p.session.selection = Selection::Layer(l);
                    p.remember_frame_layer();
                });
            }
            Act::DeleteLayer(l) => {
                ed.edit(|p| p.remove_layer(l));
            }
            Act::DeleteGroup(g) => {
                ed.edit(|p| p.remove_group(g));
            }
            Act::DeleteFrame(f) => {
                ed.edit(|p| p.remove_frame(f));
            }
            Act::DeleteAnimation(a) => {
                ed.edit(|p| p.remove_animation(a));
            }
            Act::SelectLevelItem(s) => {
                ed.edit(|p| {
                    p.session.selection = s;
                    match s {
                        Selection::Level(k)
                        | Selection::LevelTile { level: k, .. }
                        | Selection::LevelBackground { level: k, .. }
                        | Selection::LevelAccessory { level: k, .. } => {
                            p.session.active.level = Some(k)
                        }
                        _ => {}
                    }
                });
            }
            Act::NewTile => {
                ed.edit(|p| {
                    let t = p.new_tile();
                    p.session.selection = Selection::Tile(t);
                });
            }
            Act::NewBackground => {
                ed.edit(|p| {
                    let b = p.new_background();
                    p.session.selection = Selection::Background(b);
                });
            }
            Act::NewAccessory => {
                ed.edit(|p| {
                    let a = p.new_accessory();
                    p.session.selection = Selection::Accessory(a);
                });
            }
        }
    }
}

impl Row {
    fn mark_add(mut self) -> Self {
        self.add = true;
        self
    }
}

fn header(rows: &mut Vec<Row>, label: &str) {
    let mut r = Row::item(0, label.to_uppercase(), Act::None, false);
    r.header = true;
    rows.push(r);
}

fn sub_header(rows: &mut Vec<Row>, label: &str) {
    let mut r = Row::item(2, label.to_string(), Act::None, false);
    r.header = true;
    rows.push(r);
}

fn disp(name: String, fallback: &str) -> String {
    if name.trim().is_empty() { fallback.to_string() } else { name }
}


impl Behavior for OutlineTreeView {
    fn render(&mut self, ctx: &mut RenderContext) {
        let (w, h) = ctx.size();
        let rows = self.rows();
        let r = &mut *ctx.renderer;
        r.fill_rect(Rect::new(0.0, 0.0, w, h), PANEL_BG);
        r.fill_rect(Rect::new(w - 1.0, 0.0, 1.0, h), BORDER);

        let mut hits: Vec<HitRow> = Vec::with_capacity(rows.len());

        if !self.editor.has_project() {
            text(r, "No project", 12.0, 12.0, 12.0, DIM);
            self.hits.replace(hits);
            return;
        }

        r.push_clip(Rect::new(0.0, 0.0, w, h));
        let mut y = 6.0 - self.scroll.get();
        for row in &rows {
            let mut hit = HitRow { click: row.click, ..Default::default() };
            if y + ROW_H > 0.0 && y < h {
                let x = BASE_X + row.depth as f32 * INDENT;
                if row.selected {
                    r.fill_rect(Rect::new(0.0, y, w, ROW_H), ACCENT_BG);
                }
                if let Some((e, open)) = row.twisty {
                    let cx = x - 2.0;
                    let cy = y + ROW_H * 0.5;
                    if open {
                        r.fill_rect(Rect::new(cx - 3.0, cy - 2.0, 7.0, 2.0), DIM);
                    } else {
                        r.fill_rect(Rect::new(cx, cy - 3.0, 2.0, 8.0), DIM);
                        r.fill_rect(Rect::new(cx - 3.0, cy, 8.0, 2.0), DIM);
                    }
                    hit.twisty = Some(e);
                    hit.twisty_x = x + 8.0;
                }
                let (size, color) = if row.header {
                    (10.5, DIM)
                } else if row.add {
                    (12.0, ACCENT)
                } else {
                    (12.0, if row.selected { ACCENT } else { INK })
                };
                let tx = if row.twisty.is_some() { x + 10.0 } else { x };
                text(r, &row.label, tx, y + 5.0, size, color);
                if row.bold {
                    text(r, &row.label, tx + 0.4, y + 5.0, size, color);
                }

                // Buttons after the label. A Delete button is always
                // pinned to the right edge so it can't be pushed off by a
                // crowded, deeply-indented row.
                let bstyle = TextStyle { size: 10.0, color: DIM, font: FontId::DEFAULT };
                let draw_btn = |r: &mut Renderer, rect: Rect, b: Btn| {
                    r.fill_rounded_rect(rect, 7.0, PANEL_BG_ALT);
                    outline(r, rect);
                    let c = if b == Btn::Delete { Color::hex(0x9a4a4a) } else { DIM };
                    text(r, b.label(), rect.x + 7.0, rect.y + 2.0, 10.0, c);
                };
                let by = y + (ROW_H - BTN_H) * 0.5;

                let del = row.buttons.iter().find(|(b, _)| *b == Btn::Delete).copied();
                let mut right = w - 6.0;
                if let Some((_, act)) = del {
                    let dw = r.measure("Delete", &bstyle) + 14.0;
                    let rx = right - dw;
                    draw_btn(r, Rect::new(rx, by, dw, BTN_H), Btn::Delete);
                    hit.buttons.push((rx, dw, act));
                    right = rx - BTN_GAP;
                }

                let lw = r.measure(&row.label, &TextStyle { size, color, font: FontId::DEFAULT });
                let mut bx = tx + lw + 10.0;
                for (b, act) in row.buttons.iter().filter(|(b, _)| *b != Btn::Delete) {
                    let bw = r.measure(b.label(), &bstyle) + 14.0;
                    if bx + bw > right {
                        break;
                    }
                    draw_btn(r, Rect::new(bx, by, bw, BTN_H), *b);
                    hit.buttons.push((bx, bw, *act));
                    bx += bw + BTN_GAP;
                }
            } else {
                // Off-screen: still record button geometry for hit-testing.
                if let Some((e, _)) = row.twisty {
                    let x = BASE_X + row.depth as f32 * INDENT;
                    hit.twisty = Some(e);
                    hit.twisty_x = x + 8.0;
                }
            }
            hits.push(hit);
            y += ROW_H;
        }
        r.pop_clip();
        self.hits.replace(hits);
    }

    fn pointer_event(&mut self, ctx: &mut EventContext, event: PointerEvent) {
        let b = ctx.ui.absolute_box(ctx.node);
        match event {
            PointerEvent::Wheel { delta, .. } => {
                let n = self.rows().len() as f32;
                let max = (n * ROW_H - b.height + 12.0).max(0.0);
                self.scroll.set((self.scroll.get() - delta * 40.0).clamp(0.0, max));
                ctx.stop_propagation();
            }
            PointerEvent::Down { button: MouseButton::Left, x, y } => {
                let ly = y - b.y - 6.0 + self.scroll.get();
                if ly < 0.0 {
                    return;
                }
                let idx = (ly / ROW_H) as usize;
                let lx = x - b.x;
                let hits = self.hits.borrow();
                let Some(hit) = hits.get(idx) else { return };

                for (bx, bw, act) in &hit.buttons {
                    if lx >= *bx && lx <= *bx + *bw {
                        let act = *act;
                        drop(hits);
                        self.apply(act);
                        ctx.stop_propagation();
                        return;
                    }
                }
                if let Some(e) = hit.twisty {
                    if lx < hit.twisty_x {
                        drop(hits);
                        self.toggle(e);
                        ctx.stop_propagation();
                        return;
                    }
                }
                let act = hit.click;
                drop(hits);
                self.apply(act);
                ctx.stop_propagation();
            }
            _ => {}
        }
    }
}

fn outline(r: &mut Renderer, rect: Rect) {
    r.fill_rect(Rect::new(rect.x, rect.y, rect.width, 1.0), BORDER);
    r.fill_rect(Rect::new(rect.x, rect.y + rect.height - 1.0, rect.width, 1.0), BORDER);
    r.fill_rect(Rect::new(rect.x, rect.y, 1.0, rect.height), BORDER);
    r.fill_rect(Rect::new(rect.x + rect.width - 1.0, rect.y, 1.0, rect.height), BORDER);
}
