//! The pet window. Starts the ingest server in-process (ADR 6) and
//! exposes its state to the web view through Tauri commands, so the
//! renderer never touches HTTP.

use sidepaw_daemon::registry::Snapshot;
use sidepaw_daemon::server::{self, Running};
use tauri::{Manager, State};

#[tauri::command]
fn snapshot(server: State<'_, Running>) -> Snapshot {
    server.shared.snapshot()
}

#[tauri::command]
fn server_port(server: State<'_, Running>) -> u16 {
    server.port
}

pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    tauri::Builder::default()
        .setup(|app| {
            // `start` spawns onto the runtime it is called from; Tauri's
            // async runtime is tokio, so the server lives as long as the app.
            let running = tauri::async_runtime::block_on(server::start(0))?;
            tracing::info!("embedded server on 127.0.0.1:{}", running.port);
            app.manage(running);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![snapshot, server_port])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
