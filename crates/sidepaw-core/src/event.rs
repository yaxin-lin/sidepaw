//! The unified event every host adapter produces.
//! Mirrors `docs/contracts/event.schema.json`; keep the two in sync.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Host {
    ClaudeCode,
    Codex,
    Opencode,
    Dsh,
    Kimi,
    Gemini,
    Cursor,
    Wrapped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    SessionStart,
    Prompt,
    Thinking,
    ToolStart,
    ToolEnd,
    ToolFail,
    ApprovalWait,
    ApprovalDone,
    TurnEnd,
    Error,
    Interrupt,
    SessionEnd,
    Note,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolCategory {
    Shell,
    Edit,
    Read,
    Search,
    Web,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tool {
    pub name: String,
    pub category: ToolCategory,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorInfo {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Tokens {
    #[serde(default, rename = "in")]
    pub input: u64,
    #[serde(default, rename = "out")]
    pub output: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Event {
    pub v: u8,
    pub host: Host,
    pub session: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    /// RFC 3339 timestamp in UTC. Kept as a string on purpose; the core
    /// crate does not depend on a time library.
    pub ts: String,
    pub kind: Kind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool: Option<Tool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokens: Option<Tokens>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw: Option<serde_json::Value>,
}

impl Event {
    /// Session key used by the store and the state machine registry.
    pub fn session_key(&self) -> String {
        format!("{}:{}", self.host.as_str(), self.session)
    }
}

impl Host {
    pub fn as_str(self) -> &'static str {
        match self {
            Host::ClaudeCode => "claude-code",
            Host::Codex => "codex",
            Host::Opencode => "opencode",
            Host::Dsh => "dsh",
            Host::Kimi => "kimi",
            Host::Gemini => "gemini",
            Host::Cursor => "cursor",
            Host::Wrapped => "wrapped",
        }
    }
}

/// Classify a tool by its host-reported name. Shared across adapters
/// because the common hosts converge on the same names; an adapter can
/// still override for host-specific tools.
pub fn classify_tool(name: &str) -> ToolCategory {
    let n = name.to_ascii_lowercase();
    match n.as_str() {
        "bash" | "shell" | "exec" | "terminal" | "run_command" | "tool-bash" => ToolCategory::Shell,
        "edit" | "write" | "multiedit" | "notebookedit" | "apply_patch" | "fs/write" => {
            ToolCategory::Edit
        }
        "read" | "cat" | "fs/read" | "view" => ToolCategory::Read,
        "grep" | "glob" | "search" | "ls" | "find" | "fs/search" => ToolCategory::Search,
        _ if n.contains("web") || n.contains("fetch") || n.contains("browser") => ToolCategory::Web,
        _ => ToolCategory::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_json() {
        let json = r#"{
            "v": 1,
            "host": "claude-code",
            "session": "abc",
            "ts": "2026-10-25T13:04:05.123Z",
            "kind": "tool_start",
            "tool": { "name": "Bash", "category": "shell" },
            "text": "npm install"
        }"#;
        let event: Event = serde_json::from_str(json).unwrap();
        assert_eq!(event.host, Host::ClaudeCode);
        assert_eq!(event.kind, Kind::ToolStart);
        assert_eq!(event.tool.as_ref().unwrap().category, ToolCategory::Shell);
        assert_eq!(event.session_key(), "claude-code:abc");

        let back = serde_json::to_string(&event).unwrap();
        let again: Event = serde_json::from_str(&back).unwrap();
        assert_eq!(event, again);
    }

    #[test]
    fn rejects_unknown_kind() {
        let json = r#"{"v":1,"host":"codex","session":"x","ts":"t","kind":"dance"}"#;
        assert!(serde_json::from_str::<Event>(json).is_err());
    }

    #[test]
    fn classifies_common_tools() {
        assert_eq!(classify_tool("Bash"), ToolCategory::Shell);
        assert_eq!(classify_tool("Edit"), ToolCategory::Edit);
        assert_eq!(classify_tool("WebFetch"), ToolCategory::Web);
        assert_eq!(classify_tool("Grep"), ToolCategory::Search);
        assert_eq!(classify_tool("mcp__weird__thing"), ToolCategory::Other);
    }
}
