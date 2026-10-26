//! Per-session state machine. See the "State machine" section of
//! `docs/architecture.md` for the rules this implements.
//!
//! `Error` survives `tool_end` (the failed call's siblings finishing is
//! not progress) but yields to any new activity: `tool_start`, `prompt`,
//! `thinking` or `turn_end`.
//!
//! Time is passed in as milliseconds so the machine is deterministic in
//! tests and the daemon decides where "now" comes from.

use crate::event::{Event, Kind, ToolCategory};
use serde::{Deserialize, Serialize};

/// How long `Done` is shown before falling back to `Idle`.
pub const DONE_MS: u64 = 10_000;
/// How long `Idle` lasts without events before `Sleeping`.
pub const IDLE_MS: u64 = 120_000;
/// Hosts send nothing when the user interrupts a turn or closes a
/// terminal mid-run, so busy states must expire on their own or one
/// abandoned session keeps the pet busy forever. `Waiting` never
/// expires: a permission prompt really can sit there all afternoon.
pub const THINKING_STALE_MS: u64 = 180_000;
/// Long enough for a maximal Bash call (10 min) plus slack.
pub const TOOL_STALE_MS: u64 = 900_000;
pub const ERROR_STALE_MS: u64 = 120_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "state", content = "detail")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum State {
    Sleeping,
    Idle,
    Done,
    Thinking,
    Tool(ToolCategory),
    Error,
    Waiting,
}

