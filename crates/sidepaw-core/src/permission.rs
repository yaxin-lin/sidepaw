//! Permission levels and the field filter applied before storage.
//! See `docs/contracts/permissions.md`.

use crate::event::Event;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "u8", into = "u8")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum Level {
    Observe = 0,
    Record = 1,
    Read = 2,
    Notify = 3,
    Act = 4,
}

impl Level {
    pub const DEFAULT: Level = Level::Record;

    /// Strip fields the level does not allow. The event is modified in
    /// place so adapters can build the full event and let the gate
    /// decide what survives.
    pub fn filter(self, event: &mut Event) {
        if self < Level::Read {
            event.cwd = None;
            event.text = None;
            event.raw = None;
            if let Some(err) = event.error.as_mut() {
                err.message = None;
            }
        }
        if self < Level::Record {
            event.tokens = None;
            if let Some(tool) = event.tool.as_mut() {
                tool.name.clear();
            }
            if let Some(err) = event.error.as_mut() {
                err.kind = "error".into();
            }
        }
    }

    pub fn may_notify(self) -> bool {
        self >= Level::Notify
    }

    pub fn may_act(self) -> bool {
        self >= Level::Act
    }
}

impl TryFrom<u8> for Level {
    type Error = InvalidLevel;

    fn try_from(n: u8) -> Result<Self, InvalidLevel> {
        match n {
            0 => Ok(Level::Observe),
            1 => Ok(Level::Record),
            2 => Ok(Level::Read),
            3 => Ok(Level::Notify),
            4 => Ok(Level::Act),
            _ => Err(InvalidLevel(n)),
        }
    }
}

impl From<Level> for u8 {
    fn from(level: Level) -> u8 {
        level as u8
    }
}

#[derive(Debug, thiserror::Error)]
#[error("permission level must be 0..=4, got {0}")]
pub struct InvalidLevel(pub u8);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{ErrorInfo, Host, Kind, Tokens, Tool, ToolCategory};

    fn full_event() -> Event {
        Event {
            v: 1,
            host: Host::Codex,
            session: "s".into(),
            cwd: Some("/home/me/secret-project".into()),
            ts: "t".into(),
            kind: Kind::ToolFail,
            tool: Some(Tool {
                name: "Bash".into(),
                category: ToolCategory::Shell,
            }),
            text: Some("rm -rf build".into()),
            error: Some(ErrorInfo {
                kind: "exit_status".into(),
                message: Some("exit 1".into()),
            }),
            tokens: Some(Tokens {
                input: 10,
                output: 20,
            }),
            raw: Some(serde_json::json!({"anything": true})),
        }
    }

    #[test]
    fn observe_keeps_only_shape() {
        let mut e = full_event();
        Level::Observe.filter(&mut e);
        assert!(e.cwd.is_none() && e.text.is_none() && e.raw.is_none());
        assert!(e.tokens.is_none());
        assert_eq!(e.tool.as_ref().unwrap().name, "");
        assert_eq!(e.tool.as_ref().unwrap().category, ToolCategory::Shell);
        assert_eq!(e.error.as_ref().unwrap().kind, "error");
        assert!(e.error.as_ref().unwrap().message.is_none());
    }

    #[test]
    fn record_keeps_names_and_counts() {
        let mut e = full_event();
        Level::Record.filter(&mut e);
        assert!(e.cwd.is_none() && e.text.is_none() && e.raw.is_none());
        assert_eq!(e.tool.as_ref().unwrap().name, "Bash");
        assert_eq!(e.error.as_ref().unwrap().kind, "exit_status");
        assert!(e.error.as_ref().unwrap().message.is_none());
        assert!(e.tokens.is_some());
    }

    #[test]
    fn read_and_above_keep_everything() {
        for level in [Level::Read, Level::Notify, Level::Act] {
            let mut e = full_event();
            level.filter(&mut e);
            assert_eq!(e, full_event());
        }
    }

    #[test]
    fn parses_from_number() {
        assert_eq!(Level::try_from(2).unwrap(), Level::Read);
        assert!(Level::try_from(5).is_err());
        let parsed: Level = serde_json::from_str("3").unwrap();
        assert!(parsed.may_notify() && !parsed.may_act());
    }
}
