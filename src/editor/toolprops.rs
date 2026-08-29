//! Row 1 of the middle column: settings for the active tool, laid out as
//! one horizontal row of bordered pill controls (left group + a
//! right-aligned group), bound to `session.tools.*`.

use rustle_core::{GridSnap, MarqueeMode, Project, Tool};
use rustle_ui::prelude::*;

use super::theme::*;
use super::Editor;

type Mut = Box<dyn FnOnce(&mut Project)>;

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

struct Cell {
    w: f32,
    draw: Box<dyn Fn(&mut Renderer, f32)>,
    hit: Box<dyn Fn(f32) -> Option<Mut>>,
}

#[derive(Default)]
struct RowForm {
    left: Vec<Cell>,
    right: Vec<Cell>,
    to_right: bool,
}

impl RowForm {
    fn right_group(&mut self) {
        self.to_right = true;
    }

    fn push(
        &mut self,
        w: f32,
        draw: impl Fn(&mut Renderer, f32) + 'static,
        hit: impl Fn(f32) -> Option<Mut> + 'static,
    ) {
        let cell = Cell { w, draw: Box::new(draw), hit: Box::new(hit) };
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

    fn hit(&self, px: f32, py: f32, width: f32) -> Option<Mut> {
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

    /// A bordered numeric pill: `label  ‹ value unit ›`. Left third
    /// decrements, right third increments.
    fn num(
        &mut self,
        label: &'static str,
        value: i64,
        unit: &'static str,
        min: i64,
        max: i64,
        step: i64,
        set: impl Fn(&mut Project, i64) + Copy + 'static,
    ) {
        let value = value.clamp(min, max);
        let w = width_for(label, 66.0);
        self.push(
            w,
            move |r, x| {
                let rect = Rect::new(x, PILL_Y, w, PILL_H);
                pill(r, rect);
                text(r, label, x + 10.0, PILL_Y + 8.0, 11.0, DIM);
                let val = if unit.is_empty() {
                    value.to_string()
                } else {
                    format!("{value} {unit}")
                };
                text_right(r, &val, x + w - 20.0, PILL_Y + 8.0, 11.5, INK);
                text(r, "‹", x + w - 15.0, PILL_Y + 6.0, 12.0, FAINT);
                text(r, "›", x + w - 9.0, PILL_Y + 6.0, 12.0, FAINT);
            },
            move |lx| {
                let nv = if lx < 22.0 {
                    (value - step).clamp(min, max)
                } else if lx > w - 22.0 {
                    (value + step).clamp(min, max)
                } else {
                    return None;
                };
                Some(Box::new(move |p: &mut Project| set(p, nv)) as Mut)
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
                Some(Box::new(move |p: &mut Project| set(p, nv)) as Mut)
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
            move |_lx| Some(Box::new(move |p: &mut Project| set(p, !on)) as Mut),
        );
    }
}

pub struct ToolPropertiesWidget {
    editor: Editor,
}

impl ToolPropertiesWidget {
    pub fn new(editor: Editor) -> Self {
        Self { editor }
    }

    fn build(&self) -> RowForm {
        let mut f = RowForm::default();
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
}

impl Behavior for ToolPropertiesWidget {
    fn render(&mut self, ctx: &mut RenderContext) {
        let (w, h) = ctx.size();
        ctx.renderer.fill_rect(Rect::new(0.0, 0.0, w, h), PANEL_BG);
        ctx.renderer.fill_rect(Rect::new(0.0, h - 1.0, w, 1.0), BORDER);

        if !self.editor.has_project() {
            text(ctx.renderer, "No project", 16.0, 16.0, 12.0, DIM);
            return;
        }

        self.build().render(ctx.renderer, w);
    }

    fn pointer_event(&mut self, ctx: &mut EventContext, event: PointerEvent) {
        if let PointerEvent::Down { button: MouseButton::Left, x, y } = event {
            let b = ctx.ui.absolute_box(ctx.node);
            if let Some(act) = self.build().hit(x - b.x, y - b.y, b.width) {
                self.editor.edit(|p| act(p));
                ctx.stop_propagation();
            }
        }
    }
}
