//! Live preview shown at the top of the Selected-Properties panel. Has
//! its own zoom / pan (persisted in `session.preview_view`) and plays the
//! active animation when there is one.

use std::any::Any;

use rustle_core::EditorMode;
use rustle_ui::prelude::*;
use rustle_ui::widgets::ViewportContent;

use super::render::{composite_canvas, composite_frame};
use super::theme::*;
use super::Editor;

pub struct PreviewViewport {
    editor: Editor,
    #[allow(dead_code)]
    kind: EditorMode,
    anim_t: f32,
    drag_from: Option<(f32, f32, f32, f32)>,
}

impl PreviewViewport {
    pub fn new(editor: Editor, kind: EditorMode) -> Self {
        Self { editor, kind, anim_t: 0.0, drag_from: None }
    }

    /// The pixels to show this frame: an animation frame if one is
    /// playing, otherwise the open canvas.
    fn content(&self) -> Option<(u32, u32, Vec<u8>)> {
        self.editor.with_project(|p| {
            if let Some(anim) = p.session.active.animation.and_then(|k| p.animations.get(k)) {
                let frames: Vec<_> = anim.frames.iter().copied().collect();
                if !frames.is_empty() {
                    let total: f32 = frames
                        .iter()
                        .map(|f| p.frames.get(*f).map(|fr| fr.delay_ms.max(1)).unwrap_or(100) as f32)
                        .sum::<f32>()
                        / 1000.0;
                    let mut t = self.anim_t % total.max(0.001);
                    let mut chosen = frames[0];
                    for f in &frames {
                        let d =
                            p.frames.get(*f).map(|fr| fr.delay_ms.max(1)).unwrap_or(100) as f32 / 1000.0;
                        chosen = *f;
                        if t < d {
                            break;
                        }
                        t -= d;
                    }
                    return composite_frame(p, chosen);
                }
            }
            composite_canvas(p)
        })
        .flatten()
    }
}

impl ViewportContent for PreviewViewport {
    fn update(&mut self, dt: f32, _bounds: Rect) {
        self.anim_t += dt;
    }

    fn draw(&mut self, r: &mut Renderer, bounds: Rect) {
        checkerboard(r, bounds, 8.0);
        if !self.editor.has_project() {
            return;
        }
        let Some((cw, ch, buf)) = self.content() else { return };
        let img = ImageData::from_rgba(cw, ch, buf);

        let (z, panx, pany) = self
            .editor
            .session(|s| (s.preview_view.zoom, s.preview_view.pan_x, s.preview_view.pan_y))
            .unwrap_or((1.0, 0.0, 0.0));
        let dw = cw as f32 * z;
        let dh = ch as f32 * z;
        let ox = panx + (bounds.width - dw) * 0.5;
        let oy = pany + (bounds.height - dh) * 0.5;
        r.image(Rect::new(ox, oy, dw, dh), &img);
    }

    fn pointer_event(&mut self, event: PointerEvent, _bounds: Rect) -> bool {
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
