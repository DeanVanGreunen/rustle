//! The editor workspaces (Sprite / Animation / Level) shown in the middle
//! column below the nav bar.
//!
//! Every panel is a custom self-drawing [`Behavior`] that reads shared
//! state each frame via the cloneable [`Editor`] context. The only real
//! sub-nodes are the two pixel surfaces ([`MainViewport`], preview),
//! which use rustle_ui's `Viewport`.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use rustle_ui::prelude::*;
use rustle_ui::widgets::{Panel, Viewport};

use rustle_core::{EditorMode, Project, Selection, Session, Tool};

mod colorpicker;
mod details;
mod form;
pub mod io;
mod mainview;
mod newlevel;
mod outline;
mod projprops;
mod preview;
mod properties;
mod render;
mod selected;
mod textraster;
mod timeline;
pub mod theme;
mod toolprops;

use textraster::TextFont;

use details::ViewportDetails;
use mainview::MainViewport;
use outline::OutlineTreeView;
use toolprops::ToolPropertiesWidget;

/// Shared document handle: `None` until a project is opened.
pub type App = Rc<RefCell<Option<Project>>>;

/// Per-frame input snapshot the main loop fills in (the `Viewport`
/// content API doesn't forward modifier state).
#[derive(Default, Clone, Copy)]
pub struct InputSnapshot {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub space: bool,
    pub mouse_down: bool,
}

/// A nav-menu item the main loop must act on (menu closures have no
/// context of their own).
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum MenuAction {
    #[default]
    None,
    Import,
    Export,
    Save,
    SaveAs,
    ProjectProps,
}

const HISTORY_MAX: usize = 80;

/// Undo / redo ring. Snapshots are full `Project` clones; edits within
/// one "generation" (a mouse gesture or key press) coalesce into a
/// single entry.
#[derive(Default)]
struct History {
    undo: Vec<Project>,
    redo: Vec<Project>,
    last_gen: u64,
}

/// Cloneable editor context threaded into every editor widget.
#[derive(Clone)]
pub struct Editor {
    pub app: App,
    /// Live workspace selector — shared with `NavState.mode`.
    pub mode: Rc<Cell<EditorMode>>,
    /// Bumped on every document mutation; viewports cache against it.
    pub revision: Rc<Cell<u64>>,
    /// Set when the document changed since the last save.
    pub dirty: Rc<Cell<bool>>,
    pub input: Rc<RefCell<InputSnapshot>>,
    /// Texel coordinate under the cursor in the main viewport.
    pub main_cursor: Rc<Cell<(f32, f32)>>,
    /// Active marquee rectangle in texel space `(x, y, w, h)`.
    pub main_marquee: Rc<Cell<Option<(f32, f32, f32, f32)>>>,
    /// Copied pixel region `(w, h, rgba)`.
    pub clipboard: Rc<RefCell<Option<(u32, u32, Vec<u8>)>>>,
    /// Raised by the outline's "Add Level" button.
    pub new_level_open: Rc<Cell<bool>>,
    /// True while the Text tool has an active edit caret (suppresses
    /// single-key shortcuts in the main loop).
    pub text_editing: Rc<Cell<bool>>,
    /// Pending nav-menu action for the main loop.
    pub menu_action: Rc<Cell<MenuAction>>,
    /// Raised by Project Properties.
    pub project_props_open: Rc<Cell<bool>>,
    /// System font for the Text tool (may be unavailable).
    pub text_font: TextFont,
    hist: Rc<RefCell<History>>,
    /// Bumped once per gesture / key press by the main loop; edits sharing
    /// a generation coalesce into one undo entry.
    generation: Rc<Cell<u64>>,
}

impl Editor {
    pub fn new(app: App, mode: Rc<Cell<EditorMode>>) -> Self {
        Self {
            app,
            mode,
            revision: Rc::new(Cell::new(0)),
            dirty: Rc::new(Cell::new(false)),
            input: Rc::new(RefCell::new(InputSnapshot::default())),
            main_cursor: Rc::new(Cell::new((0.0, 0.0))),
            main_marquee: Rc::new(Cell::new(None)),
            clipboard: Rc::new(RefCell::new(None)),
            new_level_open: Rc::new(Cell::new(false)),
            text_editing: Rc::new(Cell::new(false)),
            menu_action: Rc::new(Cell::new(MenuAction::None)),
            project_props_open: Rc::new(Cell::new(false)),
            text_font: TextFont::load(),
            hist: Rc::new(RefCell::new(History::default())),
            generation: Rc::new(Cell::new(0)),
        }
    }

