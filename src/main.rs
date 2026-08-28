mod about;
mod nav;
mod startup;
mod tools;

use std::cell::RefCell;
use std::rc::Rc;

use macroquad::prelude::*;

use about::{about_flag, spawn_about_dialog};
use nav::{EditorMode, MenuItem, NavState, spawn_nav_bar};
use startup::{Signal, spawn_startup};
use tools::{Tool, spawn_tool_panel};
use rustle_core::{FILE_EXT, Project, RecentProjects};
use rustle_ui::backend::macroquad as mq_backend;
use rustle_ui::prelude::*;
use rustle_ui::widgets::TextField;
use rustle_ui::{Color as UiColor, Style as UiStyle};

/// Live application document. `None` until a project is created / opened
/// from the launch dialog.
type App = Rc<RefCell<Option<Project>>>;

/// Install `project` as the live document: update the nav bar, remember
/// it in the recent list, and dismiss the launch overlay.
fn open_project(
    ui: &mut UiTree,
    startup: &startup::Startup,
    nav_state: &NavState,
    recent: &mut RecentProjects,
    app: &App,
    project: Project,
) {
    recent.record(&project.project_name, &project.file_path);
    nav_state.set_project_name(project.project_name.clone());
    nav_state.set_saved(true);
    *app.borrow_mut() = Some(project);
    ui.set_display(startup.overlay, false);
}

#[macroquad::main("Rustle")]
async fn main() {
    let mut ui = UiTree::new();

    // Root: a column — nav bar on top, editor area filling the rest.
    let root = ui
        .spawn_root(
            UiStyle::column().stretch(),
            Panel::new().background(UiColor::hex(0xFFFFFF)),
        )
        .unwrap();

    let nav_state = NavState::new(EditorMode::Level);
    let about = about_flag();
    let menu_items = vec![
        MenuItem::action("New", Some("Ctrl + N"), || println!("New")),
        MenuItem::action("Open", Some("Ctrl + O"), || println!("Open")),
        MenuItem::action("Open Recent", Some("Shift + Ctrl + O"), || println!("Open Recent")),
        MenuItem::separator(),
        MenuItem::action("Project Properties", Some("Ctrl + R"), || println!("Rename")),
        MenuItem::action("Save", Some("Ctrl + S"), || println!("Save")),
        MenuItem::action("Save As", Some("Shift + Ctrl + S"), || println!("Save As")),
        MenuItem::separator(),
        MenuItem::action("Import", Some("Ctrl + I"), || println!("Import")),
        MenuItem::action("Export", Some("Ctrl + E"), || println!("Export")),
        MenuItem::separator(),
        MenuItem::action("Shortcuts", Some("Ctrl + Alt + S"), || println!("Shortcuts")),
        MenuItem::action("Help", Some("Ctrl + H"), || println!("Help")),
        MenuItem::action("About", Some("Ctrl + Alt + A"), {
            let about = about.clone();
            move || about.set(true)
        }),
    ];
    spawn_nav_bar(&mut ui, root, &nav_state, menu_items);
    nav_state.set_project_name("Untitled Project");
    nav_state.set_version(format!("v{}", env!("CARGO_PKG_VERSION")));

    // Content area below the nav bar: a panel split into 3 equal columns.
    let mut body_style = UiStyle::row().grow().gap(1.0);
    body_style.taffy.min_size.height = taffy::prelude::length(0.0);
    let body = ui
        .spawn(root, body_style, Panel::new().background(UiColor::hex(0xDDDDDD)))
        .unwrap();

    let mut columns = Vec::new();
    for i in 0..3 {
        let mut col = UiStyle::row();
        col.taffy.size.height = taffy::prelude::percent(1.0);
        if i == 0 {
            // First column shrink-wraps to its children (the tool strip).
            col.taffy.flex_grow = 0.0;
            col.taffy.flex_shrink = 0.0;
        } else {
            col.taffy.flex_grow = 1.0;
            col.taffy.flex_basis = taffy::prelude::length(0.0);
            col.taffy.min_size.width = taffy::prelude::length(0.0);
        }
        let shade = if i % 2 == 0 { 0xFFFFFF } else { 0xFAFAFA };
        columns.push(
            ui.spawn(body, col, Panel::new().background(UiColor::hex(shade)))
                .unwrap(),
        );
    }

    // First column: the tool palette.
    let tool_state = spawn_tool_panel(&mut ui, columns[0]);

    // Modal About dialog (hidden until the menu action raises `about`).
    spawn_about_dialog(&mut ui, root, about.clone());

    // Launch dialog + live document state.
    let app: App = Rc::new(RefCell::new(None));
    let mut recent = RecentProjects::load();
    let startup = spawn_startup(&mut ui, root, &recent.entries);

    let fonts = mq_backend::FontBook::new();
    let mut renderer = Renderer::new();
    mq_backend::install(&mut ui, &mut renderer, &fonts);

    loop {
        mq_backend::pump_input(&mut ui);

        let launching = !ui.is_hidden(startup.overlay);

        // Launch-dialog actions (native file dialog runs synchronously).
        match startup.signal.replace(Signal::None) {
            Signal::New => {
                let name = ui
                    .widget::<TextField>(startup.name_field)
                    .map(|f| f.value.trim().to_string())
                    .unwrap_or_default();
                let name = if name.is_empty() {
                    "Untitled Project".to_string()
                } else {
                    name
                };
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Rustle Project", &[FILE_EXT])
                    .set_file_name(format!("{name}.{FILE_EXT}"))
                    .save_file()
                {
                    let path = path.to_string_lossy().into_owned();
                    let project = Project::new(&name, &path);
                    if let Err(e) = project.save() {
                        eprintln!("could not write {path}: {e}");
                    } else {
                        open_project(&mut ui, &startup, &nav_state, &mut recent, &app, project);
                    }
                }
            }
            Signal::OpenRecent(i) => {
                if let Some(entry) = recent.entries.get(i).cloned() {
                    match Project::load(&entry.path) {
                        Ok(project) => {
                            open_project(&mut ui, &startup, &nav_state, &mut recent, &app, project)
                        }
                        Err(e) => eprintln!("could not open {}: {e}", entry.path),
                    }
                }
            }
            Signal::None => {}
        }

        // About: Ctrl+Alt+A opens
        let ctrl = is_key_down(KeyCode::LeftControl) || is_key_down(KeyCode::RightControl);
        if !launching && ctrl && is_key_down(KeyCode::LeftAlt) && is_key_pressed(KeyCode::A) {
            about.set(true);
        }

        // Tool shortcuts (single letters, no modifier held) — not while
        // the launch dialog is up (typing a project name).
        let mods = is_key_down(KeyCode::LeftControl) || is_key_down(KeyCode::LeftSuper);
        while let Some(c) = get_char_pressed() {
            if !launching && !mods {
                if let Some(t) = Tool::from_key(c) {
                    tool_state.set(t);
                }
            }
        }

        ui.tick(get_frame_time());
        ui.layout(screen_width(), screen_height()).unwrap();

        renderer.begin_frame();
        ui.render(&mut renderer);

        clear_background(WHITE);
        mq_backend::draw_commands(&renderer);

        next_frame().await;
    }
}
