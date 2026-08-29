//! Top navigation bar: editor-mode tabs (Level / Sprite / Animation) with
//! an active-state underline, plus a save-status indicator.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use rustle_ui::node::MeasuredContent;
use rustle_ui::prelude::*;
use rustle_ui::widgets::Panel;
use rustle_ui::EmptyBehavior;

mod menu;
pub use menu::{MenuButton, MenuItem};

pub use rustle_core::EditorMode;

/// Shared nav-bar state. Clone the `Rc` handles into your app so the rest
/// of the program can read the active mode and flip the saved flag.
#[derive(Clone)]
pub struct NavState {
    pub mode: Rc<Cell<EditorMode>>,
    pub saved: Rc<Cell<bool>>,
    pub project_name: Rc<RefCell<String>>,
    pub version: Rc<RefCell<String>>,
}

impl NavState {
    pub fn new(mode: EditorMode) -> Self {
        Self {
            mode: Rc::new(Cell::new(mode)),
            saved: Rc::new(Cell::new(false)),
            project_name: Rc::new(RefCell::new("Untitled Project".to_string())),
            version: Rc::new(RefCell::new(String::new())),
        }
    }

    /// Call when the project is saved / becomes dirty.
    pub fn set_saved(&self, saved: bool) {
        self.saved.set(saved);
    }

    pub fn set_project_name(&self, name: impl Into<String>) {
        *self.project_name.borrow_mut() = name.into();
    }

    pub fn set_version(&self, version: impl Into<String>) {
        *self.version.borrow_mut() = version.into();
    }
}

/// A single line of text whose content is pulled from a shared string
/// every frame, so external code can update it by writing the `RefCell`.
struct DynText {
    text: Rc<RefCell<String>>,
    size: f32,
    color: Color,
}

impl Behavior for DynText {
    fn measured_content(&self) -> MeasuredContent {
        MeasuredContent::Text {
            text: self.text.borrow().clone(),
            size: self.size,
            font: FontId::DEFAULT,
            padding: (0.0, 10.0),
        }
    }

    fn render(&mut self, ctx: &mut RenderContext) {
        let (_, h) = ctx.size();
        let text = self.text.borrow();
        let style = TextStyle { size: self.size, color: self.color, font: FontId::DEFAULT };
        ctx.renderer
            .text_styled(&text, Vec2 { x: 0.0, y: h * 0.5 + self.size * 0.32 }, style);
    }
}

const TAB_SIZE: f32 = 18.0;
const BAR_HEIGHT: f32 = 52.0;
const TAB_GAP: f32 = 28.0;

const INK: Color = Color::hex(0x2a2a2a);
const INK_DIM: Color = Color::hex(0x8a8a8a);
const ACCENT: Color = Color::hex(0x3b6fd4);
const DIRTY: Color = Color::hex(0xd9534f);
const SAVED: Color = Color::hex(0x3faa5a);
const BAR_BG: Color = Color::hex(0xf5f5f5);

/// One clickable editor-mode tab. Draws its label, and an underline along
/// the text's bottom edge while it is the active mode.
struct NavTab {
    mode: EditorMode,
    state: NavState,
    hovered: bool,
    pressed: bool,
}

impl NavTab {
    fn active(&self) -> bool {
        self.state.mode.get() == self.mode
    }
}

impl Behavior for NavTab {
    fn measured_content(&self) -> MeasuredContent {
        MeasuredContent::Text {
            text: self.mode.label().to_string(),
            size: TAB_SIZE,
            font: FontId::DEFAULT,
            padding: (2.0, 10.0),
        }
    }

    fn focusable(&self) -> bool {
        true
    }

    fn pointer_event(&mut self, ctx: &mut EventContext, event: PointerEvent) {
        match event {
            PointerEvent::Enter => self.hovered = true,
            PointerEvent::Leave => self.hovered = false,
            PointerEvent::Down {
                button: MouseButton::Left,
                ..
            } => {
                self.pressed = true;
                ctx.capture_pointer();
                ctx.stop_propagation();
            }
            PointerEvent::Up {
                button: MouseButton::Left,
                x,
                y,
            } if self.pressed => {
                self.pressed = false;
                ctx.release_pointer();
                if ctx.ui.absolute_box(ctx.node).contains(x, y) {
                    self.state.mode.set(self.mode);
                }
                ctx.stop_propagation();
            }
            _ => {}
        }
    }

