//! Non-document editor state that still lives *in* the project file:
//! which workspace/tool is active, per-tool settings, the current
//! selection, viewport zoom/pan, and the colour swatches.

use serde::{Deserialize, Serialize};

use crate::{
    AccessoryId, AnimationId, BackgroundId, FrameId, GroupId, LayerId, LevelId, TileId,
};

/// Which editor workspace is shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum EditorMode {
    #[default]
    Level,
    Sprite,
    Animation,
}

impl EditorMode {
    pub const ALL: [EditorMode; 3] = [EditorMode::Level, EditorMode::Sprite, EditorMode::Animation];

    pub fn label(self) -> &'static str {
        match self {
            EditorMode::Level => "Level",
            EditorMode::Sprite => "Sprite",
            EditorMode::Animation => "Animation",
        }
    }
}

/// The editing tools (tool palette / first column).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Tool {
    #[default]
    Select,
    Marquee,
    Pencil,
    Eyedropper,
    Zoom,
    Move,
    Line,
    Rectangle,
    Fill,
    Text,
}

impl Tool {
    pub const ALL: [Tool; 10] = [
        Tool::Select,
        Tool::Marquee,
        Tool::Pencil,
        Tool::Eyedropper,
        Tool::Zoom,
        Tool::Move,
        Tool::Line,
        Tool::Rectangle,
        Tool::Fill,
        Tool::Text,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Tool::Select => "Select",
            Tool::Marquee => "Marquee",
            Tool::Pencil => "Pencil",
            Tool::Eyedropper => "Eyedropper",
            Tool::Zoom => "Zoom",
            Tool::Move => "Move",
            Tool::Line => "Line",
            Tool::Rectangle => "Rectangle",
            Tool::Fill => "Bucket Fill",
            Tool::Text => "Text",
        }
    }

    pub fn shortcut(self) -> &'static str {
        match self {
            Tool::Select => "V",
            Tool::Marquee => "M",
            Tool::Pencil => "B",
            Tool::Eyedropper => "I",
            Tool::Zoom => "Z",
            Tool::Move => "H",
            Tool::Line => "L",
            Tool::Rectangle => "R",
            Tool::Fill => "F",
            Tool::Text => "T",
        }
    }

    pub fn from_key(c: char) -> Option<Tool> {
        let up = c.to_ascii_uppercase().to_string();
        Tool::ALL.into_iter().find(|t| t.shortcut() == up)
    }
}

// --- per-tool settings ----------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum MarqueeMode {
    #[default]
    Replace,
    Add,
    Subtract,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PencilSettings {
    pub size: u32,
    pub opacity: u8,
}
impl Default for PencilSettings {
    fn default() -> Self {
        Self { size: 1, opacity: 255 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct MarqueeSettings {
    pub mode: MarqueeMode,
    pub feather: u32,
    pub lock_aspect: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ZoomSettings {
    pub step: f32,
}
impl Default for ZoomSettings {
    fn default() -> Self {
        Self { step: 2.0 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct MoveSettings {
    pub snap: bool,
    pub snap_step: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LineSettings {
    pub width: u32,
}
impl Default for LineSettings {
    fn default() -> Self {
        Self { width: 1 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RectSettings {
    pub filled: bool,
    pub stroke: u32,
}
impl Default for RectSettings {
    fn default() -> Self {
        Self { filled: false, stroke: 1 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct FillSettings {
    pub tolerance: u8,
    pub contiguous: bool,
}
impl Default for FillSettings {
    fn default() -> Self {
        Self { tolerance: 0, contiguous: true }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum GridSnap {
    #[default]
    Off,
    Half,
    Full,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextSettings {
    /// Registered font name; empty = the built-in font.
    pub font: String,
    pub size: u32,
    pub char_spacing: i32,
    pub line_spacing: i32,
    pub free_placement: bool,
    pub grid_snap: GridSnap,
    pub text: String,
}
impl Default for TextSettings {
    fn default() -> Self {
        Self {
            font: String::new(),
            size: 16,
            char_spacing: 0,
            line_spacing: 0,
            free_placement: false,
            grid_snap: GridSnap::Off,
            text: String::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct EyedropperSettings {
    pub sample_merged: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct SelectSettings {
    /// Select the whole containing group instead of the leaf entity.
    pub whole_group: bool,
}

/// One settings block per tool type.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ToolSettings {
    pub select: SelectSettings,
    pub marquee: MarqueeSettings,
    pub pencil: PencilSettings,
    pub eyedropper: EyedropperSettings,
    pub zoom: ZoomSettings,
    #[serde(rename = "move")]
    pub move_: MoveSettings,
    pub line: LineSettings,
    pub rectangle: RectSettings,
    pub fill: FillSettings,
    pub text: TextSettings,
}

// --- selection / active targets -----------------------------------

/// The last-selected entity (drives the properties panel).
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub enum Selection {
    #[default]
    None,
    Frame(FrameId),
    Layer(LayerId),
    Group(GroupId),
    Tile(TileId),
    Background(BackgroundId),
    Accessory(AccessoryId),
    Animation(AnimationId),
    Level(LevelId),
    LevelTile { level: LevelId, index: usize },
    LevelBackground { level: LevelId, index: usize },
}

/// What each workspace is currently editing.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct ActiveTargets {
    pub frame: Option<FrameId>,
    pub layer: Option<LayerId>,
    pub animation: Option<AnimationId>,
    pub level: Option<LevelId>,
}

// --- viewport / colours ------------------------------------------

/// Persisted pan / zoom for a viewport. Zoom is texels-per-screen-pixel
/// scale factor (1.0 = 100%).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ViewportState {
    pub zoom: f32,
    pub pan_x: f32,
    pub pan_y: f32,
}
impl Default for ViewportState {
    fn default() -> Self {
        Self { zoom: 1.0, pan_x: 0.0, pan_y: 0.0 }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ColorState {
    pub foreground: [u8; 4],
    pub background: [u8; 4],
    pub swatches: Vec<[u8; 4]>,
}
impl Default for ColorState {
    fn default() -> Self {
        Self {
            foreground: [255, 255, 255, 255],
            background: [0, 0, 0, 255],
            swatches: vec![[150, 40, 220, 255], [40, 200, 90, 255]],
        }
    }
}

// --- the aggregate ---------------------------------------------

/// Tab shown in the Selected-Properties panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum PropsTab {
    #[default]
    SwatchesPreview,
    Properties,
}

/// Everything the editor UI remembers, saved inside the `.rustle` file.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Session {
    pub mode: EditorMode,
    pub active_tool: Tool,
    pub tools: ToolSettings,
    pub selection: Selection,
    pub active: ActiveTargets,
    pub main_view: ViewportState,
    pub preview_view: ViewportState,
    pub colors: ColorState,
    pub props_tab: PropsTab,
}
