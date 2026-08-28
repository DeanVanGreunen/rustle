//! The launch dialog: shown over everything until a project is opened.
//! Title + version, a "new project" row (name field + button), and a
//! scrollable list of recent projects.
//!
//! It is built from real UI nodes (so the text field / buttons work
//! normally) parented as an absolutely-positioned, full-window child of
//! the root. Button clicks only set a [`Signal`]; `main` performs the
//! file-dialog / load work and hides the overlay.

use std::cell::Cell;
use std::rc::Rc;

use rustle_ui::prelude::*;
use rustle_ui::widgets::{Button, Label, Panel, TextField};
use rustle_core::RecentEntry;

use taffy::prelude::{length, percent};
use taffy::style::{AlignItems, JustifyContent};

/// What the user asked for on the launch dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Signal {
    None,
    /// Create a new project (name comes from the text field).
    New,
    /// Open recent entry `#i`.
    OpenRecent(usize),
}

pub struct Startup {
    /// The full-window overlay node — hide it with `ui.set_display(.., false)`.
    pub overlay: NodeId,
    /// The project-name text field.
    pub name_field: NodeId,
    pub signal: Rc<Cell<Signal>>,
}

const SCRIM: Color = Color::rgba(0.0, 0.0, 0.0, 0.42);
const CARD_BG: Color = Color::WHITE;
const TITLE_C: Color = Color::hex(0x1e1e1e);
const DIM_C: Color = Color::hex(0x8a8a8a);
const SUBHEAD_C: Color = Color::hex(0x666666);

fn col(gap: f32) -> Style {
    Style::column().gap(gap)
}

fn full_width(mut s: Style) -> Style {
    s.taffy.size.width = percent(1.0);
    s
}

pub fn spawn_startup(ui: &mut UiTree, root: NodeId, recent: &[RecentEntry]) -> Startup {
    let signal = Rc::new(Cell::new(Signal::None));

    // --- overlay (absolute, fills the window, centers the card) --------
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
        .spawn(root, ov, Panel::new().background(SCRIM))
        .unwrap();

    // --- card --------------------------------------------------------
    let mut card_style = col(16.0);
    card_style.taffy.size.width = length(468.0);
    card_style.taffy.flex_shrink = 0.0;
    card_style.taffy.padding = taffy::geometry::Rect {
        left: length(28.0),
        right: length(28.0),
        top: length(24.0),
        bottom: length(26.0),
    };
    let card = ui
        .spawn(
            overlay,
            card_style,
            Panel::new().background(CARD_BG).corner_radius(12.0),
        )
        .unwrap();

    // Header: "Rustle" .......... "Version vX.Y.Z"
    let mut header = full_width(Style::row());
    header.taffy.justify_content = Some(JustifyContent::SPACE_BETWEEN);
    header.taffy.align_items = Some(AlignItems::CENTER);
    let header = ui.spawn(card, header, Panel::new()).unwrap();
    ui.spawn(header, Style::default(), Label::new("Rustle").size(22.0).color(TITLE_C))
        .unwrap();
    ui.spawn(
        header,
        Style::default(),
        Label::new(format!("Version v{}", env!("CARGO_PKG_VERSION")))
            .size(13.0)
            .color(DIM_C),
    )
    .unwrap();

    // Row: [ project name field ] [ New ]
    let mut row = full_width(Style::row().gap(10.0));
    row.taffy.align_items = Some(AlignItems::CENTER);
    let row = ui.spawn(card, row, Panel::new()).unwrap();

    let mut field_style = Style::default();
    field_style.taffy.flex_grow = 1.0;
    field_style.taffy.flex_basis = length(0.0);
    field_style.taffy.min_size.width = length(0.0);
    field_style.taffy.size.height = length(52.0);
    let name_field = ui
        .spawn(row, field_style, TextField::new("Project Name"))
        .unwrap();

    ui.spawn(
        row,
        Style::default().height(40.0),
        Button::new("New").on_click({
            let signal = signal.clone();
            move || signal.set(Signal::New)
        }),
    )
    .unwrap();

    // Sub-header
    ui.spawn(card, Style::default(), Label::new("Recent").size(13.0).color(SUBHEAD_C))
        .unwrap();

    // Recent list (scrolls if long)
    let mut list = full_width(col(6.0));
    list.taffy.max_size.height = length(260.0);
    list = list.scroll_y();
    let list = ui.spawn(card, list, Panel::new()).unwrap();

    if recent.is_empty() {
        ui.spawn(
            list,
            Style::default().height(28.0),
            Label::new("No recent projects").size(13.0).color(DIM_C),
        )
        .unwrap();
    } else {
        for (i, entry) in recent.iter().enumerate() {
            let label = if entry.path.is_empty() {
                entry.name.clone()
            } else {
                format!("{}   —   {}", entry.name, entry.path)
            };
            let mut btn = Button::new(label).on_click({
                let signal = signal.clone();
                move || signal.set(Signal::OpenRecent(i))
            });
            btn.background = Color::hex(0xf4f4f4);
            ui.spawn(list, full_width(Style::default().height(34.0)), btn)
                .unwrap();
        }
    }

    Startup {
        overlay,
        name_field,
        signal,
    }
}
