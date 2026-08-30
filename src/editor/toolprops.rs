//! Row 1 of the middle column: settings for the active tool, laid out as
//! one horizontal row of bordered pill controls (left group + a
//! right-aligned group), bound to `session.tools.*`.

use std::cell::{Cell, RefCell};

use rustle_core::{GridSnap, MarqueeMode, OnionSide, Project, Tool};
use rustle_ui::prelude::*;

use super::form::{Form, FormHit};
use super::hsvpanel::HsvPanel;
use super::numinput::{draw_number, fkey, NumState, NumberHit, SetFn};
use super::theme::*;
use super::Editor;

const SIG_PREV_COLOR: u64 = 1;
const SIG_NEXT_COLOR: u64 = 2;


const PILL_Y: f32 = 12.0;
const PILL_H: f32 = 28.0;
const GAP: f32 = 10.0;
const START_X: f32 = 16.0;
const EDGE: f32 = 16.0;

fn pill(r: &mut Renderer, rect: Rect) {
    r.fill_rounded_rect(rect, 6.0, PANEL_BG_ALT);
    stroke_round(r, rect);
}

fn stroke_round(r: &mut Renderer, rect: Rect) {
    r.fill_rect(Rect::new(rect.x, rect.y, rect.width, 1.0), BORDER);
    r.fill_rect(Rect::new(rect.x, rect.y + rect.height - 1.0, rect.width, 1.0), BORDER);
    r.fill_rect(Rect::new(rect.x, rect.y, 1.0, rect.height), BORDER);
    r.fill_rect(Rect::new(rect.x + rect.width - 1.0, rect.y, 1.0, rect.height), BORDER);
}

fn width_for(label: &str, extra: f32) -> f32 {
    18.0 + label.chars().count() as f32 * 6.6 + extra
}

struct RowCell {
    w: f32,
    draw: Box<dyn Fn(&mut Renderer, f32)>,
    hit: Box<dyn Fn(f32) -> Option<FormHit>>,
}

#[derive(Default)]
struct RowForm {
    left: Vec<RowCell>,
    right: Vec<RowCell>,
    to_right: bool,
    edit: Option<(u64, String)>,
}

impl RowForm {
    fn editing(mut self, e: Option<(u64, String)>) -> Self {
        self.edit = e;
        self
    }

    fn right_group(&mut self) {
        self.to_right = true;
    }

    fn push(
        &mut self,
        w: f32,
        draw: impl Fn(&mut Renderer, f32) + 'static,
        hit: impl Fn(f32) -> Option<FormHit> + 'static,
    ) {
        let cell = RowCell { w, draw: Box::new(draw), hit: Box::new(hit) };
        if self.to_right {
            self.right.push(cell);
        } else {
            self.left.push(cell);
        }
    }

    fn positions(&self, width: f32) -> (Vec<f32>, Vec<f32>) {
        let mut lx = Vec::new();
        let mut x = START_X;
        for c in &self.left {
            lx.push(x);
            x += c.w + GAP;
        }
        let total: f32 = self.right.iter().map(|c| c.w + GAP).sum();
        let mut rx = Vec::new();
        let mut x = width - EDGE - (total - GAP).max(0.0);
        for c in &self.right {
            rx.push(x);
            x += c.w + GAP;
        }
        (lx, rx)
    }

    fn render(&self, r: &mut Renderer, width: f32) {
        let (lx, rx) = self.positions(width);
        for (c, x) in self.left.iter().zip(lx) {
            (c.draw)(r, x);
        }
        for (c, x) in self.right.iter().zip(rx) {
            (c.draw)(r, x);
        }
    }

    fn hit(&self, px: f32, py: f32, width: f32) -> Option<FormHit> {
        if py < PILL_Y - 6.0 || py > PILL_Y + PILL_H + 6.0 {
            return None;
        }
        let (lx, rx) = self.positions(width);
        for (c, x) in self.left.iter().zip(lx) {
            if px >= x && px < x + c.w {
                return (c.hit)(px - x);
            }
        }
        for (c, x) in self.right.iter().zip(rx) {
            if px >= x && px < x + c.w {
                return (c.hit)(px - x);
            }
        }
        None
    }

