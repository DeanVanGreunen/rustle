//! Animation-workspace timeline: play/loop controls and a strip of frame
//! cells for the active animation (jump / reorder / add / remove).

use std::cell::Cell;

use rustle_core::{FrameId, Selection};
use rustle_ui::prelude::*;

use super::theme::*;
use super::Editor;

const CTRL_W: f32 = 64.0;
const CELL_W: f32 = 46.0;
const PAD: f32 = 8.0;

pub struct AnimationTimeline {
    editor: Editor,
    playing: Cell<bool>,
    looping: Cell<bool>,
    t: Cell<f32>,
    drag_from: Cell<Option<usize>>,
    scroll: Cell<f32>,
}

impl AnimationTimeline {
    pub fn new(editor: Editor) -> Self {
        Self {
            editor,
            playing: Cell::new(false),
            looping: Cell::new(true),
            t: Cell::new(0.0),
            drag_from: Cell::new(None),
            scroll: Cell::new(0.0),
        }
    }

    fn frames(&self) -> Vec<(FrameId, u64)> {
        self.editor
            .with_project(|p| {
                let anim = p.session.active.animation.and_then(|k| p.animations.get(k))?;
                Some(
                    anim.frames
                        .iter()
                        .map(|f| (*f, p.frames.get(*f).map(|fr| fr.delay_ms.max(1)).unwrap_or(100)))
                        .collect(),
                )
            })
            .flatten()
            .unwrap_or_default()
    }

    fn add_frame(&self) {
        self.editor.edit(|p| {
            let f = p.add_frame_seeded();
            if let Some(ak) = p.session.active.animation {
                if let Some(a) = p.animations.get_mut(ak) {
                    a.frames.push_back(f);
                }
            }
            p.session.active.frame = Some(f);
            p.session.selection = Selection::Frame(f);
        });
    }

    fn remove_active(&self) {
        self.editor.edit(|p| {
            if let (Some(ak), Some(fk)) = (p.session.active.animation, p.session.active.frame) {
                if let Some(a) = p.animations.get_mut(ak) {
                    a.frames = a.frames.iter().copied().filter(|&x| x != fk).collect();
                }
            }
        });
    }

    fn swap(&self, i: usize, j: usize) {
        self.editor.edit(|p| {
            if let Some(ak) = p.session.active.animation {
                if let Some(a) = p.animations.get_mut(ak) {
                    let mut v: Vec<_> = a.frames.iter().copied().collect();
                    if i < v.len() && j < v.len() {
                        v.swap(i, j);
                        a.frames = v.into_iter().collect();
                    }
                }
            }
        });
    }

    fn jump(&self, f: FrameId) {
        self.playing.set(false);
        self.editor.edit(|p| {
            p.session.active.frame = Some(f);
            p.session.selection = Selection::Frame(f);
        });
    }

    fn cell_index_at(&self, lx: f32) -> Option<usize> {
        let x = lx - CTRL_W - PAD + self.scroll.get();
        if x < 0.0 {
            return None;
        }
        Some((x / CELL_W) as usize)
    }
}

impl Behavior for AnimationTimeline {
    fn update(&mut self, ctx: &mut UpdateContext) {
        if !self.playing.get() {
            return;
        }
        let frames = self.frames();
        if frames.is_empty() {
            self.playing.set(false);
            return;
        }
        let total: f32 = frames.iter().map(|(_, d)| *d as f32).sum::<f32>() / 1000.0;
        let mut t = self.t.get() + ctx.dt;
        if t >= total {
            if self.looping.get() {
                t %= total.max(0.001);
            } else {
                t = total;
                self.playing.set(false);
            }
        }
        self.t.set(t);
        let mut acc = 0.0;
        let mut cur = frames[0].0;
        for (f, d) in &frames {
            acc += *d as f32 / 1000.0;
            cur = *f;
            if t < acc {
                break;
            }
        }
        self.editor.set_playback_frame(cur);
    }

