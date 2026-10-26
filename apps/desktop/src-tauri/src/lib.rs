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
use sidepaw_daemon::install::{self, Report, Status};
use sidepaw_daemon::registry::Snapshot;
use sidepaw_daemon::server::{self, Running};
use sidepaw_daemon::store::DayReport;
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
    notify::send(&app, "sidepaw", text::test_notification()).map_err(|e| e.to_string())
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
        .inner_size(520.0, 640.0)
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

            // First run: nothing is hooked up yet, so show the setup window
            // instead of a pet that never moves.
            let running = app.state::<Running>();
            let first_run = install::status(Host::ClaudeCode, running.port, running.shared.token())
                .map(|r| matches!(r.status, Status::Absent | Status::NoFile))
                .unwrap_or(false);
            if first_run {
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
            open_setup
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