    /// `label` then `[<] [value unit] [>]` — typeable + scrubbable.
    fn num(
        &mut self,
        label: &'static str,
        value: i64,
        unit: &'static str,
        min: i64,
        max: i64,
        step: i64,
        set: impl Fn(&mut Project, i64) + 'static,
    ) {
        let key = fkey(label);
        let edit_buf = self.edit.as_ref().filter(|(k, _)| *k == key).map(|(_, b)| b.clone());
        let value = value.clamp(min, max);
        let label_w = width_for(label, 6.0);
        let field_w = 96.0;
        let w = label_w + field_w;
        let set: SetFn = std::rc::Rc::new(set);
        let unit = unit.to_string();
        self.push(
            w,
            move |r, x| {
                text(r, label, x, PILL_Y + 8.0, 11.0, DIM);
                let vt = if unit.is_empty() { value.to_string() } else { format!("{value} {unit}") };
                draw_number(r, Rect::new(x + label_w, PILL_Y, field_w, PILL_H), &vt, edit_buf.as_deref());
            },
            move |lx| {
                if lx < label_w {
                    return None;
                }
                let local = lx - label_w;
                let btn = 18.0f32.min(field_w * 0.28);
                if local < btn {
                    return Some(FormHit::Number(NumberHit::Step(set.clone(), (value - step).clamp(min, max))));
                }
                if local > field_w - btn {
                    return Some(FormHit::Number(NumberHit::Step(set.clone(), (value + step).clamp(min, max))));
                }
                Some(FormHit::Number(NumberHit::Body { key, value, min, max, step, set: set.clone() }))
            },
        );
    }

    /// A bordered pill with a chevron that cycles through `options`.
    fn dropdown(
        &mut self,
        placeholder: &'static str,
        current: String,
        count: usize,
        index: usize,
        set: impl Fn(&mut Project, usize) + 'static,
    ) {
        let shown = if current.is_empty() { placeholder.to_string() } else { current };
        let w = width_for(&shown, 30.0).max(120.0);
        let set = std::rc::Rc::new(set);
        self.push(
            w,
            move |r, x| {
                let rect = Rect::new(x, PILL_Y, w, PILL_H);
                pill(r, rect);
                let dim = shown == placeholder;
                text(r, &shown, x + 10.0, PILL_Y + 8.0, 11.5, if dim { DIM } else { INK });
                text(r, "v", x + w - 15.0, PILL_Y + 7.0, 10.0, DIM);
            },
            move |_lx| {
                let n = count.max(1);
                let nv = (index + 1) % n;
                let set = set.clone();
                Some(FormHit::Mut(Box::new(move |p: &mut Project| set(p, nv))))
            },
        );
    }

    /// A bare checkbox + label (no surrounding pill).
    fn check(
        &mut self,
        label: &'static str,
        on: bool,
        set: impl Fn(&mut Project, bool) + Copy + 'static,
    ) {
        let w = width_for(label, 22.0);
        self.push(
            w,
            move |r, x| {
                let b = Rect::new(x, PILL_Y + 6.0, 15.0, 15.0);
                r.fill_rounded_rect(b, 3.0, if on { ACCENT } else { PANEL_BG_ALT });
                if on {
                    text(r, "x", b.x + 3.0, PILL_Y + 4.0, 12.0, Color::WHITE);
                } else {
                    stroke_round(r, b);
                }
                text(r, label, x + 22.0, PILL_Y + 8.0, 11.5, INK);
            },
            move |_lx| Some(FormHit::Mut(Box::new(move |p: &mut Project| set(p, !on)))),
        );
    }
}

const RIGHT_W: f32 = 330.0;

#[derive(Clone, Copy, PartialEq)]
enum Popup {
    None,
    Grid,
    Onion,
    PrevColor,
    NextColor,
}

pub struct ToolPropertiesWidget {
    editor: Editor,
    popup: Cell<Popup>,
    anim_t: Cell<f32>,
    abs: Cell<(f32, f32)>,
    /// Screen-local x of the right-column controls (snap, onion, tick, resize).
    ctl_x: Cell<[f32; 4]>,
    nums: RefCell<NumState>,
    hsv: HsvPanel,
}

