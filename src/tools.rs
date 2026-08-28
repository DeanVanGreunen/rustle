//! First column: a vertical strip of borderless icon buttons (a tool
//! palette). Each button shows only an icon; hovering it pops a tooltip
//! with the tool's name and keyboard shortcut.
//!
//! Icons are rasterised once at startup into small white RGBA bitmaps and
//! tinted at draw time (grey when idle, accent when active). The whole
//! strip is one `Behavior` — it lays out its own buttons, tracks hover,
//! and draws the tooltip in the `overlay` pass so it floats on top.

use std::cell::Cell;
use std::rc::Rc;

use rustle_ui::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
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

    /// Match a typed character to a tool (case-insensitive).
    pub fn from_key(c: char) -> Option<Tool> {
        let up = c.to_ascii_uppercase().to_string();
        Tool::ALL.into_iter().find(|t| t.shortcut() == up)
    }

    /// Match a just-pressed macroquad key to a tool.
    pub fn from_keycode(kc: macroquad::prelude::KeyCode) -> Option<Tool> {
        use macroquad::prelude::KeyCode as K;
        let c = match kc {
            K::V => 'V',
            K::M => 'M',
            K::B => 'B',
            K::I => 'I',
            K::Z => 'Z',
            K::H => 'H',
            K::L => 'L',
            K::R => 'R',
            K::F => 'F',
            K::T => 'T',
            _ => return None,
        };
        Tool::from_key(c)
    }
}

/// Shared "which tool is active" handle. Clone into the rest of the app.
#[derive(Clone)]
pub struct ToolState {
    pub active: Rc<Cell<Tool>>,
}

impl ToolState {
    pub fn new() -> Self {
        Self { active: Rc::new(Cell::new(Tool::Select)) }
    }
    pub fn get(&self) -> Tool {
        self.active.get()
    }
    pub fn set(&self, t: Tool) {
        self.active.set(t);
    }
}

// --- layout / colours -------------------------------------------------

const STRIP_PAD: f32 = 6.0;
const BTN: f32 = 40.0;
const ICON_DRAW: f32 = 24.0;

const IDLE: Color = Color::hex(0x555555);
const ACTIVE: Color = Color::hex(0x2f7fd8);
const HOVER_BG: Color = Color::hex(0xe9e9e9);
const ACTIVE_BG: Color = Color::hex(0xe2eefb);
const BORDER: Color = Color::hex(0xE4E4E4);
const TIP_BG: Color = Color::hex(0xFFFFFF);
const TIP_FG: Color = Color::hex(0x707070);
const TIP_DIM: Color = Color::hex(0x9fb0c8);

// --- the widget ------------------------------------------------------

struct ToolBar {
    state: ToolState,
    icons: Vec<ImageData>,
    hovered: Option<usize>,
    abs: (f32, f32),
}

impl ToolBar {
    fn slot_top(&self, i: usize) -> f32 {
        STRIP_PAD + i as f32 * BTN
    }

    fn hit(&self, lx: f32, ly: f32) -> Option<usize> {
        if lx < STRIP_PAD || lx > STRIP_PAD + BTN {
            return None;
        }
        let i = ((ly - STRIP_PAD) / BTN).floor();
        if i < 0.0 {
            return None;
        }
        let i = i as usize;
        (i < Tool::ALL.len()).then_some(i)
    }
}

impl Behavior for ToolBar {
    fn update(&mut self, ctx: &mut UpdateContext) {
        let b = ctx.ui.absolute_box(ctx.node);
        self.abs = (b.x, b.y);
    }

    fn pointer_event(&mut self, ctx: &mut EventContext, event: PointerEvent) {
        let b = ctx.ui.absolute_box(ctx.node);
        match event {
            PointerEvent::Leave => self.hovered = None,
            PointerEvent::Move { x, y } => {
                self.hovered = self.hit(x - b.x, y - b.y);
            }
            PointerEvent::Down { button: MouseButton::Left, x, y } => {
                if let Some(i) = self.hit(x - b.x, y - b.y) {
                    self.state.set(Tool::ALL[i]);
                    ctx.stop_propagation();
                }
            }
            _ => {}
        }
    }