    /// Start a new undo generation — call on each mouse-down / key press.
    pub fn bump_generation(&self) {
        self.generation.set(self.generation.get().wrapping_add(1));
    }

    /// Forget all history (e.g. after opening a project).
    pub fn clear_history(&self) {
        *self.hist.borrow_mut() = History::default();
    }

    #[allow(dead_code)]
    pub fn can_undo(&self) -> bool {
        !self.hist.borrow().undo.is_empty()
    }

    pub fn undo(&self) {
        self.swap_history(true);
    }

    pub fn redo(&self) {
        self.swap_history(false);
    }

    fn swap_history(&self, is_undo: bool) {
        let mut h = self.hist.borrow_mut();
        let popped = if is_undo { h.undo.pop() } else { h.redo.pop() };
        let Some(mut restore) = popped else { return };
        let mut guard = self.app.borrow_mut();
        let Some(cur) = guard.as_mut() else { return };
        restore.file_path = cur.file_path.clone();
        let snapshot = std::mem::replace(cur, restore);
        if is_undo {
            h.redo.push(snapshot);
        } else {
            h.undo.push(snapshot);
        }
        h.last_gen = u64::MAX; // force the next edit to open a fresh entry
        drop(guard);
        drop(h);
        self.revision.set(self.revision.get().wrapping_add(1));
        self.dirty.set(true);
    }

    pub fn has_project(&self) -> bool {
        self.app.borrow().is_some()
    }

    pub fn mode(&self) -> EditorMode {
        self.mode.get()
    }

    pub fn with_project<R>(&self, f: impl FnOnce(&Project) -> R) -> Option<R> {
        self.app.borrow().as_ref().map(f)
    }

    /// Mutate the project; snapshots for undo (coalesced per generation),
    /// bumps the revision, and marks the doc dirty.
    pub fn edit<R>(&self, f: impl FnOnce(&mut Project) -> R) -> Option<R> {
        let g = self.generation.get();
        {
            let mut h = self.hist.borrow_mut();
            if h.last_gen != g {
                if let Some(p) = self.app.borrow().as_ref() {
                    h.undo.push(p.clone());
                    let overflow = h.undo.len().saturating_sub(HISTORY_MAX);
                    if overflow > 0 {
                        h.undo.drain(0..overflow);
                    }
                    h.redo.clear();
                    h.last_gen = g;
                }
            }
        }
        let mut guard = self.app.borrow_mut();
        let project = guard.as_mut()?;
        let r = f(project);
        self.revision.set(self.revision.get().wrapping_add(1));
        self.dirty.set(true);
        Some(r)
    }

    /// Edit only the session block (common case for panels).
    pub fn edit_session(&self, f: impl FnOnce(&mut Session)) {
        self.edit(|p| f(&mut p.session));
    }

    pub fn session<R>(&self, f: impl FnOnce(&Session) -> R) -> Option<R> {
        self.with_project(|p| f(&p.session))
    }

    pub fn tool(&self) -> Tool {
        self.session(|s| s.active_tool).unwrap_or_default()
    }

    pub fn set_tool(&self, tool: Tool) {
        self.edit_session(|s| s.active_tool = tool);
    }

    pub fn selection(&self) -> Selection {
        self.session(|s| s.selection).unwrap_or(Selection::None)
    }

    pub fn select(&self, sel: Selection) {
        self.edit_session(|s| s.selection = sel);
    }

    /// Set the shown frame during animation playback: refreshes the
    /// viewport but does not touch history or the dirty flag.
    pub fn set_playback_frame(&self, f: rustle_core::FrameId) {
        if let Some(p) = self.app.borrow_mut().as_mut() {
            if p.session.active.frame != Some(f) {
                p.session.active.frame = Some(f);
                self.revision.set(self.revision.get().wrapping_add(1));
            }
        }
    }

