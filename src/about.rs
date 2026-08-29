//! Modal "About" dialog. A single `Behavior` that watches a shared
//! `open` flag; while set it becomes the tree's modal pointer target and
//! paints a centered card (over a scrim) in the `overlay` pass.

use std::cell::Cell;
use std::rc::Rc;

use rustle_ui::prelude::*;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const AUTHOR: &str = "Dean Van Greunen";
pub const EMAIL: &str = "deanvg9000@gmail.com";
pub const REPO: &str = "https://github.com/DeanVanGreunen/rustle";
pub const BUILD_DATE: &str = env!("BUILD_DATE");
pub const BUILD_RELEASE_TYPE: &str = match option_env!("BUILD_RELEASE_TYPE") {
    Some(v) => v,
    None => "Development",
};
pub const BUILD_ID: &str = env!("BUILD_ID");

const LICENSE: &str = "
The \"Software\" refers to this app \"Rustle\"

Permission is hereby granted, free of charge, to any person obtaining a
copy of this software and associated documentation files (the \"Software\"),
to use, copy, modify, and redistribute the Software, subject to the
following conditions:

1. The Software and any modified or derivative version of the Software may
only be used, distributed, or made available for non-commercial purposes.

2. No person or organization may sell, license, rent, lease, sublicense,
or otherwise commercially exploit the Software or any derivative work
based substantially on the Software.

3. No person or organization may charge a fee for distributing the
Software or a derivative work.

4. Modified versions may be distributed, provided that the modified source
code is made available under these same terms.

5. The original copyright notice and this license must be included in all
copies or substantial portions of the Software.

6. The Software may not be incorporated into a commercial product or
service without explicit written permission from the copyright holder.

Copyright Dean Van Greunen 2026
";

const CARD_W: f32 = 480.0;
const PAD: f32 = 28.0;
const SCRIM: Color = Color::rgba(0.0, 0.0, 0.0, 0.35);
const CARD_BG: Color = Color::WHITE;
const TITLE_C: Color = Color::hex(0x1f1f1f);
const BODY_C: Color = Color::hex(0x444444);
const DIM_C: Color = Color::hex(0x8a8a8a);
const RULE_C: Color = Color::hex(0xE4E4E4);
const CLOSE_C: Color = Color::hex(0x999999);
const CLOSE_HOVER_BG: Color = Color::hex(0xEEEEEE);

/// Handle the rest of the app uses to pop the dialog: `flag.set(true)`.
pub type AboutFlag = Rc<Cell<bool>>;

struct AboutDialog {
    open: AboutFlag,
    abs: (f32, f32),
    surface: (f32, f32),
    card: Rect,
    close_btn: Rect,
    hover_close: bool,
}

fn wrap(r: &Renderer, text: &str, style: &TextStyle, max_w: f32) -> Vec<String> {
    let mut out = Vec::new();
    for raw in text.split('\n') {
        if raw.trim().is_empty() {
            out.push(String::new());
            continue;
        }
        let mut cur = String::new();
        for word in raw.split_whitespace() {
            let trial = if cur.is_empty() {
                word.to_string()
            } else {
                format!("{cur} {word}")
            };
            if cur.is_empty() || r.measure(&trial, style) <= max_w {
                cur = trial;
            } else {
                out.push(std::mem::take(&mut cur));
                cur = word.to_string();
            }
        }
        if !cur.is_empty() {
            out.push(cur);
        }
    }
    out
}

impl Behavior for AboutDialog {
    fn update(&mut self, ctx: &mut UpdateContext) {
        let b = ctx.ui.absolute_box(ctx.node);
        self.abs = (b.x, b.y);
        self.surface = ctx.ui.surface_size();

        let want = self.open.get();
        let is_modal = ctx.ui.modal() == Some(ctx.node);
        if want && !is_modal {
            let n = ctx.node;
            ctx.ui.set_modal(Some(n));
        } else if !want && is_modal {
            ctx.ui.set_modal(None);
        }
    }

    fn pointer_event(&mut self, ctx: &mut EventContext, event: PointerEvent) {
        if !self.open.get() {
            return;
        }
        match event {
            PointerEvent::Move { x, y } => {
                self.hover_close = self.close_btn.contains(x, y);
            }
            PointerEvent::Down { button: MouseButton::Left, x, y } => {
                if self.close_btn.contains(x, y) || !self.card.contains(x, y) {
                    self.open.set(false);
                }
                ctx.stop_propagation();
            }
            PointerEvent::Down { .. } | PointerEvent::Up { .. } | PointerEvent::Wheel { .. } => {
                ctx.stop_propagation();
            }
            _ => {}
        }
    }

    fn overlay(&mut self, ctx: &mut RenderContext) {
        if !self.open.get() {
            return;
        }
        let o = Vec2 { x: -self.abs.0, y: -self.abs.1 };
        let (sw, sh) = self.surface;

        let title_s = TextStyle { size: 20.0, color: TITLE_C, font: FontId::DEFAULT };
        let meta_s = TextStyle { size: 16.0, color: DIM_C, font: FontId::DEFAULT };
        let body_s = TextStyle { size: 12.5, color: BODY_C, font: FontId::DEFAULT };

        let text_w = CARD_W - PAD * 2.0;
        let lines = wrap(ctx.renderer, LICENSE, &body_s, text_w);
        let line_h = 17.0;

        let header_h = 30.0 + 22.0 + 20.0 + 20.0 + 20.0 + 20.0; // title + version + author
        let card_h = PAD + header_h + 14.0 + lines.len() as f32 * line_h + PAD;

        let cx = ((sw - CARD_W) * 0.5).max(0.0);
        let cy = ((sh - card_h) * 0.5).max(0.0);
        self.card = Rect::new(cx, cy, CARD_W, card_h);

        // Scrim + card.
        ctx.renderer.fill_rect(Rect::new(o.x, o.y, sw, sh), SCRIM);
        ctx.renderer
            .fill_rounded_rect(Rect::new(cx + o.x, cy + o.y, CARD_W, card_h), 10.0, CARD_BG);

        let lx = cx + PAD + o.x;
        let mut y = cy + PAD + o.y + 22.0;
        ctx.renderer.text_styled("About Rustle", Vec2 { x: lx, y }, title_s);

        // Close button (top-right).
        let cb = Rect::new(cx + CARD_W - 34.0, cy + 10.0, 24.0, 24.0);
        self.close_btn = cb;
        if self.hover_close {
            ctx.renderer
                .fill_rounded_rect(Rect::new(cb.x + o.x, cb.y + o.y, cb.width, cb.height), 5.0, CLOSE_HOVER_BG);
        }
        let m = 6.5;
        ctx.renderer.text_styled(
            "x",
            Vec2 { x: cb.x + o.x + m, y: cb.y + o.y + cb.height - m },
            TextStyle { size: 15.0, color: CLOSE_C, font: FontId::DEFAULT },
        );

        y += 24.0;
        ctx.renderer
            .text_styled(&format!("Version: {VERSION}"), Vec2 { x: lx, y }, meta_s);
        y += 20.0;
        ctx.renderer
            .text_styled(&format!("Build Date: {BUILD_DATE}"), Vec2 { x: lx, y }, meta_s);
        y += 20.0;
        ctx.renderer
            .text_styled(&format!("Build Type: {BUILD_RELEASE_TYPE}"), Vec2 { x: lx, y }, meta_s);
        y += 20.0;
        ctx.renderer
            .text_styled(&format!("Build ID: {BUILD_ID}"), Vec2 { x: lx, y }, meta_s);
        y += 20.0;
        ctx.renderer.text_styled(
            &format!("Created by: {AUTHOR}"),
            Vec2 { x: lx, y },
            meta_s,
        );
        y += 20.0;
        ctx.renderer.text_styled(
            &format!("Support Email: {EMAIL}"),
            Vec2 { x: lx, y },
            meta_s,
        );
        y += 20.0;
        ctx.renderer.text_styled(
            &format!("Repo: {REPO}"),
            Vec2 { x: lx, y },
            meta_s,
        );
        y += 20.0;
        ctx.renderer.text_styled(
            &format!("LICENSE"),
            Vec2 { x: lx, y },
            meta_s,
        );
        y += 18.0;
        ctx.renderer
            .fill_rect(Rect::new(lx, y, text_w, 1.0), RULE_C);
        y += 16.0;
        for line in &lines {
            if !line.is_empty() {
                ctx.renderer.text_styled(line, Vec2 { x: lx, y }, body_s);
            }
            y += line_h;
        }
    }
}

/// A fresh, closed dialog flag. Share it with the trigger (menu action)
/// and pass it to [`spawn_about_dialog`].
pub fn about_flag() -> AboutFlag {
    Rc::new(Cell::new(false))
}

/// Spawn the (initially hidden) About dialog under `parent`, driven by
/// `flag` — raise it anywhere with `flag.set(true)`.
pub fn spawn_about_dialog(ui: &mut UiTree, parent: NodeId, flag: AboutFlag) {
    ui.spawn(
        parent,
        Style::default(),
        AboutDialog {
            open: flag,
            abs: (0.0, 0.0),
            surface: (0.0, 0.0),
            card: Rect::new(0.0, 0.0, 0.0, 0.0),
            close_btn: Rect::new(0.0, 0.0, 0.0, 0.0),
            hover_close: false,
        },
    )
    .unwrap();
}