    fn render(&mut self, ctx: &mut RenderContext) {
        let active = self.state.get();
        let icon_off = (BTN - ICON_DRAW) * 0.5;
        let (w, h) = ctx.size();

        // 1px right border.
        ctx.renderer
            .fill_rect(Rect::new(w - 1.0, 0.0, 1.0, h), BORDER);

        for (i, tool) in Tool::ALL.into_iter().enumerate() {
            let top = self.slot_top(i);
            let slot = Rect::new(STRIP_PAD, top, BTN, BTN);

            if tool == active {
                ctx.renderer.fill_rounded_rect(slot, 6.0, ACTIVE_BG);
            } else if self.hovered == Some(i) {
                ctx.renderer.fill_rounded_rect(slot, 6.0, HOVER_BG);
            }

            let tint = if tool == active { ACTIVE } else { IDLE };
            ctx.renderer.image_tinted(
                Rect::new(STRIP_PAD + icon_off, top + icon_off, ICON_DRAW, ICON_DRAW),
                &self.icons[i],
                tint,
            );
        }
    }

    fn overlay(&mut self, ctx: &mut RenderContext) {
        let Some(i) = self.hovered else { return };
        let tool = Tool::ALL[i];
        let o = Vec2 { x: -self.abs.0, y: -self.abs.1 };

        let name = tool.name();
        let sc = tool.shortcut();
        let name_style = TextStyle { size: 13.0, color: TIP_FG, font: FontId::DEFAULT };
        let sc_style = TextStyle { size: 13.0, color: TIP_DIM, font: FontId::DEFAULT };
        let name_w = ctx.renderer.measure(name, &name_style);
        let sc_w = ctx.renderer.measure(sc, &sc_style);

        let pad = 9.0;
        let gap = 10.0;
        let tip_w = pad * 2.0 + name_w + gap + sc_w;
        let tip_h = 26.0;
        let slot_top = self.slot_top(i);
        let x = self.abs.0 + STRIP_PAD + BTN + 8.0 + o.x;
        let y = self.abs.1 + slot_top + (BTN - tip_h) * 0.5 + o.y;

        ctx.renderer
            .fill_rounded_rect(Rect::new(x, y, tip_w, tip_h), 5.0, TIP_BG);
        let baseline = y + tip_h * 0.5 + 4.5;
        ctx.renderer
            .text_styled(name, Vec2 { x: x + pad, y: baseline }, name_style);
        ctx.renderer
            .text_styled(sc, Vec2 { x: x + pad + name_w + gap, y: baseline }, sc_style);
    }
}

/// Spawn the tool strip filling `column`. Returns the shared tool state.
pub fn spawn_tool_panel(ui: &mut UiTree, column: NodeId) -> ToolState {
    let state = ToolState::new();
    let icons = Tool::ALL.into_iter().map(render_icon).collect();

    let mut style = Style::default();
    style.taffy.size.width = taffy::prelude::length(STRIP_PAD * 2.0 + BTN + 1.0);
    style.taffy.size.height = taffy::prelude::percent(1.0);
    style.taffy.flex_shrink = 0.0;
    style.taffy.flex_grow = 0.0;

    ui.spawn(
        column,
        style,
        ToolBar { state: state.clone(), icons, hovered: None, abs: (0.0, 0.0) },
    )
    .unwrap();

    state
}

// --- icon rasteriser -------------------------------------------------

const G: usize = 48; // canvas grid; icons are drawn on 48x48 then scaled

struct IconCanvas {
    px: Vec<u8>,
}

impl IconCanvas {
    fn new() -> Self {
        Self { px: vec![0u8; G * G * 4] }
    }

