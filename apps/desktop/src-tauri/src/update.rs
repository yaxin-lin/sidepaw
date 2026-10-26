//! Self-update. Release builds check the GitHub release feed once at
//! startup, install silently and restart; a pet should not nag. Dev
//! builds skip the check so hacking on the app never phones home.

use tauri::AppHandle;
use tauri_plugin_updater::UpdaterExt;

pub fn check_in_background(app: AppHandle) {
    if cfg!(debug_assertions) {
        return;
    }
    tauri::async_runtime::spawn(async move {
        if let Err(e) = check(&app).await {
            tracing::warn!("update check: {e}");
        }
    });
}

async fn check(app: &AppHandle) -> tauri_plugin_updater::Result<()> {
    let Some(update) = app.updater()?.check().await? else {
        return Ok(());
    };
    tracing::info!("updating to {}", update.version);
    update.download_and_install(|_, _| {}, || {}).await?;
    app.restart();
}
