//! Menu bar / system tray icon. With no dock icon and no taskbar entry
//! (the pet window skips both), this is how the app is found once the
//! pet is hidden, and the only way to quit without a terminal. The menu
//! doubles as the session monitor: one line per live session, the ones
//! that need the user first, and a count next to the icon when any do.

use crate::{prefs, text};
use sidepaw_core::State;
use sidepaw_daemon::config::{PetPrefs, PetSize};
use sidepaw_daemon::registry::Snapshot;
use sidepaw_daemon::server::Running;
use std::time::Duration;
use tauri::image::Image;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::{TrayIcon, TrayIconBuilder};
use tauri::{AppHandle, Manager};
use tauri_plugin_autostart::ManagerExt;

// Embedded rather than taken from the window icon: in dev there may be
// no window icon, and a tray item without an image is an invisible
// blank in the macOS menu bar.
const ICON: &[u8] = include_bytes!("../icons/32x32.png");

const DND: &str = "dnd";
const MINI: &str = "mini";
const AUTOSTART: &str = "autostart";
const SIZE_PREFIX: &str = "size:";
const SIZES: [PetSize; 3] = [PetSize::S, PetSize::M, PetSize::L];
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
            &Toggles::read(app),
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
            DND => prefs::change(app, |p| p.dnd = !p.dnd),
            MINI => prefs::change(app, |p| p.mini = !p.mini),
            AUTOSTART => {
                let launcher = app.autolaunch();
                let on = launcher.is_enabled().unwrap_or(false);
                let result = if on {
                    launcher.disable()
                } else {
                    launcher.enable()
                };
                if let Err(e) = result {
                    tracing::warn!("start at login: {e}");
                }
            }
            QUIT => app.exit(0),
            id if id.starts_with(SIZE_PREFIX) => {
                if let Some(size) = SIZES
                    .iter()
                    .find(|s| id == format!("{SIZE_PREFIX}{}", s.as_str()))
                {
                    let size = *size;
                    prefs::change(app, |p| p.size = size);
                }
            }
            id => {
                if let Some(key) = id.strip_prefix("session:") {
                    if let Err(e) = super::focus_by_key(&app.state::<Running>(), key) {
                        tracing::warn!("focus {key}: {e}");
                    }
                }
            }
        })
        .build(app)?;
    spawn_refresher(app.clone(), tray);
    Ok(())
}

/// Everything the menu shows besides the sessions; when it changes the
/// menu is rebuilt.
#[derive(Clone, PartialEq)]
struct Toggles {
    hidden: bool,
    prefs: PetPrefs,
    autostart: bool,
}

impl Toggles {
    fn read(app: &AppHandle) -> Self {
        Toggles {
            hidden: app
                .get_webview_window("pet")
                .and_then(|w| w.is_visible().ok())
                .map(|v| !v)
                .unwrap_or(false),
            prefs: app.state::<Running>().shared.pet_prefs(),
            autostart: app.autolaunch().is_enabled().unwrap_or(false),
        }
    }
}

fn build_menu(
    app: &AppHandle,
    snap: &Snapshot,
    toggles: &Toggles,
) -> tauri::Result<Menu<tauri::Wry>> {
    let toggle_text = if toggles.hidden {
        text::show_pet()
    } else {
        text::hide_pet()
    };
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
            s.origin.is_some(),
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
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    let p = toggles.prefs;
    menu.append(&CheckMenuItem::with_id(
        app,
        DND,
        text::dnd(),
        true,
        p.dnd,
        None::<&str>,
    )?)?;
    menu.append(&CheckMenuItem::with_id(
        app,
        MINI,
        text::mini(),
        true,
        p.mini,
        None::<&str>,
    )?)?;
    let sizes = SIZES
        .iter()
        .map(|s| {
            CheckMenuItem::with_id(
                app,
                format!("{SIZE_PREFIX}{}", s.as_str()),
                text::size_name(*s),
                true,
                *s == p.size,
                None::<&str>,
            )
        })
        .collect::<tauri::Result<Vec<_>>>()?;
    let refs: Vec<&dyn tauri::menu::IsMenuItem<tauri::Wry>> = sizes
        .iter()
        .map(|i| i as &dyn tauri::menu::IsMenuItem<tauri::Wry>)
        .collect();
    menu.append(&Submenu::with_id_and_items(
        app,
        "size",
        text::size(),
        true,
        &refs,
    )?)?;
    menu.append(&CheckMenuItem::with_id(
        app,
        AUTOSTART,
        text::autostart(),
        true,
        toggles.autostart,
        None::<&str>,
    )?)?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;
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
        let mut last_toggles: Option<Toggles> = None;
        let mut attention = false;
        loop {
            tokio::time::sleep(REFRESH).await;
            let snap = app.state::<Running>().shared.snapshot();
            let toggles = Toggles::read(&app);
            let key: String = snap
                .sessions
                .iter()
                .map(|s| format!("{}{}", session_line(s), s.origin.is_some()))
                .chain(std::iter::once(text::hide_pet().to_string()))
                .collect::<Vec<_>>()
                .join("\n");
            if key != last_key || last_toggles.as_ref() != Some(&toggles) {
                last_key = key;
                last_toggles = Some(toggles.clone());
                if let Ok(menu) = build_menu(&app, &snap, &toggles) {
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
