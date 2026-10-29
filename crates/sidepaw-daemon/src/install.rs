//! Writing hook configuration into a host's settings file. Today only
//! Claude Code has a file we can merge into; hosts that need the
//! `sidepaw hook` shim get a snippet to paste instead (see `hook.rs`).
//!
//! Claude Code's handlers are command hooks that run this program as
//! `sidepaw hook --host claude-code` (ADR 9): the shim reads the port
//! and token from `state.json` and exits 0 when the app is closed, so
//! a closed pet costs the agent nothing and the transcript stays clean.
//! Our handlers are recognised by those arguments (and by the URL of
//! the HTTP handlers earlier versions wrote), so install is idempotent
//! and remove only touches what we added. Every write first copies the
//! file to `<name>.sidepaw-backup`.

use anyhow::{bail, Context, Result};
use serde::Serialize;
use serde_json::{json, Map, Value};
use sidepaw_core::Host;
use std::path::{Path, PathBuf};

/// Events the pet reacts to; see `adapters/claude_code.rs`.
pub const CLAUDE_CODE_EVENTS: [&str; 10] = [
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

// Seconds Claude Code waits for the shim. The server answers in
// milliseconds and the shim gives up sooner (`hook::wait_for`), so the
// cap is only a backstop. PermissionRequest is the exception: with
// `act` on, the server holds it open for the user, so its cap has to
// cover the longest act window anyone would configure.
pub(crate) const TIMEOUT_S: u64 = 5;
pub(crate) const PERMISSION_TIMEOUT_S: u64 = 60;

const SHIM_ARGS: [&str; 3] = ["hook", "--host", "claude-code"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum Status {
    /// Every event has our handler, running this program.
    Installed,
    /// Our handler is there but runs another copy of the program, or is
    /// the HTTP handler earlier versions wrote.
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

/// This program, which the hooks run. The desktop app and the `sidepaw`
/// binary both answer `hook`; whichever wrote the hooks last is used.
pub fn exe() -> Result<String> {
    let exe = std::env::current_exe()
        .context("cannot find this program's path")?
        .display()
        .to_string();
    check_lasting(&exe)?;
    Ok(exe)
}

/// macOS runs an unsigned app opened from where it was downloaded from
/// a random read-only copy that is gone after a restart; hooks pointing
/// there would fail on every event.
fn check_lasting(exe: &str) -> Result<()> {
    if exe.contains("/AppTranslocation/") {
        bail!("sidepaw is running from a temporary copy macOS made; move it to Applications and open it again");
    }
    Ok(())
}

/// Exec form (`args` set): Claude Code spawns `exe` directly, with no
/// shell to quote a path for, on Windows without Git Bash as well.
fn handler(event: &str, exe: &str) -> Value {
    let timeout = if event == "PermissionRequest" {
        PERMISSION_TIMEOUT_S
    } else {
        TIMEOUT_S
    };
    json!({
        "type": "command",
        "command": exe,
        "args": SHIM_ARGS,
        "timeout": timeout,
    })
}

/// The `{"hooks": {...}}` object for a host, or `None` when the host
/// has no settings file we merge into.
pub fn hook_block(host: Host, exe: &str) -> Option<Value> {
    match host {
        Host::ClaudeCode => {
            let hooks: Map<String, Value> = CLAUDE_CODE_EVENTS
                .iter()
                .map(|e| (e.to_string(), json!([{ "hooks": [handler(e, exe)] }])))
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

/// Whether the host looks installed: its settings folder exists or its
/// command is on PATH. Only used to word the welcome window; install
/// works either way.
pub fn host_found(host: Host) -> bool {
    let (dir, cmd) = match host {
        Host::ClaudeCode => (
            settings_path(host).and_then(|p| p.parent().map(Path::to_path_buf)),
            "claude",
        ),
        _ => return false,
    };
    dir.is_some_and(|d| d.is_dir()) || on_path(cmd)
}

fn on_path(cmd: &str) -> bool {
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&path)
        .any(|d| d.join(cmd).is_file() || (cfg!(windows) && d.join(format!("{cmd}.exe")).is_file()))
}

fn is_ours(handler: &Value) -> bool {
    match handler.get("type").and_then(Value::as_str) {
        Some("command") => handler.get("args") == Some(&json!(SHIM_ARGS)),
        // What versions before ADR 9 wrote; still ours to replace.
        Some("http") => handler.get("url").and_then(Value::as_str).is_some_and(|u| {
            u.starts_with("http://127.0.0.1:") && u.ends_with("/v1/native/claude-code")
        }),
        _ => false,
    }
}

/// Current means identical to what install would write now, so a
/// changed timeout or path also reads as stale.
fn is_current(event: &str, found: &Value, exe: &str) -> bool {
    *found == handler(event, exe)
}

/// Classify a parsed settings document.
pub fn detect(settings: &Value, exe: &str) -> Status {
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
            if is_current(event, h, exe) {
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
                .is_none_or(|l| !l.is_empty())
        });
    }
    hooks.retain(|_, v| v.as_array().is_none_or(|a| !a.is_empty()));
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

fn target(host: Host, exe: &str) -> Result<(PathBuf, Value)> {
    let Some(path) = settings_path(host) else {
        bail!("{} has no settings file to merge into", host.as_str());
    };
    let Some(block) = hook_block(host, exe) else {
        bail!("{} has no settings file to merge into", host.as_str());
    };
    Ok((path, block))
}

pub fn status(host: Host) -> Result<Report> {
    let exe = exe()?;
    let (path, block) = target(host, &exe)?;
    let status = match read(&path)? {
        Some(settings) => detect(&settings, &exe),
        None => Status::NoFile,
    };
    Ok(report(host, &path, status, &block))
}

/// Nothing is hooked up yet: the desktop app shows the welcome window
/// instead of a pet that never moves. Only Claude Code counts until
/// another host can be installed from the app.
pub fn is_first_run() -> bool {
    status(Host::ClaudeCode)
        .map(|r| matches!(r.status, Status::Absent | Status::NoFile))
        .unwrap_or(false)
}

/// Hooks we installed earlier that run another copy of the program, or
/// are the old HTTP handlers, are ours to fix: rewrite them. Returns
/// the report when something was rewritten. Hosts without a settings
/// file or without our hooks are left alone; that is the user's call.
pub fn refresh_stale(host: Host) -> Result<Option<Report>> {
    match status(host)?.status {
        Status::Stale => apply(host).map(Some),
        _ => Ok(None),
    }
}

pub fn apply(host: Host) -> Result<Report> {
    let exe = exe()?;
    let (path, block) = target(host, &exe)?;
    let mut settings = read(&path)?.unwrap_or_else(|| json!({}));
    merge(&mut settings, &block);
    write(&path, &settings)?;
    Ok(report(host, &path, detect(&settings, &exe), &block))
}

pub fn remove(host: Host) -> Result<Report> {
    let (path, block) = target(host, &exe()?)?;
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

    const EXE: &str = "/Applications/sidepaw.app/Contents/MacOS/sidepaw-desktop";

    fn block() -> Value {
        hook_block(Host::ClaudeCode, EXE).unwrap()
    }

    #[test]
    fn block_covers_every_event_with_an_exec_form_shim() {
        let b = block();
        for e in CLAUDE_CODE_EVENTS {
            let h = &b["hooks"][e][0]["hooks"][0];
            assert_eq!(h["type"], "command");
            assert_eq!(h["command"], EXE);
            assert_eq!(h["args"], json!(["hook", "--host", "claude-code"]));
            assert!(
                h.get("async").is_none(),
                "a held permission needs the answer"
            );
            let expected = if e == "PermissionRequest" { 60 } else { 5 };
            assert_eq!(h["timeout"], expected, "{e}");
        }
        assert!(
            b.get("env").is_none(),
            "the shim never goes through a proxy"
        );
    }

    #[test]
    fn old_http_handlers_are_ours_and_stale() {
        let old = json!({
            "type": "http",
            "url": "http://127.0.0.1:59785/v1/native/claude-code",
            "timeout": 5,
            "headers": { "Authorization": "Bearer tok" }
        });
        let mut s = json!({ "hooks": { "Stop": [{ "hooks": [old] }] } });
        assert_eq!(detect(&s, EXE), Status::Stale);
        merge(&mut s, &block());
        assert_eq!(detect(&s, EXE), Status::Installed);
        let stop = s["hooks"]["Stop"].as_array().unwrap();
        assert_eq!(stop.len(), 1, "the HTTP handler was replaced, not kept");
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
        assert_eq!(detect(&s, EXE), Status::Installed);
    }

    #[test]
    fn stale_when_another_copy_wrote_them() {
        let mut s = json!({});
        merge(&mut s, &block());
        assert_eq!(
            detect(&s, "/tmp/target/debug/sidepaw-desktop"),
            Status::Stale
        );
        s["hooks"]["PermissionRequest"][0]["hooks"][0]["timeout"] = json!(5);
        assert_eq!(
            detect(&s, EXE),
            Status::Stale,
            "an older handler shape is stale"
        );
        assert_eq!(detect(&json!({"hooks": {}}), EXE), Status::Absent);
    }

    #[test]
    fn a_translocated_copy_is_refused() {
        assert!(check_lasting(EXE).is_ok());
        let temp = "/private/var/folders/x/T/AppTranslocation/1F2E/d/sidepaw.app/Contents/MacOS/sidepaw-desktop";
        assert!(check_lasting(temp).is_err());
    }

    #[test]
    fn a_users_own_command_hook_is_not_ours() {
        let mine = json!({ "type": "command", "command": EXE, "args": ["pack", "x.zip"] });
        assert!(!is_ours(&mine));
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
        let mut s = json!({ "theme": "dark", "env": { "NO_PROXY": "127.0.0.1,localhost" } });
        merge(&mut s, &block());
        strip(&mut s);
        assert_eq!(
            s,
            json!({ "theme": "dark", "env": { "NO_PROXY": "127.0.0.1,localhost" } }),
            "an env an earlier version added stays; it is harmless"
        );
    }

    #[test]
    fn merge_into_non_object_settings() {
        let mut s = json!([1, 2]);
        merge(&mut s, &block());
        assert_eq!(detect(&s, EXE), Status::Installed);
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
        assert_eq!(
            std::fs::read_to_string(dir.join("settings.json.sidepaw-backup")).unwrap(),
            "{\n  \"theme\": \"dark\"\n}\n",
            "the backup is the file as it was"
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
