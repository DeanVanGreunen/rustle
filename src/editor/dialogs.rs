//! The sprite-workspace modals: New Sprite, Add Animation, and + Frame.
//! Built from real nodes (TextField + rustle_ui Dropdown) with an
//! always-alive controller sibling per dialog, like `newlevel.rs`.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use rustle_core::{AnimationId, FrameId, Selection, SpriteCanvas, SpriteEntity, SpriteKind};
use rustle_ui::prelude::*;
use rustle_ui::widgets::{Button, Dropdown, Label, Panel, TextField};

use taffy::prelude::{length, percent};
use taffy::style::{AlignItems, JustifyContent};

use super::theme::*;
use super::{Editor, GroupTarget, QuitChoice};

pub fn spawn_sprite_dialogs(ui: &mut UiTree, root: NodeId, editor: &Editor) {
    spawn_new_sprite(ui, root, editor);
    spawn_add_animation(ui, root, editor);
    spawn_add_frame(ui, root, editor);
    spawn_new_group(ui, root, editor);
    spawn_resize_canvas(ui, root, editor);
}

// --- Quit prompt (unsaved changes) --------------------------

pub fn spawn_quit_prompt(ui: &mut UiTree, root: NodeId, editor: &Editor) {
    let overlay = ui
        .spawn(root, overlay_style(), Panel::new().background(Color::rgba(0.0, 0.0, 0.0, 0.45)))
        .unwrap();
    ui.set_display(overlay, false);
    let card = ui
        .spawn(overlay, card_style(400.0), Panel::new().background(Color::WHITE).corner_radius(12.0))
        .unwrap();

    // Header: title + close (X) that cancels the quit.
    let mut header = Style::row().gap(10.0);
    header.taffy.size.width = percent(1.0);
    header.taffy.justify_content = Some(JustifyContent::SPACE_BETWEEN);
    header.taffy.align_items = Some(AlignItems::CENTER);
    let header = ui.spawn(card, header, Panel::new()).unwrap();
    ui.spawn(header, Style::default(), Label::new("Unsaved Changes").size(18.0).color(INK)).unwrap();
    ui.spawn(
        header,
        Style::default().width(26.0).height(26.0),
        Button::new("X").on_click({
            let c = editor.quit_choice.clone();
            move || c.set(QuitChoice::Cancel)
        }),
    )
    .unwrap();

    ui.spawn(
        card,
        Style::default(),
        Label::new("Save your changes before closing?").size(13.0).color(DIM),
    )
    .unwrap();

    let mut row = Style::row().gap(10.0);
    row.taffy.size.width = percent(1.0);
    row.taffy.justify_content = Some(JustifyContent::FLEX_END);
    let row = ui.spawn(card, row, Panel::new()).unwrap();
    ui.spawn(row, Style::default().height(34.0), Button::new("Don't Save").on_click({
        let c = editor.quit_choice.clone();
        move || c.set(QuitChoice::Discard)
    }))
    .unwrap();
    ui.spawn(row, Style::default().height(34.0), Button::new("Save").on_click({
        let c = editor.quit_choice.clone();
        move || c.set(QuitChoice::Save)
    }))
    .unwrap();

    ui.spawn(root, Style::default(), QuitPromptCtl { editor: editor.clone(), overlay }).unwrap();
}

struct QuitPromptCtl {
    editor: Editor,
    overlay: NodeId,
}

impl Behavior for QuitPromptCtl {
    fn update(&mut self, ctx: &mut UpdateContext) {
        ctx.ui.set_display(self.overlay, self.editor.quit_prompt.get());
    }
}

// --- Resize Canvas -------------------------------------------

/// 3×3 anchor picker: which edge / corner stays pinned as the canvas
/// grows or shrinks. Writes `(h, v)` each 0..2 into the shared cell.
struct AnchorGrid {
    sel: Rc<Cell<(u8, u8)>>,
    size: Cell<(f32, f32)>,
}

