//! Menu bar / system tray icon. With no dock icon and no taskbar entry
//! (the pet window skips both), this is how the app is found once the
//! pet is hidden, and the only way to quit without a terminal.

use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager};

const SETUP: &str = "setup";
const TOGGLE: &str = "toggle";
const QUIT: &str = "quit";

pub fn install(app: &AppHandle) -> tauri::Result<()> {
    let setup = MenuItem::with_id(app, SETUP, "Setup...", true, None::<&str>)?;
    let toggle = MenuItem::with_id(app, TOGGLE, "Hide pet", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, QUIT, "Quit sidepaw", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&setup, &toggle, &quit])?;

    let mut builder = TrayIconBuilder::with_id("main")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .tooltip("sidepaw");
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder
        .on_menu_event(move |app, event| match event.id().as_ref() {
            SETUP => {
                let _ = super::open_setup(app.clone());
            }
            TOGGLE => {
                if let Some(pet) = app.get_webview_window("pet") {
                    let visible = pet.is_visible().unwrap_or(true);
                    let _ = if visible { pet.hide() } else { pet.show() };
                    let _ = toggle.set_text(if visible { "Show pet" } else { "Hide pet" });
                }
            }
            QUIT => app.exit(0),
            _ => {}
        })
        .build(app)?;
    Ok(())
}
