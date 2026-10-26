//! System notifications for the moments that need a person: an agent
//! waiting for approval, an error, a finished turn. Gated on the
//! permission level (`Level::may_notify`, level 3 and up) because the
//! text names the host and shows on a screen others may see.

use sidepaw_core::State;
use sidepaw_daemon::server::Running;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};
use tauri_plugin_notification::NotificationExt;

const POLL: Duration = Duration::from_millis(500);
/// One notification per session and state within this window; a
/// permission prompt the user is already looking at should not nag.
const REPEAT: Duration = Duration::from_secs(60);

fn text(state: State) -> Option<&'static str> {
    match state {
        State::Waiting => Some("needs your approval"),
        State::Error => Some("hit an error"),
        State::Done => Some("finished a turn"),
        _ => None,
    }
}

/// `claude-code:abc…` reads as "Claude Code" in a notification.
fn host_label(key: &str) -> &str {
    match key.split(':').next().unwrap_or("") {
        "claude-code" => "Claude Code",
        "codex" => "Codex",
        "opencode" => "OpenCode",
        "dsh" => "DeepSeek Harness",
        "kimi" => "Kimi Code",
        "gemini" => "Gemini CLI",
        "cursor" => "Cursor",
        _ => "An agent",
    }
}

pub fn send(app: &AppHandle, title: &str, body: &str) -> tauri_plugin_notification::Result<()> {
    app.notification()
        .builder()
        .title(title)
        .body(body)
        .sound("default")
        .show()
}

pub fn spawn_watcher(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut seen: HashMap<String, State> = HashMap::new();
        let mut sent: HashMap<(String, State), Instant> = HashMap::new();
        loop {
            tokio::time::sleep(POLL).await;
            let running = app.state::<Running>();
            let snapshot = running.shared.snapshot();
            let allowed = running.shared.level().may_notify();
            let mut live = HashMap::new();
            for s in &snapshot.sessions {
                live.insert(s.key.clone(), s.state);
                let changed = seen.get(&s.key) != Some(&s.state);
                if !changed || !allowed {
                    continue;
                }
                let Some(what) = text(s.state) else {
                    continue;
                };
                let slot = (s.key.clone(), s.state);
                if sent.get(&slot).is_some_and(|t| t.elapsed() < REPEAT) {
                    continue;
                }
                sent.insert(slot, Instant::now());
                let body = format!("{} {what}", host_label(&s.key));
                if let Err(e) = send(&app, "sidepaw", &body) {
                    tracing::warn!("notification: {e}");
                }
            }
            seen = live;
        }
    });
}
