mod about;
mod editor;
mod icon;
mod nav;
mod startup;
mod tools;

use std::cell::RefCell;
use std::rc::Rc;

use macroquad::prelude::*;

use about::{about_flag, spawn_about_dialog};
use editor::{App, Editor, spawn_dialogs, spawn_workspaces};
use nav::{EditorMode, MenuItem, NavState, spawn_nav_bar};
use startup::{Signal, spawn_startup};
use tools::{spawn_tool_panel, tool_from_keycode};
use rustle_core::{FILE_EXT, Project, RecentProjects};
use rustle_ui::backend::macroquad as mq_backend;
use rustle_ui::prelude::*;
use rustle_ui::widgets::TextField;
use rustle_ui::{Color as UiColor, Style as UiStyle};

use crate::about::{BUILD_ID, BUILD_RELEASE_TYPE, VERSION};

/// Install `project` as the live document: sync the nav bar / workspace,
/// remember it in the recent list, and dismiss the launch overlay.
fn open_project(
    ui: &mut UiTree,
    startup: &startup::Startup,
    nav_state: &NavState,
    editor: &Editor,
    recent: &mut RecentProjects,
    app: &App,
    project: Project,
) {
    recent.record(&project.project_name, &project.file_path);
    nav_state.set_project_name(project.project_name.clone());
    nav_state.mode.set(project.session.mode);
    nav_state.set_saved(true);
    *app.borrow_mut() = Some(project);
    editor.clear_history();
    editor.revision.set(editor.revision.get().wrapping_add(1));
    editor.dirty.set(false);
    ui.set_display(startup.overlay, false);
}

