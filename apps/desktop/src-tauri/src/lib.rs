//! The pet window. Starts the ingest server in-process (ADR 6) and
//! exposes its state to the web view through Tauri commands, so the
//! renderer never touches HTTP.

mod hit;
mod tray;
mod update;

use hit::{HitMask, Mask};
use sidepaw_core::Host;
use sidepaw_daemon::install::{self, Report, Status};
use sidepaw_daemon::registry::Snapshot;
use sidepaw_daemon::server::{self, Running};
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

fn parse_host(s: &str) -> Result<Host, String> {
    serde_json::from_value(serde_json::Value::String(s.to_string()))
        .map_err(|_| format!("unknown host {s:?}"))
}

#[tauri::command]
fn host_status(server: State<'_, Running>, host: String) -> Result<Report, String> {
    install::status(parse_host(&host)?, server.port, server.shared.token())
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn host_install(server: State<'_, Running>, host: String) -> Result<Report, String> {
    install::apply(parse_host(&host)?, server.port, server.shared.token())
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn host_remove(server: State<'_, Running>, host: String) -> Result<Report, String> {
    install::remove(parse_host(&host)?, server.port, server.shared.token())
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
            set_hit_mask,
            host_status,
            host_install,
            host_remove,
            open_setup
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
