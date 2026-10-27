//! System notifications for the moments that need a person: an agent
//! waiting for approval, an error, a finished turn. Gated on the
//! permission level (`Level::may_notify`, level 3 and up) because the
//! text names the host and shows on a screen others may see.

use sidepaw_core::State;
use sidepaw_daemon::server::Running;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};
#[cfg(not(target_os = "macos"))]
use tauri_plugin_notification::NotificationExt;

const POLL: Duration = Duration::from_millis(500);
/// One notification per session and state within this window; a
/// permission prompt the user is already looking at should not nag.
const REPEAT: Duration = Duration::from_secs(60);

/// On macOS the plugin's backend is the deprecated NSUserNotificationCenter,
/// which current macOS silently drops for an ad-hoc signed bundle, so the
/// notification goes through `osascript` instead. The sender then reads
/// as Script Editor; a proper UNUserNotificationCenter path needs a
/// signed bundle and is planned with code signing.
#[cfg(target_os = "macos")]
pub fn send(_app: &AppHandle, title: &str, body: &str) -> Result<(), String> {
    let esc = |s: &str| s.replace('\\', "\\\\").replace('"', "\\\"");
    let script = format!(
        "display notification \"{}\" with title \"{}\" sound name \"default\"",
        esc(body),
        esc(title)
    );
    let status = std::process::Command::new("osascript")
        .arg("-e")
        .arg(script)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("osascript exited with {status}"))
    }
}

#[cfg(not(target_os = "macos"))]
pub fn send(app: &AppHandle, title: &str, body: &str) -> Result<(), String> {
    app.notification()
        .builder()
        .title(title)
        .body(body)
        .sound("default")
        .show()
        .map_err(|e| e.to_string())
}

pub fn spawn_watcher(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut seen: HashMap<String, State> = HashMap::new();
        let mut sent: HashMap<(String, State), Instant> = HashMap::new();
        loop {
            tokio::time::sleep(POLL).await;
            let running = app.state::<Running>();
            let snapshot = running.shared.snapshot();
            let allowed = running.shared.level().may_notify() && !running.shared.pet_prefs().dnd;
            let mut live = HashMap::new();
            for s in &snapshot.sessions {
                live.insert(s.key.clone(), s.state);
                let changed = seen.get(&s.key) != Some(&s.state);
                if !changed || !allowed {
                    continue;
                }
                let Some(body) = crate::text::notification(&s.key, s.state) else {
                    continue;
                };
                let slot = (s.key.clone(), s.state);
                if sent.get(&slot).is_some_and(|t| t.elapsed() < REPEAT) {
                    continue;
                }
                sent.insert(slot, Instant::now());
                if let Err(e) = send(&app, "sidepaw", &body) {
                    tracing::warn!("notification: {e}");
                }
            }
            seen = live;
        }
    });
}
