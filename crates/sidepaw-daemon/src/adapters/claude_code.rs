//! Claude Code hook payloads. Reference: https://code.claude.com/docs/en/hooks
//!
//! Claude Code supports an `http` hook type, so it posts directly to the
//! daemon without the `sidepaw hook` shim. Every event we care about
//! carries `hook_event_name`, `session_id` and `cwd`.
//!
//! `approval_wait` is observed through `PermissionRequest`, which fires
//! the moment the prompt is shown. `Notification` with
//! `permission_prompt` would also work but arrives several seconds
//! later, so it is ignored to avoid a duplicate. Approval itself has no
//! hook; see the note in `from_hook`.

use super::now_rfc3339;
use serde_json::Value;
use sidepaw_core::event::{classify_tool, ErrorInfo};
use sidepaw_core::{Event, Host, Kind, Tool};

pub fn from_hook(p: &Value) -> Option<Event> {
    let name = p.get("hook_event_name")?.as_str()?;
    let session = p.get("session_id")?.as_str()?.to_owned();
    let tool_name = p.get("tool_name").and_then(Value::as_str);

    let (kind, text, error) = match name {
        "SessionStart" => (Kind::SessionStart, None, None),
        "SessionEnd" => (Kind::SessionEnd, None, None),
        "UserPromptSubmit" => (Kind::Prompt, str_field(p, "prompt"), None),
        "PreToolUse" => (Kind::ToolStart, tool_summary(p), None),
        "PostToolUse" => (Kind::ToolEnd, None, None),
        "PostToolUseFailure" => (
            Kind::ToolFail,
            None,
            Some(ErrorInfo {
                kind: "tool_failure".into(),
                message: str_field(p, "error"),
            }),
        ),
        "PermissionRequest" => (Kind::ApprovalWait, tool_summary(p), None),
        "PermissionDenied" => (Kind::ApprovalDone, None, None),
        "Stop" => (Kind::TurnEnd, str_field(p, "last_assistant_message"), None),
        "StopFailure" => (
            Kind::Error,
            None,
            Some(ErrorInfo {
                kind: str_field(p, "error_type").unwrap_or_else(|| "stop_failure".into()),
                message: str_field(p, "error"),
            }),
        ),
        // Subagents, compaction, config changes and the rest do not move
        // the pet.
        _ => return None,
    };

    // Order on the approve path is PreToolUse, PermissionRequest, the tool
    // runs, PostToolUse. Nothing fires when the user clicks approve, so
    // the state machine treats the following ToolEnd as the answer.
    // PermissionDenied is the only explicit answer Claude Code sends.

    Some(Event {
        v: 1,
        host: Host::ClaudeCode,
        session,
        cwd: str_field(p, "cwd"),
        ts: now_rfc3339(),
        kind,
        tool: tool_name.map(|n| Tool {
            name: n.to_owned(),
            category: classify_tool(n),
        }),
        text,
        error,
        tokens: None,
        raw: Some(p.clone()),
    })
}

fn str_field(p: &Value, key: &str) -> Option<String> {
    p.get(key).and_then(Value::as_str).map(str::to_owned)
}

/// A one-line description of the tool call for the bubble. Only the
/// command for shell tools; file path for edits; nothing else.
fn tool_summary(p: &Value) -> Option<String> {
    let input = p.get("tool_input")?;
    input
        .get("command")
        .or_else(|| input.get("file_path"))
        .or_else(|| input.get("pattern"))
        .and_then(Value::as_str)
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sidepaw_core::ToolCategory;

    fn fixture(name: &str) -> Value {
        let path = format!(
            "{}/tests/fixtures/claude-code/{name}.json",
            env!("CARGO_MANIFEST_DIR")
        );
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
    }

    #[test]
    fn pre_tool_use_maps_to_tool_start() {
        let e = from_hook(&fixture("pre-tool-use")).unwrap();
        assert_eq!(e.kind, Kind::ToolStart);
        assert_eq!(e.session, "abc123");
        let tool = e.tool.unwrap();
        assert_eq!(tool.name, "Bash");
        assert_eq!(tool.category, ToolCategory::Shell);
        assert_eq!(e.text.as_deref(), Some("npm install"));
        assert_eq!(e.cwd.as_deref(), Some("/home/me/project"));
    }

    #[test]
    fn permission_request_maps_to_waiting() {
        let e = from_hook(&fixture("permission-request")).unwrap();
        assert_eq!(e.kind, Kind::ApprovalWait);
    }

    #[test]
    fn stop_carries_last_message() {
        let e = from_hook(&fixture("stop")).unwrap();
        assert_eq!(e.kind, Kind::TurnEnd);
        assert!(e.text.unwrap().starts_with("Done."));
    }

    #[test]
    fn ignores_unrelated_hooks() {
        let v = serde_json::json!({
            "hook_event_name": "PreCompact",
            "session_id": "x",
            "cwd": "/"
        });
        assert!(from_hook(&v).is_none());
    }

    #[test]
    fn missing_session_is_dropped() {
        let v = serde_json::json!({"hook_event_name": "Stop"});
        assert!(from_hook(&v).is_none());
    }
}
