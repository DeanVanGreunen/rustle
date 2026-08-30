//! Rustle core: the on-disk project data model plus save / load helpers.
//!
//! A [`Project`] is the whole application state. It is a bag of
//! [`slotmap::DenseSlotMap`] arenas keyed by typed ids; cross-references
//! between entities are stored as those ids.

use std::collections::LinkedList;

use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use slotmap::{DenseSlotMap, new_key_type};
pub use uuid::Uuid;

/// A stable, file-portable identifier stamped on every entity when it is
/// created. `DenseSlotMap` keys are an in-memory allocation detail and
/// are not meant to be relied on across processes — game runtime code and
/// external tools reference entities by this `Uuid`.
fn new_id() -> Uuid {
    Uuid::new_v4()
}

/// Signed offset the old content shifts by when a dimension changes from
/// `old` to `new`, given an anchor (0 = start, 1 = centre, 2 = end).
fn anchor_offset(anchor: u8, old: u32, new: u32) -> i64 {
    let delta = new as i64 - old as i64;
    match anchor {
        1 => delta / 2,
        2 => delta,
        _ => 0,
    }
}

/// Blit an RGBA buffer into a new `nw`×`nh` buffer, translated by
/// `(dx, dy)` and cropped to the new bounds. Missing / mismatched source
/// data yields a fully transparent result.
fn shift_resample(src: &[u8], ow: u32, oh: u32, nw: u32, nh: u32, dx: i64, dy: i64) -> Vec<u8> {
    let mut out = vec![0u8; (nw as usize) * (nh as usize) * 4];
    if src.len() != (ow as usize) * (oh as usize) * 4 {
        return out;
    }
    for y in 0..oh as i64 {
        let ny = y + dy;
        if ny < 0 || ny >= nh as i64 {
            continue;
        }
        for x in 0..ow as i64 {
            let nx = x + dx;
            if nx < 0 || nx >= nw as i64 {
                continue;
            }
            let si = ((y * ow as i64 + x) * 4) as usize;
            let di = ((ny * nw as i64 + nx) * 4) as usize;
            out[di..di + 4].copy_from_slice(&src[si..si + 4]);
        }
    }
    out
}

mod recent;
mod session;
pub use recent::{RecentEntry, RecentProjects};
pub use session::*;

/// File extension for a saved project (no leading dot).
pub const FILE_EXT: &str = "rustle";

new_key_type! {
    pub struct FrameId;
    pub struct GroupId;
    pub struct LayerId;
    pub struct TileId;
    pub struct AnimationId;
    pub struct BackgroundId;
    pub struct AccessoryId;
    pub struct LevelId;
    pub struct BaseFrameId;
}

/// A 2D point, used for entity origins and level placement.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

/// How a layer or group composites onto what's below it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
pub enum BlendMode {
    #[default]
    Normal = 0,
    Multiply = 1,
    Screen = 2,
    Overlay = 3,
    Add = 4,
    Subtract = 5,
}

impl BlendMode {
    pub const ALL: [BlendMode; 6] = [
        BlendMode::Normal,
        BlendMode::Multiply,
        BlendMode::Screen,
        BlendMode::Overlay,
        BlendMode::Add,
        BlendMode::Subtract,
    ];

    pub fn label(self) -> &'static str {
        match self {
            BlendMode::Normal => "Normal",
            BlendMode::Multiply => "Multiply",
            BlendMode::Screen => "Screen",
            BlendMode::Overlay => "Overlay",
            BlendMode::Add => "Add",
            BlendMode::Subtract => "Subtract",
        }
    }
}

/// A raster layer: tightly-packed RGBA8 pixels, row-major. The pixel
/// buffer is NOT part of serde output — the project file stores it as a
/// PNG in a separate data table.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Layer {
    #[serde(default)]
    pub id: Uuid,
    pub width: u32,
    pub height: u32,
    /// `width * height * 4` bytes (R, G, B, A). Serialised separately.
    #[serde(skip)]
    pub pixels: Vec<u8>,
    pub visible: bool,
    #[serde(default)]
    pub blend_mode: BlendMode,
}

impl Layer {
    /// A fully-transparent layer of the given size.
    pub fn blank(width: u32, height: u32) -> Self {
        Self {
            id: new_id(),
            width,
            height,
            pixels: vec![0; width as usize * height as usize * 4],
            visible: true,
            blend_mode: BlendMode::Normal,
        }
    }
}

/// A container node: an ordered set of child layers and child groups.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Group {
    #[serde(default)]
    pub id: Uuid,
    #[serde(default)]
    pub name: String,
    pub layers: Vec<LayerId>,
    pub groups: Vec<GroupId>,
    pub visible: bool,
    #[serde(default)]
    pub blend_mode: BlendMode,
    #[serde(default)]
    pub width: u32,
    #[serde(default)]
    pub height: u32,
}