    fn render(&mut self, ctx: &mut RenderContext) {
        let (w, h) = ctx.size();
        let active = self.active();

        let color = if active {
            INK
        } else if self.hovered {
            INK
        } else {
            INK_DIM
        };

        let style = TextStyle {
            size: TAB_SIZE,
            color,
            font: FontId::DEFAULT,
        };
        let tw = ctx.renderer.measure(self.mode.label(), &style);
        let tx = ((w - tw) * 0.5).max(0.0);
        let baseline = h * 0.5 + TAB_SIZE * 0.32;
        ctx.renderer
            .text_styled(self.mode.label(), Vec2 { x: tx, y: baseline }, style);

        if active {
            let underline = Rect {
                x: tx,
                y: baseline + 5.0,
                width: tw,
                height: 2.0,
            };
            ctx.renderer.fill_rect(underline, ACCENT);
        }
    }
}

/// Save-status text. Reads `NavState::saved` every frame: "Unsaved
/// Project" in red, "Project Saved" in green.
struct SaveStatus {
    state: NavState,
}

impl SaveStatus {
    fn text(&self) -> &'static str {
        if self.state.saved.get() {
            "Project Saved"
        } else {
            "Unsaved Project"
        }
    }
}

impl Behavior for SaveStatus {
    fn measured_content(&self) -> MeasuredContent {
        MeasuredContent::Text {
            text: self.text().to_string(),
            size: 13.0,
            font: FontId::DEFAULT,
            padding: (0.0, 10.0),
        }
    }

    fn render(&mut self, ctx: &mut RenderContext) {
        let (_, h) = ctx.size();
        let color = if self.state.saved.get() { SAVED } else { DIRTY };
        ctx.renderer.text_styled(
            self.text(),
            Vec2 {
                x: 0.0,
                y: h * 0.5 + 13.0 * 0.32,
            },
            TextStyle {
                size: 13.0,
                color,
                font: FontId::DEFAULT,
            },
        );
    }
}

/// Spawn the nav bar as a child of `parent` and return the shared state.
pub fn spawn_nav_bar(
    ui: &mut UiTree,
    parent: NodeId,
    state: &NavState,
    menu_items: Vec<MenuItem>,
) -> NodeId {
    let mut bar_style = Style::row().gap(TAB_GAP);
    bar_style.taffy.size.width = taffy::prelude::percent(1.0);
    bar_style.taffy.size.height = taffy::prelude::length(BAR_HEIGHT);
    bar_style.taffy.align_items = Some(taffy::style::AlignItems::CENTER);
    bar_style.taffy.padding = taffy::geometry::Rect {
        left: taffy::prelude::length(20.0),
        right: taffy::prelude::length(20.0),
        top: taffy::prelude::length(0.0),
        bottom: taffy::prelude::length(0.0),
    };

    let bar = ui
        .spawn(parent, bar_style, Panel::new().background(BAR_BG))
        .unwrap();

    ui.spawn(bar, Style::default().sized(22.0, 22.0), MenuButton::new(menu_items))
        .unwrap();

    for mode in EditorMode::ALL {
        ui.spawn(
            bar,
            Style::default(),
            NavTab {
                mode,
                state: state.clone(),
                hovered: false,
                pressed: false,
            },
        )
        .unwrap();
    }

    // Save-status sits directly after the mode tabs.
    ui.spawn(
        bar,
        Style::default(),
        SaveStatus { state: state.clone() },
    )
    .unwrap();

    // A flexible spacer on each side of the project name keeps it centered
    // in the leftover space. Because everything is a real flex child, the
    // spacers collapse to zero on a narrow window and items butt up
    // against each other instead of overlapping.
    let spacer = || {
        let mut s = Style::default();
        s.taffy.flex_grow = 1.0;
        s.taffy.flex_basis = taffy::prelude::length(0.0);
        s.taffy.min_size.width = taffy::prelude::length(0.0);
        s
    };

    ui.spawn(bar, spacer(), EmptyBehavior).unwrap();

    ui.spawn(
        bar,
        Style::default(),
        DynText {
            text: state.project_name.clone(),
            size: 15.0,
            color: INK,
        },
    )
    .unwrap();

    ui.spawn(bar, spacer(), EmptyBehavior).unwrap();

    // Far-right: dynamic app version.
    ui.spawn(
        bar,
        Style::default(),
        DynText {
            text: state.version.clone(),
            size: 12.0,
            color: INK_DIM,
        },
    )
    .unwrap();

    bar
}
