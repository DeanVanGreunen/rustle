//! Column 3: the Selected-Properties panel — a live preview on top, then
//! a two-tab strip switching between the colour picker ("Swatches &
//! Preview") and the properties form for the current selection.

use rustle_core::{EditorMode, PropsTab};
use rustle_ui::prelude::*;
use rustle_ui::widgets::{Panel, Viewport};

use super::colorpicker::ColorPicker;
use super::preview::PreviewViewport;
use super::properties::PropertiesForm;
use super::theme::*;
use super::Editor;

const PREVIEW_H: f32 = 188.0;
const BAR_H: f32 = 26.0;
const TABS_H: f32 = 34.0;

fn leaf(height: f32) -> Style {
    let mut s = Style::default();
    s.taffy.size.height = taffy::prelude::length(height);
    s.taffy.size.width = taffy::prelude::percent(1.0);
    s.taffy.flex_shrink = 0.0;
    s
}

fn grow() -> Style {
    let mut s = Style::default();
    s.taffy.flex_grow = 1.0;
    s.taffy.flex_basis = taffy::prelude::length(0.0);
    s.taffy.min_size.height = taffy::prelude::length(0.0);
    s.taffy.size.width = taffy::prelude::percent(1.0);
    s
}

pub fn spawn_selected_props(ui: &mut UiTree, parent: NodeId, editor: &Editor, kind: EditorMode) {
    // Panel background + left border via a wrapping fill.
    ui.spawn(
        parent,
        leaf(PREVIEW_H),
        Viewport::new(PreviewViewport::new(editor.clone(), kind)).focusable(true),
    )
    .unwrap();

    ui.spawn(parent, leaf(BAR_H), PreviewBar { editor: editor.clone() })
        .unwrap();

    // Reserve the tab-strip slot first (so it sits above the content),
    // then fill in the content nodes and hand their ids to the strip.
    let tabs = ui.spawn(parent, leaf(TABS_H), Panel::new()).unwrap();

    let colors = ui
        .spawn(parent, grow(), ColorPicker::new(editor.clone()))
        .unwrap();
    let props = ui
        .spawn(parent, grow(), PropertiesForm::new(editor.clone(), kind))
        .unwrap();

    ui.node_mut(tabs).unwrap().behavior =
        Box::new(PropsTabs { editor: editor.clone(), colors, props });
}

// --- preview bar ---------------------------------------------------

struct PreviewBar {
    editor: Editor,
}

impl Behavior for PreviewBar {
    fn render(&mut self, ctx: &mut RenderContext) {
        let (w, h) = ctx.size();
        let r = &mut *ctx.renderer;
        r.fill_rect(Rect::new(0.0, 0.0, w, h), PANEL_BG);
        r.fill_rect(Rect::new(0.0, h - 1.0, w, 1.0), BORDER);
        r.fill_rect(Rect::new(0.0, 0.0, w, 1.0), BORDER);

        let zoom = self.editor.session(|s| s.preview_view.zoom).unwrap_or(1.0);
        text(r, "Live Preview", 12.0, (h - 11.0) * 0.5, 11.0, INK);
        text(
            r,
            &format!("Zoom {:.0}%", zoom * 100.0),
            92.0,
            (h - 11.0) * 0.5,
            11.0,
            DIM,
        );

        let btn = Rect::new(w - 88.0, 3.0, 78.0, h - 6.0);
        r.fill_rounded_rect(btn, 4.0, FIELD_BG);
        text(r, "Reset View", btn.x + 10.0, btn.y + 4.0, 11.0, INK);
    }

    fn pointer_event(&mut self, ctx: &mut EventContext, event: PointerEvent) {
        if let PointerEvent::Down { button: MouseButton::Left, x, .. } = event {
            let b = ctx.ui.absolute_box(ctx.node);
            if x - b.x > b.width - 90.0 {
                self.editor.edit_session(|s| s.preview_view = Default::default());
                ctx.stop_propagation();
            }
        }
    }
}

// --- tab strip ---------------------------------------------------

struct PropsTabs {
    editor: Editor,
    colors: NodeId,
    props: NodeId,
}

impl PropsTabs {
    fn tab(&self) -> PropsTab {
        self.editor.session(|s| s.props_tab).unwrap_or_default()
    }
}

impl Behavior for PropsTabs {
    fn update(&mut self, ctx: &mut UpdateContext) {
        let show_colors = self.tab() == PropsTab::SwatchesPreview;
        ctx.ui.set_display(self.colors, show_colors);
        ctx.ui.set_display(self.props, !show_colors);
    }

    fn render(&mut self, ctx: &mut RenderContext) {
        let (w, h) = ctx.size();
        let r = &mut *ctx.renderer;
        r.fill_rect(Rect::new(0.0, 0.0, w, h), PANEL_BG);
        r.fill_rect(Rect::new(0.0, h - 1.0, w, 1.0), BORDER);

        let tab = self.tab();
        let half = w / 2.0;
        for (i, (label, kind)) in [
            ("Swatches & Preview", PropsTab::SwatchesPreview),
            ("Properties", PropsTab::Properties),
        ]
        .into_iter()
        .enumerate()
        {
            let x = i as f32 * half;
            let active = tab == kind;
            if active {
                r.fill_rect(Rect::new(x, h - 2.0, half, 2.0), ACCENT);
            }
            let tw = r.measure(label, &TextStyle { size: 11.5, color: INK, font: FontId::DEFAULT });
            text(
                r,
                label,
                x + (half - tw) * 0.5,
                (h - 12.0) * 0.5,
                11.5,
                if active { INK } else { DIM },
            );
        }
    }

    fn pointer_event(&mut self, ctx: &mut EventContext, event: PointerEvent) {
        if let PointerEvent::Down { button: MouseButton::Left, x, .. } = event {
            let b = ctx.ui.absolute_box(ctx.node);
            let tab = if x - b.x < b.width / 2.0 {
                PropsTab::SwatchesPreview
            } else {
                PropsTab::Properties
            };
            self.editor.edit_session(|s| s.props_tab = tab);
            ctx.stop_propagation();
        }
    }
}