impl Behavior for AnchorGrid {
    fn render(&mut self, ctx: &mut RenderContext) {
        let (w, h) = ctx.size();
        self.size.set((w, h));
        let r = &mut *ctx.renderer;
        let (cw, ch) = (w / 3.0, h / 3.0);
        let (sc, sr) = self.sel.get();
        for gy in 0u8..3 {
            for gx in 0u8..3 {
                let rect = Rect::new(gx as f32 * cw + 2.0, gy as f32 * ch + 2.0, cw - 4.0, ch - 4.0);
                let on = gx == sc && gy == sr;
                r.fill_rounded_rect(rect, 3.0, if on { ACCENT } else { FIELD_BG });
                // little dot marking the pinned corner/edge
                let dot = Rect::new(
                    rect.x + rect.width * 0.5 - 2.0,
                    rect.y + rect.height * 0.5 - 2.0,
                    4.0,
                    4.0,
                );
                r.fill_rect(dot, if on { Color::WHITE } else { INK });
            }
        }
    }

    fn pointer_event(&mut self, ctx: &mut EventContext, event: PointerEvent) {
        if let PointerEvent::Down { button: MouseButton::Left, x, y } = event {
            let (w, h) = self.size.get();
            let (w, h) = (w.max(1.0), h.max(1.0));
            let gx = ((x / (w / 3.0)).floor() as i32).clamp(0, 2) as u8;
            let gy = ((y / (h / 3.0)).floor() as i32).clamp(0, 2) as u8;
            self.sel.set((gx, gy));
            ctx.stop_propagation();
        }
    }
}

fn spawn_resize_canvas(ui: &mut UiTree, root: NodeId, editor: &Editor) {
    let overlay = ui
        .spawn(root, overlay_style(), Panel::new().background(Color::rgba(0.0, 0.0, 0.0, 0.4)))
        .unwrap();
    ui.set_display(overlay, false);
    let card = ui
        .spawn(overlay, card_style(420.0), Panel::new().background(Color::WHITE).corner_radius(12.0))
        .unwrap();

    ui.spawn(card, Style::default(), Label::new("Resize Canvas").size(18.0).color(INK)).unwrap();

    let pair = |ui: &mut UiTree, card, a: &str, av: &str, b: &str, bv: &str| {
        let mut row = Style::row().gap(10.0);
        row.taffy.size.width = percent(1.0);
        let row = ui.spawn(card, row, Panel::new()).unwrap();
        let mut half = Style::default();
        half.taffy.flex_grow = 1.0;
        half.taffy.flex_basis = length(0.0);
        half.taffy.size.height = length(50.0);
        let x = ui.spawn(row, half.clone(), TextField::new(a).value(av)).unwrap();
        let y = ui.spawn(row, half, TextField::new(b).value(bv)).unwrap();
        (x, y)
    };

    let (w, h) = pair(ui, card, "Width", "0", "Height", "0");
    ui.spawn(card, Style::default(), Label::new("Borders").size(13.0).color(DIM)).unwrap();
    let (top, bottom) = pair(ui, card, "Top", "0", "Bottom", "0");
    let (left, right) = pair(ui, card, "Left", "0", "Right", "0");

    let sel = Rc::new(Cell::new((1u8, 1u8)));
    let mut grid_row = Style::row();
    grid_row.taffy.size.width = percent(1.0);
    grid_row.taffy.justify_content = Some(JustifyContent::CENTER);
    let grid_row = ui.spawn(card, grid_row, Panel::new()).unwrap();
    let mut gstyle = Style::default();
    gstyle.taffy.size.width = length(120.0);
    gstyle.taffy.size.height = length(120.0);
    gstyle.taffy.flex_shrink = 0.0;
    ui.spawn(grid_row, gstyle, AnchorGrid { sel: sel.clone(), size: Cell::new((120.0, 120.0)) }).unwrap();

    let action = Rc::new(Cell::new(0u8));
    buttons_row(ui, card, action.clone());

    ui.spawn(
        root,
        Style::default(),
        ResizeCanvasCtl {
            editor: editor.clone(),
            overlay,
            fields: [w, h, top, bottom, left, right],
            sel,
            action,
            primed: false,
            orig: (1, 1),
            last: RefCell::new(std::array::from_fn(|_| String::new())),
        },
    )
    .unwrap();
}