    /// Additive white coverage at an integer pixel.
    fn plot(&mut self, x: i32, y: i32, a: f32) {
        if x < 0 || y < 0 || x >= G as i32 || y >= G as i32 || a <= 0.0 {
            return;
        }
        let idx = (y as usize * G + x as usize) * 4;
        let cur = self.px[idx + 3] as f32 / 255.0;
        let na = (cur + a).min(1.0);
        self.px[idx] = 255;
        self.px[idx + 1] = 255;
        self.px[idx + 2] = 255;
        self.px[idx + 3] = (na * 255.0) as u8;
    }

    /// Anti-aliased filled disc.
    fn disc(&mut self, cx: f32, cy: f32, r: f32) {
        let x0 = (cx - r - 1.0).floor() as i32;
        let x1 = (cx + r + 1.0).ceil() as i32;
        let y0 = (cy - r - 1.0).floor() as i32;
        let y1 = (cy + r + 1.0).ceil() as i32;
        for y in y0..=y1 {
            for x in x0..=x1 {
                let d = ((x as f32 + 0.5 - cx).powi(2) + (y as f32 + 0.5 - cy).powi(2)).sqrt();
                self.plot(x, y, (r - d + 0.5).clamp(0.0, 1.0));
            }
        }
    }

    /// Round-capped stroke of given width.
    fn line(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, w: f32) {
        let len = ((x1 - x0).powi(2) + (y1 - y0).powi(2)).sqrt().max(1.0);
        let steps = (len * 2.0) as i32;
        for s in 0..=steps {
            let t = s as f32 / steps as f32;
            self.disc(x0 + (x1 - x0) * t, y0 + (y1 - y0) * t, w * 0.5);
        }
    }

    fn polyline(&mut self, pts: &[(f32, f32)], w: f32, closed: bool) {
        for i in 0..pts.len().saturating_sub(1) {
            self.line(pts[i].0, pts[i].1, pts[i + 1].0, pts[i + 1].1, w);
        }
        if closed && pts.len() > 2 {
            let a = pts[pts.len() - 1];
            let b = pts[0];
            self.line(a.0, a.1, b.0, b.1, w);
        }
    }

    /// Scan-line fill of a simple polygon.
    fn fill_poly(&mut self, pts: &[(f32, f32)]) {
        let (mut ymin, mut ymax) = (f32::MAX, f32::MIN);
        for p in pts {
            ymin = ymin.min(p.1);
            ymax = ymax.max(p.1);
        }
        let y0 = ymin.floor().max(0.0) as i32;
        let y1 = ymax.ceil().min(G as f32) as i32;
        for y in y0..y1 {
            let yc = y as f32 + 0.5;
            let mut xs: Vec<f32> = Vec::new();
            for i in 0..pts.len() {
                let (ax, ay) = pts[i];
                let (bx, by) = pts[(i + 1) % pts.len()];
                if (ay <= yc && by > yc) || (by <= yc && ay > yc) {
                    xs.push(ax + (yc - ay) / (by - ay) * (bx - ax));
                }
            }
            xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
            for pair in xs.chunks(2) {
                if let [xa, xb] = pair {
                    for x in xa.floor() as i32..xb.ceil() as i32 {
                        self.plot(x, y, 1.0);
                    }
                }
            }
        }
    }

    fn stroke_rect(&mut self, x: f32, y: f32, w: f32, h: f32, t: f32) {
        self.polyline(
            &[(x, y), (x + w, y), (x + w, y + h), (x, y + h)],
            t,
            true,
        );
    }

    fn into_image(self) -> ImageData {
        ImageData::from_rgba(G as u32, G as u32, self.px)
    }
}

