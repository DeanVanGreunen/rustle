//! A tiny retained-free form builder used by the tool-properties and
//! selected-properties panels. Rows are rebuilt every frame; each carries
//! a draw closure and a hit closure that returns the mutation to run.
//!
//! ```ignore
//! let mut f = Form::new(12.0, 12.0, width - 24.0);
//! f.heading("Pencil");
//! f.stepper("Size", size as i64, 1, 64, 1, |s, v| s.tools.pencil.size = v as u32);
//! f.toggle("Snap", snap, |s, on| s.tools.move_.snap = on);
//! f.render(r); // in Behavior::render
//! if let Some(act) = f.hit(lx, ly) { editor.edit_session(|s| act(s)); } // in pointer_event
//! ```

use rustle_core::Project;
use rustle_ui::prelude::*;

use super::numinput::{draw_number, fkey, NumberHit, SetFn};
use super::theme::*;

pub type Mut = Box<dyn FnOnce(&mut Project)>;

/// Result of a form hit.
pub enum FormHit {
    Mut(Mut),
    Number(NumberHit),
    /// A tagged button was pressed; the host decides what to do.
    Signal(u64),
}

const ROW_H: f32 = 26.0;
const HEAD_H: f32 = 28.0;
const GAP: f32 = 4.0;

struct Row {
    y: f32,
    h: f32,
    draw: Box<dyn Fn(&mut Renderer, f32, f32, f32)>,
    hit: Box<dyn Fn(f32, f32) -> Option<FormHit>>,
}

pub struct Form {
    x: f32,
    width: f32,
    cursor: f32,
    rows: Vec<Row>,
    /// (key, buffer) of the number field currently being typed into.
    edit: Option<(u64, String)>,
}

impl Form {
    pub fn new(x: f32, y: f32, width: f32) -> Self {
        Self { x, width, cursor: y, rows: Vec::new(), edit: None }
    }

    /// Tell the form which number field is mid-edit (key + buffer text).
    pub fn editing(mut self, e: Option<(u64, String)>) -> Self {
        self.edit = e;
        self
    }

    /// Total height consumed so far.
    pub fn height(&self) -> f32 {
        self.cursor
    }

    pub fn render(&self, r: &mut Renderer) {
        for row in &self.rows {
            (row.draw)(r, self.x, row.y, self.width);
        }
    }

    /// `lx` / `ly` are local to the widget (same space as `x` / `y`
    /// passed to [`Form::new`]).
    pub fn hit(&self, lx: f32, ly: f32) -> Option<FormHit> {
        for row in &self.rows {
            if ly >= row.y && ly < row.y + row.h {
                return (row.hit)(lx - self.x, ly - row.y);
            }
        }
        None
    }

    fn push(
        &mut self,
        h: f32,
        draw: impl Fn(&mut Renderer, f32, f32, f32) + 'static,
        hit: impl Fn(f32, f32) -> Option<FormHit> + 'static,
    ) {
        self.rows.push(Row {
            y: self.cursor,
            h,
            draw: Box::new(draw),
            hit: Box::new(hit),
        });
        self.cursor += h + GAP;
    }

    pub fn heading(&mut self, label: &str) {
        let label = label.to_string();
        self.push(
            HEAD_H,
            move |r, x, y, w| {
                text(r, &label, x, y + 6.0, 13.0, INK);
                r.fill_rect(Rect::new(x, y + HEAD_H - 4.0, w, 1.0), BORDER);
            },
            |_, _| None,
        );
    }

    pub fn readonly(&mut self, label: &str, value: String) {
        let label = label.to_string();
        self.push(
            ROW_H,
            move |r, x, y, w| {
                text(r, &label, x, y + 6.0, 12.0, DIM);
                text_right(r, &value, x + w, y + 6.0, 12.0, INK);
            },
            |_, _| None,
        );
    }