/// A drawable tile definition.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Tile {
    #[serde(default)]
    pub id: Uuid,
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub base_frame: Option<BaseFrameId>,
    pub animations: Vec<AnimationId>,
    /// Free-form properties, stored as a JSON string.
    pub properties: String,
    pub origin: Point,
}

/// A background image definition. Like a [`Tile`] it owns a base image
/// plus any number of animations.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Background {
    #[serde(default)]
    pub id: Uuid,
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub origin: Point,
    #[serde(default)]
    pub base_frame: Option<BaseFrameId>,
    #[serde(default)]
    pub animations: Vec<AnimationId>,
    #[serde(default)]
    pub parallax: bool,
    #[serde(default)]
    pub z_index: i32,
}

/// An accessory image definition. Like a [`Tile`] it owns a base image
/// plus any number of animations.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Accessory {
    #[serde(default)]
    pub id: Uuid,
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub origin: Point,
    #[serde(default)]
    pub base_frame: Option<BaseFrameId>,
    #[serde(default)]
    pub animations: Vec<AnimationId>,
}

/// An animation: an ordered (linked) list of frame ids.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Animation {
    #[serde(default)]
    pub id: Uuid,
    #[serde(default)]
    pub name: String,
    pub frames: LinkedList<FrameId>,
}

/// A single animation frame: its content plus how long it is shown.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Frame {
    #[serde(default)]
    pub id: Uuid,
    pub groups: Vec<GroupId>,
    pub layers: Vec<LayerId>,
    pub delay_ms: u64,
}

/// One placed tile inside a [`Level`]. `x` / `y` are the placement
/// position (not the tile's own origin).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LevelTile {
    pub tile: TileId,
    pub x: f32,
    pub y: f32,
    pub width: u32,
    pub height: u32,
}

/// One placed BackgroundId inside a [`Level`]. `x` / `y` are the placement
/// position (not the tile's own origin).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LevelBackground {
    pub background: BackgroundId,
    pub x: f32,
    pub y: f32,
    pub width: u32,
    pub height: u32,
}

/// One placed tile inside a [`Level`]. `x` / `y` are the placement
/// position (not the tile's own origin).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LevelAccessory {
    pub accessory: AccessoryId,
    pub x: f32,
    pub y: f32,
    pub width: u32,
    pub height: u32,
}

/// A level: a named collection of placed tiles.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Level {
    /// Stable id — game runtime references levels by this.
    #[serde(default)]
    pub id: Uuid,
    pub name: String,
    pub tiles: Vec<LevelTile>,
    pub backgrounds: Vec<LevelBackground>,
    pub accessories: Vec<LevelAccessory>,
}

/// The canonical/base frame content for a tile.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BaseFrame {
    #[serde(default)]
    pub id: Uuid,
    pub width: u32,
    pub height: u32,
    pub groups: Vec<GroupId>,
    pub layers: Vec<LayerId>,
}

/// The entire application / document state.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Project {
    pub project_name: String,
    pub file_version: u64,
    pub file_path: String,

    pub frames: DenseSlotMap<FrameId, Frame>,
    pub groups: DenseSlotMap<GroupId, Group>,
    pub layers: DenseSlotMap<LayerId, Layer>,
    pub tiles: DenseSlotMap<TileId, Tile>,
    pub animations: DenseSlotMap<AnimationId, Animation>,
    pub backgrounds: DenseSlotMap<BackgroundId, Background>,
    pub accessories: DenseSlotMap<AccessoryId, Accessory>,
    pub levels: DenseSlotMap<LevelId, Level>,
    pub base_frame: DenseSlotMap<BaseFrameId, BaseFrame>,

    /// Editor UI state (workspace, tool, selection, viewport, swatches).
    #[serde(default)]
    pub session: Session,
}

/// Current save-file format version written by [`Project::new`].
pub const CURRENT_FILE_VERSION: u64 = 1;

impl Project {
    /// A fresh project targeting `file_path`, seeded with one base frame,
    /// one animation frame holding a blank layer, and one empty level so
    /// every workspace has something to show.
    pub fn new(project_name: impl Into<String>, file_path: impl Into<String>) -> Self {
        let mut p = Self {
            project_name: project_name.into(),
            file_version: CURRENT_FILE_VERSION,
            file_path: file_path.into(),
            ..Default::default()
        };

        let level = p.add_level("Level 1");
        p.session.active.level = Some(level);

        // One starter tile with a base image so the sprite workspace has
        // something to show.
        let tile = p.new_tile();
        let entity = SpriteEntity::Tile(tile);
        let layer = p.add_layer_to_base(entity);
        let base = p.entity_base_frame(entity).unwrap();

        p.session.active.sprite = Some(entity);
        p.session.active.canvas = SpriteCanvas::Base(base);
        p.session.active.layer = Some(layer);
        p.session.selection = Selection::Layer(layer);
        p
    }

