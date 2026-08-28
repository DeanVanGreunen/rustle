//! Hamburger menu button + the full-height side panel it opens.
//!
//! The panel is drawn entirely in [`Behavior::overlay`] (like `Dropdown`'s
//! popup) so it paints on top of the whole UI, and the widget makes itself
//! the tree's modal pointer target while open so a click anywhere outside
//! the panel closes it.

use rustle_ui::prelude::*;

type Action = Box<dyn FnMut()>;

/// One entry in the menu.
pub enum MenuItem {
    /// A clickable row: label, optional shortcut hint, whether to show a
    /// submenu chevron, and the click handler.
    Action {
        label: String,
        shortcut: Option<String>,
        chevron: bool,
        on_click: Option<Action>,
    },
    /// A horizontal divider.
    Separator,
}

impl MenuItem {
    pub fn action(label: impl Into<String>, shortcut: Option<&str>, on_click: impl FnMut() + 'static) -> Self {
        MenuItem::Action {
            label: label.into(),
            shortcut: shortcut.map(str::to_string),
            chevron: false,
            on_click: Some(Box::new(on_click)),
        }
    }

    /// A row with a submenu chevron on the right.
    pub fn submenu(label: impl Into<String>, on_click: impl FnMut() + 'static) -> Self {
        MenuItem::Action {
            label: label.into(),
            shortcut: None,
            chevron: true,
            on_click: Some(Box::new(on_click)),
        }
    }

    pub fn separator() -> Self {
        MenuItem::Separator
    }

    fn height(&self) -> f32 {
        match self {
            MenuItem::Action { .. } => ROW_H,
            MenuItem::Separator => SEP_H,
        }
    }

    fn is_action(&self) -> bool {
        matches!(self, MenuItem::Action { .. })
    }
}

const PANEL_W: f32 = 300.0;
const ROW_H: f32 = 46.0;
const SEP_H: f32 = 17.0;
const PAD_TOP: f32 = 8.0;
const TEXT_X: f32 = 20.0;

const LABEL: Color = Color::hex(0x2f2f2f);
const SHORTCUT: Color = Color::hex(0x9a9a9a);
const HOVER_BG: Color = Color::hex(0xf0f0f0);
const PANEL_BG: Color = Color::WHITE;
const LINE: Color = Color::hex(0xe2e2e2);
const SCRIM: Color = Color::rgba(0.0, 0.0, 0.0, 0.18);
const ICON: Color = Color::hex(0x333333);

/// The hamburger button. Owns the side-panel state.
pub struct MenuButton {
    items: Vec<MenuItem>,
    open: bool,
    hovered_row: Option<usize>,
    /// Cached absolute position of this node + surface size, refreshed each
    /// frame in `update` (needed because `overlay` only gets local coords).
    abs: (f32, f32),
    surface: (f32, f32),
}

impl MenuButton {
    pub fn new(items: Vec<MenuItem>) -> Self {
        Self {
            items,
            open: false,
            hovered_row: None,
            abs: (0.0, 0.0),
            surface: (0.0, 0.0),
        }
    }

    /// Panel rect in screen space.
    fn panel_rect(&self) -> Rect {
        Rect::new(0.0, 0.0, PANEL_W, self.surface.1)
    }

    /// Screen-space top of row `i` (rows start after PAD_TOP).
    fn row_top(&self, i: usize) -> f32 {
        PAD_TOP + self.items[..i].iter().map(MenuItem::height).sum::<f32>()
    }

    fn row_at(&self, sx: f32, sy: f32) -> Option<usize> {
        if !self.panel_rect().contains(sx, sy) {
            return None;
        }
        for i in 0..self.items.len() {
            let top = self.row_top(i);
            if sy >= top && sy < top + self.items[i].height() {
                return self.items[i].is_action().then_some(i);
            }
        }
        None
    }

    fn close(&mut self) {
        self.open = false;
        self.hovered_row = None;
    }
}

impl Behavior for MenuButton {
    fn focusable(&self) -> bool {
        true
    }

    fn update(&mut self, ctx: &mut UpdateContext) {
        let abs = ctx.ui.absolute_box(ctx.node);
        self.abs = (abs.x, abs.y);
        self.surface = ctx.ui.surface_size();

        let want_modal = self.open;
        let is_modal = ctx.ui.modal() == Some(ctx.node);
        if want_modal && !is_modal {
            let n = ctx.node;
            ctx.ui.set_modal(Some(n));
        } else if !want_modal && is_modal {
            ctx.ui.set_modal(None);
        }
    }

