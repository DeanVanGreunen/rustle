//! "Project Properties" modal: rename the project and view file info.

use rustle_ui::prelude::*;
use rustle_ui::widgets::{Button, Label, Panel, TextField};

use taffy::prelude::{length, percent};
use taffy::style::{AlignItems, JustifyContent};

use super::theme::*;
use super::Editor;

pub fn spawn_project_props_dialog(ui: &mut UiTree, root: NodeId, editor: &Editor) {
    let mut ov = Style::column();
    ov.taffy.position = taffy::style::Position::Absolute;
    ov.taffy.inset = taffy::geometry::Rect { left: length(0.0), right: length(0.0), top: length(0.0), bottom: length(0.0) };
    ov.taffy.justify_content = Some(JustifyContent::CENTER);
    ov.taffy.align_items = Some(AlignItems::CENTER);
    let overlay = ui
        .spawn(root, ov, Panel::new().background(Color::rgba(0.0, 0.0, 0.0, 0.4)))
        .unwrap();
    ui.set_display(overlay, false);

    let mut card = Style::column().gap(12.0);
    card.taffy.size.width = length(400.0);
    card.taffy.flex_shrink = 0.0;
    card.taffy.padding = taffy::geometry::Rect { left: length(24.0), right: length(24.0), top: length(20.0), bottom: length(20.0) };
    let card = ui
        .spawn(overlay, card, Panel::new().background(Color::WHITE).corner_radius(12.0))
        .unwrap();

    ui.spawn(card, Style::default(), Label::new("Project Properties").size(18.0).color(INK))
        .unwrap();

    let mut fs = Style::default();
    fs.taffy.size.width = percent(1.0);
    fs.taffy.size.height = length(52.0);
    let field = ui.spawn(card, fs, TextField::new("Project name")).unwrap();

    let info = ui.spawn(card, Style::column().gap(4.0), Panel::new()).unwrap();
    let path_lbl = ui.spawn(info, Style::default(), Label::new("").size(11.0).color(DIM)).unwrap();
    let stat_lbl = ui.spawn(info, Style::default(), Label::new("").size(11.0).color(DIM)).unwrap();

    let mut row = Style::row().gap(10.0);
    row.taffy.size.width = percent(1.0);
    row.taffy.justify_content = Some(JustifyContent::FLEX_END);
    let row = ui.spawn(card, row, Panel::new()).unwrap();

    let action: std::rc::Rc<std::cell::Cell<u8>> = std::rc::Rc::new(std::cell::Cell::new(0));
    ui.spawn(row, Style::default().height(34.0), Button::new("Cancel").on_click({
        let a = action.clone();
        move || a.set(2)
    })).unwrap();
    ui.spawn(row, Style::default().height(34.0), Button::new("Save").on_click({
        let a = action.clone();
        move || a.set(1)
    })).unwrap();

    ui.spawn(root, Style::default(), Controller {
        editor: editor.clone(),
        overlay,
        field,
        path_lbl,
        stat_lbl,
        action,
        primed: false,
    })
    .unwrap();
}

struct Controller {
    editor: Editor,
    overlay: NodeId,
    field: NodeId,
    path_lbl: NodeId,
    stat_lbl: NodeId,
    action: std::rc::Rc<std::cell::Cell<u8>>,
    primed: bool,
}

impl Behavior for Controller {
    fn update(&mut self, ctx: &mut UpdateContext) {
        let open = self.editor.project_props_open.get();
        ctx.ui.set_display(self.overlay, open);
        if !open {
            self.primed = false;
            self.action.set(0);
            return;
        }

        // On first frame open: prefill the field and info labels.
        if !self.primed {
            self.primed = true;
            let (name, path, counts) = self
                .editor
                .with_project(|p| {
                    (
                        p.project_name.clone(),
                        p.file_path.clone(),
                        format!(
                            "v{} · {} frames · {} layers · {} tiles · {} levels",
                            p.file_version,
                            p.frames.len(),
                            p.layers.len(),
                            p.tiles.len(),
                            p.levels.len()
                        ),
                    )
                })
                .unwrap_or_default();
            if let Some(f) = ctx.ui.widget_mut::<TextField>(self.field) {
                f.value = name;
                f.cursor = f.value.chars().count();
            }
            if let Some(l) = ctx.ui.widget_mut::<rustle_ui::widgets::Label>(self.path_lbl) {
                l.text = path;
            }
            if let Some(l) = ctx.ui.widget_mut::<rustle_ui::widgets::Label>(self.stat_lbl) {
                l.text = counts;
            }
        }

        match self.action.replace(0) {
            1 => {
                let name = ctx
                    .ui
                    .widget::<TextField>(self.field)
                    .map(|f| f.value.trim().to_string())
                    .unwrap_or_default();
                if !name.is_empty() {
                    self.editor.edit(|p| p.project_name = name);
                }
                self.editor.project_props_open.set(false);
            }
            2 => self.editor.project_props_open.set(false),
            _ => {}
        }
    }
}