struct ResizeCanvasCtl {
    editor: Editor,
    overlay: NodeId,
    /// [width, height, top, bottom, left, right]
    fields: [NodeId; 6],
    sel: Rc<Cell<(u8, u8)>>,
    action: Rc<Cell<u8>>,
    primed: bool,
    orig: (u32, u32),
    last: RefCell<[String; 6]>,
}

impl ResizeCanvasCtl {
    fn field_val(ctx: &UpdateContext, n: NodeId) -> String {
        ctx.ui.widget::<TextField>(n).map(|f| f.value.clone()).unwrap_or_default()
    }
    fn set_field(ctx: &mut UpdateContext, n: NodeId, v: &str) {
        if let Some(f) = ctx.ui.widget_mut::<TextField>(n) {
            if f.value != v {
                f.value = v.to_string();
                f.cursor = f.value.chars().count();
            }
        }
    }
}

impl Behavior for ResizeCanvasCtl {
    fn update(&mut self, ctx: &mut UpdateContext) {
        let open = self.editor.resize_canvas_open.get();
        ctx.ui.set_display(self.overlay, open);
        if !open {
            self.primed = false;
            self.action.set(0);
            return;
        }

        let entity = self.editor.session(|s| s.active.sprite).flatten();
        let Some(entity) = entity else {
            // nothing to resize
            self.editor.resize_canvas_open.set(false);
            return;
        };

        if !self.primed {
            self.primed = true;
            self.orig = self.editor.with_project(|p| p.entity_size(entity)).unwrap_or((1, 1));
            self.sel.set((1, 1));
            let vals = [
                self.orig.0.to_string(),
                self.orig.1.to_string(),
                "0".to_string(),
                "0".to_string(),
                "0".to_string(),
                "0".to_string(),
            ];
            for (i, n) in self.fields.iter().enumerate() {
                Self::set_field(ctx, *n, &vals[i]);
            }
            *self.last.borrow_mut() = vals;
        }

        // Live coupling between the border fields and W/H.
        let cur: [String; 6] = std::array::from_fn(|i| Self::field_val(ctx, self.fields[i]));
        let pi = |s: &str| s.trim().parse::<i64>().unwrap_or(0);
        let last = self.last.borrow().clone();
        let border_changed = (2..6).any(|i| cur[i] != last[i]);
        let wh_changed = cur[0] != last[0] || cur[1] != last[1];
        let (ow, oh) = (self.orig.0 as i64, self.orig.1 as i64);

        let mut next = cur.clone();
        if border_changed {
            let w = (ow + pi(&cur[4]) + pi(&cur[5])).clamp(1, 8192);
            let h = (oh + pi(&cur[2]) + pi(&cur[3])).clamp(1, 8192);
            next[0] = w.to_string();
            next[1] = h.to_string();
        } else if wh_changed {
            let w = pi(&cur[0]).max(1);
            let h = pi(&cur[1]).max(1);
            next[5] = (w - ow - pi(&cur[4])).to_string(); // right
            next[3] = (h - oh - pi(&cur[2])).to_string(); // bottom
        }
        if next != cur {
            for (i, n) in self.fields.iter().enumerate() {
                Self::set_field(ctx, *n, &next[i]);
            }
        }
        *self.last.borrow_mut() = std::array::from_fn(|i| Self::field_val(ctx, self.fields[i]));

        match self.action.replace(0) {
            1 => {
                let w = Self::field_val(ctx, self.fields[0]).trim().parse::<u32>().unwrap_or(self.orig.0).clamp(1, 8192);
                let h = Self::field_val(ctx, self.fields[1]).trim().parse::<u32>().unwrap_or(self.orig.1).clamp(1, 8192);
                let anchor = self.sel.get();
                self.editor.edit(|p| p.resize_sprite(entity, w, h, anchor));
                self.editor.fit_request.set(true);
                self.editor.resize_canvas_open.set(false);
            }
            2 => self.editor.resize_canvas_open.set(false),
            _ => {}
        }
    }
}

