//! `sidepaw hook --host <host>`: the shim hosts call from a command
//! hook. Reads one JSON payload from stdin, posts it to the daemon and
//! exits 0 regardless, because a failing hook must never block the
//! agent.
//!
//! `sidepaw install --host <host>` prints the snippet for the host's
//! config; with `--apply` it merges the snippet in through `install.rs`,
//! which is also what the desktop setup window calls.

use crate::home::DaemonState;
use anyhow::{bail, Result};
use serde::Serialize;
use sidepaw_core::{Event, Host};
use std::io::Read;

pub fn forward_stdin(host: Host) -> Result<()> {
    let mut body = String::new();
    std::io::stdin().read_to_string(&mut body)?;
    let payload: serde_json::Value = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!("hook payload is not JSON: {e}");
            return Ok(());
        }
    };
    post(&format!("/v1/native/{}", host.as_str()), &payload);
    Ok(())
}

/// Post a unified event; used by `sidepaw run`.
pub fn post_event(event: &Event) {
    post("/v1/events", event);
}

/// Best effort: a missing or unreachable app is logged and ignored,
/// because whatever called us must carry on either way.
fn post(path: &str, body: &impl Serialize) {
    let state = match DaemonState::load() {
        Ok(s) => s,
        Err(_) => return, // the app is not running; nothing to do
    };
    let url = format!("http://127.0.0.1:{}{path}", state.port);
    // The target is always loopback. ureq would otherwise honour
    // HTTP_PROXY, and a local proxy (Clash, Surge, ...) that refuses or
    // mangles localhost traffic would silently eat every event.
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .proxy(None)
        .timeout_global(Some(std::time::Duration::from_millis(800)))
        .build()
        .into();
    let result = agent
        .post(&url)
        .header("Authorization", &format!("Bearer {}", state.token))
        .send_json(body);
    // Debug, not warn: with the app closed every wrapped command and
    // hook would otherwise print this into someone's terminal.
    if let Err(e) = result {
        tracing::debug!("could not reach the app: {e}");
    }
}

pub fn install_snippet(host: Host) -> Result<String> {
    let state = DaemonState::load().unwrap_or(DaemonState {
        port: 0,
        token: "<token>".into(),
    });
    let exe = std::env::current_exe()?.display().to_string();
    let snippet = match host {
        Host::ClaudeCode => {
            let block = crate::install::hook_block(host, state.port, &state.token)
                .expect("claude code has a hook block");
            serde_json::to_string_pretty(&block)?
        }
        Host::Codex | Host::Kimi | Host::Gemini | Host::Cursor => {
            format!(
                "# command hook for {}: pipe the payload into\n{exe} hook --host {}",
                host.as_str(),
                host.as_str()
            )
        }
        Host::Opencode => {
            "# opencode: nothing to install, the daemon subscribes to GET /event".into()
        }
        Host::Dsh => "# dsh: nothing to install, the daemon tails ~/.dsh/sessions".into(),
        Host::Wrapped => bail!("wrapped commands use `sidepaw run -- <cmd>`"),
    };
    Ok(snippet)
}
