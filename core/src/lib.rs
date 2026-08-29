//! Rustle core: the on-disk project data model plus save / load helpers.
//!
//! A [`Project`] is the whole application state. It is a bag of
//! [`slotmap::DenseSlotMap`] arenas keyed by typed ids; cross-references
//! between entities are stored as those ids.

use std::collections::LinkedList;
use std::io::{self, BufReader, BufWriter};
use std::path::Path;

use serde::{Deserialize, Serialize};
use slotmap::{DenseSlotMap, new_key_type};
pub use uuid::Uuid;

/// A stable, file-portable identifier stamped on every entity when it is
/// created. `DenseSlotMap` keys are an in-memory allocation detail and
/// are not meant to be relied on across processes — game runtime code and
/// external tools reference entities by this `Uuid`.
fn new_id() -> Uuid {
    Uuid::new_v4()
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

/// A raster layer: tightly-packed RGBA8 pixels, row-major.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Layer {
    #[serde(default)]
    pub id: Uuid,
    pub width: u32,
    pub height: u32,
    /// `width * height * 4` bytes (R, G, B, A).
    pub pixels: Vec<u8>,
    pub visible: bool,
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
        }
    }
}

/// A container node: an ordered set of child layers and child groups.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Group {
    #[serde(default)]
    pub id: Uuid,
    pub layers: Vec<LayerId>,
    pub groups: Vec<GroupId>,
    pub visible: bool,
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

/// A background image definition.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Background {
    #[serde(default)]
    pub id: Uuid,
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub origin: Point,
}

/// An accessory image definition.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Accessory {
    #[serde(default)]
    pub id: Uuid,
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub origin: Point,
}

/// An animation: an ordered (linked) list of frame ids.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Animation {
    #[serde(default)]
    pub id: Uuid,
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

        let layer = p.add_layer(Layer::blank(64, 64));
        p.add_base_frame(BaseFrame {
            width: 64,
            height: 64,
            layers: vec![layer],
            ..Default::default()
        });
        let frame = p.add_frame(Frame { layers: vec![layer], delay_ms: 100, ..Default::default() });
        let level = p.add_level("Level 1");

        p.session.active.frame = Some(frame);
        p.session.active.layer = Some(layer);
        p.session.active.level = Some(level);
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
        };
        if !sel_ok {
            self.session.selection = Selection::None;
        }
    }

    /// Serialize the project to `self.file_path` as pretty JSON.
    pub fn save(&self) -> io::Result<()> {
        let file = std::fs::File::create(&self.file_path)?;
        serde_json::to_writer_pretty(BufWriter::new(file), self).map_err(io::Error::other)
    }

    /// Load a project from `path`; `file_path` is set to `path`.
    pub fn load(path: impl AsRef<Path>) -> io::Result<Self> {
        let path = path.as_ref();
        let file = std::fs::File::open(path)?;
        let mut project: Project =
            serde_json::from_reader(BufReader::new(file)).map_err(io::Error::other)?;
        project.file_path = path.to_string_lossy().into_owned();
        project.ensure_ids();
        project.validate_session();
        Ok(project)
    }
}