// --- New Group ------------------------------------------------

fn spawn_new_group(ui: &mut UiTree, root: NodeId, editor: &Editor) {
    let overlay = ui
        .spawn(root, overlay_style(), Panel::new().background(Color::rgba(0.0, 0.0, 0.0, 0.4)))
        .unwrap();
    ui.set_display(overlay, false);
    let card = ui
        .spawn(overlay, card_style(360.0), Panel::new().background(Color::WHITE).corner_radius(12.0))
        .unwrap();

    ui.spawn(card, Style::default(), Label::new("New Group").size(18.0).color(INK)).unwrap();
    let name = ui.spawn(card, full(52.0), TextField::new("Group name")).unwrap();

    let action = Rc::new(Cell::new(0u8));
    buttons_row(ui, card, action.clone());

    ui.spawn(
        root,
        Style::default(),
        NewGroupCtl { editor: editor.clone(), overlay, name, action, primed: false },
    )
    .unwrap();
}

struct NewGroupCtl {
    editor: Editor,
    overlay: NodeId,
    name: NodeId,
    action: Rc<Cell<u8>>,
    primed: bool,
}

impl Behavior for NewGroupCtl {
    fn update(&mut self, ctx: &mut UpdateContext) {
        let target = self.editor.add_group_open.get();
        let open = target.is_some();
        ctx.ui.set_display(self.overlay, open);
        if !open {
            self.primed = false;
            self.action.set(0);
            return;
        }
        if !self.primed {
            self.primed = true;
            if let Some(t) = ctx.ui.widget_mut::<TextField>(self.name) {
                t.value.clear();
            }
        }
        match self.action.replace(0) {
            1 => {
                let name = ctx
                    .ui
                    .widget::<TextField>(self.name)
                    .map(|f| f.value.trim().to_string())
                    .unwrap_or_default();
                let target = target.unwrap();
                self.editor.edit(|p| {
                    let g = match target {
                        GroupTarget::Base(e) => Some(p.add_group_to_base(e)),
                        GroupTarget::Frame(f) => p.add_group_to_frame(f),
                        GroupTarget::Group(parent) => p.add_group_to_group(parent),
                    };
                    if let Some(g) = g {
                        if !name.is_empty() {
                            if let Some(grp) = p.groups.get_mut(g) {
                                grp.name = name.clone();
                            }
                        }
                        p.session.selection = Selection::Group(g);
                    }
                });
                self.editor.add_group_open.set(None);
            }
            2 => self.editor.add_group_open.set(None),
            _ => {}
        }
    }
}

// --- shared layout helpers ---------------------------------------

fn overlay_style() -> Style {
    let mut s = Style::column();
    s.taffy.position = taffy::style::Position::Absolute;
    s.taffy.inset = taffy::geometry::Rect { left: length(0.0), right: length(0.0), top: length(0.0), bottom: length(0.0) };
    s.taffy.justify_content = Some(JustifyContent::CENTER);
    s.taffy.align_items = Some(AlignItems::CENTER);
    s
}

fn card_style(width: f32) -> Style {
    let mut s = Style::column().gap(12.0);
    s.taffy.size.width = length(width);
    s.taffy.flex_shrink = 0.0;
    s.taffy.padding = taffy::geometry::Rect { left: length(24.0), right: length(24.0), top: length(20.0), bottom: length(20.0) };
    s
}

fn full(h: f32) -> Style {
    let mut s = Style::default();
    s.taffy.size.width = percent(1.0);
    s.taffy.size.height = length(h);
    s.taffy.flex_shrink = 0.0;
    s
}

fn buttons_row(ui: &mut UiTree, card: NodeId, action: Rc<Cell<u8>>) {
    let mut row = Style::row().gap(10.0);
    row.taffy.size.width = percent(1.0);
    row.taffy.justify_content = Some(JustifyContent::FLEX_END);
    let row = ui.spawn(card, row, Panel::new()).unwrap();
    ui.spawn(row, Style::default().height(34.0), Button::new("Cancel").on_click({
        let a = action.clone();
        move || a.set(2)
    }))
    .unwrap();
    ui.spawn(row, Style::default().height(34.0), Button::new("Confirm").on_click({
        let a = action.clone();
        move || a.set(1)
    }))
    .unwrap();
}