impl ToolPropertiesWidget {
    pub fn new(editor: Editor) -> Self {
        Self {
            editor,
            popup: Cell::new(Popup::None),
            anim_t: Cell::new(0.0),
            abs: Cell::new((0.0, 0.0)),
            ctl_x: Cell::new([0.0; 4]),
            nums: RefCell::new(NumState::default()),
            hsv: HsvPanel::new(),
        }
    }

    fn onion_color(&self) -> [u8; 4] {
        match self.popup.get() {
            Popup::PrevColor => self.editor.session(|s| s.onion.prev_color),
            Popup::NextColor => self.editor.session(|s| s.onion.next_color),
            _ => None,
        }
        .unwrap_or([255, 255, 255, 255])
    }

    fn set_onion_color(&self, c: [u8; 4]) {
        match self.popup.get() {
            Popup::PrevColor => self.editor.edit_session(move |s| s.onion.prev_color = c),
            Popup::NextColor => self.editor.edit_session(move |s| s.onion.next_color = c),
            _ => {}
        }
    }

    fn num_edit(&self) -> Option<(u64, String)> {
        let n = self.nums.borrow();
        n.editing_key().map(|k| (k, n.buffer_for(k).unwrap_or("").to_string()))
    }

    fn build(&self) -> RowForm {
        let mut f = RowForm::default().editing(self.num_edit());
        let Some(t) = self.editor.session(|s| s.tools.clone()) else {
            return f;
        };
        match self.editor.tool() {
            Tool::Select => {
                f.check("Select whole group", t.select.whole_group, |p, on| {
                    p.session.tools.select.whole_group = on
                });
            }
            Tool::Marquee => {
                let idx = match t.marquee.mode {
                    MarqueeMode::Replace => 0,
                    MarqueeMode::Add => 1,
                    MarqueeMode::Subtract => 2,
                };
                let names = ["Replace", "Add", "Subtract"];
                f.dropdown("Mode", names[idx].into(), 3, idx, |p, i| {
                    p.session.tools.marquee.mode = match i {
                        1 => MarqueeMode::Add,
                        2 => MarqueeMode::Subtract,
                        _ => MarqueeMode::Replace,
                    }
                });
                f.num("Feather", t.marquee.feather as i64, "", 0, 64, 1, |p, v| {
                    p.session.tools.marquee.feather = v as u32
                });
                f.check("Lock aspect", t.marquee.lock_aspect, |p, on| {
                    p.session.tools.marquee.lock_aspect = on
                });
            }
            Tool::Pencil => {
                f.num("Size", t.pencil.size as i64, "px", 1, 64, 1, |p, v| {
                    p.session.tools.pencil.size = v as u32
                });
                f.num("Opacity", t.pencil.opacity as i64, "", 0, 255, 5, |p, v| {
                    p.session.tools.pencil.opacity = v as u8
                });
            }
            Tool::Eraser => {
                f.num("Size", t.eraser.size as i64, "px", 1, 64, 1, |p, v| {
                    p.session.tools.eraser.size = v as u32
                });
            }
            Tool::Eyedropper => {
                f.check("Sample merged", t.eyedropper.sample_merged, |p, on| {
                    p.session.tools.eyedropper.sample_merged = on
                });
            }
            Tool::Zoom => {
                f.num("Step", (t.zoom.step * 100.0) as i64, "%", 125, 800, 25, |p, v| {
                    p.session.tools.zoom.step = v as f32 / 100.0
                });
            }
            Tool::Move => {
                f.check("Snap to grid", t.move_.snap, |p, on| p.session.tools.move_.snap = on);
                f.num("Grid step", t.move_.snap_step.max(1) as i64, "px", 1, 128, 1, |p, v| {
                    p.session.tools.move_.snap_step = v as u32
                });
            }
            Tool::Line => {
                f.num("Width", t.line.width as i64, "px", 1, 64, 1, |p, v| {
                    p.session.tools.line.width = v as u32
                });
            }
            Tool::Rectangle => {
                f.check("Filled", t.rectangle.filled, |p, on| {
                    p.session.tools.rectangle.filled = on
                });
                f.num("Stroke", t.rectangle.stroke as i64, "px", 1, 32, 1, |p, v| {
                    p.session.tools.rectangle.stroke = v as u32
                });
            }
            Tool::Fill => {
                f.num("Tolerance", t.fill.tolerance as i64, "", 0, 255, 5, |p, v| {
                    p.session.tools.fill.tolerance = v as u8
                });
                f.check("Contiguous", t.fill.contiguous, |p, on| {
                    p.session.tools.fill.contiguous = on
                });
            }
            Tool::Text => {
                let fonts = self.editor.text_font.names();
                let fi = fonts.iter().position(|n| *n == t.text.font).unwrap_or(0);
                let fname = fonts.get(fi).cloned().unwrap_or_else(|| "System".into());
                let fonts2 = fonts.clone();
                f.dropdown("Selected Font", fname, fonts.len().max(1), fi, move |p, i| {
                    if let Some(n) = fonts2.get(i) {
                        p.session.tools.text.font = n.clone();
                    }
                });
                f.num("Font Size", t.text.size as i64, "Pt", 6, 200, 2, |p, v| {
                    p.session.tools.text.size = v as u32
                });
                f.num("Character Spacing", t.text.char_spacing as i64, "", -20, 40, 1, |p, v| {
                    p.session.tools.text.char_spacing = v as i32
                });
                f.num("Line Spacing", t.text.line_spacing as i64, "", -20, 60, 1, |p, v| {
                    p.session.tools.text.line_spacing = v as i32
                });

                f.right_group();
                f.check("Free Placement", t.text.free_placement, |p, on| {
                    p.session.tools.text.free_placement = on
                });
                let (gi, gname) = match t.text.grid_snap {
                    GridSnap::Off => (0, "Grid Snapping: Off"),
                    GridSnap::Half => (1, "Grid Snapping: Half"),
                    GridSnap::Full => (2, "Grid Snapping: Full"),
                };
                f.dropdown("Grid Snapping", gname.into(), 3, gi, |p, i| {
                    p.session.tools.text.grid_snap = match i {
                        1 => GridSnap::Half,
                        2 => GridSnap::Full,
                        _ => GridSnap::Off,
                    }
                });
            }
        }
        f
    }

