//! Menu bar / system tray icon. With no dock icon and no taskbar entry
//! (the pet window skips both), this is how the app is found once the
//! pet is hidden, and the only way to quit without a terminal. The menu
//! doubles as the session monitor: one line per live session, the ones
//! that need the user first, and a count next to the icon when any do.

use crate::text;
use sidepaw_core::State;
use sidepaw_daemon::registry::Snapshot;
use sidepaw_daemon::server::Running;
use std::time::Duration;
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{TrayIcon, TrayIconBuilder};
use tauri::{AppHandle, Manager};

// Embedded rather than taken from the window icon: in dev there may be
// no window icon, and a tray item without an image is an invisible
// blank in the macOS menu bar.
const ICON: &[u8] = include_bytes!("../icons/32x32.png");

const CHAT: &str = "chat";
const SETUP: &str = "setup";
const TOGGLE: &str = "toggle";
const QUIT: &str = "quit";
const REFRESH: Duration = Duration::from_secs(1);

pub fn install(app: &AppHandle) -> tauri::Result<()> {
    let tray = TrayIconBuilder::with_id("main")
        .icon(Image::from_bytes(ICON)?)
        .menu(&build_menu(
            app,
            &Snapshot::default_empty(),
            text::hide_pet(),
        )?)
        .show_menu_on_left_click(true)
        .tooltip("sidepaw")
        .on_menu_event(|app, event| match event.id().as_ref() {
            CHAT => {
                let _ = super::open_chat(app.clone());
            }
            SETUP => {
                let _ = super::open_setup(app.clone());
            }
            TOGGLE => {
                if let Some(pet) = app.get_webview_window("pet") {
                    let visible = pet.is_visible().unwrap_or(true);
                    let _ = if visible { pet.hide() } else { pet.show() };
                }
            }
            QUIT => app.exit(0),
            _ => {}
        })
        .build(app)?;
    spawn_refresher(app.clone(), tray);
    Ok(())
}

fn build_menu(
    app: &AppHandle,
    snap: &Snapshot,
    toggle_text: &str,
) -> tauri::Result<Menu<tauri::Wry>> {
    let menu = Menu::new(app)?;
    if snap.sessions.is_empty() {
        menu.append(&MenuItem::with_id(
            app,
            "none",
            text::no_sessions(),
            false,
            None::<&str>,
        )?)?;
    }
    for s in &snap.sessions {
        let id = format!("session:{}", s.key);
        menu.append(&MenuItem::with_id(
            app,
            id,
            session_line(s),
            false,
            None::<&str>,
        )?)?;
    }
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&MenuItem::with_id(
        app,
        CHAT,
        text::chat(),
        true,
        None::<&str>,
    )?)?;
    menu.append(&MenuItem::with_id(
        app,
        SETUP,
        text::setup(),
        true,
        None::<&str>,
    )?)?;
    menu.append(&MenuItem::with_id(
        app,
        TOGGLE,
        toggle_text,
        true,
        None::<&str>,
    )?)?;
    menu.append(&MenuItem::with_id(
        app,
        QUIT,
        text::quit(),
        true,
        None::<&str>,
    )?)?;
    Ok(menu)
}

fn session_line(s: &sidepaw_daemon::registry::SessionView) -> String {
    let mark = match s.state {
        State::Waiting => "●",
        State::Error => "✕",
        _ => "○",
    };
    let project = s
        .project
        .as_deref()
        .map(|p| format!(" · {p}"))
        .unwrap_or_default();
    format!(
        "{mark} {}{project} · {} {}",
        text::host(&s.key),
        text::state(s.state),
        text::duration(s.for_ms)
    )
}

/// Swap the icon's blue for orange while a session waits for the user.
fn attention_icon() -> tauri::Result<Image<'static>> {
    let base = Image::from_bytes(ICON)?;
    let mut rgba = base.rgba().to_vec();
    for px in rgba.chunks_exact_mut(4) {
        px.swap(0, 2);
    }
    Ok(Image::new_owned(rgba, base.width(), base.height()))
}

fn spawn_refresher(app: AppHandle, tray: TrayIcon) {
    tauri::async_runtime::spawn(async move {
        let mut last_key = String::new();
        let mut attention = false;
        loop {
            tokio::time::sleep(REFRESH).await;
            let snap = app.state::<Running>().shared.snapshot();
            let hidden = app
                .get_webview_window("pet")
                .and_then(|w| w.is_visible().ok())
                .map(|v| !v)
                .unwrap_or(false);
            let toggle_text = if hidden {
                text::show_pet()
            } else {
                text::hide_pet()
            };
            let key: String = snap
                .sessions
                .iter()
                .map(session_line)
                .chain(std::iter::once(toggle_text.to_string()))
                .collect::<Vec<_>>()
                .join("\n");
            if key != last_key {
                last_key = key;
                if let Ok(menu) = build_menu(&app, &snap, toggle_text) {
                    let _ = tray.set_menu(Some(menu));
                }
            }
            let waiting = snap
                .sessions
                .iter()
                .filter(|s| s.state == State::Waiting)
                .count();
            let want = waiting > 0;
            if want != attention {
                attention = want;
                let icon = if want {
                    attention_icon()
                } else {
                    Image::from_bytes(ICON)
                };
                if let Ok(icon) = icon {
                    let _ = tray.set_icon(Some(icon));
                }
            }
            #[cfg(target_os = "macos")]
            {
                let title = if waiting > 0 {
                    Some(waiting.to_string())
                } else {
                    None
                };
                let _ = tray.set_title(title);
            }
        }
    });
}