    /// A choice control: `label ....  ‹ current ›`. Click the left / right
    /// chevron zones to step through `count` options.
    pub fn cycle(
        &mut self,
        label: &str,
        current: String,
        count: usize,
        current_idx: usize,
        set: impl Fn(&mut Project, usize) + Copy + 'static,
    ) {
        let label = label.to_string();
        let w = self.width;
        let n = count.max(1);
        let box_w = (w * 0.55).min(150.0);
        let box_x = w - box_w;
        self.push(
            ROW_H,
            move |r, x, y, _w| {
                text(r, &label, x, y + 6.0, 12.0, DIM);
                let rect = Rect::new(x + box_x, y + 3.0, box_w, 20.0);
                r.fill_rounded_rect(rect, 4.0, FIELD_BG);
                text(r, "\u{2039}", rect.x + 6.0, y + 4.0, 13.0, DIM);
                text(r, "\u{203a}", rect.x + rect.width - 12.0, y + 4.0, 13.0, DIM);
                let tw = r.measure(&current, &TextStyle { size: 11.0, color: INK, font: FontId::DEFAULT });
                text(r, &current, rect.x + (rect.width - tw) * 0.5, y + 6.0, 11.0, INK);
            },
            move |lx, _ly| {
                if lx < box_x {
                    return None;
                }
                let mid = box_x + box_w * 0.5;
                let nv = if lx < mid {
                    (current_idx + n - 1) % n
                } else {
                    (current_idx + 1) % n
                };
                Some(FormHit::Mut(Box::new(move |p: &mut Project| set(p, nv))))
            },
        );
    }

    pub fn stepper(
        &mut self,
        label: &str,
        value: i64,
        min: i64,
        max: i64,
        step: i64,
        set: impl Fn(&mut Project, i64) + 'static,
    ) {
        self.stepper_unit(label, value, "", min, max, step, set);
    }

    /// `[<] [value unit] [>]` — typeable, click-drag scrubbable.
    pub fn stepper_unit(
        &mut self,
        label: &str,
        value: i64,
        unit: &str,
        min: i64,
        max: i64,
        step: i64,
        set: impl Fn(&mut Project, i64) + 'static,
    ) {
        let key = fkey(label);
        let label = label.to_string();
        let unit = unit.to_string();
        let value = value.clamp(min, max);
        let editing = self.edit.as_ref().filter(|(k, _)| *k == key).map(|(_, b)| b.clone());
        let field_w = 118.0f32.min(self.width * 0.55);
        let field_x = self.width - field_w;
        let set: SetFn = std::rc::Rc::new(set);

        self.push(
            ROW_H,
            move |r, x, y, _w| {
                text(r, &label, x, y + 6.0, 12.0, DIM);
                let vt = if unit.is_empty() {
                    value.to_string()
                } else {
                    format!("{value} {unit}")
                };
                draw_number(
                    r,
                    Rect::new(x + field_x, y + 3.0, field_w, 20.0),
                    &vt,
                    editing.as_deref(),
                );
            },
            move |lx, _ly| {
                if lx < field_x {
                    return None;
                }
                let local = lx - field_x;
                let btn = 18.0f32.min(field_w * 0.28);
                if local < btn {
                    return Some(FormHit::Number(NumberHit::Step(set.clone(), (value - step).clamp(min, max))));
                }
                if local > field_w - btn {
                    return Some(FormHit::Number(NumberHit::Step(set.clone(), (value + step).clamp(min, max))));
                }
                Some(FormHit::Number(NumberHit::Body {
                    key,
                    value,
                    min,
                    max,
                    step,
                    set: set.clone(),
                }))
            },
        );
    }

    pub fn toggle(
        &mut self,
        label: &str,
        on: bool,
        set: impl Fn(&mut Project, bool) + Copy + 'static,
    ) {
        let label = label.to_string();
        let w = self.width;
        self.push(
            ROW_H,
            move |r, x, y, _w| {
                text(r, &label, x, y + 6.0, 12.0, DIM);
                let box_ = Rect::new(x + w - 16.0, y + 4.0, 16.0, 16.0);
                r.fill_rounded_rect(box_, 3.0, if on { ACCENT } else { FIELD_BG });
                if on {
                    text(r, "x", box_.x + 4.0, y + 4.0, 12.0, Color::WHITE);
                }
            },
            move |_lx, _ly| Some(FormHit::Mut(Box::new(move |p: &mut Project| set(p, !on)))),
        );
    }

