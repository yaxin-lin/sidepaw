//! The pet window. Starts the ingest server in-process (ADR 6) and
//! exposes its state to the web view through Tauri commands, so the
//! renderer never touches HTTP.

mod hit;
mod notify;
mod text;
mod tray;
mod update;

use hit::{HitMask, Mask};
use sidepaw_core::event::UnknownHost;
use sidepaw_core::{Host, Level};
use sidepaw_daemon::config::ModelConfig;
use sidepaw_daemon::install::{self, Report};
use sidepaw_daemon::pets;
use sidepaw_daemon::registry::Snapshot;
use sidepaw_daemon::secret::{self, KeySource};
use sidepaw_daemon::server::{self, Running};
use sidepaw_daemon::store::{AuditRow, DayReport};
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
    *mask.0.lock().unwrap() = Mask { cell, rows };
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

/// The cursor relative to the pet window, for reactions to a cursor
/// that is near but not over it (the web view only sees the cursor
/// while it is over opaque pixels).
#[tauri::command]
fn cursor_relative(app: AppHandle, window: tauri::WebviewWindow) -> Option<hit::CursorRelative> {
    hit::cursor_relative(&app, &window)
}

/// The setup window is created on demand and reused while open.
#[tauri::command]
fn open_setup(app: AppHandle) -> Result<(), String> {
    if let Some(w) = app.get_webview_window("setup") {
        w.show()
            .and_then(|_| w.set_focus())
            .map_err(|e| e.to_string())?;
        return Ok(());
    }
    WebviewWindowBuilder::new(&app, "setup", WebviewUrl::App("setup.html".into()))
        .title("sidepaw setup")
        .inner_size(560.0, 760.0)
        .build()
        .map(|_| ())
        .map_err(|e| e.to_string())
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
        // Model requests go through the Rust HTTP client so the web
        // view's origin rules do not apply; the scope is in capabilities.
        .plugin(tauri_plugin_http::init())
        .setup(|app| {
            // `start` spawns onto the runtime it is called from; Tauri's
            // async runtime is tokio, so the server lives as long as the app.
            let running = tauri::async_runtime::block_on(server::start(0))?;
            tracing::info!("embedded server on 127.0.0.1:{}", running.port);
            app.manage(running);
            app.manage(HitMask::default());
            if let Some(window) = app.get_webview_window("pet") {
                hit::spawn_poller(app.handle().clone(), window);
            }
            // A pet lives in the menu bar, not the dock.
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            tray::install(app.handle())?;
            notify::spawn_watcher(app.handle().clone());
            update::check_in_background(app.handle().clone());

            let running = app.state::<Running>();
            if install::is_first_run(running.port, running.shared.token()) {
                let _ = open_setup(app.handle().clone());
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
            host_status,
            host_install,
            host_remove,
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
            cursor_relative,
            open_setup
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