    // --- right column: snap-to-grid + onion controls ----------------

    fn draw_right_column(&self, r: &mut Renderer, w: f32) {
        let col_x = w - RIGHT_W;
        r.fill_rect(Rect::new(col_x, 6.0, 1.0, PILL_H + 12.0), BORDER);

        let (gw, gh, grid_on, onion_on) = self
            .editor
            .session(|s| (s.grid.width, s.grid.height, s.grid.enabled, s.onion.enabled))
            .unwrap_or((16, 16, false, false));

        let mut x = col_x + 10.0;
        // Snap-to-grid pill.
        let snap_label = format!("Snap  {gw}\u{00d7}{gh}");
        let sw = width_for(&snap_label, 20.0);
        pill(r, Rect::new(x, PILL_Y, sw, PILL_H));
        text(r, &snap_label, x + 9.0, PILL_Y + 8.0, 11.0, if grid_on { ACCENT } else { INK });
        text(r, "v", x + sw - 13.0, PILL_Y + 7.0, 10.0, DIM);
        let x0 = x;
        x += sw + 8.0;

        // Onion pill.
        let ow = width_for("Onion", 20.0);
        pill(r, Rect::new(x, PILL_Y, ow, PILL_H));
        text(r, "Onion", x + 9.0, PILL_Y + 8.0, 11.0, if onion_on { ACCENT } else { INK });
        text(r, "v", x + ow - 13.0, PILL_Y + 7.0, 10.0, DIM);
        let x1 = x;
        x += ow + 8.0;

        // Onion enable tick.
        let cb = Rect::new(x, PILL_Y + 6.0, 16.0, 16.0);
        r.fill_rounded_rect(cb, 3.0, if onion_on { ACCENT } else { PANEL_BG_ALT });
        if onion_on {
            text(r, "x", cb.x + 3.0, PILL_Y + 4.0, 12.0, Color::WHITE);
        } else {
            stroke_round(r, cb);
        }
        let x2 = x;
        x += 16.0 + 12.0;

        // Resize-canvas pill.
        let rw = width_for("Resize Canvas", 14.0);
        pill(r, Rect::new(x, PILL_Y, rw, PILL_H));
        text(r, "Resize Canvas", x + 9.0, PILL_Y + 8.0, 11.0, INK);
        let x3 = x;

        self.ctl_x.set([x0, x1, x2, x3]);
    }

