//! `sidepaw hook --host <host>`: the shim hosts call from a command
//! hook. Reads one JSON payload from stdin, posts it to the daemon,
//! prints the daemon's answer (a held permission's decision) and exits
//! 0 regardless, because a failing hook must never block the agent nor
//! show up in its transcript.
//!
//! `sidepaw install --host <host>` prints the snippet for the host's
//! config; with `--apply` it merges the snippet in through `install.rs`,
//! which is also what the desktop setup window calls.

use crate::home::DaemonState;
use crate::install::{PERMISSION_TIMEOUT_S, TIMEOUT_S};
use anyhow::{bail, Result};
use serde::Serialize;
use sidepaw_core::{Event, Host};
use std::io::{Read, Write};
use std::time::Duration;

/// How long to wait for the app: a second under the host's own cap, so
/// the shim gives up and exits 0 before the host kills it and reports a
/// timeout. A held permission waits for the user; nothing else should
/// take more than milliseconds.
fn wait_for(payload: &serde_json::Value) -> Duration {
    let held = payload.get("hook_event_name").and_then(|v| v.as_str()) == Some("PermissionRequest");
    Duration::from_secs(
        if held {
            PERMISSION_TIMEOUT_S
        } else {
            TIMEOUT_S
        } - 1,
    )
}

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
    let path = format!("/v1/native/{}", host.as_str());
    if let Some(answer) = post(&path, &payload, wait_for(&payload)) {
        // Stdout is the host's to read; an error here has nowhere to go.
        let _ = std::io::stdout().write_all(answer.as_bytes());
    }
    Ok(())
}

/// Post a unified event; used by `sidepaw run`.
pub fn post_event(event: &Event) {
    post("/v1/events", event, Duration::from_millis(800));
}

/// Best effort: a missing or unreachable app is logged and ignored,
/// because whatever called us must carry on either way. Returns the
/// body of a 200, which only a decided permission gets.
fn post(path: &str, body: &impl Serialize, wait: Duration) -> Option<String> {
    // No state file: the app has never run; nothing to do.
    let state = DaemonState::load().ok()?;
    let url = format!("http://127.0.0.1:{}{path}", state.port);
    // The target is always loopback. ureq would otherwise honour
    // HTTP_PROXY, and a local proxy (Clash, Surge, ...) that refuses or
    // mangles localhost traffic would silently eat every event.
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .proxy(None)
        .timeout_global(Some(wait))
        .build()
        .into();
    let result = agent
        .post(&url)
        .header("Authorization", &format!("Bearer {}", state.token))
        .send_json(body);
    // Debug, not warn: with the app closed every wrapped command and
    // hook would otherwise print this into someone's terminal.
    let mut res = match result {
        Ok(res) => res,
        Err(e) => {
            tracing::debug!("could not reach the app: {e}");
            return None;
        }
    };
    if res.status() != 200 {
        return None;
    }
    res.body_mut().read_to_string().ok()
}

pub fn install_snippet(host: Host) -> Result<String> {
    let exe = crate::install::exe()?;
    let snippet = match host {
        Host::ClaudeCode => {
            let block =
                crate::install::hook_block(host, &exe).expect("claude code has a hook block");
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn only_a_permission_request_waits_long() {
        let held = json!({ "hook_event_name": "PermissionRequest" });
        assert_eq!(
            wait_for(&held),
            Duration::from_secs(PERMISSION_TIMEOUT_S - 1)
        );
        assert_eq!(
            wait_for(&json!({ "hook_event_name": "Stop" })),
            Duration::from_secs(TIMEOUT_S - 1)
        );
        assert_eq!(wait_for(&json!({})), Duration::from_secs(TIMEOUT_S - 1));
    }
}