    // --- id-stamping inserts -------------------------------------------
    //
    // Always create entities through these so every one gets a `Uuid`.

    pub fn add_layer(&mut self, mut layer: Layer) -> LayerId {
        if layer.id.is_nil() {
            layer.id = new_id();
        }
        self.layers.insert(layer)
    }

    pub fn add_group(&mut self, mut group: Group) -> GroupId {
        if group.id.is_nil() {
            group.id = new_id();
        }
        if group.name.trim().is_empty() {
            group.name = format!("Group {}", self.groups.len() + 1);
        }
        self.groups.insert(group)
    }

    pub fn add_frame(&mut self, mut frame: Frame) -> FrameId {
        if frame.id.is_nil() {
            frame.id = new_id();
        }
        self.frames.insert(frame)
    }

    pub fn add_base_frame(&mut self, mut bf: BaseFrame) -> BaseFrameId {
        if bf.id.is_nil() {
            bf.id = new_id();
        }
        self.base_frame.insert(bf)
    }

    pub fn add_tile(&mut self, mut tile: Tile) -> TileId {
        if tile.id.is_nil() {
            tile.id = new_id();
        }
        self.tiles.insert(tile)
    }

    pub fn add_background(&mut self, mut bg: Background) -> BackgroundId {
        if bg.id.is_nil() {
            bg.id = new_id();
        }
        self.backgrounds.insert(bg)
    }

    pub fn add_accessory(&mut self, mut a: Accessory) -> AccessoryId {
        if a.id.is_nil() {
            a.id = new_id();
        }
        self.accessories.insert(a)
    }

    pub fn add_animation(&mut self, mut anim: Animation) -> AnimationId {
        if anim.id.is_nil() {
            anim.id = new_id();
        }
        self.animations.insert(anim)
    }

    /// Create a new, empty named level. Returns its slot key; its
    /// [`Uuid`] is `self.levels[key].id`.
    pub fn add_level(&mut self, name: impl Into<String>) -> LevelId {
        self.levels.insert(Level {
            id: new_id(),
            name: name.into(),
            ..Default::default()
        })
    }

    // --- convenience creators used by the outline "+" rows ------------

    fn frame_canvas_size(&self, frame: FrameId) -> (u32, u32) {
        self.frames
            .get(frame)
            .and_then(|f| f.layers.iter().filter_map(|l| self.layers.get(*l)).next())
            .map(|l| (l.width, l.height))
            .unwrap_or((64, 64))
    }

    pub fn add_layer_to_frame(&mut self, frame: FrameId) -> Option<LayerId> {
        let (w, h) = self.frame_canvas_size(frame);
        let id = self.add_layer(Layer::blank(w, h));
        self.frames.get_mut(frame)?.layers.push(id);
        Some(id)
    }

    pub fn add_group_to_frame(&mut self, frame: FrameId) -> Option<GroupId> {
        let id = self.add_group(Group { visible: true, ..Default::default() });
        self.frames.get_mut(frame)?.groups.push(id);
        Some(id)
    }

    pub fn add_frame_seeded(&mut self) -> FrameId {
        let layer = self.add_layer(Layer::blank(64, 64));
        self.add_frame(Frame { layers: vec![layer], delay_ms: 100, ..Default::default() })
    }

    pub fn new_tile(&mut self) -> TileId {
        let n = self.tiles.len() + 1;
        self.add_tile(Tile {
            name: format!("Tile {n}"),
            width: 16,
            height: 16,
            ..Default::default()
        })
    }

    pub fn new_background(&mut self) -> BackgroundId {
        let n = self.backgrounds.len() + 1;
        self.add_background(Background {
            name: format!("Background {n}"),
            width: 64,
            height: 64,
            ..Default::default()
        })
    }

    pub fn new_accessory(&mut self) -> AccessoryId {
        let n = self.accessories.len() + 1;
        self.add_accessory(Accessory {
            name: format!("Accessory {n}"),
            width: 16,
            height: 16,
            ..Default::default()
        })
    }