    fn grid_popup(&self, width: f32) -> Form {
        let mut f = Form::new(0.0, 8.0, width).editing(self.num_edit());
        f.heading("Snap to Grid");
        let (on, gw, gh) = self.editor.session(|s| (s.grid.enabled, s.grid.width, s.grid.height)).unwrap_or((false, 16, 16));
        f.toggle("Enabled", on, |p, v| p.session.grid.enabled = v);
        f.stepper("Cell width", gw as i64, 1, 512, 1, |p, v| p.session.grid.width = v as u32);
        f.stepper("Cell height", gh as i64, 1, 512, 1, |p, v| p.session.grid.height = v as u32);
        f
    }

    fn onion_popup(&self, width: f32) -> Form {
        let mut f = Form::new(0.0, 8.0, width).editing(self.num_edit());
        let o = self.editor.session(|s| s.onion).unwrap_or_default();

        f.heading("Onion Skin");
        f.toggle("Enabled", o.enabled, |p, v| p.session.onion.enabled = v);
        f.slider("Opacity", o.opacity, 0.0, 0.6, |p, v| p.session.onion.opacity = v);

        f.heading("Previous Frame");
        f.toggle("Show", o.prev_enabled, |p, v| p.session.onion.prev_enabled = v);
        f.segmented("Draw", &["Below", "Above"], (o.prev_side == OnionSide::Above) as usize, |p, i| {
            p.session.onion.prev_side = if i == 1 { OnionSide::Above } else { OnionSide::Below };
        });
        f.swatch("Previous frame colour", o.prev_color, SIG_PREV_COLOR);

        f.heading("Next Frame");
        f.toggle("Show", o.next_enabled, |p, v| p.session.onion.next_enabled = v);
        f.segmented("Draw", &["Below", "Above"], (o.next_side == OnionSide::Above) as usize, |p, i| {
            p.session.onion.next_side = if i == 1 { OnionSide::Above } else { OnionSide::Below };
        });
        f.swatch("Next frame colour", o.next_color, SIG_NEXT_COLOR);
        f
    }

    fn draw_stickman_preview(&self, r: &mut Renderer, area: Rect) {
        let o = self.editor.session(|s| s.onion).unwrap_or_default();
        r.fill_rounded_rect(area, 6.0, PANEL_BG);
        stroke_round(r, area);
        let ground = area.y + area.height - 12.0;
        let t = self.anim_t.get();
        // Ease in/out bounce 0..1..0
        let phase = (t * 1.4).sin() * 0.5 + 0.5;
        let px = |u: f32| area.x + 18.0 + u * (area.width - 36.0);
        let arc = |u: f32| -34.0 * (u - u * u) * 4.0; // parabola peak at u=0.5

        let prev_c = Color::rgba(o.prev_color[0] as f32 / 255.0, o.prev_color[1] as f32 / 255.0, o.prev_color[2] as f32 / 255.0, 0.7);
        let next_c = Color::rgba(o.next_color[0] as f32 / 255.0, o.next_color[1] as f32 / 255.0, o.next_color[2] as f32 / 255.0, 0.7);
        stickman(r, px(0.0), ground, prev_c);
        stickman(r, px(1.0), ground, next_c);
        stickman(r, px(phase), ground + arc(phase), INK);
    }

    /// Popup geometry (widget-local rect) + its content.
    fn popup_layout(&self, widget_w: f32) -> Option<(Rect, PopupContent)> {
        let ctl = self.ctl_x.get();
        let py = PILL_Y + PILL_H + 6.0;
        let clamp = |pw: f32, base: f32| base.min(widget_w - pw - 6.0).max(6.0);

        match self.popup.get() {
            Popup::None => None,
            Popup::Grid => {
                let f = self.grid_popup(240.0 - 24.0);
                let r = Rect::new(clamp(240.0, ctl[0]), py, 240.0, f.height() + 16.0);
                Some((r, PopupContent::Form(f, 0.0)))
            }
            Popup::Onion => {
                let f = self.onion_popup(300.0 - 24.0);
                let r = Rect::new(clamp(300.0, ctl[1]), py, 300.0, f.height() + 16.0 + 96.0);
                Some((r, PopupContent::Form(f, 96.0)))
            }
            Popup::PrevColor | Popup::NextColor => {
                let pw = 220.0;
                let inner = pw - 24.0;
                let h = self.hsv.height(inner) + 20.0;
                let base = if self.popup.get() == Popup::PrevColor { ctl[1] } else { ctl[1] + 20.0 };
                Some((Rect::new(clamp(pw, base), py, pw, h), PopupContent::Color))
            }
        }
    }

