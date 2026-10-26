//! All live sessions and their state machines, plus the combined view
//! the desktop app renders.

use serde::Serialize;
use sidepaw_core::{Event, Kind, Session, State};
use std::collections::HashMap;

#[derive(Default)]
pub struct Registry {
    sessions: HashMap<String, Session>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct Snapshot {
    /// Highest-priority state across sessions, `sleeping` when none.
    pub overall: State,
    pub sessions: Vec<SessionView>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct SessionView {
    pub key: String,
    #[serde(flatten)]
    pub state: State,
    // serde writes u64 as a JSON number, so the TS side must not see bigint.
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub for_ms: u64,
}

impl Registry {
    /// Feed an event. Returns the session key and new state if it changed.
    pub fn apply(&mut self, event: &Event, now_ms: u64) -> Option<(String, State)> {
        let key = event.session_key();
        if event.kind == Kind::SessionEnd {
            self.sessions.remove(&key);
            return Some((key, State::Idle));
        }
        let session = self
            .sessions
            .entry(key.clone())
            .or_insert_with(|| Session::new(now_ms));
        session.apply(event, now_ms).map(|s| (key, s))
    }

    /// Advance decay timers. Returns every change.
    pub fn tick(&mut self, now_ms: u64) -> Vec<(String, State)> {
        self.sessions
            .iter_mut()
            .filter_map(|(k, s)| s.tick(now_ms).map(|st| (k.clone(), st)))
            .collect()
    }

    pub fn snapshot(&self, now_ms: u64) -> Snapshot {
        let mut sessions: Vec<SessionView> = self
            .sessions
            .iter()
            .map(|(k, s)| SessionView {
                key: k.clone(),
                state: s.state(),
                for_ms: s.in_state_for(now_ms),
            })
            .collect();
        sessions.sort_by_key(|s| std::cmp::Reverse(s.state.priority()));
        let overall = sessions.first().map(|s| s.state).unwrap_or(State::Sleeping);
        Snapshot { overall, sessions }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sidepaw_core::{Host, Tool, ToolCategory};

    fn ev(host: Host, session: &str, kind: Kind) -> Event {
        Event {
            v: 1,
            host,
            session: session.into(),
            cwd: None,
            ts: "t".into(),
            kind,
            tool: Some(Tool {
                name: "Bash".into(),
                category: ToolCategory::Shell,
            }),
            text: None,
            error: None,
            tokens: None,
            raw: None,
        }
    }

    #[test]
    fn overall_is_highest_priority() {
        let mut r = Registry::default();
        r.apply(&ev(Host::Codex, "a", Kind::Prompt), 0);
        r.apply(&ev(Host::ClaudeCode, "b", Kind::ApprovalWait), 0);
        let snap = r.snapshot(5);
        assert_eq!(snap.overall, State::Waiting);
        assert_eq!(snap.sessions[0].key, "claude-code:b");
    }

    #[test]
    fn session_end_removes() {
        let mut r = Registry::default();
        r.apply(&ev(Host::Dsh, "x", Kind::Prompt), 0);
        r.apply(&ev(Host::Dsh, "x", Kind::SessionEnd), 1);
        assert!(r.snapshot(2).sessions.is_empty());
        assert_eq!(r.snapshot(2).overall, State::Sleeping);
    }

    #[test]
    fn snapshot_serialises_flat_state() {
        let mut r = Registry::default();
        r.apply(&ev(Host::Dsh, "x", Kind::ToolStart), 0);
        let json = serde_json::to_value(r.snapshot(0)).unwrap();
        assert_eq!(json["sessions"][0]["state"], "tool");
        assert_eq!(json["sessions"][0]["detail"], "shell");
    }
}