    /// Create a new sprite of `kind` with the given name and pixel size,
    /// seeded with a base image holding one correctly-sized layer.
    pub fn create_sprite(
        &mut self,
        kind: SpriteKind,
        name: impl Into<String>,
        width: u32,
        height: u32,
    ) -> SpriteEntity {
        let (w, h) = (width.max(1), height.max(1));
        let name = name.into();
        let layer = self.add_layer(Layer::blank(w, h));
        let base = self.add_base_frame(BaseFrame {
            width: w,
            height: h,
            layers: vec![layer],
            ..Default::default()
        });
        match kind {
            SpriteKind::Background => SpriteEntity::Background(self.add_background(Background {
                name,
                width: w,
                height: h,
                base_frame: Some(base),
                ..Default::default()
            })),
            SpriteKind::Tile => SpriteEntity::Tile(self.add_tile(Tile {
                name,
                width: w,
                height: h,
                base_frame: Some(base),
                ..Default::default()
            })),
            SpriteKind::Accessory => SpriteEntity::Accessory(self.add_accessory(Accessory {
                name,
                width: w,
                height: h,
                base_frame: Some(base),
                ..Default::default()
            })),
        }
    }

    // --- sprite / animation workspace helpers ------------------------

    pub fn entity_name(&self, e: SpriteEntity) -> String {
        match e {
            SpriteEntity::Tile(k) => self.tiles.get(k).map(|t| t.name.clone()),
            SpriteEntity::Background(k) => self.backgrounds.get(k).map(|b| b.name.clone()),
            SpriteEntity::Accessory(k) => self.accessories.get(k).map(|a| a.name.clone()),
        }
        .unwrap_or_default()
    }

    pub fn entity_size(&self, e: SpriteEntity) -> (u32, u32) {
        let d = match e {
            SpriteEntity::Tile(k) => self.tiles.get(k).map(|t| (t.width, t.height)),
            SpriteEntity::Background(k) => self.backgrounds.get(k).map(|b| (b.width, b.height)),
            SpriteEntity::Accessory(k) => self.accessories.get(k).map(|a| (a.width, a.height)),
        };
        let (w, h) = d.unwrap_or((16, 16));
        (w.max(1), h.max(1))
    }

    pub fn entity_exists(&self, e: SpriteEntity) -> bool {
        match e {
            SpriteEntity::Tile(k) => self.tiles.contains_key(k),
            SpriteEntity::Background(k) => self.backgrounds.contains_key(k),
            SpriteEntity::Accessory(k) => self.accessories.contains_key(k),
        }
    }

    pub fn entity_base_frame(&self, e: SpriteEntity) -> Option<BaseFrameId> {
        match e {
            SpriteEntity::Tile(k) => self.tiles.get(k)?.base_frame,
            SpriteEntity::Background(k) => self.backgrounds.get(k)?.base_frame,
            SpriteEntity::Accessory(k) => self.accessories.get(k)?.base_frame,
        }
    }

    pub fn entity_animations(&self, e: SpriteEntity) -> Vec<AnimationId> {
        match e {
            SpriteEntity::Tile(k) => self.tiles.get(k).map(|t| t.animations.clone()),
            SpriteEntity::Background(k) => self.backgrounds.get(k).map(|b| b.animations.clone()),
            SpriteEntity::Accessory(k) => self.accessories.get(k).map(|a| a.animations.clone()),
        }
        .unwrap_or_default()
    }

    fn set_entity_base_frame(&mut self, e: SpriteEntity, bf: BaseFrameId) {
        match e {
            SpriteEntity::Tile(k) => {
                if let Some(t) = self.tiles.get_mut(k) {
                    t.base_frame = Some(bf);
                }
            }
            SpriteEntity::Background(k) => {
                if let Some(b) = self.backgrounds.get_mut(k) {
                    b.base_frame = Some(bf);
                }
            }
            SpriteEntity::Accessory(k) => {
                if let Some(a) = self.accessories.get_mut(k) {
                    a.base_frame = Some(bf);
                }
            }
        }
    }

    fn push_entity_animation(&mut self, e: SpriteEntity, a: AnimationId) {
        match e {
            SpriteEntity::Tile(k) => {
                if let Some(t) = self.tiles.get_mut(k) {
                    t.animations.push(a);
                }
            }
            SpriteEntity::Background(k) => {
                if let Some(b) = self.backgrounds.get_mut(k) {
                    b.animations.push(a);
                }
            }
            SpriteEntity::Accessory(k) => {
                if let Some(x) = self.accessories.get_mut(k) {
                    x.animations.push(a);
                }
            }
        }
    }

    /// Get (creating if needed) the entity's base image frame.
    pub fn ensure_base_frame(&mut self, e: SpriteEntity) -> BaseFrameId {
        if let Some(bf) = self.entity_base_frame(e) {
            return bf;
        }
        let (w, h) = self.entity_size(e);
        let bf = self.add_base_frame(BaseFrame { width: w, height: h, ..Default::default() });
        self.set_entity_base_frame(e, bf);
        bf
    }

