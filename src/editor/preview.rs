//! Live preview shown at the top of the Selected-Properties panel. Its
//! own zoom / pan (persisted in `session.preview_view`); animates through
//! the active animation's frames in Animation mode.

use std::any::Any;

use rustle_core::{EditorMode, FrameId};
use rustle_ui::prelude::*;
use rustle_ui::widgets::ViewportContent;

use super::render::composite_frame;
use super::theme::*;
use super::Editor;

pub struct PreviewViewport {
    editor: Editor,
    kind: EditorMode,
    cache: Option<(u64, FrameId, ImageData, (u32, u32))>,
    anim_t: f32,
    drag_from: Option<(f32, f32, f32, f32)>,
}

impl PreviewViewport {
    pub fn new(editor: Editor, kind: EditorMode) -> Self {
        Self { editor, kind, cache: None, anim_t: 0.0, drag_from: None }
    }

    fn current_frame(&self) -> Option<FrameId> {
        self.editor.with_project(|p| {
            if self.kind == EditorMode::Animation {
                if let Some(anim) = p.session.active.animation.and_then(|k| p.animations.get(k)) {
                    let frames: Vec<FrameId> = anim.frames.iter().copied().collect();
                    if !frames.is_empty() {
                        let total: f32 = frames
                            .iter()
                            .map(|f| p.frames.get(*f).map(|fr| fr.delay_ms.max(1)).unwrap_or(100) as f32)
                            .sum::<f32>()
                            / 1000.0;
                        let mut t = self.anim_t % total.max(0.001);
                        for f in &frames {
                            let d = p.frames.get(*f).map(|fr| fr.delay_ms.max(1)).unwrap_or(100) as f32
                                / 1000.0;
                            if t < d {
                                return Some(*f);
                            }
                            t -= d;
                        }
                        return frames.first().copied();
                    }
                }
            }
            p.session.active.frame
        }).flatten()
    }
}

impl ViewportContent for PreviewViewport {
    fn update(&mut self, dt: f32, _bounds: Rect) {
        if self.kind == EditorMode::Animation {
            self.anim_t += dt;
        }
    }

    fn draw(&mut self, r: &mut Renderer, bounds: Rect) {
        checkerboard(r, bounds, 8.0);
        if !self.editor.has_project() {
            return;
        }
        let Some(frame) = self.current_frame() else { return };
        let rev = self.editor.revision.get();

        let stale = !matches!(&self.cache, Some((cr, cf, ..)) if *cr == rev && *cf == frame);
        if stale {
            if let Some(Some((w, h, buf))) =
                self.editor.with_project(|p| composite_frame(p, frame))
            {
                self.cache = Some((rev, frame, ImageData::from_rgba(w, h, buf), (w, h)));
            } else {
                self.cache = None;
            }
        }

        let Some((_, _, img, (cw, ch))) = &self.cache else { return };
        let (z, panx, pany) = self
            .editor
            .session(|s| (s.preview_view.zoom, s.preview_view.pan_x, s.preview_view.pan_y))
            .unwrap_or((1.0, 0.0, 0.0));

        // Centre the canvas when pan is zero.
        let dw = *cw as f32 * z;
        let dh = *ch as f32 * z;
        let ox = panx + (bounds.width - dw) * 0.5;
        let oy = pany + (bounds.height - dh) * 0.5;
        r.image(Rect::new(ox, oy, dw, dh), img);
    }

    fn pointer_event(&mut self, event: PointerEvent, bounds: Rect) -> bool {
        match event {
            PointerEvent::Wheel { delta, .. } => {
                self.editor.edit_session(|s| {
                    let f = if delta > 0.0 { 1.1 } else { 1.0 / 1.1 };
                    s.preview_view.zoom = (s.preview_view.zoom * f).clamp(0.1, 32.0);
                });
                true
            }
            PointerEvent::Down { button: MouseButton::Left, x, y } => {
                let (_, px, py) = self
                    .editor
                    .session(|s| (s.preview_view.zoom, s.preview_view.pan_x, s.preview_view.pan_y))
                    .unwrap_or((1.0, 0.0, 0.0));
                self.drag_from = Some((x, y, px, py));
                true
            }
            PointerEvent::Move { x, y } => {
                if let Some((sx, sy, px, py)) = self.drag_from {
                    self.editor.edit_session(|s| {
                        s.preview_view.pan_x = px + (x - sx);
                        s.preview_view.pan_y = py + (y - sy);
                    });
                    let _ = bounds;
                    true
                } else {
                    false
                }
            }
            PointerEvent::Up { .. } => {
                self.drag_from = None;
                true
            }
            _ => false,
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
