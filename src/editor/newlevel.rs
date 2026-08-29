//! "New Level" modal: a name field + Create/Cancel, raised by the level
//! outline's Add button (`editor.new_level_open`).
//!
//! Built from real nodes (so the `TextField` works). A tiny always-alive
//! controller sibling toggles the overlay's visibility and applies the
//! result — the overlay node itself is `display:none` while closed, so
//! its own `update` would not run.

use rustle_ui::prelude::*;
use rustle_ui::widgets::{Button, Label, Panel, TextField};

use taffy::prelude::length;
use taffy::style::{AlignItems, JustifyContent};

use rustle_core::Selection;

use super::theme::*;
use super::Editor;

pub fn spawn_new_level_dialog(ui: &mut UiTree, root: NodeId, editor: &Editor) {
    // Overlay (absolute, fills window, centres the card).
    let mut ov = Style::column();
    ov.taffy.position = taffy::style::Position::Absolute;
    ov.taffy.inset = taffy::geometry::Rect {
        left: length(0.0),
        right: length(0.0),
        top: length(0.0),
        bottom: length(0.0),
    };
    ov.taffy.justify_content = Some(JustifyContent::CENTER);
    ov.taffy.align_items = Some(AlignItems::CENTER);
    let overlay = ui
        .spawn(root, ov, Panel::new().background(Color::rgba(0.0, 0.0, 0.0, 0.4)))
        .unwrap();
    ui.set_display(overlay, false);

    // Card.
    let mut card = Style::column().gap(14.0);
    card.taffy.size.width = length(360.0);
    card.taffy.flex_shrink = 0.0;
    card.taffy.padding = taffy::geometry::Rect {
        left: length(24.0),
        right: length(24.0),
        top: length(20.0),
        bottom: length(20.0),
    };
    let card = ui
        .spawn(overlay, card, Panel::new().background(Color::WHITE).corner_radius(12.0))
        .unwrap();

    ui.spawn(card, Style::default(), Label::new("New Level").size(18.0).color(INK))
        .unwrap();

    let mut field_style = Style::default();
    field_style.taffy.size.width = taffy::prelude::percent(1.0);
    field_style.taffy.size.height = length(52.0);
    let field = ui
        .spawn(card, field_style, TextField::new("Level name"))
        .unwrap();

    let mut row = Style::row().gap(10.0);
    row.taffy.size.width = taffy::prelude::percent(1.0);
    row.taffy.justify_content = Some(JustifyContent::FLEX_END);
    let row = ui.spawn(card, row, Panel::new()).unwrap();

    let action: std::rc::Rc<std::cell::Cell<u8>> = std::rc::Rc::new(std::cell::Cell::new(0));
    ui.spawn(
        row,
        Style::default().height(34.0),
        Button::new("Cancel").on_click({
            let a = action.clone();
            move || a.set(2)
        }),
    )
    .unwrap();
    ui.spawn(
        row,
        Style::default().height(34.0),
        Button::new("Create").on_click({
            let a = action.clone();
            move || a.set(1)
        }),
    )
    .unwrap();

    // Always-alive controller.
    ui.spawn(
        root,
        Style::default(),
        Controller { editor: editor.clone(), overlay, field, action },
    )
    .unwrap();
}

struct Controller {
    editor: Editor,
    overlay: NodeId,
    field: NodeId,
    action: std::rc::Rc<std::cell::Cell<u8>>,
}

impl Behavior for Controller {
    fn update(&mut self, ctx: &mut UpdateContext) {
        let open = self.editor.new_level_open.get();
        ctx.ui.set_display(self.overlay, open);
        if !open {
            self.action.set(0);
            return;
        }

        match self.action.replace(0) {
            1 => {
                let name = ctx
                    .ui
                    .widget::<TextField>(self.field)
                    .map(|f| f.value.trim().to_string())
                    .unwrap_or_default();
                let name = if name.is_empty() { "New Level".to_string() } else { name };
                let key = self.editor.edit(|p| p.add_level(name));
                if let Some(key) = key {
                    self.editor.edit_session(|s| {
                        s.active.level = Some(key);
                        s.selection = Selection::Level(key);
                    });
                }
                if let Some(f) = ctx.ui.widget_mut::<TextField>(self.field) {
                    f.value.clear();
                    f.cursor = 0;
                }
                self.editor.new_level_open.set(false);
            }
            2 => {
                if let Some(f) = ctx.ui.widget_mut::<TextField>(self.field) {
                    f.value.clear();
                    f.cursor = 0;
                }
                self.editor.new_level_open.set(false);
            }
            _ => {}
        }
    }
}
