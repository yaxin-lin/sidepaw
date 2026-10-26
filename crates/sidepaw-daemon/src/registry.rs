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
    /// What the pet shows: `Waiting` if any session waits for the user,
    /// otherwise the state of the session the host spoke about most
    /// recently, `Sleeping` when there are none. Recency beats priority
    /// because several terminals are usually one person switching
    /// between them, and an interrupted one must not mask the live one.
    pub overall: State,
    /// Sorted the same way: waiting first, then most recent activity.
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
    /// Milliseconds since the host last reported anything for it.
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub quiet_ms: u64,
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
                quiet_ms: now_ms.saturating_sub(s.last_event_ms()),
            })
            .collect();
        sessions.sort_by_key(|s| (s.state != State::Waiting, s.quiet_ms, s.key.clone()));
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
    fn waiting_anywhere_wins() {
        let mut r = Registry::default();
        r.apply(&ev(Host::ClaudeCode, "b", Kind::ApprovalWait), 0);
        r.apply(&ev(Host::Codex, "a", Kind::Prompt), 10);
        let snap = r.snapshot(15);
        assert_eq!(snap.overall, State::Waiting);
        assert_eq!(snap.sessions[0].key, "claude-code:b");
        assert_eq!(snap.sessions[1].quiet_ms, 5);
    }

    /// Terminal 1 was interrupted mid-thought (no event for that), then
    /// the user finished a turn in terminal 2. The pet follows terminal 2.
    #[test]
    fn most_recent_session_wins_over_a_stale_busy_one() {
        let mut r = Registry::default();
        r.apply(&ev(Host::ClaudeCode, "t1", Kind::Prompt), 0);
        r.apply(&ev(Host::ClaudeCode, "t2", Kind::Prompt), 1_000);
        r.apply(&ev(Host::ClaudeCode, "t2", Kind::TurnEnd), 2_000);
        let snap = r.snapshot(2_500);
        assert_eq!(snap.overall, State::Done);
        assert_eq!(snap.sessions[0].key, "claude-code:t2");
        // Touching terminal 1 again makes it current.
        r.apply(&ev(Host::ClaudeCode, "t1", Kind::ToolStart), 3_000);
        assert_eq!(r.snapshot(3_100).overall, State::Tool(ToolCategory::Shell));
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
