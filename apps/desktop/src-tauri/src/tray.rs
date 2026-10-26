//! Menu bar / system tray icon. With no dock icon and no taskbar entry
//! (the pet window skips both), this is how the app is found once the
//! pet is hidden, and the only way to quit without a terminal.

use tauri::image::Image;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager};

// Embedded rather than taken from the window icon: in dev there may be
// no window icon, and a tray item without an image is an invisible
// blank in the macOS menu bar.
const ICON: &[u8] = include_bytes!("../icons/32x32.png");

const SETUP: &str = "setup";
const TOGGLE: &str = "toggle";
const QUIT: &str = "quit";

pub fn install(app: &AppHandle) -> tauri::Result<()> {
    let setup = MenuItem::with_id(app, SETUP, "Setup...", true, None::<&str>)?;
    let toggle = MenuItem::with_id(app, TOGGLE, "Hide pet", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, QUIT, "Quit sidepaw", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&setup, &toggle, &quit])?;

    TrayIconBuilder::with_id("main")
        .icon(Image::from_bytes(ICON)?)
        .menu(&menu)
        .show_menu_on_left_click(true)
        .tooltip("sidepaw")
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