    fn keyboard_event(&mut self, ctx: &mut EventContext, event: KeyboardEvent) {
        if let KeyboardEvent::KeyDown { key: Key::Escape, .. } = event {
            if self.open {
                self.close();
                ctx.stop_propagation();
            }
        }
    }

    fn pointer_event(&mut self, ctx: &mut EventContext, event: PointerEvent) {
        let node_box = ctx.ui.absolute_box(ctx.node);

        if !self.open {
            if let PointerEvent::Down { button: MouseButton::Left, x, y } = event {
                if node_box.contains(x, y) {
                    self.open = true;
                    ctx.focus();
                    ctx.stop_propagation();
                }
            }
            return;
        }

        // Open: this node is the modal target, so every pointer event lands
        // here regardless of position.
        match event {
            PointerEvent::Move { x, y } => {
                self.hovered_row = self.row_at(x, y);
            }
            PointerEvent::Wheel { .. } => {
                ctx.stop_propagation();
            }
            PointerEvent::Down { button: MouseButton::Left, x, y } => {
                if node_box.contains(x, y) {
                    self.close();
                } else if let Some(i) = self.row_at(x, y) {
                    if let MenuItem::Action { on_click, .. } = &mut self.items[i] {
                        if let Some(cb) = on_click.as_mut() {
                            cb();
                        }
                    }
                    self.close();
                } else {
                    // Click on the scrim / outside the panel.
                    self.close();
                }
                ctx.stop_propagation();
            }
            PointerEvent::Down { .. } | PointerEvent::Up { .. } => {
                ctx.stop_propagation();
            }
            _ => {}
        }
    }

    fn render(&mut self, ctx: &mut RenderContext) {
        let (w, h) = ctx.size();
        let bar_w = w.min(20.0);
        let x0 = (w - bar_w) * 0.5;
        for k in 0..3 {
            ctx.renderer.fill_rect(
                Rect { x: x0, y: h * 0.5 - 6.0 + k as f32 * 6.0, width: bar_w, height: 2.0 },
                ICON,
            );
        }
    }

    fn overlay(&mut self, ctx: &mut RenderContext) {
        if !self.open {
            return;
        }
        // Everything below is drawn in screen space; shift the local origin
        // back to (0, 0) of the window.
        let o = Vec2 { x: -self.abs.0, y: -self.abs.1 };
        let at = |x: f32, y: f32| Vec2 { x: x + o.x, y: y + o.y };
        let rect = |x: f32, y: f32, w: f32, h: f32| Rect::new(x + o.x, y + o.y, w, h);

        let (sw, sh) = self.surface;

        // Scrim over the whole window, then the panel.
        ctx.renderer.fill_rect(rect(0.0, 0.0, sw, sh), SCRIM);
        ctx.renderer.fill_rect(rect(0.0, 0.0, PANEL_W, sh), PANEL_BG);

        for (i, item) in self.items.iter().enumerate() {
            let top = self.row_top(i);
            match item {
                MenuItem::Separator => {
                    ctx.renderer.fill_rect(
                        rect(TEXT_X, top + SEP_H * 0.5, PANEL_W - TEXT_X * 2.0, 1.0),
                        LINE,
                    );
                }
                MenuItem::Action { label, shortcut, chevron, .. } => {
                    if self.hovered_row == Some(i) {
                        ctx.renderer.fill_rect(rect(0.0, top, PANEL_W, ROW_H), HOVER_BG);
                    }
                    let baseline = top + ROW_H * 0.5 + 5.0;
                    ctx.renderer.text_styled(
                        label,
                        at(TEXT_X, baseline),
                        TextStyle { size: 16.0, color: LABEL, font: FontId::DEFAULT },
                    );
                    if let Some(sc) = shortcut {
                        let st = TextStyle { size: 13.0, color: SHORTCUT, font: FontId::DEFAULT };
                        let tw = ctx.renderer.measure(sc, &st);
                        ctx.renderer.text_styled(sc, at(PANEL_W - TEXT_X - tw, baseline), st);
                    }
                    if *chevron {
                        let cx = PANEL_W - TEXT_X - 4.0;
                        let cy = top + ROW_H * 0.5;
                        for k in 0..5 {
                            let d = k as f32;
                            ctx.renderer.fill_rect(rect(cx - 4.0 + d, cy - 4.0 + d, 2.0, 2.0), SHORTCUT);
                            ctx.renderer.fill_rect(rect(cx - 4.0 + d, cy + 4.0 - d, 2.0, 2.0), SHORTCUT);
                        }
                    }
                }
            }
        }
    }
}