fn render_icon(tool: Tool) -> ImageData {
    let mut c = IconCanvas::new();
    let t = 3.0;
    match tool {
        Tool::Select => {
            c.fill_poly(&[
                (13.0, 8.0),
                (13.0, 36.0),
                (20.0, 29.0),
                (25.0, 40.0),
                (29.0, 38.0),
                (24.0, 27.0),
                (34.0, 27.0),
            ]);
        }
        Tool::Marquee => {
            // dashed rectangle
            let (x, y, w, h) = (9.0, 9.0, 30.0, 30.0);
            let dash = 5.0;
            let mut px = x;
            while px < x + w {
                c.line(px, y, (px + dash).min(x + w), y, t);
                c.line(px, y + h, (px + dash).min(x + w), y + h, t);
                px += dash * 2.0;
            }
            let mut py = y;
            while py < y + h {
                c.line(x, py, x, (py + dash).min(y + h), t);
                c.line(x + w, py, x + w, (py + dash).min(y + h), t);
                py += dash * 2.0;
            }
        }
        Tool::Pencil => {
            c.fill_poly(&[(10.0, 38.0), (14.0, 30.0), (30.0, 14.0), (34.0, 18.0), (18.0, 34.0)]);
            c.fill_poly(&[(30.0, 14.0), (34.0, 10.0), (38.0, 14.0), (34.0, 18.0)]);
            c.fill_poly(&[(10.0, 38.0), (14.0, 36.0), (12.0, 40.0)]);
        }
        Tool::Eyedropper => {
            c.line(12.0, 36.0, 28.0, 20.0, 4.0);
            c.disc(31.0, 17.0, 5.0);
            c.line(29.0, 11.0, 37.0, 19.0, 5.0);
        }
        Tool::Zoom => {
            // magnifier ring + handle + plus
            c.disc(19.0, 19.0, 10.0);
            // knock out the centre
            for y in 0..G as i32 {
                for x in 0..G as i32 {
                    let d = (((x as f32 + 0.5) - 19.0).powi(2) + ((y as f32 + 0.5) - 19.0).powi(2)).sqrt();
                    if d < 7.0 {
                        let idx = (y as usize * G + x as usize) * 4;
                        c.px[idx + 3] = 0;
                    }
                }
            }
            c.line(27.0, 27.0, 38.0, 38.0, 4.0);
            c.line(14.0, 19.0, 24.0, 19.0, 2.5);
            c.line(19.0, 14.0, 19.0, 24.0, 2.5);
        }
        Tool::Move => {
            c.line(24.0, 8.0, 24.0, 40.0, t);
            c.line(8.0, 24.0, 40.0, 24.0, t);
            c.fill_poly(&[(24.0, 5.0), (19.0, 12.0), (29.0, 12.0)]);
            c.fill_poly(&[(24.0, 43.0), (19.0, 36.0), (29.0, 36.0)]);
            c.fill_poly(&[(5.0, 24.0), (12.0, 19.0), (12.0, 29.0)]);
            c.fill_poly(&[(43.0, 24.0), (36.0, 19.0), (36.0, 29.0)]);
        }
        Tool::Line => {
            c.line(9.0, 39.0, 39.0, 9.0, t);
        }
        Tool::Rectangle => {
            c.stroke_rect(9.0, 12.0, 30.0, 24.0, t);
        }
        Tool::Fill => {
            c.disc(24.0, 24.0, 15.0);
            for y in 0..G as i32 {
                for x in 0..G as i32 {
                    let idx = (y as usize * G + x as usize) * 4;
                    if c.px[idx + 3] == 0 {
                        continue;
                    }
                    let f = (x as f32 - 9.0) / 30.0; // 0 (left) -> 1 (right)
                    c.px[idx + 3] = ((1.0 - f).clamp(0.0, 1.0) * 255.0) as u8;
                }
            }
        }
        Tool::Text => {
            c.line(10.0, 11.0, 38.0, 11.0, 4.0); // bar
            c.line(24.0, 11.0, 24.0, 39.0, 4.0); // stem
        }
    }
    c.into_image()
}