fn parse_dim(ui: &UiTree, field: NodeId, default: u32) -> u32 {
    ui.widget::<TextField>(field)
        .and_then(|f| f.value.trim().parse::<u32>().ok())
        .unwrap_or(default)
        .clamp(1, 8192)
}

// --- New Sprite -------------------------------------------------

fn spawn_new_sprite(ui: &mut UiTree, root: NodeId, editor: &Editor) {
    let overlay = ui
        .spawn(root, overlay_style(), Panel::new().background(Color::rgba(0.0, 0.0, 0.0, 0.4)))
        .unwrap();
    ui.set_display(overlay, false);
    let card = ui
        .spawn(overlay, card_style(380.0), Panel::new().background(Color::WHITE).corner_radius(12.0))
        .unwrap();

    ui.spawn(card, Style::default(), Label::new("New Sprite").size(18.0).color(INK)).unwrap();

    let kind = ui
        .spawn(card, full(38.0), Dropdown::new(["Background", "Tile", "Accessory"]).selected(0))
        .unwrap();
    let name = ui.spawn(card, full(52.0), TextField::new("Sprite name")).unwrap();

    let mut size_row = Style::row().gap(10.0);
    size_row.taffy.size.width = percent(1.0);
    let size_row = ui.spawn(card, size_row, Panel::new()).unwrap();
    let mut half = Style::default();
    half.taffy.flex_grow = 1.0;
    half.taffy.flex_basis = length(0.0);
    half.taffy.size.height = length(52.0);
    let w = ui.spawn(size_row, half.clone(), TextField::new("Width").value("64")).unwrap();
    let h = ui.spawn(size_row, half, TextField::new("Height").value("64")).unwrap();

    let action = Rc::new(Cell::new(0u8));
    buttons_row(ui, card, action.clone());

    ui.spawn(
        root,
        Style::default(),
        NewSpriteCtl { editor: editor.clone(), overlay, kind, name, w, h, action, primed: false },
    )
    .unwrap();
}

struct NewSpriteCtl {
    editor: Editor,
    overlay: NodeId,
    kind: NodeId,
    name: NodeId,
    w: NodeId,
    h: NodeId,
    action: Rc<Cell<u8>>,
    primed: bool,
}

impl Behavior for NewSpriteCtl {
    fn update(&mut self, ctx: &mut UpdateContext) {
        let open = self.editor.new_sprite_open.get();
        ctx.ui.set_display(self.overlay, open);
        if !open {
            self.primed = false;
            self.action.set(0);
            return;
        }
        if !self.primed {
            self.primed = true;
            for f in [self.name, self.w, self.h] {
                if let Some(t) = ctx.ui.widget_mut::<TextField>(f) {
                    if f == self.name {
                        t.value.clear();
                    }
                }
            }
        }
        match self.action.replace(0) {
            1 => {
                let ki = ctx.ui.widget::<Dropdown>(self.kind).and_then(|d| d.selected).unwrap_or(0);
                let kind = SpriteKind::ALL[ki.min(2)];
                let name = ctx
                    .ui
                    .widget::<TextField>(self.name)
                    .map(|f| f.value.trim().to_string())
                    .unwrap_or_default();
                let name = if name.is_empty() { format!("New {}", kind.label()) } else { name };
                let w = parse_dim(ctx.ui, self.w, 64);
                let h = parse_dim(ctx.ui, self.h, 64);
                let e = self.editor.edit(|p| p.create_sprite(kind, name, w, h));
                if let Some(e) = e {
                    self.editor.open_entity(e);
                }
                self.editor.new_sprite_open.set(false);
            }
            2 => self.editor.new_sprite_open.set(false),
            _ => {}
        }
    }
}

