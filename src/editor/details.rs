//! Row 3 of the middle column: a thin status bar showing the cursor
//! position, zoom, and (with the marquee tool) the selection size.

use rustle_core::Tool;
use rustle_ui::prelude::*;

use super::theme::*;
use super::Editor;

pub struct ViewportDetails {
    editor: Editor,
}

impl ViewportDetails {
    pub fn new(editor: Editor) -> Self {
        Self { editor }
    }
}

impl Behavior for ViewportDetails {
    fn render(&mut self, ctx: &mut RenderContext) {
        let (w, h) = ctx.size();
        ctx.renderer.fill_rect(Rect::new(0.0, 0.0, w, h), PANEL_BG_ALT);
        ctx.renderer.fill_rect(Rect::new(0.0, 0.0, w, 1.0), BORDER);

        let (cx, cy) = self.editor.main_cursor.get();
        let zoom = self.editor.session(|s| s.main_view.zoom).unwrap_or(1.0);

        let mut parts = vec![
            self.editor.mode().label().to_string(),
            format!("X {:>4}", cx.floor() as i64),
            format!("Y {:>4}", cy.floor() as i64),
            format!("Zoom {:.0}%", zoom * 100.0),
        ];

        if self.editor.tool() == Tool::Marquee {
            if let Some((_, _, mw, mh)) = self.editor.main_marquee.get() {
                parts.push(format!("W {}  H {}", mw.round() as i64, mh.round() as i64));
            }
        }

        let y = (h - 11.0) * 0.5;
        let mut x = 12.0;
        for (i, p) in parts.iter().enumerate() {
            if i > 0 {
                ctx.renderer
                    .fill_rect(Rect::new(x - 8.0, y - 1.0, 1.0, 13.0), BORDER);
            }
            text(ctx.renderer, p, x, y, 11.0, DIM);
            x += ctx
                .renderer
                .measure(p, &TextStyle { size: 11.0, color: DIM, font: FontId::DEFAULT })
                + 18.0;
        }
    }
}