    /// The active marquee as inclusive integer texel bounds.
    fn marquee_bounds(&self) -> Option<(i64, i64, i64, i64)> {
        match self.main_marquee.get() {
            Some((x, y, w, h)) if w >= 0.5 && h >= 0.5 => Some((
                x.floor() as i64,
                y.floor() as i64,
                (x + w).ceil() as i64 - 1,
                (y + h).ceil() as i64 - 1,
            )),
            _ => None,
        }
    }

    /// True while a pixel-editing workspace is active.
    pub fn is_pixel_mode(&self) -> bool {
        self.mode() != EditorMode::Level
    }

    /// Clear (make transparent) the pixels inside the marquee.
    pub fn clear_marquee_pixels(&self) {
        let Some((x0, y0, x1, y1)) = self.marquee_bounds() else { return };
        self.edit(|p| {
            if let Some(l) = p.session.active.layer.and_then(|k| p.layers.get_mut(k)) {
                for y in y0.max(0)..=y1.min(l.height as i64 - 1) {
                    for x in x0.max(0)..=x1.min(l.width as i64 - 1) {
                        let i = ((y * l.width as i64 + x) * 4) as usize;
                        l.pixels[i..i + 4].copy_from_slice(&[0, 0, 0, 0]);
                    }
                }
            }
        });
    }

    /// Copy the marquee region of the active layer into the clipboard.
    pub fn copy_marquee(&self) {
        let Some((x0, y0, x1, y1)) = self.marquee_bounds() else { return };
        let region = self.with_project(|p| {
            let l = p.session.active.layer.and_then(|k| p.layers.get(k))?;
            let (lw, lh) = (l.width as i64, l.height as i64);
            let (x0, y0) = (x0.max(0), y0.max(0));
            let (x1, y1) = (x1.min(lw - 1), y1.min(lh - 1));
            if x1 < x0 || y1 < y0 {
                return None;
            }
            let (rw, rh) = ((x1 - x0 + 1) as u32, (y1 - y0 + 1) as u32);
            let mut buf = vec![0u8; (rw * rh * 4) as usize];
            for ry in 0..rh as i64 {
                for rx in 0..rw as i64 {
                    let si = (((y0 + ry) * lw + (x0 + rx)) * 4) as usize;
                    let di = ((ry * rw as i64 + rx) * 4) as usize;
                    buf[di..di + 4].copy_from_slice(&l.pixels[si..si + 4]);
                }
            }
            Some((rw, rh, buf))
        });
        if let Some(Some(r)) = region {
            *self.clipboard.borrow_mut() = Some(r);
        }
    }

    /// Blit the clipboard onto the active layer, top-left at the cursor.
    pub fn paste_at_cursor(&self) {
        let Some((rw, rh, buf)) = self.clipboard.borrow().clone() else { return };
        let (cx, cy) = self.main_cursor.get();
        let (ox, oy) = (cx.floor() as i64, cy.floor() as i64);
        self.edit(|p| {
            if let Some(l) = p.session.active.layer.and_then(|k| p.layers.get_mut(k)) {
                let (lw, lh) = (l.width as i64, l.height as i64);
                for ry in 0..rh as i64 {
                    for rx in 0..rw as i64 {
                        let (x, y) = (ox + rx, oy + ry);
                        if x < 0 || y < 0 || x >= lw || y >= lh {
                            continue;
                        }
                        let si = ((ry * rw as i64 + rx) * 4) as usize;
                        if buf[si + 3] == 0 {
                            continue;
                        }
                        let di = ((y * lw + x) * 4) as usize;
                        l.pixels[di..di + 4].copy_from_slice(&buf[si..si + 4]);
                    }
                }
            }
        });
    }

    /// Delete whatever is currently selected (entity or level placement).
    pub fn delete_selection(&self) {
        let sel = self.selection();
        if sel == Selection::None {
            return;
        }
        self.edit(|p| {
            match sel {
                Selection::Layer(k) => p.remove_layer(k),
                Selection::Group(k) => p.remove_group(k),
                Selection::Frame(k) => p.remove_frame(k),
                Selection::Animation(k) => p.remove_animation(k),
                Selection::Tile(k) => p.remove_tile(k),
                Selection::Background(k) => p.remove_background(k),
                Selection::Accessory(k) => p.remove_accessory(k),
                Selection::Level(k) => p.remove_level(k),
                Selection::LevelTile { level, index } => {
                    if let Some(l) = p.levels.get_mut(level) {
                        if index < l.tiles.len() {
                            l.tiles.remove(index);
                        }
                    }
                }
                Selection::LevelBackground { level, index } => {
                    if let Some(l) = p.levels.get_mut(level) {
                        if index < l.backgrounds.len() {
                            l.backgrounds.remove(index);
                        }
                    }
                }
                Selection::None => {}
            }
            p.session.selection = Selection::None;
        });
    }
}

