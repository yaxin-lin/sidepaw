//! Global shortcuts for answering permission prompts held by the pet
//! (`[keys]`). They are registered when the first prompt arrives and
//! released when the last one is answered or times out, so outside
//! those seconds the key combinations belong to other apps again. A
//! keyboard answer goes to the oldest prompt, the one the bubble shows.

use serde::Serialize;
use sidepaw_daemon::server::Running;
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutEvent, ShortcutState};

const POLL: Duration = Duration::from_millis(200);

/// What the bubble shows next to its buttons; absent while the keys are
/// not registered (none configured, or another app holds them).
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Labels {
    pub allow: Option<String>,
    pub deny: Option<String>,
}

#[derive(Default)]
pub struct Active {
    allow: Mutex<Option<Shortcut>>,
    deny: Mutex<Option<Shortcut>>,
    labels: Mutex<Labels>,
}

impl Active {
    pub fn labels(&self) -> Labels {
        self.labels.lock().unwrap().clone()
    }
}

/// The plugin's handler: a press of either key answers the oldest
/// prompt.
pub fn on_shortcut(app: &AppHandle, shortcut: &Shortcut, event: ShortcutEvent) {
    if event.state() != ShortcutState::Pressed {
        return;
    }
    let active = app.state::<Active>();
    let allow = if active.allow.lock().unwrap().as_ref() == Some(shortcut) {
        true
    } else if active.deny.lock().unwrap().as_ref() == Some(shortcut) {
        false
    } else {
        return;
    };
    let running = app.state::<Running>();
    if let Some((id, _)) = running.shared.first_pending() {
        if let Err(e) = running.shared.decide(id, allow) {
            tracing::warn!("answer from keyboard: {e:#}");
        }
    }
}

pub fn spawn_watcher(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut registered = false;
        loop {
            tokio::time::sleep(POLL).await;
            let waiting = app.state::<Running>().shared.first_pending().is_some();
            if waiting && !registered {
                register(&app);
                registered = true;
            } else if !waiting && registered {
                unregister(&app);
                registered = false;
            }
        }
    });
}

fn register(app: &AppHandle) {
    let keys = app.state::<Running>().shared.keys();
    let active = app.state::<Active>();
    let mut labels = Labels::default();
    for (spec, slot, label) in [
        (&keys.allow, &active.allow, &mut labels.allow),
        (&keys.deny, &active.deny, &mut labels.deny),
    ] {
        if spec.trim().is_empty() {
            continue;
        }
        let shortcut = match spec.parse::<Shortcut>() {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!("[keys] {spec:?} is not a shortcut: {e}");
                continue;
            }
        };
        match app.global_shortcut().register(shortcut) {
            Ok(()) => {
                *slot.lock().unwrap() = Some(shortcut);
                *label = Some(label_for(spec));
            }
            Err(e) => tracing::warn!("{spec} is taken: {e}"),
        }
    }
    *active.labels.lock().unwrap() = labels;
}

fn unregister(app: &AppHandle) {
    let active = app.state::<Active>();
    for slot in [&active.allow, &active.deny] {
        if let Some(s) = slot.lock().unwrap().take() {
            let _ = app.global_shortcut().unregister(s);
        }
    }
    *active.labels.lock().unwrap() = Labels::default();
}

/// `Control+Alt+Y` as the platform writes it: `⌃⌥Y` on macOS,
/// `Ctrl+Alt+Y` elsewhere.
pub fn label_for(spec: &str) -> String {
    let mac = cfg!(target_os = "macos");
    let parts: Vec<String> = spec
        .split('+')
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
        .map(|p| {
            let upper = p.to_ascii_uppercase();
            let name = match (upper.as_str(), mac) {
                ("CONTROL" | "CTRL", true) => "⌃",
                ("CONTROL" | "CTRL", false) => "Ctrl",
                ("ALT" | "OPTION", true) => "⌥",
                ("ALT" | "OPTION", false) => "Alt",
                ("SHIFT", true) => "⇧",
                ("SHIFT", false) => "Shift",
                ("SUPER" | "COMMAND" | "CMD", true) => "⌘",
                ("SUPER" | "COMMAND" | "CMD", false) => "Win",
                ("COMMANDORCONTROL" | "CMDORCTRL" | "COMMANDORCTRL" | "CMDORCONTROL", true) => "⌘",
                ("COMMANDORCONTROL" | "CMDORCTRL" | "COMMANDORCTRL" | "CMDORCONTROL", false) => {
                    "Ctrl"
                }
                _ => return upper.trim_start_matches("KEY").to_owned(),
            };
            name.to_owned()
        })
        .collect();
    parts.join(if mac { "" } else { "+" })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_follow_the_platform() {
        let l = label_for("Control+Alt+Y");
        if cfg!(target_os = "macos") {
            assert_eq!(l, "⌃⌥Y");
            assert_eq!(label_for("CmdOrCtrl+Shift+KeyN"), "⌘⇧N");
        } else {
            assert_eq!(l, "Ctrl+Alt+Y");
            assert_eq!(label_for("CmdOrCtrl+Shift+KeyN"), "Ctrl+Shift+N");
        }
    }

    #[test]
    fn default_keys_parse() {
        let keys = sidepaw_daemon::config::Keys::default();
        assert!(keys.allow.parse::<Shortcut>().is_ok());
        assert!(keys.deny.parse::<Shortcut>().is_ok());
    }
}