fn window_conf() -> Conf {
    Conf {
        window_title: format!("Rustle - v{}", env!("CARGO_PKG_VERSION")),
        icon: icon::window_icon(),
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let mut ui = UiTree::new();

    let root = ui
        .spawn_root(
            UiStyle::column().stretch(),
            Panel::new().background(UiColor::hex(0xFFFFFF)),
        )
        .unwrap();

    let nav_state = NavState::new(EditorMode::Level);
    let app: App = Rc::new(RefCell::new(None));
    let editor = Editor::new(app.clone(), nav_state.mode.clone());

    let about = about_flag();
    let ma = |act: editor::MenuAction| {
        let cell = editor.menu_action.clone();
        move || cell.set(act)
    };
    let menu_items = vec![
        MenuItem::action("New", Some("Ctrl + N"), || println!("New")),
        MenuItem::action("Open", Some("Ctrl + O"), || println!("Open")),
        MenuItem::action("Open Recent", Some("Shift + Ctrl + O"), || println!("Open Recent")),
        MenuItem::separator(),
        MenuItem::action("Project Properties", Some("Ctrl + R"), ma(editor::MenuAction::ProjectProps)),
        MenuItem::action("Save", Some("Ctrl + S"), ma(editor::MenuAction::Save)),
        MenuItem::action("Save As", Some("Shift + Ctrl + S"), ma(editor::MenuAction::SaveAs)),
        MenuItem::separator(),
        MenuItem::action("Import", Some("Ctrl + I"), ma(editor::MenuAction::Import)),
        MenuItem::action("Export", Some("Ctrl + E"), ma(editor::MenuAction::Export)),
        MenuItem::separator(),
        MenuItem::action("Shortcuts", Some("Ctrl + Alt + S"), || println!("Shortcuts")),
        MenuItem::action("Help", Some("Ctrl + H"), || println!("Help")),
        MenuItem::action("About", Some("Ctrl + Alt + A"), {
            let about = about.clone();
            move || about.set(true)
        }),
    ];
    spawn_nav_bar(&mut ui, root, &nav_state, menu_items);
    nav_state.set_project_name("No Project");
    nav_state.set_version(format!("v{} - {} [{}]", VERSION, BUILD_RELEASE_TYPE, BUILD_ID));

    // Body: tool strip + workspaces host.
    let mut body_style = UiStyle::row().grow();
    body_style.taffy.min_size.height = taffy::prelude::length(0.0);
    let body = ui
        .spawn(root, body_style, Panel::new().background(UiColor::hex(0xE4E4E4)))
        .unwrap();

    let mut strip_col = UiStyle::row();
    strip_col.taffy.size.height = taffy::prelude::percent(1.0);
    strip_col.taffy.flex_shrink = 0.0;
    let strip = ui.spawn(body, strip_col, Panel::new()).unwrap();
    spawn_tool_panel(&mut ui, strip, &editor);

    let mut host = UiStyle::row();
    host.taffy.flex_grow = 1.0;
    host.taffy.flex_basis = taffy::prelude::length(0.0);
    host.taffy.min_size.width = taffy::prelude::length(0.0);
    host.taffy.size.height = taffy::prelude::percent(1.0);
    let host = ui.spawn(body, host, Panel::new()).unwrap();
    let workspaces = spawn_workspaces(&mut ui, host, &editor);
    spawn_dialogs(&mut ui, root, &editor);

    spawn_about_dialog(&mut ui, root, about.clone());

    let mut recent = RecentProjects::load();
    let startup = spawn_startup(&mut ui, root, &recent.entries);

    let fonts = mq_backend::FontBook::new();
    let mut renderer = Renderer::new();
    mq_backend::install(&mut ui, &mut renderer, &fonts);
    mq_backend::set_text_scale(1.10);

    loop {
        mq_backend::pump_input(&mut ui);

        let launching = !ui.is_hidden(startup.overlay);

        // --- launch dialog -------------------------------------------
        match startup.signal.replace(Signal::None) {
            Signal::New => {
                let name = ui
                    .widget::<TextField>(startup.name_field)
                    .map(|f| f.value.trim().to_string())
                    .unwrap_or_default();
                let name = if name.is_empty() { "Untitled Project".to_string() } else { name };
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
                        open_project(&mut ui, &startup, &nav_state, &editor, &mut recent, &app, project);
                    }
                }
            }
            Signal::OpenRecent(i) => {
                if let Some(entry) = recent.entries.get(i).cloned() {
                    match Project::load(&entry.path) {
                        Ok(project) => open_project(
                            &mut ui, &startup, &nav_state, &editor, &mut recent, &app, project,
                        ),
                        Err(e) => eprintln!("could not open {}: {e}", entry.path),
                    }
                }
            }
            Signal::None => {}
        }

        // --- global keys -------------------------------------------
        let ctrl = is_key_down(KeyCode::LeftControl) || is_key_down(KeyCode::RightControl);
        let alt = is_key_down(KeyCode::LeftAlt) || is_key_down(KeyCode::RightAlt);
        let logo = is_key_down(KeyCode::LeftSuper) || is_key_down(KeyCode::RightSuper);

        {
            let mut input = editor.input.borrow_mut();
            input.shift = is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift);
            input.ctrl = ctrl;
            input.alt = alt;
            input.space = is_key_down(KeyCode::Space);
            input.mouse_down = is_mouse_button_down(macroquad::input::MouseButton::Left);
        }

        // Open a new undo generation on each gesture / key press so rapid
        // edits (paint drags, and a whole text entry) coalesce into one.
        let keys = get_keys_pressed();
        if is_mouse_button_pressed(macroquad::input::MouseButton::Left) {
            editor.bump_generation();
        } else if !keys.is_empty() && !editor.text_editing.get() {
            editor.bump_generation();
        }

        if !launching && ctrl && alt && is_key_pressed(KeyCode::A) {
            about.set(true);
        }
        if is_key_pressed(KeyCode::Escape) {
            about.set(false);
            editor.new_level_open.set(false);
            editor.project_props_open.set(false);
            editor.main_marquee.set(None);
        }

        // Menu / file actions: keyboard shortcuts feed the same channel as
        // the nav menu.
        let shift_down = is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift);
        if !launching && (ctrl || logo) && !editor.text_editing.get() {
            if is_key_pressed(KeyCode::S) {
                editor.menu_action.set(if shift_down {
                    editor::MenuAction::SaveAs
                } else {
                    editor::MenuAction::Save
                });
            }
            if is_key_pressed(KeyCode::I) {
                editor.menu_action.set(editor::MenuAction::Import);
            }
            if is_key_pressed(KeyCode::E) {
                editor.menu_action.set(editor::MenuAction::Export);
            }
            if is_key_pressed(KeyCode::R) {
                editor.menu_action.set(editor::MenuAction::ProjectProps);
            }
        }
        match editor.menu_action.replace(editor::MenuAction::None) {
            editor::MenuAction::Save => {
                if let Some(p) = app.borrow().as_ref() {
                    match p.save() {
                        Ok(()) => editor.dirty.set(false),
                        Err(e) => eprintln!("save failed: {e}"),
                    }
                }
            }
            editor::MenuAction::SaveAs => {
                if let Some((name, path)) = editor::io::save_as(&editor) {
                    recent.record(&name, &path);
                    nav_state.set_project_name(name);
                }
            }
            editor::MenuAction::Import => editor::io::import_image(&editor),
            editor::MenuAction::Export => editor::io::export_image(&editor),
            editor::MenuAction::ProjectProps => editor.project_props_open.set(true),
            editor::MenuAction::None => {}
        }

        // Undo / redo.
        if !launching && (ctrl || logo) {
            let shift = is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift);
            if is_key_pressed(KeyCode::Z) && !shift {
                editor.undo();
            }
            if is_key_pressed(KeyCode::Y) || (shift && is_key_pressed(KeyCode::Z)) {
                editor.redo();
            }
        }

        // Delete: clear the marquee region in pixel mode, else drop the
        // selected entity.
        if !launching && !editor.new_level_open.get() && is_key_pressed(KeyCode::Delete) {
            if editor.is_pixel_mode() && editor.main_marquee.get().is_some() {
                editor.clear_marquee_pixels();
            } else {
                editor.delete_selection();
            }
        }

        // Marquee copy / paste.
        if !launching && (ctrl || logo) && editor.is_pixel_mode() {
            if is_key_pressed(KeyCode::C) {
                editor.copy_marquee();
            }
            if is_key_pressed(KeyCode::V) {
                editor.paste_at_cursor();
            }
        }

        // Tool shortcuts (raw key presses; `pump_input` ate the char queue).
        if !launching && !editor.text_editing.get() && !(ctrl || alt || logo) {
            for kc in keys {
                if let Some(t) = tool_from_keycode(kc) {
                    editor.set_tool(t);
                }
            }
        }

        // --- workspace visibility + nav sync -----------------------
        let mode = nav_state.mode.get();
        if editor.session(|s| s.mode) != Some(mode) {
            editor.edit_session(|s| s.mode = mode);
        }
        for (i, &w) in workspaces.iter().enumerate() {
            let show = !launching && EditorMode::ALL[i] == mode;
            ui.set_display(w, show);
        }
        if let Some(name) = editor.with_project(|p| p.project_name.clone()) {
            nav_state.set_project_name(name);
            nav_state.set_saved(!editor.dirty.get());
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