    /// A click-to-position slider. `value` / `min` / `max` are floats;
    /// `set` receives the new value.
    pub fn slider(
        &mut self,
        label: &str,
        value: f32,
        min: f32,
        max: f32,
        set: impl Fn(&mut Project, f32) + Copy + 'static,
    ) {
        let label = label.to_string();
        let w = self.width;
        let track_x = w * 0.42;
        let track_w = w - track_x;
        let range = (max - min).max(0.0001);
        let t = ((value - min) / range).clamp(0.0, 1.0);
        self.push(
            ROW_H,
            move |r, x, y, _w| {
                text(r, &label, x, y + 6.0, 12.0, DIM);
                let ty = y + ROW_H * 0.5;
                r.fill_rect(Rect::new(x + track_x, ty - 1.5, track_w, 3.0), FIELD_BG);
                r.fill_rect(Rect::new(x + track_x, ty - 1.5, track_w * t, 3.0), ACCENT);
                let kx = x + track_x + track_w * t;
                r.fill_rounded_rect(Rect::new(kx - 4.0, ty - 6.0, 8.0, 12.0), 3.0, ACCENT);
                text_right(r, &format!("{value:.2}"), x + track_x - 8.0, y + 6.0, 11.0, INK);
            },
            move |lx, _ly| {
                if lx < track_x {
                    return None;
                }
                let nt = ((lx - track_x) / track_w).clamp(0.0, 1.0);
                let nv = min + nt * range;
                Some(FormHit::Mut(Box::new(move |p: &mut Project| set(p, nv))))
            },
        );
    }

    /// A swatch row: `label ........ [ colour chip ]`. Clicking emits
    /// `FormHit::Signal(signal)` so the host can open a colour picker.
    pub fn swatch(&mut self, label: &str, colour: [u8; 4], signal: u64) {
        let label = label.to_string();
        let w = self.width;
        self.push(
            ROW_H,
            move |r, x, y, _w| {
                text(r, &label, x, y + 6.0, 12.0, DIM);
                let chip = Rect::new(x + w - 56.0, y + 3.0, 56.0, 20.0);
                r.fill_rounded_rect(
                    chip,
                    3.0,
                    Color::rgba(colour[0] as f32 / 255.0, colour[1] as f32 / 255.0, colour[2] as f32 / 255.0, 1.0),
                );
                r.fill_rect(Rect::new(chip.x, chip.y, chip.width, 1.0), BORDER);
                text_right(r, "\u{203a}", chip.x - 4.0, y + 6.0, 12.0, DIM);
            },
            move |_lx, _ly| Some(FormHit::Signal(signal)),
        );
    }

    pub fn segmented(
        &mut self,
        label: &str,
        options: &[&str],
        selected: usize,
        set: impl Fn(&mut Project, usize) + Copy + 'static,
    ) {
        let label = label.to_string();
        let opts: Vec<String> = options.iter().map(|s| s.to_string()).collect();
        let w = self.width;
        let n = opts.len().max(1);
        let seg_w = (w * 0.62) / n as f32;
        let seg_x0 = w - seg_w * n as f32;
        let opts_draw = opts.clone();
        self.push(
            ROW_H + 2.0,
            move |r, x, y, _w| {
                text(r, &label, x, y + 7.0, 12.0, DIM);
                for (i, o) in opts_draw.iter().enumerate() {
                    let bx = x + seg_x0 + i as f32 * seg_w;
                    let rect = Rect::new(bx, y + 2.0, seg_w - 2.0, 20.0);
                    let sel = i == selected;
                    r.fill_rounded_rect(rect, 4.0, if sel { ACCENT } else { FIELD_BG });
                    text(
                        r,
                        o,
                        bx + 6.0,
                        y + 5.0,
                        11.0,
                        if sel { Color::WHITE } else { INK },
                    );
                }
            },
            move |lx, _ly| {
                if lx < seg_x0 {
                    return None;
                }
                let i = ((lx - seg_x0) / seg_w).floor() as usize;
                if i < n {
                    return Some(FormHit::Mut(Box::new(move |p: &mut Project| set(p, i))));
                }
                None
            },
        );
    }
}
