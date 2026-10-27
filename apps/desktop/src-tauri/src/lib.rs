//! The pet window. Starts the ingest server in-process (ADR 6) and
//! exposes its state to the web view through Tauri commands, so the
//! renderer never touches HTTP.

mod hit;
mod keys;
mod notify;
mod prefs;
mod shelf;
mod terminal;
mod text;
mod tray;
mod update;

use hit::{HitMask, Mask};
use sidepaw_core::event::UnknownHost;
use sidepaw_core::{Host, Level};
use sidepaw_daemon::config::{ModelConfig, PetPrefs, UiLanguage};
use sidepaw_daemon::install::{self, Report};
use sidepaw_daemon::pets;
use sidepaw_daemon::registry::Snapshot;
use sidepaw_daemon::secret::{self, KeySource};
use sidepaw_daemon::server::{self, Running};
use sidepaw_daemon::store::{AuditRow, DayReport, EventRow, History, Memory, TriggerStat};
use sidepaw_daemon::{command, packs};
use tauri::{AppHandle, Manager, State, WebviewUrl, WebviewWindowBuilder};

#[tauri::command]
fn snapshot(server: State<'_, Running>) -> Snapshot {
    server.shared.snapshot()
}

#[tauri::command]
fn server_port(server: State<'_, Running>) -> u16 {
    server.port
}

/// The renderer calls this whenever the drawn frame changes.
#[tauri::command]
fn set_hit_mask(mask: State<'_, HitMask>, cell: u32, rows: Vec<u64>) {
    *mask.mask.lock().unwrap() = Mask { cell, rows };
}

/// The keyboard shortcuts in effect for the prompt on screen.
#[tauri::command]
fn approval_keys(active: State<'_, keys::Active>) -> keys::Labels {
    active.labels()
}

#[tauri::command]
fn pet_prefs(server: State<'_, Running>) -> PetPrefs {
    server.shared.pet_prefs()
}

#[tauri::command]
fn get_level(server: State<'_, Running>) -> u8 {
    server.shared.level().into()
}

#[tauri::command]
fn set_level(server: State<'_, Running>, level: u8) -> Result<(), String> {
    let level = Level::try_from(level).map_err(|e| e.to_string())?;
    server.shared.set_level(level).map_err(|e| e.to_string())
}

#[tauri::command]
fn test_notification(app: AppHandle) -> Result<(), String> {
    notify::send(&app, "sidepaw", text::test_notification())
}