    fn open_popup(&self, ctx: &mut RenderContext) {
        let (w, _) = ctx.size();
        let Some((panel, content)) = self.popup_layout(w) else { return };

        ctx.renderer.fill_rounded_rect(panel, 8.0, PANEL_BG_ALT);
        stroke_round(ctx.renderer, panel);
        ctx.renderer.push_transform(Vec2 { x: panel.x + 12.0, y: panel.y });
        match content {
            PopupContent::Form(form, extra_h) => {
                form.render(ctx.renderer);
                if extra_h > 0.0 {
                    self.draw_stickman_preview(
                        ctx.renderer,
                        Rect::new(0.0, form.height() + 8.0, panel.width - 24.0, 82.0),
                    );
                }
            }
            PopupContent::Color => {
                text(ctx.renderer, "Onion Colour", 0.0, 2.0, 12.0, INK);
                self.hsv.draw(ctx.renderer, 0.0, 20.0, panel.width - 24.0, self.onion_color());
            }
        }
        ctx.renderer.pop_transform();
    }
}

enum PopupContent {
    Form(Form, f32),
    Color,
}

fn stickman(r: &mut Renderer, x: f32, y: f32, c: Color) {
    // y is the feet position; build upward.
    r.fill_rounded_rect(Rect::new(x - 3.0, y - 26.0, 6.0, 6.0), 3.0, c); // head
    r.fill_rect(Rect::new(x - 1.0, y - 20.0, 2.0, 12.0), c); // torso
    r.fill_rect(Rect::new(x - 6.0, y - 16.0, 12.0, 2.0), c); // arms
    r.fill_rect(Rect::new(x - 5.0, y - 8.0, 2.0, 8.0), c); // leg
    r.fill_rect(Rect::new(x + 3.0, y - 8.0, 2.0, 8.0), c); // leg
}

impl Behavior for ToolPropertiesWidget {
    fn update(&mut self, ctx: &mut UpdateContext) {
        let b = ctx.ui.absolute_box(ctx.node);
        self.abs.set((b.x, b.y));
        if self.popup.get() == Popup::Onion {
            self.anim_t.set(self.anim_t.get() + ctx.dt);
        }
        // Grab the modal pointer target while a popup is open so clicks in
        // the panel (which overflows this node's box) still reach us.
        let want = self.popup.get() != Popup::None;
        let is = ctx.ui.modal() == Some(ctx.node);
        if want && !is {
            let n = ctx.node;
            ctx.ui.set_modal(Some(n));
        } else if !want && is {
            ctx.ui.set_modal(None);
        }
    }

    fn render(&mut self, ctx: &mut RenderContext) {
        let (w, h) = ctx.size();
        ctx.renderer.fill_rect(Rect::new(0.0, 0.0, w, h), PANEL_BG);
        ctx.renderer.fill_rect(Rect::new(0.0, h - 1.0, w, 1.0), BORDER);

        if !self.editor.has_project() {
            text(ctx.renderer, "No project", 16.0, 16.0, 12.0, DIM);
            return;
        }

        self.build().render(ctx.renderer, w - RIGHT_W);
        self.draw_right_column(ctx.renderer, w);
    }

    fn overlay(&mut self, ctx: &mut RenderContext) {
        self.open_popup(ctx);
    }

    fn focusable(&self) -> bool {
        true
    }

    fn keyboard_event(&mut self, ctx: &mut EventContext, event: KeyboardEvent) {
        if matches!(self.popup.get(), Popup::PrevColor | Popup::NextColor) {
            if let Some(c) = self.hsv.on_key(&event, self.onion_color()) {
                self.set_onion_color(c);
            }
            ctx.stop_propagation();
            return;
        }
        if self.nums.borrow_mut().on_key(&event, &self.editor) {
            ctx.stop_propagation();
        }
    }