impl State {
    /// Higher wins when the renderer has to pick one state for several
    /// sessions.
    pub fn priority(self) -> u8 {
        match self {
            State::Sleeping => 0,
            State::Idle => 1,
            State::Done => 2,
            State::Thinking => 3,
            State::Tool(_) => 4,
            State::Error => 5,
            State::Waiting => 6,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Session {
    state: State,
    since_ms: u64,
    last_event_ms: u64,
    open_tools: u32,
    last_tool: Option<ToolCategory>,
    waiting: bool,
}

impl Session {
    pub fn new(now_ms: u64) -> Self {
        Session {
            state: State::Idle,
            since_ms: now_ms,
            last_event_ms: now_ms,
            open_tools: 0,
            last_tool: None,
            waiting: false,
        }
    }

    pub fn state(&self) -> State {
        self.state
    }

    /// Milliseconds the session has been in its current state.
    pub fn in_state_for(&self, now_ms: u64) -> u64 {
        now_ms.saturating_sub(self.since_ms)
    }

    /// When the host last said anything about this session.
    pub fn last_event_ms(&self) -> u64 {
        self.last_event_ms
    }

    /// Apply an event. Returns the new state when it changed.
    pub fn apply(&mut self, event: &Event, now_ms: u64) -> Option<State> {
        self.last_event_ms = now_ms;
        let next = match event.kind {
            Kind::SessionStart | Kind::Interrupt | Kind::SessionEnd => {
                self.open_tools = 0;
                self.waiting = false;
                State::Idle
            }
            Kind::Prompt | Kind::Thinking => {
                self.waiting = false;
                State::Thinking
            }
            Kind::ToolStart => {
                self.open_tools += 1;
                self.last_tool = event.tool.as_ref().map(|t| t.category);
                if self.waiting {
                    State::Waiting
                } else {
                    State::Tool(self.last_tool.unwrap_or(ToolCategory::Other))
                }
            }
            Kind::ToolEnd => {
                self.open_tools = self.open_tools.saturating_sub(1);
                // A permission prompt blocks the tool it guards, so a tool
                // finishing means the user answered. Hosts that emit an
                // explicit `approval_done` (Codex, OpenCode) clear it earlier;
                // Claude Code does not, so this is the only approve-path exit.
                self.waiting = false;
                if self.state == State::Error {
                    State::Error
                } else {
                    self.resume()
                }
            }
            Kind::ToolFail | Kind::Error => {
                self.open_tools = self.open_tools.saturating_sub(1);
                self.waiting = false;
                State::Error
            }
            Kind::ApprovalWait => {
                self.waiting = true;
                State::Waiting
            }
            Kind::ApprovalDone => {
                self.waiting = false;
                self.resume()
            }
            Kind::TurnEnd => {
                self.open_tools = 0;
                self.waiting = false;
                State::Done
            }
            Kind::Note => return None,
        };
        self.transition(next, now_ms)
    }

    /// Advance time without an event: `Done` and `Idle` decay, and busy
    /// states expire when the host has gone silent.
    pub fn tick(&mut self, now_ms: u64) -> Option<State> {
        let elapsed = self.in_state_for(now_ms);
        let expired = match self.state {
            State::Done => elapsed >= DONE_MS,
            State::Thinking => elapsed >= THINKING_STALE_MS,
            State::Tool(_) => elapsed >= TOOL_STALE_MS,
            State::Error => elapsed >= ERROR_STALE_MS,
            State::Idle => {
                return if elapsed >= IDLE_MS {
                    self.transition(State::Sleeping, now_ms)
                } else {
                    None
                };
            }
            State::Waiting | State::Sleeping => false,
        };
        if !expired {
            return None;
        }
        self.open_tools = 0;
        self.waiting = false;
        self.transition(State::Idle, now_ms)
    }

    fn resume(&self) -> State {
        if self.open_tools > 0 {
            State::Tool(self.last_tool.unwrap_or(ToolCategory::Other))
        } else {
            State::Thinking
        }
    }

    fn transition(&mut self, next: State, now_ms: u64) -> Option<State> {
        if next == self.state {
            return None;
        }
        self.state = next;
        self.since_ms = now_ms;
        Some(next)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{Host, Tool};

    fn ev(kind: Kind) -> Event {
        Event {
            v: 1,
            host: Host::ClaudeCode,
            session: "s".into(),
            cwd: None,
            ts: "t".into(),
            kind,
            tool: None,
            text: None,
            error: None,
            tokens: None,
            raw: None,
        }
    }

    fn tool(kind: Kind, category: ToolCategory) -> Event {
        let mut e = ev(kind);
        e.tool = Some(Tool {
            name: "x".into(),
            category,
        });
        e
    }

    #[test]
    fn ordinary_turn() {
        let mut s = Session::new(0);
        assert_eq!(s.apply(&ev(Kind::Prompt), 1), Some(State::Thinking));
        assert_eq!(
            s.apply(&tool(Kind::ToolStart, ToolCategory::Shell), 2),
            Some(State::Tool(ToolCategory::Shell))
        );
        assert_eq!(s.apply(&ev(Kind::ToolEnd), 3), Some(State::Thinking));
        assert_eq!(s.apply(&ev(Kind::TurnEnd), 4), Some(State::Done));
    }

    #[test]
    fn explicit_approval_resumes() {
        let mut s = Session::new(0);
        s.apply(&ev(Kind::Prompt), 1);
        s.apply(&tool(Kind::ToolStart, ToolCategory::Shell), 2);
        assert_eq!(s.apply(&ev(Kind::ApprovalWait), 3), Some(State::Waiting));
        assert_eq!(
            s.apply(&ev(Kind::ApprovalDone), 4),
            Some(State::Tool(ToolCategory::Shell))
        );
    }

    /// Claude Code order: PreToolUse, PermissionRequest, the tool runs,
    /// PostToolUse. There is no approval event on the approve path, so
    /// the tool finishing must clear Waiting or the pet is stuck until
    /// the turn ends.
    #[test]
    fn tool_finishing_clears_waiting() {
        let mut s = Session::new(0);
        s.apply(&ev(Kind::Prompt), 1);
        s.apply(&tool(Kind::ToolStart, ToolCategory::Shell), 2);
        assert_eq!(s.apply(&ev(Kind::ApprovalWait), 3), Some(State::Waiting));
        assert_eq!(s.apply(&ev(Kind::ToolEnd), 4), Some(State::Thinking));
        assert_eq!(
            s.apply(&tool(Kind::ToolStart, ToolCategory::Edit), 5),
            Some(State::Tool(ToolCategory::Edit))
        );
    }

    #[test]
    fn tool_failing_clears_waiting() {
        let mut s = Session::new(0);
        s.apply(&tool(Kind::ToolStart, ToolCategory::Shell), 1);
        s.apply(&ev(Kind::ApprovalWait), 2);
        assert_eq!(s.apply(&ev(Kind::ToolFail), 3), Some(State::Error));
        assert_eq!(s.apply(&ev(Kind::Thinking), 4), Some(State::Thinking));
    }

    #[test]
    fn new_tool_while_waiting_stays_waiting() {
        let mut s = Session::new(0);
        s.apply(&tool(Kind::ToolStart, ToolCategory::Shell), 1);
        s.apply(&ev(Kind::ApprovalWait), 2);
        assert_eq!(s.apply(&tool(Kind::ToolStart, ToolCategory::Read), 3), None);
        assert_eq!(s.state(), State::Waiting);
    }

    #[test]
    fn error_is_sticky_until_progress() {
        let mut s = Session::new(0);
        s.apply(&ev(Kind::Prompt), 1);
        s.apply(&tool(Kind::ToolStart, ToolCategory::Edit), 2);
        assert_eq!(s.apply(&ev(Kind::ToolFail), 3), Some(State::Error));
        assert_eq!(s.apply(&ev(Kind::ToolEnd), 4), None);
        assert_eq!(s.state(), State::Error);
        assert_eq!(s.apply(&ev(Kind::Thinking), 5), Some(State::Thinking));
    }

    #[test]
    fn done_decays_then_sleeps() {
        let mut s = Session::new(0);
        s.apply(&ev(Kind::TurnEnd), 0);
        assert_eq!(s.tick(DONE_MS - 1), None);
        assert_eq!(s.tick(DONE_MS), Some(State::Idle));
        assert_eq!(s.tick(DONE_MS + IDLE_MS - 1), None);
        assert_eq!(s.tick(DONE_MS + IDLE_MS), Some(State::Sleeping));
    }

    #[test]
    fn abandoned_busy_states_expire_but_waiting_does_not() {
        let mut s = Session::new(0);
        s.apply(&ev(Kind::Prompt), 0);
        assert_eq!(s.tick(THINKING_STALE_MS - 1), None);
        assert_eq!(s.tick(THINKING_STALE_MS), Some(State::Idle));

        let mut s = Session::new(0);
        s.apply(&tool(Kind::ToolStart, ToolCategory::Shell), 0);
        assert_eq!(s.tick(THINKING_STALE_MS), None, "tools get longer");
        assert_eq!(s.tick(TOOL_STALE_MS), Some(State::Idle));
        // The expired tool is forgotten: the next end does not resume it.
        assert_eq!(
            s.apply(&ev(Kind::ToolEnd), TOOL_STALE_MS + 1),
            Some(State::Thinking)
        );

        let mut s = Session::new(0);
        s.apply(&ev(Kind::ToolFail), 0);
        assert_eq!(s.tick(ERROR_STALE_MS), Some(State::Idle));

        let mut s = Session::new(0);
        s.apply(&ev(Kind::ApprovalWait), 0);
        assert_eq!(s.tick(TOOL_STALE_MS * 10), None);
        assert_eq!(s.state(), State::Waiting);
    }

    #[test]
    fn notes_change_nothing() {
        let mut s = Session::new(0);
        assert_eq!(s.apply(&ev(Kind::Note), 1), None);
        assert_eq!(s.state(), State::Idle);
    }

    #[test]
    fn priority_order() {
        let order = [
            State::Sleeping,
            State::Idle,
            State::Done,
            State::Thinking,
            State::Tool(ToolCategory::Other),
            State::Error,
            State::Waiting,
        ];
        for w in order.windows(2) {
            assert!(w[0].priority() < w[1].priority());
        }
    }
}