    pub fn add_layer_to_base(&mut self, e: SpriteEntity) -> LayerId {
        let bf = self.ensure_base_frame(e);
        let (w, h) = self.entity_size(e);
        let l = self.add_layer(Layer::blank(w, h));
        if let Some(b) = self.base_frame.get_mut(bf) {
            b.layers.push(l);
        }
        l
    }

    pub fn add_group_to_base(&mut self, e: SpriteEntity) -> GroupId {
        let bf = self.ensure_base_frame(e);
        let g = self.add_group(Group { visible: true, ..Default::default() });
        if let Some(b) = self.base_frame.get_mut(bf) {
            b.groups.push(g);
        }
        g
    }

    /// Create an animation for the entity, seeded with one frame + layer.
    pub fn add_animation_to_entity(&mut self, e: SpriteEntity) -> AnimationId {
        let (w, h) = self.entity_size(e);
        let layer = self.add_layer(Layer::blank(w, h));
        let frame = self.add_frame(Frame { layers: vec![layer], delay_ms: 100, ..Default::default() });
        let anim = self.add_animation(Animation::default());
        if let Some(a) = self.animations.get_mut(anim) {
            a.frames.push_back(frame);
        }
        self.push_entity_animation(e, anim);
        anim
    }

    pub fn add_frame_to_animation(&mut self, anim: AnimationId) -> Option<FrameId> {
        let (w, h) = self
            .animations
            .get(anim)?
            .frames
            .front()
            .and_then(|f| self.frames.get(*f))
            .and_then(|f| f.layers.first().and_then(|l| self.layers.get(*l)))
            .map(|l| (l.width, l.height))
            .unwrap_or((16, 16));
        let layer = self.add_layer(Layer::blank(w, h));
        let frame = self.add_frame(Frame { layers: vec![layer], delay_ms: 100, ..Default::default() });
        self.animations.get_mut(anim)?.frames.push_back(frame);
        Some(frame)
    }

    pub fn add_layer_to_group(&mut self, g: GroupId) -> Option<LayerId> {
        let l = self.add_layer(Layer::blank(16, 16));
        self.groups.get_mut(g)?.layers.push(l);
        Some(l)
    }

    pub fn add_group_to_group(&mut self, g: GroupId) -> Option<GroupId> {
        let child = self.add_group(Group { visible: true, ..Default::default() });
        self.groups.get_mut(g)?.groups.push(child);
        Some(child)
    }

    /// Layers + groups of the currently-shown sprite canvas.
    pub fn canvas_layers_groups(&self) -> (Vec<LayerId>, Vec<GroupId>) {
        match self.session.active.canvas {
            SpriteCanvas::Base(k) => self
                .base_frame
                .get(k)
                .map(|b| (b.layers.clone(), b.groups.clone()))
                .unwrap_or_default(),
            SpriteCanvas::Frame(k) => self
                .frames
                .get(k)
                .map(|f| (f.layers.clone(), f.groups.clone()))
                .unwrap_or_default(),
            SpriteCanvas::None => (Vec::new(), Vec::new()),
        }
    }

    pub fn canvas_size(&self) -> (u32, u32) {
        let (layers, _) = self.canvas_layers_groups();
        layers
            .iter()
            .filter_map(|l| self.layers.get(*l))
            .next()
            .map(|l| (l.width.max(1), l.height.max(1)))
            .or_else(|| self.session.active.sprite.map(|e| self.entity_size(e)))
            .unwrap_or((16, 16))
    }

    pub fn canvas_add_layer(&mut self) -> Option<LayerId> {
        let (w, h) = self.canvas_size();
        let l = self.add_layer(Layer::blank(w, h));
        match self.session.active.canvas {
            SpriteCanvas::Base(k) => self.base_frame.get_mut(k)?.layers.push(l),
            SpriteCanvas::Frame(k) => self.frames.get_mut(k)?.layers.push(l),
            SpriteCanvas::None => return None,
        }
        self.session.active.layer = Some(l);
        Some(l)
    }

    pub fn canvas_add_group(&mut self) -> Option<GroupId> {
        let g = self.add_group(Group { visible: true, ..Default::default() });
        match self.session.active.canvas {
            SpriteCanvas::Base(k) => self.base_frame.get_mut(k)?.groups.push(g),
            SpriteCanvas::Frame(k) => self.frames.get_mut(k)?.groups.push(g),
            SpriteCanvas::None => return None,
        }
        Some(g)
    }