/// Answer a permission prompt the server is holding (level 4, `act` on).
#[tauri::command]
fn decide(server: State<'_, Running>, id: u64, allow: bool) -> Result<(), String> {
    server.shared.decide(id, allow).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_host_act(server: State<'_, Running>, host_name: String) -> Result<bool, String> {
    Ok(server.shared.host_act(host(&host_name)?))
}

#[tauri::command]
fn set_host_act(server: State<'_, Running>, host_name: String, act: bool) -> Result<(), String> {
    server
        .shared
        .set_host_act(host(&host_name)?, act)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn report_today(server: State<'_, Running>) -> Result<DayReport, String> {
    server.shared.report_today().map_err(|e| e.to_string())
}

fn host(name: &str) -> Result<Host, String> {
    name.parse().map_err(|e: UnknownHost| e.to_string())
}

#[tauri::command]
fn host_status(server: State<'_, Running>, host_name: String) -> Result<Report, String> {
    install::status(host(&host_name)?, server.port, server.shared.token())
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn host_install(server: State<'_, Running>, host_name: String) -> Result<Report, String> {
    install::apply(host(&host_name)?, server.port, server.shared.token()).map_err(|e| e.to_string())
}

#[tauri::command]
fn host_remove(server: State<'_, Running>, host_name: String) -> Result<Report, String> {
    install::remove(host(&host_name)?, server.port, server.shared.token())
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn get_ui_language(server: State<'_, Running>) -> UiLanguage {
    server.shared.ui_language()
}

#[tauri::command]
fn set_ui_language(server: State<'_, Running>, language: UiLanguage) -> Result<(), String> {
    server
        .shared
        .set_ui_language(language)
        .map_err(|e| e.to_string())?;
    text::apply_setting(language);
    Ok(())
}

#[tauri::command]
fn get_model(server: State<'_, Running>) -> ModelConfig {
    server.shared.model()
}

#[tauri::command]
fn set_model(server: State<'_, Running>, model: ModelConfig) -> Result<(), String> {
    server.shared.set_model(model).map_err(|e| e.to_string())
}

#[tauri::command]
fn key_source() -> KeySource {
    secret::source()
}

/// The brain needs the key itself to talk to the provider (ADR 7); it
/// never leaves the web view except in that request.
#[tauri::command]
fn api_key() -> Result<Option<String>, String> {
    secret::get().map_err(|e| e.to_string())
}

#[tauri::command]
fn set_api_key(key: String) -> Result<(), String> {
    secret::set(&key).map_err(|e| e.to_string())
}

#[tauri::command]
fn clear_api_key() -> Result<(), String> {
    secret::clear().map_err(|e| e.to_string())
}

#[tauri::command]
fn pet_file_read(id: String, name: String) -> Result<Option<String>, String> {
    pets::read(&id, &name).map_err(|e| e.to_string())
}

#[tauri::command]
fn pet_file_write(id: String, name: String, text: String) -> Result<(), String> {
    pets::write(&id, &name, &text).map_err(|e| e.to_string())
}

#[tauri::command]
fn audit_call(
    server: State<'_, Running>,
    provider: String,
    purpose: String,
    bytes: u64,
    sha256: String,
) -> Result<(), String> {
    server
        .shared
        .audit(&provider, &purpose, bytes, &sha256)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn audit_recent(server: State<'_, Running>, limit: u32) -> Result<Vec<AuditRow>, String> {
    server.shared.audit_recent(limit).map_err(|e| e.to_string())
}

fn data_dir() -> Result<std::path::PathBuf, String> {
    sidepaw_daemon::home::dir().map_err(|e| e.to_string())
}

#[tauri::command]
fn packs_installed() -> Result<Vec<packs::PackInfo>, String> {
    Ok(packs::list(&data_dir()?))
}

/// Pets in Codex's and petdex's own folders, not installed here yet.
#[tauri::command]
fn packs_found() -> Result<Vec<packs::Found>, String> {
    let user = sidepaw_daemon::home::user_home().ok_or("cannot find home directory")?;
    Ok(packs::discover(&data_dir()?, &user))
}

/// A zip's bytes as the raw request body (a petdex download), with
/// where it came from in the `x-source` header as JSON.
#[tauri::command]
fn pack_install_zip(request: tauri::ipc::Request<'_>) -> Result<packs::PackInfo, String> {
    let tauri::ipc::InvokeBody::Raw(bytes) = request.body() else {
        return Err("expected the zip as raw bytes".into());
    };
    let source = request
        .headers()
        .get("x-source")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or(packs::Source {
            kind: "zip".into(),
            ..Default::default()
        });
    packs::install_zip(&data_dir()?, bytes, source).map_err(|e| format!("{e:#}"))
}

/// A zip file or a pack folder on disk: picked in a dialog, dropped on
/// the pet, or found in Codex's pets.
#[tauri::command]
fn pack_install_path(path: String, kind: Option<String>) -> Result<packs::PackInfo, String> {
    let path = std::path::PathBuf::from(path);
    let home = data_dir()?;
    let result = if path.is_dir() {
        let source = packs::Source {
            kind: kind.unwrap_or_else(|| "folder".into()),
            ..Default::default()
        };
        packs::install_folder(&home, &path, source)
    } else {
        let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
        let source = packs::Source {
            kind: kind.unwrap_or_else(|| "zip".into()),
            ..Default::default()
        };
        packs::install_zip(&home, &bytes, source)
    };
    result.map_err(|e| format!("{e:#}"))
}

/// A file of an installed pack as raw bytes; empty when it is absent.
#[tauri::command]
fn pack_file(id: String, name: String) -> Result<tauri::ipc::Response, String> {
    let bytes = packs::read(&data_dir()?, &id, &name).map_err(|e| e.to_string())?;
    Ok(tauri::ipc::Response::new(bytes.unwrap_or_default()))
}

#[tauri::command]
fn pack_remove(server: State<'_, Running>, id: String) -> Result<(), String> {
    packs::remove(&data_dir()?, &id).map_err(|e| e.to_string())?;
    if server.shared.active_pack() == id {
        server
            .shared
            .set_active_pack("placeholder")
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn get_active_pack(server: State<'_, Running>) -> String {
    server.shared.active_pack()
}

/// Save the choice and tell every window, so the pet swaps at once.
#[tauri::command]
fn set_active_pack(app: AppHandle, server: State<'_, Running>, id: String) -> Result<(), String> {
    server
        .shared
        .set_active_pack(&id)
        .map_err(|e| e.to_string())?;
    use tauri::Emitter;
    app.emit("pack-active-changed", id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn memories(server: State<'_, Running>) -> Result<Vec<Memory>, String> {
    server.shared.memories().map_err(|e| e.to_string())
}

#[tauri::command]
fn add_memory(
    server: State<'_, Running>,
    source: String,
    body: String,
    retention: String,
) -> Result<i64, String> {
    server
        .shared
        .add_memory(&source, &body, &retention)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn update_memory(
    server: State<'_, Running>,
    id: i64,
    body: String,
    retention: String,
) -> Result<(), String> {
    server
        .shared
        .update_memory(id, &body, &retention)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_memory(server: State<'_, Running>, id: i64) -> Result<(), String> {
    server.shared.delete_memory(id).map_err(|e| e.to_string())
}

#[tauri::command]
fn trigger_stats(server: State<'_, Running>) -> Result<Vec<TriggerStat>, String> {
    server.shared.trigger_stats().map_err(|e| e.to_string())
}

#[tauri::command]
fn record_trigger(
    server: State<'_, Running>,
    trigger: String,
    outcome: String,
    base_s: u64,
) -> Result<TriggerStat, String> {
    server
        .shared
        .record_trigger(&trigger, &outcome, base_s)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn quiet_hours(server: State<'_, Running>) -> sidepaw_daemon::config::Quiet {
    server.shared.quiet_hours()
}

/// The cursor in screen pixels, for noticing that the user is back.
#[tauri::command]
fn cursor_global(app: AppHandle) -> Option<(f64, f64)> {
    app.cursor_position().ok().map(|p| (p.x, p.y))
}

#[tauri::command]
fn history(server: State<'_, Running>, days: u32) -> Result<History, String> {
    server.shared.history(days).map_err(|e| e.to_string())
}

#[tauri::command]
fn recent_events(server: State<'_, Running>, limit: u32) -> Result<Vec<EventRow>, String> {
    server
        .shared
        .recent_events(limit)
        .map_err(|e| e.to_string())
}

/// The cursor relative to the pet window, for reactions to a cursor
/// that is near but not over it (the web view only sees the cursor
/// while it is over opaque pixels).
#[tauri::command]
fn cursor_relative(app: AppHandle, window: tauri::WebviewWindow) -> Option<hit::CursorRelative> {
    hit::cursor_relative(&app, &window)
}

fn exe() -> Result<std::path::PathBuf, String> {
    std::env::current_exe().map_err(|e| e.to_string())
}

#[tauri::command]
fn command_status() -> Result<command::CommandLink, String> {
    command::status(&exe()?).map_err(|e| e.to_string())
}

#[tauri::command]
fn command_install() -> Result<command::CommandLink, String> {
    command::install(&exe()?).map_err(|e| e.to_string())
}

#[tauri::command]
fn command_remove() -> Result<command::CommandLink, String> {
    command::remove(&exe()?).map_err(|e| e.to_string())
}

/// Bring the terminal of a session to the front.
#[tauri::command]
fn focus_session(server: State<'_, Running>, key: String) -> Result<(), String> {
    focus_by_key(&server, &key)
}

pub(crate) fn focus_by_key(server: &Running, key: &str) -> Result<(), String> {
    let origin = server
        .shared
        .session_origin(key)
        .ok_or("the session's terminal is not known yet")?;
    terminal::focus(&origin)
}

/// Other apps' windows and whether the one in front is full screen,
/// in the pet window's physical pixels.
#[tauri::command]
fn scene(window: tauri::WebviewWindow) -> shelf::Scene {
    shelf::scene(window.scale_factor().unwrap_or(1.0))
}

/// Secondary windows are created on demand and reused while open.
fn open_window(
    app: &AppHandle,
    label: &str,
    page: &str,
    title: &str,
    size: (f64, f64),
) -> Result<(), String> {
    if let Some(w) = app.get_webview_window(label) {
        w.show()
            .and_then(|_| w.set_focus())
            .map_err(|e| e.to_string())?;
        return Ok(());
    }
    WebviewWindowBuilder::new(app, label, WebviewUrl::App(page.into()))
        .title(title)
        .inner_size(size.0, size.1)
        .build()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// `section` scrolls the window to one part (`level` when the pet
/// offers a higher permission level); an open window is told by event.
#[tauri::command]
fn open_setup(app: AppHandle, section: Option<String>) -> Result<(), String> {
    let open = app.get_webview_window("setup").is_some();
    let page = match (&section, open) {
        (Some(s), false) => format!("setup.html#{s}"),
        _ => "setup.html".into(),
    };
    open_window(&app, "setup", &page, "sidepaw setup", (560.0, 760.0))?;
    if let (Some(s), true) = (section, open) {
        use tauri::Emitter;
        let _ = app.emit_to("setup", "setup-focus", s);
    }
    Ok(())
}

#[tauri::command]
fn usage(server: State<'_, Running>) -> Result<sidepaw_daemon::store::Usage, String> {
    server.shared.usage().map_err(|e| e.to_string())
}

/// Turned on only after the user accepted the preview (ADR 8).
#[tauri::command]
fn set_live_lines(app: AppHandle, on: bool) {
    prefs::change(&app, |p| p.live_lines = on);
}

#[tauri::command]
fn open_history(app: AppHandle) -> Result<(), String> {
    open_window(&app, "history", "history.html", "sidepaw", (720.0, 640.0))
}

#[tauri::command]
fn open_chat(app: AppHandle) -> Result<(), String> {
    open_window(&app, "chat", "chat.html", "sidepaw", (380.0, 540.0))
}

pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(keys::on_shortcut)
                .build(),
        )
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        // Model requests go through the Rust HTTP client so the web
        // view's origin rules do not apply; the scope is in capabilities.
        .plugin(tauri_plugin_http::init())
        .setup(|app| {
            // `start` spawns onto the runtime it is called from; Tauri's
            // async runtime is tokio, so the server lives as long as the app.
            let running = tauri::async_runtime::block_on(server::start(0))?;
            tracing::info!("embedded server on 127.0.0.1:{}", running.port);
            text::apply_setting(running.shared.ui_language());
            let size = running.shared.pet_prefs().size;
            app.manage(running);
            app.manage(HitMask::default());
            app.manage(keys::Active::default());
            prefs::apply_size(app.handle(), size);
            if let Some(window) = app.get_webview_window("pet") {
                hit::spawn_poller(app.handle().clone(), window);
            }
            // A pet lives in the menu bar, not the dock.
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            tray::install(app.handle())?;
            notify::spawn_watcher(app.handle().clone());
            keys::spawn_watcher(app.handle().clone());
            if let Some(window) = app.get_webview_window("pet") {
                shelf::spawn_fullscreen_watcher(window);
            }
            update::check_in_background(app.handle().clone());

            let running = app.state::<Running>();
            match install::refresh_stale(Host::ClaudeCode, running.port, running.shared.token()) {
                Ok(Some(r)) => tracing::info!("rewrote stale hooks in {}", r.path),
                Ok(None) => {}
                Err(e) => tracing::warn!("could not refresh hooks: {e:#}"),
            }
            if install::is_first_run(running.port, running.shared.token()) {
                let _ = open_setup(app.handle().clone(), None);
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            snapshot,
            server_port,
            report_today,
            decide,
            get_host_act,
            set_host_act,
            get_level,
            set_level,
            test_notification,
            set_hit_mask,
            pet_prefs,
            approval_keys,
            host_status,
            host_install,
            host_remove,
            get_ui_language,
            set_ui_language,
            get_model,
            set_model,
            key_source,
            api_key,
            set_api_key,
            clear_api_key,
            pet_file_read,
            pet_file_write,
            audit_call,
            audit_recent,
            recent_events,
            history,
            packs_installed,
            packs_found,
            pack_install_zip,
            pack_install_path,
            pack_file,
            pack_remove,
            get_active_pack,
            set_active_pack,
            memories,
            add_memory,
            update_memory,
            delete_memory,
            trigger_stats,
            record_trigger,
            quiet_hours,
            cursor_global,
            cursor_relative,
            scene,
            focus_session,
            command_status,
            command_install,
            command_remove,
            open_setup,
            usage,
            set_live_lines,
            open_chat,
            open_history
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
