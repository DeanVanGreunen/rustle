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

use super::theme::*;

pub type Mut = Box<dyn FnOnce(&mut Project)>;

const ROW_H: f32 = 26.0;
const HEAD_H: f32 = 28.0;
const GAP: f32 = 4.0;

struct Row {
    y: f32,
    h: f32,
    draw: Box<dyn Fn(&mut Renderer, f32, f32, f32)>,
    hit: Box<dyn Fn(f32, f32) -> Option<Mut>>,
}

pub struct Form {
    x: f32,
    width: f32,
    cursor: f32,
    rows: Vec<Row>,
}

impl Form {
    pub fn new(x: f32, y: f32, width: f32) -> Self {
        Self { x, width, cursor: y, rows: Vec::new() }
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
    pub fn hit(&self, lx: f32, ly: f32) -> Option<Mut> {
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
        hit: impl Fn(f32, f32) -> Option<Mut> + 'static,
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

    pub fn stepper(
        &mut self,
        label: &str,
        value: i64,
        min: i64,
        max: i64,
        step: i64,
        set: impl Fn(&mut Project, i64) + Copy + 'static,
    ) {
        let label = label.to_string();
        let value = value.clamp(min, max);
        let btn = 20.0;
        // Geometry, in widget-local x (0 = form's x): [ - ] value [ + ]
        let minus_x = self.width - btn * 2.0 - 44.0;
        let plus_x = self.width - btn;
        self.push(
            ROW_H,
            move |r, x, y, _w| {
                text(r, &label, x, y + 6.0, 12.0, DIM);
                let minus = Rect::new(x + minus_x, y + 3.0, btn, 20.0);
                let plus = Rect::new(x + plus_x, y + 3.0, btn, 20.0);
                r.fill_rounded_rect(minus, 4.0, FIELD_BG);
                r.fill_rounded_rect(plus, 4.0, FIELD_BG);
                text(r, "-", minus.x + 7.0, y + 5.0, 13.0, INK);
                text(r, "+", plus.x + 6.0, y + 5.0, 13.0, INK);
                text_right(r, &value.to_string(), plus.x - 6.0, y + 6.0, 12.0, INK);
            },
            move |lx, _ly| {
                if lx >= minus_x && lx <= minus_x + btn {
                    let nv = (value - step).clamp(min, max);
                    return Some(Box::new(move |p: &mut Project| set(p, nv)));
                }
                if lx >= plus_x && lx <= plus_x + btn {
                    let nv = (value + step).clamp(min, max);
                    return Some(Box::new(move |p: &mut Project| set(p, nv)));
                }
                None
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
            move |_lx, _ly| Some(Box::new(move |p: &mut Project| set(p, !on))),
        );
    }

    #[allow(dead_code)]
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
                    return Some(Box::new(move |p: &mut Project| set(p, i)));
                }
                None
            },
        );
    }
}