// --- layout ---------------------------------------------------------

const OUTLINE_W: f32 = 244.0;
const PROPS_W: f32 = 320.0;
const TOOLPROPS_H: f32 = 52.0;
const DETAILS_H: f32 = 26.0;

fn fixed_col(width: f32) -> Style {
    let mut s = Style::column();
    s.taffy.size.width = taffy::prelude::length(width);
    s.taffy.size.height = taffy::prelude::percent(1.0);
    s.taffy.flex_shrink = 0.0;
    s
}

fn grow_col() -> Style {
    let mut s = Style::column();
    s.taffy.flex_grow = 1.0;
    s.taffy.flex_basis = taffy::prelude::length(0.0);
    s.taffy.min_size.width = taffy::prelude::length(0.0);
    s.taffy.size.height = taffy::prelude::percent(1.0);
    s
}

fn fixed_row(height: f32) -> Style {
    let mut s = Style::row();
    s.taffy.size.height = taffy::prelude::length(height);
    s.taffy.flex_shrink = 0.0;
    s
}

fn grow_row() -> Style {
    let mut s = Style::row();
    s.taffy.flex_grow = 1.0;
    s.taffy.flex_basis = taffy::prelude::length(0.0);
    s.taffy.min_size.height = taffy::prelude::length(0.0);
    s
}

/// Build the three workspaces as children of `parent`. Returns their
/// root node ids in [`EditorMode::ALL`] order so the caller can toggle
/// visibility with `ui.set_display`.
pub fn spawn_workspaces(ui: &mut UiTree, parent: NodeId, editor: &Editor) -> [NodeId; 3] {
    EditorMode::ALL.map(|mode| spawn_workspace(ui, parent, editor, mode))
}

/// Spawn the shared editor modal dialogs (currently: New Level) under
/// `root`. Call once.
pub fn spawn_dialogs(ui: &mut UiTree, root: NodeId, editor: &Editor) {
    newlevel::spawn_new_level_dialog(ui, root, editor);
    projprops::spawn_project_props_dialog(ui, root, editor);
}

fn spawn_workspace(ui: &mut UiTree, parent: NodeId, editor: &Editor, mode: EditorMode) -> NodeId {
    let mut root_style = Style::row();
    root_style.taffy.flex_grow = 1.0;
    root_style.taffy.flex_basis = taffy::prelude::length(0.0);
    root_style.taffy.min_size.height = taffy::prelude::length(0.0);
    root_style.taffy.size.width = taffy::prelude::percent(1.0);
    let root = ui
        .spawn(parent, root_style, Panel::new().background(theme::PANEL_BG))
        .unwrap();

    // Column 1 — outline tree.
    ui.spawn(
        root,
        fixed_col(OUTLINE_W),
        OutlineTreeView::new(editor.clone(), mode),
    )
    .unwrap();

    // Column 2 — tool props / main viewport / details.
    let mid = ui.spawn(root, grow_col(), Panel::new()).unwrap();

    ui.spawn(
        mid,
        fixed_row(TOOLPROPS_H),
        ToolPropertiesWidget::new(editor.clone()),
    )
    .unwrap();

    let mut vp_row = grow_row();
    vp_row.taffy.size.width = taffy::prelude::percent(1.0);
    ui.spawn(
        mid,
        vp_row,
        Viewport::new(MainViewport::new(editor.clone(), mode)).focusable(true),
    )
    .unwrap();

    ui.spawn(mid, fixed_row(DETAILS_H), ViewportDetails::new(editor.clone()))
        .unwrap();

    if mode == EditorMode::Animation {
        ui.spawn(
            mid,
            fixed_row(66.0),
            timeline::AnimationTimeline::new(editor.clone()),
        )
        .unwrap();
    }

    // Column 3 — selected properties (preview + tabs).
    let props = ui.spawn(root, fixed_col(PROPS_W), Panel::new()).unwrap();
    selected::spawn_selected_props(ui, props, editor, mode);

    root
}