// --- Add Animation --------------------------------------------

fn spawn_add_animation(ui: &mut UiTree, root: NodeId, editor: &Editor) {
    let overlay = ui
        .spawn(root, overlay_style(), Panel::new().background(Color::rgba(0.0, 0.0, 0.0, 0.4)))
        .unwrap();
    ui.set_display(overlay, false);
    let card = ui
        .spawn(overlay, card_style(400.0), Panel::new().background(Color::WHITE).corner_radius(12.0))
        .unwrap();

    ui.spawn(card, Style::default(), Label::new("Add Animation").size(18.0).color(INK)).unwrap();
    let name = ui.spawn(card, full(52.0), TextField::new("Animation name")).unwrap();
    let mode = ui
        .spawn(card, full(38.0), Dropdown::new(["Empty animation", "Copy existing"]).selected(0))
        .unwrap();
    let source = ui.spawn(card, full(38.0), Dropdown::new(["(none)"]).selected(0)).unwrap();

    let action = Rc::new(Cell::new(0u8));
    buttons_row(ui, card, action.clone());

    ui.spawn(
        root,
        Style::default(),
        AddAnimCtl {
            editor: editor.clone(),
            overlay,
            name,
            mode,
            source,
            action,
            primed: false,
            src_ids: RefCell::new(Vec::new()),
        },
    )
    .unwrap();
}

struct AddAnimCtl {
    editor: Editor,
    overlay: NodeId,
    name: NodeId,
    mode: NodeId,
    source: NodeId,
    action: Rc<Cell<u8>>,
    primed: bool,
    src_ids: RefCell<Vec<AnimationId>>,
}

impl Behavior for AddAnimCtl {
    fn update(&mut self, ctx: &mut UpdateContext) {
        let target = self.editor.add_anim_open.get();
        let open = target.is_some();
        ctx.ui.set_display(self.overlay, open);
        if !open {
            self.primed = false;
            self.action.set(0);
            return;
        }
        if !self.primed {
            self.primed = true;
            if let Some(t) = ctx.ui.widget_mut::<TextField>(self.name) {
                t.value.clear();
            }
            let list: Vec<(AnimationId, String)> = self
                .editor
                .with_project(|p| p.animations.iter().map(|(k, a)| (k, if a.name.is_empty() { "Animation".into() } else { a.name.clone() })).collect())
                .unwrap_or_default();
            *self.src_ids.borrow_mut() = list.iter().map(|(k, _)| *k).collect();
            if let Some(d) = ctx.ui.widget_mut::<Dropdown>(self.source) {
                d.options = if list.is_empty() { vec!["(no animations)".into()] } else { list.iter().map(|(_, n)| n.clone()).collect() };
                d.selected = Some(0);
            }
        }

        match self.action.replace(0) {
            1 => {
                let entity = target.unwrap();
                let name = ctx.ui.widget::<TextField>(self.name).map(|f| f.value.trim().to_string()).unwrap_or_default();
                let copy = ctx.ui.widget::<Dropdown>(self.mode).and_then(|d| d.selected).unwrap_or(0) == 1;
                let src = ctx
                    .ui
                    .widget::<Dropdown>(self.source)
                    .and_then(|d| d.selected)
                    .and_then(|i| self.src_ids.borrow().get(i).copied());

                self.editor.edit(|p| {
                    let new_id = if copy {
                        src.and_then(|s| p.duplicate_animation(s))
                    } else {
                        Some(p.add_animation_to_entity(entity))
                    };
                    if let Some(a) = new_id {
                        if copy {
                            // duplicate_animation doesn't attach to an entity
                            match entity {
                                SpriteEntity::Tile(k) => {
                                    if let Some(t) = p.tiles.get_mut(k) { t.animations.push(a); }
                                }
                                SpriteEntity::Background(k) => {
                                    if let Some(b) = p.backgrounds.get_mut(k) { b.animations.push(a); }
                                }
                                SpriteEntity::Accessory(k) => {
                                    if let Some(x) = p.accessories.get_mut(k) { x.animations.push(a); }
                                }
                            }
                        }
                        if let Some(an) = p.animations.get_mut(a) {
                            an.name = if name.is_empty() { "Animation".into() } else { name.clone() };
                        }
                        p.session.active.sprite = Some(entity);
                        p.session.active.animation = Some(a);
                        p.session.selection = Selection::Animation(a);
                    }
                });
                self.editor.add_anim_open.set(None);
            }
            2 => self.editor.add_anim_open.set(None),
            _ => {}
        }
    }
}

