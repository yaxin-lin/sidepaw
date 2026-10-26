//! Writing hook configuration into a host's settings file. Today only
//! Claude Code has a file we can merge into; hosts that need the
//! `sidepaw hook` shim get a snippet to paste instead (see `hook.rs`).
//!
//! Our handlers are recognised by their URL, so install is idempotent
//! and remove only touches what we added. Every write first copies the
//! file to `<name>.sidepaw-backup`.

use anyhow::{bail, Context, Result};
use serde::Serialize;
use serde_json::{json, Map, Value};
use sidepaw_core::Host;
use std::path::{Path, PathBuf};

/// Events the pet reacts to; see `adapters/claude_code.rs`. `SessionStart`
/// is not here because Claude Code only allows `command` and `mcp_tool`
/// handlers on it; the session is created by its first other event.
pub const CLAUDE_CODE_EVENTS: [&str; 9] = [
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

/// Seconds Claude Code waits for our server. It answers in
/// milliseconds; the cap only matters when it is not running.
const TIMEOUT_S: u32 = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum Status {
    /// Every event has our handler with the current port and token.
    Installed,
    /// Our handler is there but points at another port or token.
    Stale,
    /// The settings file exists and has none of our handlers.
    Absent,
    /// No settings file yet; install will create one.
    NoFile,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct Report {
    pub host: Host,
    pub status: Status,
    pub path: String,
    /// What install would merge in, pretty-printed.
    pub preview: String,
}

fn url(port: u16) -> String {
    format!("http://127.0.0.1:{port}/v1/native/claude-code")
}

fn handler(port: u16, token: &str) -> Value {
    json!({
        "type": "http",
        "url": url(port),
        "timeout": TIMEOUT_S,
        "headers": { "Authorization": format!("Bearer {token}") }
    })
}

/// The `{"hooks": {...}}` object for a host, or `None` when the host
/// has no settings file we merge into.
pub fn hook_block(host: Host, port: u16, token: &str) -> Option<Value> {
    match host {
        Host::ClaudeCode => {
            let hooks: Map<String, Value> = CLAUDE_CODE_EVENTS
                .iter()
                .map(|e| (e.to_string(), json!([{ "hooks": [handler(port, token)] }])))
                .collect();
            Some(json!({ "hooks": hooks }))
        }
        _ => None,
    }
}

/// Where the host keeps user-level settings. Claude Code honours
/// `CLAUDE_CONFIG_DIR`.
pub fn settings_path(host: Host) -> Option<PathBuf> {
    match host {
        Host::ClaudeCode => {
            let dir = std::env::var_os("CLAUDE_CONFIG_DIR")
                .map(PathBuf::from)
                .or_else(|| crate::home::user_home().map(|h| h.join(".claude")))?;
            Some(dir.join("settings.json"))
        }
        _ => None,
    }
}

fn is_ours(handler: &Value) -> bool {
    handler.get("type").and_then(Value::as_str) == Some("http")
        && handler.get("url").and_then(Value::as_str).is_some_and(|u| {
            u.starts_with("http://127.0.0.1:") && u.ends_with("/v1/native/claude-code")
        })
}

fn is_current(handler: &Value, port: u16, token: &str) -> bool {
    handler.get("url").and_then(Value::as_str) == Some(url(port).as_str())
        && handler
            .pointer("/headers/Authorization")
            .and_then(Value::as_str)
            == Some(format!("Bearer {token}").as_str())
}

/// Classify a parsed settings document.
pub fn detect(settings: &Value, port: u16, token: &str) -> Status {
    let mut ours = 0;
    let mut current = 0;
    for event in CLAUDE_CODE_EVENTS {
        let handlers = settings
            .pointer(&format!("/hooks/{event}"))
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|group| group.get("hooks").and_then(Value::as_array))
            .flatten()
            .filter(|h| is_ours(h));
        for h in handlers {
            ours += 1;
            if is_current(h, port, token) {
                current += 1;
            }
        }
    }
    if ours == 0 {
        Status::Absent
    } else if current == CLAUDE_CODE_EVENTS.len() && ours == current {
        Status::Installed
    } else {
        Status::Stale
    }
}

/// Remove every handler of ours. Returns whether anything was removed.
pub fn strip(settings: &mut Value) -> bool {
    let Some(hooks) = settings.get_mut("hooks").and_then(Value::as_object_mut) else {
        return false;
    };
    let mut removed = false;
    for groups in hooks.values_mut() {
        let Some(groups) = groups.as_array_mut() else {
            continue;
        };
        for group in groups.iter_mut() {
            if let Some(list) = group.get_mut("hooks").and_then(Value::as_array_mut) {
                let before = list.len();
                list.retain(|h| !is_ours(h));
                removed |= list.len() != before;
            }
        }
        groups.retain(|g| {
            g.get("hooks")
                .and_then(Value::as_array)
                .map_or(true, |l| !l.is_empty())
        });
    }
    hooks.retain(|_, v| v.as_array().map_or(true, |a| !a.is_empty()));
    if hooks.is_empty() {
        settings.as_object_mut().map(|o| o.remove("hooks"));
    }
    removed
}

/// Replace our handlers with `block`'s, leaving everything else as is.
pub fn merge(settings: &mut Value, block: &Value) {
    strip(settings);
    if !settings.is_object() {
        *settings = json!({});
    }
    let root = settings.as_object_mut().expect("object");
    let hooks = root.entry("hooks").or_insert_with(|| json!({}));
    if !hooks.is_object() {
        *hooks = json!({});
    }
    let hooks = hooks.as_object_mut().expect("object");
    let Some(ours) = block.get("hooks").and_then(Value::as_object) else {
        return;
    };
    for (event, groups) in ours {
        let target = hooks.entry(event.clone()).or_insert_with(|| json!([]));
        if !target.is_array() {
            *target = json!([]);
        }
        let target = target.as_array_mut().expect("array");
        target.extend(groups.as_array().into_iter().flatten().cloned());
    }
}

fn read(path: &Path) -> Result<Option<Value>> {
    match std::fs::read_to_string(path) {
        Ok(text) => {
            let v: Value = serde_json::from_str(&text)
                .with_context(|| format!("{} is not valid JSON", path.display()))?;
            Ok(Some(v))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e).with_context(|| format!("read {}", path.display())),
    }
}

fn write(path: &Path, settings: &Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if path.exists() {
        let backup = path.with_extension("json.sidepaw-backup");
        std::fs::copy(path, &backup).with_context(|| format!("backup to {}", backup.display()))?;
    }
    let mut text = serde_json::to_string_pretty(settings)?;
    text.push('\n');
    std::fs::write(path, text).with_context(|| format!("write {}", path.display()))
}

fn report(host: Host, path: &Path, status: Status, block: &Value) -> Report {
    Report {
        host,
        status,
        path: path.display().to_string(),
        preview: serde_json::to_string_pretty(block).unwrap_or_default(),
    }
}

fn target(host: Host, port: u16, token: &str) -> Result<(PathBuf, Value)> {
    let Some(path) = settings_path(host) else {
        bail!("{} has no settings file to merge into", host.as_str());
    };
    let Some(block) = hook_block(host, port, token) else {
        bail!("{} has no settings file to merge into", host.as_str());
    };
    Ok((path, block))
}

pub fn status(host: Host, port: u16, token: &str) -> Result<Report> {
    let (path, block) = target(host, port, token)?;
    let status = match read(&path)? {
        Some(settings) => detect(&settings, port, token),
        None => Status::NoFile,
    };
    Ok(report(host, &path, status, &block))
}

pub fn apply(host: Host, port: u16, token: &str) -> Result<Report> {
    let (path, block) = target(host, port, token)?;
    let mut settings = read(&path)?.unwrap_or_else(|| json!({}));
    merge(&mut settings, &block);
    write(&path, &settings)?;
    Ok(report(host, &path, detect(&settings, port, token), &block))
}

pub fn remove(host: Host, port: u16, token: &str) -> Result<Report> {
    let (path, block) = target(host, port, token)?;
    let status = match read(&path)? {
        Some(mut settings) => {
            if strip(&mut settings) {
                write(&path, &settings)?;
            }
            Status::Absent
        }
        None => Status::NoFile,
    };
    Ok(report(host, &path, status, &block))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block() -> Value {
        hook_block(Host::ClaudeCode, 4000, "tok").unwrap()
    }

    #[test]
    fn block_covers_every_event_with_a_plain_http_handler() {
        let b = block();
        for e in CLAUDE_CODE_EVENTS {
            let h = &b["hooks"][e][0]["hooks"][0];
            assert_eq!(h["type"], "http");
            assert_eq!(h["url"], "http://127.0.0.1:4000/v1/native/claude-code");
            assert_eq!(h["headers"]["Authorization"], "Bearer tok");
            assert!(h.get("async").is_none(), "async is command-only");
        }
    }

    #[test]
    fn merge_keeps_user_content_and_is_idempotent() {
        let mut s = json!({
            "model": "opus",
            "hooks": {
                "PreToolUse": [{ "matcher": "Bash", "hooks": [{ "type": "command", "command": "lint" }] }]
            }
        });
        merge(&mut s, &block());
        merge(&mut s, &block());
        assert_eq!(s["model"], "opus");
        let pre = s["hooks"]["PreToolUse"].as_array().unwrap();
        assert_eq!(pre.len(), 2, "user group plus one of ours, not two of ours");
        assert_eq!(pre[0]["hooks"][0]["command"], "lint");
        assert_eq!(detect(&s, 4000, "tok"), Status::Installed);
    }

    #[test]
    fn stale_when_port_or_token_changed() {
        let mut s = json!({});
        merge(&mut s, &block());
        assert_eq!(detect(&s, 4001, "tok"), Status::Stale);
        assert_eq!(detect(&s, 4000, "other"), Status::Stale);
        assert_eq!(detect(&json!({"hooks": {}}), 4000, "tok"), Status::Absent);
    }

    #[test]
    fn strip_removes_only_ours() {
        let mut s = json!({
            "hooks": {
                "Stop": [{ "hooks": [{ "type": "command", "command": "say done" }] }]
            }
        });
        merge(&mut s, &block());
        assert!(strip(&mut s));
        assert_eq!(s["hooks"]["Stop"].as_array().unwrap().len(), 1);
        assert!(s["hooks"].get("PreToolUse").is_none());
        assert!(!strip(&mut s));
    }

    #[test]
    fn strip_removes_empty_hooks_key() {
        let mut s = json!({ "theme": "dark" });
        merge(&mut s, &block());
        strip(&mut s);
        assert_eq!(s, json!({ "theme": "dark" }));
    }

    #[test]
    fn merge_into_non_object_settings() {
        let mut s = json!([1, 2]);
        merge(&mut s, &block());
        assert_eq!(detect(&s, 4000, "tok"), Status::Installed);
    }

    #[test]
    fn files_round_trip_with_backup() {
        let dir = std::env::temp_dir().join(format!("sidepaw-install-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        std::fs::write(&path, "{\n  \"theme\": \"dark\"\n}\n").unwrap();

        let mut s = read(&path).unwrap().unwrap();
        merge(&mut s, &block());
        write(&path, &s).unwrap();
        assert!(
            dir.join("settings.sidepaw-backup").exists()
                || path.with_extension("json.sidepaw-backup").exists()
        );
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(
            text.starts_with("{\n  \"theme\": \"dark\""),
            "key order preserved: {text}"
        );
        assert!(text.ends_with('\n'));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
