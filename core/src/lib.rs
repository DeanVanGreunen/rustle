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

mod recent;
pub use recent::{RecentEntry, RecentProjects};

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
    pub width: u32,
    pub height: u32,
    /// `width * height * 4` bytes (R, G, B, A).
    pub pixels: Vec<u8>,
}

impl Layer {
    /// A fully-transparent layer of the given size.
    pub fn blank(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            pixels: vec![0; width as usize * height as usize * 4],
        }
    }
}

/// A container node: an ordered set of child layers and child groups.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Group {
    pub layers: Vec<LayerId>,
    pub groups: Vec<GroupId>,
}

/// A drawable tile definition.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Tile {
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
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub origin: Point,
}

/// An accessory image definition.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Accessory {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub origin: Point,
}

/// An animation: an ordered (linked) list of frame ids.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Animation {
    pub frames: LinkedList<FrameId>,
}

/// A single animation frame: its content plus how long it is shown.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Frame {
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
    pub name: String,
    pub tiles: Vec<LevelTile>,
    pub backgrounds: Vec<LevelBackground>,
    pub accessories: Vec<LevelAccessory>,
}

/// The canonical/base frame content for a tile.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BaseFrame {
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
}

/// Current save-file format version written by [`Project::new`].
pub const CURRENT_FILE_VERSION: u64 = 1;

impl Project {
    /// A fresh, empty project targeting `file_path`.
    pub fn new(project_name: impl Into<String>, file_path: impl Into<String>) -> Self {
        Self {
            project_name: project_name.into(),
            file_version: CURRENT_FILE_VERSION,
            file_path: file_path.into(),
            ..Default::default()
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
        Ok(project)
    }
}