    fn pointer_event(&mut self, ctx: &mut EventContext, event: PointerEvent) {
        let b = ctx.ui.absolute_box(ctx.node);
        let color_popup = matches!(self.popup.get(), Popup::PrevColor | Popup::NextColor);

        match event {
            PointerEvent::Move { x, y } => {
                if color_popup {
                    if let Some((panel, _)) = self.popup_layout(b.width) {
                        if let Some(c) = self.hsv.on_move(
                            x - b.x - panel.x - 12.0,
                            y - b.y - panel.y,
                            self.onion_color(),
                        ) {
                            self.set_onion_color(c);
                        }
                        ctx.stop_propagation();
                    }
                } else if self.nums.borrow_mut().on_move(x, &self.editor) {
                    ctx.stop_propagation();
                }
                return;
            }
            PointerEvent::Up { .. } => {
                self.hsv.on_up();
                self.nums.borrow_mut().on_up();
                return;
            }
            _ => {}
        }
        let PointerEvent::Down { button: MouseButton::Left, x, y } = event else {
            return;
        };
        ctx.focus();
        let (lx, ly) = (x - b.x, y - b.y);
        self.nums.borrow_mut().commit(&self.editor);

        // Right-column controls.
        let ctl = self.ctl_x.get();
        if ly >= PILL_Y - 4.0 && ly <= PILL_Y + PILL_H + 8.0 && lx >= b.width - RIGHT_W {
            if lx >= ctl[3] {
                self.editor.resize_canvas_open.set(true);
                self.popup.set(Popup::None);
                ctx.stop_propagation();
                return;
            }
            if lx >= ctl[2] && lx < ctl[3] {
                self.editor.edit_session(|s| s.onion.enabled = !s.onion.enabled);
                ctx.stop_propagation();
                return;
            }
            if lx >= ctl[1] && lx < ctl[2] {
                self.popup.set(if self.popup.get() == Popup::Onion { Popup::None } else { Popup::Onion });
                ctx.stop_propagation();
                return;
            }
            if lx >= ctl[0] && lx < ctl[1] {
                self.popup.set(if self.popup.get() == Popup::Grid { Popup::None } else { Popup::Grid });
                ctx.stop_propagation();
                return;
            }
        }

        // Open popup: route the click into it.
        if self.popup.get() != Popup::None {
            if let Some((panel, content)) = self.popup_layout(b.width) {
                let px = lx - panel.x - 12.0;
                let py = ly - panel.y;
                let inside = lx >= panel.x && lx <= panel.x + panel.width && ly >= panel.y && ly <= panel.y + panel.height;
                if inside {
                    match content {
                        PopupContent::Form(form, _) => match form.hit(px, py) {
                            Some(FormHit::Mut(m)) => {
                                let _ = self.editor.edit(|p| m(p));
                            }
                            Some(FormHit::Number(nh)) => self.nums.borrow_mut().begin(nh, x, &self.editor),
                            Some(FormHit::Signal(SIG_PREV_COLOR)) => self.popup.set(Popup::PrevColor),
                            Some(FormHit::Signal(SIG_NEXT_COLOR)) => self.popup.set(Popup::NextColor),
                            _ => {}
                        },
                        PopupContent::Color => {
                            if let Some(c) = self.hsv.on_down(px, py, self.onion_color()) {
                                self.set_onion_color(c);
                            }
                        }
                    }
                    ctx.stop_propagation();
                    return;
                }
            }
            // click outside a colour picker returns to the onion panel;
            // outside anything else closes.
            self.popup.set(if color_popup { Popup::Onion } else { Popup::None });
        }

        // Tool form (left column).
        match self.build().hit(lx, ly, b.width - RIGHT_W) {
            Some(FormHit::Mut(m)) => {
                self.editor.edit(|p| m(p));
                ctx.stop_propagation();
            }
            Some(FormHit::Number(nh)) => {
                self.nums.borrow_mut().begin(nh, x, &self.editor);
                ctx.stop_propagation();
            }
            Some(FormHit::Signal(_)) | None => {}
        }
    }
}