    fn render(&mut self, ctx: &mut RenderContext) {
        let (w, h) = ctx.size();
        let r = &mut *ctx.renderer;
        r.fill_rect(Rect::new(0.0, 0.0, w, h), PANEL_BG);
        r.fill_rect(Rect::new(0.0, 0.0, w, 1.0), BORDER);

        if !self.editor.has_project() {
            return;
        }

        // Controls.
        let mid = h * 0.5;
        let play = Rect::new(PAD, mid - 11.0, 22.0, 22.0);
        r.fill_rounded_rect(play, 4.0, FIELD_BG);
        if self.playing.get() {
            r.fill_rect(Rect::new(play.x + 6.0, play.y + 5.0, 4.0, 12.0), INK);
            r.fill_rect(Rect::new(play.x + 12.0, play.y + 5.0, 4.0, 12.0), INK);
        } else {
            for i in 0..6 {
                r.fill_rect(
                    Rect::new(play.x + 7.0 + i as f32, play.y + 4.0 + i as f32, 1.0, (12 - 2 * i).max(1) as f32),
                    INK,
                );
            }
        }
        let loopb = Rect::new(PAD + 28.0, mid - 11.0, 22.0, 22.0);
        r.fill_rounded_rect(loopb, 4.0, if self.looping.get() { ACCENT } else { FIELD_BG });
        text(r, "L", loopb.x + 7.0, loopb.y + 4.0, 12.0, if self.looping.get() { Color::WHITE } else { INK });

        // Frame cells.
        let frames = self.frames();
        let active = self.editor.session(|s| s.active.frame).flatten();
        r.push_clip(Rect::new(CTRL_W, 0.0, w - CTRL_W, h));
        let mut x = CTRL_W + PAD - self.scroll.get();
        for (i, (f, delay)) in frames.iter().enumerate() {
            let cell = Rect::new(x + 2.0, 6.0, CELL_W - 6.0, h - 14.0);
            let sel = active == Some(*f);
            r.fill_rounded_rect(cell, 4.0, if sel { ACCENT_BG } else { PANEL_BG_ALT });
            stroke(r, cell, if sel { ACCENT } else { BORDER });
            text(r, &format!("{}", i + 1), cell.x + 5.0, cell.y + 4.0, 11.0, if sel { ACCENT } else { INK });
            text(r, &format!("{delay}ms"), cell.x + 4.0, cell.y + cell.height - 14.0, 9.0, DIM);
            x += CELL_W;
        }
        // Add button.
        let add = Rect::new(x + 2.0, 6.0, 26.0, h - 14.0);
        r.fill_rounded_rect(add, 4.0, FIELD_BG);
        text(r, "+", add.x + 9.0, add.y + (add.height - 13.0) * 0.5, 14.0, ACCENT);
        r.pop_clip();
    }

    fn pointer_event(&mut self, ctx: &mut EventContext, event: PointerEvent) {
        let b = ctx.ui.absolute_box(ctx.node);
        let frames = self.frames();
        match event {
            PointerEvent::Wheel { delta, .. } => {
                let content = frames.len() as f32 * CELL_W + 40.0;
                let max = (content - (b.width - CTRL_W - PAD)).max(0.0);
                self.scroll.set((self.scroll.get() - delta * 40.0).clamp(0.0, max));
                ctx.stop_propagation();
            }
            PointerEvent::Down { button: MouseButton::Left, x, y } => {
                let (lx, _ly) = (x - b.x, y - b.y);
                let mid = b.height * 0.5;
                let _ = mid;
                if lx < 24.0 + PAD {
                    self.playing.set(!self.playing.get());
                    if self.playing.get() {
                        self.t.set(0.0);
                    }
                } else if lx < PAD + 50.0 {
                    self.looping.set(!self.looping.get());
                } else if let Some(i) = self.cell_index_at(lx) {
                    if i < frames.len() {
                        self.jump(frames[i].0);
                        self.drag_from.set(Some(i));
                    } else {
                        self.add_frame();
                    }
                }
                ctx.stop_propagation();
            }
            PointerEvent::Move { x, .. } => {
                if let Some(from) = self.drag_from.get() {
                    if let Some(to) = self.cell_index_at(x - b.x) {
                        if to < frames.len() && to != from {
                            self.swap(from, to);
                            self.drag_from.set(Some(to));
                        }
                    }
                }
            }
            PointerEvent::Up { .. } => self.drag_from.set(None),
            _ => {}
        }
    }

    fn keyboard_event(&mut self, ctx: &mut EventContext, event: KeyboardEvent) {
        if let KeyboardEvent::KeyDown { key: Key::Delete, .. } = event {
            self.remove_active();
            ctx.stop_propagation();
        }
    }
}

fn stroke(r: &mut Renderer, rect: Rect, c: Color) {
    r.fill_rect(Rect::new(rect.x, rect.y, rect.width, 1.0), c);
    r.fill_rect(Rect::new(rect.x, rect.y + rect.height - 1.0, rect.width, 1.0), c);
    r.fill_rect(Rect::new(rect.x, rect.y, 1.0, rect.height), c);
    r.fill_rect(Rect::new(rect.x + rect.width - 1.0, rect.y, 1.0, rect.height), c);
}