// --- + Frame -------------------------------------------------

fn spawn_add_frame(ui: &mut UiTree, root: NodeId, editor: &Editor) {
    let overlay = ui
        .spawn(root, overlay_style(), Panel::new().background(Color::rgba(0.0, 0.0, 0.0, 0.4)))
        .unwrap();
    ui.set_display(overlay, false);
    let card = ui
        .spawn(overlay, card_style(380.0), Panel::new().background(Color::WHITE).corner_radius(12.0))
        .unwrap();

    ui.spawn(card, Style::default(), Label::new("Add Frame").size(18.0).color(INK)).unwrap();
    let mode = ui
        .spawn(card, full(38.0), Dropdown::new(["New empty frame", "Copy existing frame"]).selected(0))
        .unwrap();
    let source = ui.spawn(card, full(38.0), Dropdown::new(["(none)"]).selected(0)).unwrap();

    let action = Rc::new(Cell::new(0u8));
    buttons_row(ui, card, action.clone());

    ui.spawn(
        root,
        Style::default(),
        AddFrameCtl {
            editor: editor.clone(),
            overlay,
            mode,
            source,
            action,
            primed: false,
            src_ids: RefCell::new(Vec::new()),
        },
    )
    .unwrap();
}

struct AddFrameCtl {
    editor: Editor,
    overlay: NodeId,
    mode: NodeId,
    source: NodeId,
    action: Rc<Cell<u8>>,
    primed: bool,
    src_ids: RefCell<Vec<FrameId>>,
}

impl Behavior for AddFrameCtl {
    fn update(&mut self, ctx: &mut UpdateContext) {
        let target = self.editor.add_frame_open.get();
        let open = target.is_some();
        ctx.ui.set_display(self.overlay, open);
        if !open {
            self.primed = false;
            self.action.set(0);
            return;
        }
        if !self.primed {
            self.primed = true;
            let frames: Vec<FrameId> = target
                .and_then(|a| self.editor.with_project(|p| p.animations.get(a).map(|x| x.frames.iter().copied().collect())))
                .flatten()
                .unwrap_or_default();
            *self.src_ids.borrow_mut() = frames.clone();
            if let Some(d) = ctx.ui.widget_mut::<Dropdown>(self.source) {
                d.options = if frames.is_empty() {
                    vec!["(no frames)".into()]
                } else {
                    (1..=frames.len()).map(|i| format!("Frame {i}")).collect()
                };
                d.selected = Some(0);
            }
        }

        match self.action.replace(0) {
            1 => {
                let anim = target.unwrap();
                let copy = ctx.ui.widget::<Dropdown>(self.mode).and_then(|d| d.selected).unwrap_or(0) == 1;
                let src = ctx
                    .ui
                    .widget::<Dropdown>(self.source)
                    .and_then(|d| d.selected)
                    .and_then(|i| self.src_ids.borrow().get(i).copied());
                self.editor.edit(|p| {
                    let new_f = if copy {
                        src.and_then(|s| p.duplicate_frame(s))
                    } else {
                        p.add_frame_to_animation(anim)
                    };
                    if let Some(f) = new_f {
                        if copy {
                            if let Some(a) = p.animations.get_mut(anim) {
                                a.frames.push_back(f);
                            }
                        }
                        p.session.active.canvas = SpriteCanvas::Frame(f);
                        p.session.active.frame = Some(f);
                        p.session.selection = Selection::Frame(f);
                    }
                });
                self.editor.add_frame_open.set(None);
            }
            2 => self.editor.add_frame_open.set(None),
            _ => {}
        }
    }
}
