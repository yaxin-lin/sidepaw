//! `sidepaw hook --host <host>`: the shim hosts call from a command
//! hook. Reads one JSON payload from stdin, posts it to the daemon and
//! exits 0 regardless, because a failing hook must never block the
//! agent.
//!
//! `sidepaw install --host <host>` prints the snippet the setup wizard
//! writes into the host's config. Printing instead of writing keeps the
//! daemon free of host-config editing; the desktop app owns that.

use crate::home::DaemonState;
use anyhow::{bail, Result};
use sidepaw_core::Host;
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
    let state = match DaemonState::load() {
        Ok(s) => s,
        Err(_) => return Ok(()), // daemon not running; nothing to do
    };
    let url = format!(
        "http://127.0.0.1:{}/v1/native/{}",
        state.port,
        host.as_str()
    );
    let result = ureq::post(&url)
        .set("Authorization", &format!("Bearer {}", state.token))
        .timeout(std::time::Duration::from_millis(800))
        .send_json(payload);
    if let Err(e) = result {
        tracing::warn!("could not reach daemon: {e}");
    }
    Ok(())
}

pub fn install_snippet(host: Host) -> Result<String> {
    let state = DaemonState::load().unwrap_or(DaemonState {
        port: 0,
        token: "<token>".into(),
    });
    let exe = std::env::current_exe()?.display().to_string();
    let snippet = match host {
        Host::ClaudeCode => {
            let events = [
                "SessionStart",
                "UserPromptSubmit",
                "PreToolUse",
                "PostToolUse",
                "PostToolUseFailure",
                "PermissionRequest",
                "PermissionDenied",
                "Stop",
                "StopFailure",
                "SessionEnd",
            ];
            let hook = serde_json::json!([{
                "hooks": [{
                    "type": "http",
                    "url": format!("http://127.0.0.1:{}/v1/native/claude-code", state.port),
                    "headers": { "Authorization": format!("Bearer {}", state.token) },
                    "async": true,
                    "onFailure": "continue"
                }]
            }]);
            let hooks: serde_json::Map<String, serde_json::Value> = events
                .iter()
                .map(|e| (e.to_string(), hook.clone()))
                .collect();
            serde_json::to_string_pretty(&serde_json::json!({ "hooks": hooks }))?
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
