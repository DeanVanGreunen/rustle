//! Shared numeric-input behaviour for the form helpers: `[<] [value] [>]`
//! where the value can be typed or click-dragged (scrubbed).

use rustle_core::Project;
use rustle_ui::prelude::*;

use super::theme::*;
use super::Editor;

pub type SetFn = std::rc::Rc<dyn Fn(&mut Project, i64)>;

/// Stable per-field key from its label.
pub fn fkey(s: &str) -> u64 {
    let mut h = 0xcbf29ce484222325u64;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// What a hit on a number control did.
pub enum NumberHit {
    /// An arrow was clicked — apply this new value.
    Step(SetFn, i64),
    /// The value body was pressed — the host starts a scrub / edit.
    Body {
        key: u64,
        value: i64,
        min: i64,
        max: i64,
        step: i64,
        set: SetFn,
    },
}

struct Scrub {
    key: u64,
    start_v: i64,
    start_x: f32,
    min: i64,
    max: i64,
    step: i64,
    set: SetFn,
    moved: bool,
}

struct Editing {
    key: u64,
    buf: String,
    min: i64,
    max: i64,
    set: SetFn,
}

/// One of these per widget that hosts numeric form fields.
#[derive(Default)]
pub struct NumState {
    scrub: Option<Scrub>,
    editing: Option<Editing>,
}

const SCRUB_PX: f32 = 4.0;

impl NumState {
    #[allow(dead_code)]
    pub fn active(&self) -> bool {
        self.scrub.is_some() || self.editing.is_some()
    }

    pub fn editing_key(&self) -> Option<u64> {
        self.editing.as_ref().map(|e| e.key)
    }

    /// Buffer to display for `key`, if it is being edited.
    pub fn buffer_for(&self, key: u64) -> Option<&str> {
        self.editing.as_ref().filter(|e| e.key == key).map(|e| e.buf.as_str())
    }

    #[allow(dead_code)]
    pub fn clear(&mut self) {
        self.scrub = None;
        self.editing = None;
    }

    /// Begin interacting with the value body pressed at `mouse_x`.
    pub fn begin(&mut self, hit: NumberHit, mouse_x: f32, editor: &Editor) {
        match hit {
            NumberHit::Step(set, nv) => {
                editor.edit(|p| set(p, nv));
            }
            NumberHit::Body { key, value, min, max, step, set } => {
                self.editing = None;
                self.scrub = Some(Scrub {
                    key,
                    start_v: value,
                    start_x: mouse_x,
                    min,
                    max,
                    step,
                    set,
                    moved: false,
                });
            }
        }
    }

    /// Pointer move — continues a scrub. Returns `true` if consumed.
    pub fn on_move(&mut self, mouse_x: f32, editor: &Editor) -> bool {
        let Some(s) = self.scrub.as_mut() else { return false };
        let dx = mouse_x - s.start_x;
        if dx.abs() > 3.0 {
            s.moved = true;
        }
        let delta = (dx / SCRUB_PX).round() as i64 * s.step;
        let nv = (s.start_v + delta).clamp(s.min, s.max);
        let set = s.set.clone();
        editor.edit(move |p| set(p, nv));
        true
    }

    /// Pointer up — a scrub with no movement becomes a text edit.
    pub fn on_up(&mut self) {
        if let Some(s) = self.scrub.take() {
            if !s.moved {
                self.editing = Some(Editing {
                    key: s.key,
                    buf: s.start_v.to_string(),
                    min: s.min,
                    max: s.max,
                    set: s.set,
                });
            }
        }
    }

    /// Keyboard while a value is being typed. Returns `true` if consumed.
    pub fn on_key(&mut self, event: &KeyboardEvent, editor: &Editor) -> bool {
        let Some(e) = self.editing.as_mut() else { return false };
        match event {
            KeyboardEvent::TextInput(c) if c.is_ascii_digit() || (*c == '-' && e.buf.is_empty()) => {
                if e.buf.len() < 9 {
                    e.buf.push(*c);
                }
                true
            }
            KeyboardEvent::KeyDown { key: Key::Backspace, .. } => {
                e.buf.pop();
                true
            }
            KeyboardEvent::KeyDown { key: Key::Enter, .. } => {
                self.commit(editor);
                true
            }
            KeyboardEvent::KeyDown { key: Key::Escape, .. } => {
                self.editing = None;
                true
            }
            _ => false,
        }
    }

    /// Commit the typed value (call on click-away too).
    pub fn commit(&mut self, editor: &Editor) {
        if let Some(e) = self.editing.take() {
            if let Ok(v) = e.buf.parse::<i64>() {
                let v = v.clamp(e.min, e.max);
                let set = e.set;
                editor.edit(move |p| set(p, v));
            }
        }
    }
}

/// Draw a `[<] [value/unit] [>]` control inside `rect`. `shown` overrides
/// the value text when the field is being edited (buffer + caret).
pub fn draw_number(r: &mut Renderer, rect: Rect, value_text: &str, editing: Option<&str>) -> (Rect, Rect, Rect) {
    let btn = 18.0f32.min(rect.width * 0.28);
    let minus = Rect::new(rect.x, rect.y, btn, rect.height);
    let plus = Rect::new(rect.x + rect.width - btn, rect.y, btn, rect.height);
    let body = Rect::new(minus.x + btn, rect.y, rect.width - btn * 2.0, rect.height);

    r.fill_rounded_rect(minus, 4.0, FIELD_BG);
    r.fill_rect(body, PANEL_BG_ALT);
    r.fill_rounded_rect(plus, 4.0, FIELD_BG);
    // body border
    r.fill_rect(Rect::new(body.x, body.y, body.width, 1.0), BORDER);
    r.fill_rect(Rect::new(body.x, body.y + body.height - 1.0, body.width, 1.0), BORDER);

    text(r, "\u{2039}", minus.x + btn * 0.5 - 3.0, rect.y + rect.height * 0.5 - 7.0, 13.0, INK);
    text(r, "\u{203a}", plus.x + btn * 0.5 - 3.0, rect.y + rect.height * 0.5 - 7.0, 13.0, INK);

    let ty = rect.y + rect.height * 0.5 - 6.0;
    match editing {
        Some(buf) => {
            text(r, buf, body.x + 5.0, ty, 11.5, INK);
            let cw = r.measure(buf, &TextStyle { size: 11.5, color: INK, font: FontId::DEFAULT });
            r.fill_rect(Rect::new(body.x + 5.0 + cw + 1.0, body.y + 3.0, 1.0, body.height - 6.0), ACCENT);
        }
        None => {
            let tw = r.measure(value_text, &TextStyle { size: 11.5, color: INK, font: FontId::DEFAULT });
            text(r, value_text, body.x + (body.width - tw) * 0.5, ty, 11.5, INK);
        }
    }
    (minus, body, plus)
}