    /// Recursively gather every layer and group reachable from a node's
    /// `(layers, groups)` lists.
    fn collect_node(
        &self,
        layers: &[LayerId],
        groups: &[GroupId],
        out_layers: &mut Vec<LayerId>,
        out_groups: &mut Vec<GroupId>,
    ) {
        out_layers.extend_from_slice(layers);
        for &g in groups {
            out_groups.push(g);
            if let Some(grp) = self.groups.get(g) {
                let (l, cg) = (grp.layers.clone(), grp.groups.clone());
                self.collect_node(&l, &cg, out_layers, out_groups);
            }
        }
    }

    /// Resize the canvas of a sprite entity to `nw`×`nh`, applied to the
    /// base image and every frame of every animation it owns (including
    /// nested groups). `anchor` is `(h, v)` where each is 0 = start,
    /// 1 = centre, 2 = end — it decides which way content shifts as the
    /// canvas grows or shrinks, keeping the chosen edge/corner pinned.
    pub fn resize_sprite(&mut self, e: SpriteEntity, nw: u32, nh: u32, anchor: (u8, u8)) {
        let (nw, nh) = (nw.max(1), nh.max(1));
        let mut layers: Vec<LayerId> = Vec::new();
        let mut groups: Vec<GroupId> = Vec::new();
        let mut bases: Vec<BaseFrameId> = Vec::new();

        if let Some(bf) = self.entity_base_frame(e) {
            if let Some(b) = self.base_frame.get(bf) {
                bases.push(bf);
                let (l, g) = (b.layers.clone(), b.groups.clone());
                self.collect_node(&l, &g, &mut layers, &mut groups);
            }
        }
        for a in self.entity_animations(e) {
            let frames: Vec<FrameId> = self
                .animations
                .get(a)
                .map(|x| x.frames.iter().copied().collect())
                .unwrap_or_default();
            for f in frames {
                if let Some(fr) = self.frames.get(f) {
                    let (l, g) = (fr.layers.clone(), fr.groups.clone());
                    self.collect_node(&l, &g, &mut layers, &mut groups);
                }
            }
        }
        layers.sort_unstable();
        layers.dedup();
        groups.sort_unstable();
        groups.dedup();

        for lk in layers {
            if let Some(l) = self.layers.get_mut(lk) {
                let (ow, oh) = (l.width, l.height);
                let dx = anchor_offset(anchor.0, ow, nw);
                let dy = anchor_offset(anchor.1, oh, nh);
                l.pixels = shift_resample(&l.pixels, ow, oh, nw, nh, dx, dy);
                l.width = nw;
                l.height = nh;
            }
        }
        for gk in groups {
            if let Some(g) = self.groups.get_mut(gk) {
                if g.width != 0 {
                    g.width = nw;
                }
                if g.height != 0 {
                    g.height = nh;
                }
            }
        }
        for bk in bases {
            if let Some(b) = self.base_frame.get_mut(bk) {
                b.width = nw;
                b.height = nh;
            }
        }
        match e {
            SpriteEntity::Tile(k) => {
                if let Some(t) = self.tiles.get_mut(k) {
                    t.width = nw;
                    t.height = nh;
                }
            }
            SpriteEntity::Background(k) => {
                if let Some(b) = self.backgrounds.get_mut(k) {
                    b.width = nw;
                    b.height = nh;
                }
            }
            SpriteEntity::Accessory(k) => {
                if let Some(x) = self.accessories.get_mut(k) {
                    x.width = nw;
                    x.height = nh;
                }
            }
        }
    }

    // --- per-frame layer memory --------------------------------------

    /// Every layer reachable from a frame, including nested groups.
    fn frame_all_layers(&self, f: FrameId) -> Vec<LayerId> {
        let Some(fr) = self.frames.get(f) else { return Vec::new() };
        let (mut layers, mut groups) = (Vec::new(), Vec::new());
        let (l, g) = (fr.layers.clone(), fr.groups.clone());
        self.collect_node(&l, &g, &mut layers, &mut groups);
        layers
    }

    /// Record the active layer as the shown frame's remembered selection.
    pub fn remember_frame_layer(&mut self) {
        if let (SpriteCanvas::Frame(f), Some(l)) =
            (self.session.active.canvas, self.session.active.layer)
        {
            self.session.frame_layers.insert(f, l);
        }
    }

    /// Point `active.layer` at frame `f`'s remembered layer if it is still
    /// valid, otherwise its first layer; refresh the stored entry.
    pub fn restore_frame_layer(&mut self, f: FrameId) {
        let avail = self.frame_all_layers(f);
        let pick = self
            .session
            .frame_layers
            .get(&f)
            .copied()
            .filter(|l| avail.contains(l))
            .or_else(|| avail.first().copied());
        self.session.active.layer = pick;
        match pick {
            Some(l) => {
                self.session.frame_layers.insert(f, l);
            }
            None => {
                self.session.frame_layers.remove(&f);
            }
        }
    }

    /// Backfill a `Uuid` on any entity that is missing one (old files).
    pub fn ensure_ids(&mut self) {
        macro_rules! fill {
            ($map:expr) => {
                for (_, e) in $map.iter_mut() {
                    if e.id.is_nil() {
                        e.id = new_id();
                    }
                }
            };
        }
        fill!(self.layers);
        fill!(self.groups);
        fill!(self.frames);
        fill!(self.base_frame);
        fill!(self.tiles);
        fill!(self.backgrounds);
        fill!(self.accessories);
        fill!(self.animations);
        fill!(self.levels);
    }

    // --- removal (scrubs cross-references) ----------------------------

    pub fn remove_layer(&mut self, k: LayerId) {
        self.layers.remove(k);
        self.session.frame_layers.retain(|_, v| *v != k);
        for (_, f) in self.frames.iter_mut() {
            f.layers.retain(|&x| x != k);
        }
        for (_, g) in self.groups.iter_mut() {
            g.layers.retain(|&x| x != k);
        }
        for (_, b) in self.base_frame.iter_mut() {
            b.layers.retain(|&x| x != k);
        }
        self.validate_session();
    }

    pub fn remove_group(&mut self, k: GroupId) {
        // Forget remembered frame selections that pointed into this group.
        let (mut inside, mut ig) = (Vec::new(), Vec::new());
        if let Some(g) = self.groups.get(k) {
            let (l, gg) = (g.layers.clone(), g.groups.clone());
            self.collect_node(&l, &gg, &mut inside, &mut ig);
        }
        self.session.frame_layers.retain(|_, v| !inside.contains(v));

        self.groups.remove(k);
        for (_, f) in self.frames.iter_mut() {
            f.groups.retain(|&x| x != k);
        }
        for (_, g) in self.groups.iter_mut() {
            g.groups.retain(|&x| x != k);
        }
        for (_, b) in self.base_frame.iter_mut() {
            b.groups.retain(|&x| x != k);
        }
        self.validate_session();
    }

    pub fn remove_frame(&mut self, k: FrameId) {
        self.frames.remove(k);
        self.session.frame_layers.remove(&k);
        for (_, a) in self.animations.iter_mut() {
            a.frames = a.frames.iter().copied().filter(|&x| x != k).collect();
        }
        self.validate_session();
    }

    pub fn remove_animation(&mut self, k: AnimationId) {
        self.animations.remove(k);
        for (_, t) in self.tiles.iter_mut() {
            t.animations.retain(|&x| x != k);
        }
        for (_, b) in self.backgrounds.iter_mut() {
            b.animations.retain(|&x| x != k);
        }
        for (_, a) in self.accessories.iter_mut() {
            a.animations.retain(|&x| x != k);
        }
        self.validate_session();
    }

    pub fn remove_tile(&mut self, k: TileId) {
        self.tiles.remove(k);
        for (_, l) in self.levels.iter_mut() {
            l.tiles.retain(|t| t.tile != k);
        }
        self.validate_session();
    }

    pub fn remove_background(&mut self, k: BackgroundId) {
        self.backgrounds.remove(k);
        for (_, l) in self.levels.iter_mut() {
            l.backgrounds.retain(|b| b.background != k);
        }
        self.validate_session();
    }

    pub fn remove_accessory(&mut self, k: AccessoryId) {
        self.accessories.remove(k);
        for (_, l) in self.levels.iter_mut() {
            l.accessories.retain(|a| a.accessory != k);
        }
        self.validate_session();
    }

    pub fn remove_level(&mut self, k: LevelId) {
        self.levels.remove(k);
        self.validate_session();
    }

    /// Resolve a level `Uuid` to its current slot key.
    pub fn level_key(&self, id: Uuid) -> Option<LevelId> {
        self.levels.iter().find(|(_, l)| l.id == id).map(|(k, _)| k)
    }

    /// Resolve a tile `Uuid` to its current slot key.
    pub fn tile_key(&self, id: Uuid) -> Option<TileId> {
        self.tiles.iter().find(|(_, t)| t.id == id).map(|(k, _)| k)
    }

    /// Drop selection / active references that no longer resolve (e.g.
    /// after hand-editing the file). Called by [`Project::load`].
    pub fn validate_session(&mut self) {
        let frames = &self.frames;
        let layers = &self.layers;
        self.session
            .frame_layers
            .retain(|f, l| frames.contains_key(*f) && layers.contains_key(*l));

        let s = &mut self.session;
        if s.active.frame.is_some_and(|k| !self.frames.contains_key(k)) {
            s.active.frame = self.frames.keys().next();
        }
        if s.active.layer.is_some_and(|k| !self.layers.contains_key(k)) {
            s.active.layer = self.layers.keys().next();
        }
        if s.active.level.is_some_and(|k| !self.levels.contains_key(k)) {
            s.active.level = self.levels.keys().next();
        }
        if s.active.animation.is_some_and(|k| !self.animations.contains_key(k)) {
            s.active.animation = self.animations.keys().next();
        }
        let sprite_ok = match s.active.sprite {
            None => true,
            Some(SpriteEntity::Tile(k)) => self.tiles.contains_key(k),
            Some(SpriteEntity::Background(k)) => self.backgrounds.contains_key(k),
            Some(SpriteEntity::Accessory(k)) => self.accessories.contains_key(k),
        };
        if !sprite_ok {
            s.active.sprite = self
                .tiles
                .keys()
                .next()
                .map(SpriteEntity::Tile)
                .or_else(|| self.backgrounds.keys().next().map(SpriteEntity::Background))
                .or_else(|| self.accessories.keys().next().map(SpriteEntity::Accessory));
        }
        match s.active.canvas {
            SpriteCanvas::Base(k) if !self.base_frame.contains_key(k) => {
                s.active.canvas = SpriteCanvas::None
            }
            SpriteCanvas::Frame(k) if !self.frames.contains_key(k) => {
                s.active.canvas = SpriteCanvas::None
            }
            _ => {}
        }
        let sel_ok = match s.selection {
            Selection::None => true,
            Selection::Frame(k) => self.frames.contains_key(k),
            Selection::Layer(k) => self.layers.contains_key(k),
            Selection::Group(k) => self.groups.contains_key(k),
            Selection::Tile(k) => self.tiles.contains_key(k),
            Selection::Background(k) => self.backgrounds.contains_key(k),
            Selection::Accessory(k) => self.accessories.contains_key(k),
            Selection::Animation(k) => self.animations.contains_key(k),
            Selection::Level(k) => self.levels.contains_key(k),
            Selection::LevelTile { level, index } => self
                .levels
                .get(level)
                .is_some_and(|l| index < l.tiles.len()),
            Selection::LevelBackground { level, index } => self
                .levels
                .get(level)
                .is_some_and(|l| index < l.backgrounds.len()),
            Selection::LevelAccessory { level, index } => self
                .levels
                .get(level)
                .is_some_and(|l| index < l.accessories.len()),
        };
        if !sel_ok {
            self.session.selection = Selection::None;
        }
    }

    /// Run after loading from a file: backfill ids, drop dangling refs.
    pub fn finish_load(&mut self) {
        self.ensure_ids();
        self.validate_session();
    }

    // --- deep copy -------------------------------------------------

    fn dup_layer(&mut self, src: LayerId) -> Option<LayerId> {
        let mut l = self.layers.get(src)?.clone();
        l.id = new_id();
        Some(self.layers.insert(l))
    }

    fn dup_group(&mut self, src: GroupId) -> Option<GroupId> {
        let (layers, groups, mut g) = {
            let g = self.groups.get(src)?;
            (g.layers.clone(), g.groups.clone(), g.clone())
        };
        g.id = new_id();
        g.layers = layers.iter().filter_map(|&l| self.dup_layer(l)).collect();
        g.groups = groups.iter().filter_map(|&c| self.dup_group(c)).collect();
        Some(self.groups.insert(g))
    }

    /// Deep-copy a frame (new slot ids + uuids for it and every layer /
    /// group it contains), keeping pixel data, blend modes, sizes.
    pub fn duplicate_frame(&mut self, src: FrameId) -> Option<FrameId> {
        let (layers, groups, delay) = {
            let f = self.frames.get(src)?;
            (f.layers.clone(), f.groups.clone(), f.delay_ms)
        };
        let new_layers: Vec<LayerId> = layers.iter().filter_map(|&l| self.dup_layer(l)).collect();
        let new_groups: Vec<GroupId> = groups.iter().filter_map(|&g| self.dup_group(g)).collect();
        Some(self.add_frame(Frame {
            layers: new_layers,
            groups: new_groups,
            delay_ms: delay,
            ..Default::default()
        }))
    }

    /// Deep-copy an animation and all of its frames.
    pub fn duplicate_animation(&mut self, src: AnimationId) -> Option<AnimationId> {
        let frames: Vec<FrameId> = self.animations.get(src)?.frames.iter().copied().collect();
        let anim = self.add_animation(Animation::default());
        for f in frames {
            if let Some(nf) = self.duplicate_frame(f) {
                if let Some(a) = self.animations.get_mut(anim) {
                    a.frames.push_back(nf);
                }
            }
        }
        Some(anim)
    }
}
