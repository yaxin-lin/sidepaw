//! SQLite store. Schema lives in `docs/contracts/schema.sql` and is
//! embedded at build time so the binary and the document cannot drift.

use anyhow::Result;
use rusqlite::{params, Connection};
use sidepaw_core::{Event, State};
use std::path::Path;

const SCHEMA: &str = include_str!("../../../docs/contracts/schema.sql");

pub struct Store {
    conn: Connection,
}

impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        conn.execute_batch(SCHEMA)?;
        Ok(Store { conn })
    }

    #[cfg(test)]
    pub fn in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch(SCHEMA)?;
        Ok(Store { conn })
    }

    /// Append an already redacted and filtered event and update the
    /// session counters.
    pub fn insert_event(&mut self, event: &Event) -> Result<()> {
        let key = event.session_key();
        let tx = self.conn.transaction()?;
        tx.execute(
            "INSERT INTO sessions (id, host, cwd, started_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(id) DO UPDATE SET cwd = COALESCE(excluded.cwd, sessions.cwd)",
            params![key, event.host.as_str(), event.cwd, event.ts],
        )?;
        tx.execute(
            "INSERT INTO events (session_id, ts, kind, tool_name, tool_cat, text,
                                 error_type, error_msg, tokens_in, tokens_out, raw)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                key,
                event.ts,
                serde_json::to_value(event.kind)?.as_str().unwrap_or("note"),
                event.tool.as_ref().map(|t| t.name.as_str()),
                event
                    .tool
                    .as_ref()
                    .map(|t| serde_json::to_value(t.category).unwrap())
                    .and_then(|v| v.as_str().map(str::to_owned)),
                event.text,
                event.error.as_ref().map(|e| e.kind.as_str()),
                event.error.as_ref().and_then(|e| e.message.as_deref()),
                event.tokens.map(|t| t.input as i64),
                event.tokens.map(|t| t.output as i64),
                event.raw.as_ref().map(|r| r.to_string()),
            ],
        )?;
        tx.execute(
            "UPDATE sessions SET
                turns      = turns + (?2 = 'turn_end'),
                tool_calls = tool_calls + (?2 = 'tool_start'),
                errors     = errors + (?2 IN ('tool_fail', 'error')),
                tokens_in  = tokens_in + COALESCE(?3, 0),
                tokens_out = tokens_out + COALESCE(?4, 0),
                ended_at   = CASE WHEN ?2 = 'session_end' THEN ?5 ELSE ended_at END
             WHERE id = ?1",
            params![
                key,
                serde_json::to_value(event.kind)?.as_str().unwrap_or("note"),
                event.tokens.map(|t| t.input as i64),
                event.tokens.map(|t| t.output as i64),
                event.ts,
            ],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn insert_state_change(&mut self, session_key: &str, ts: &str, state: State) -> Result<()> {
        let v = serde_json::to_value(state)?;
        let name = v["state"].as_str().unwrap_or("idle").to_owned();
        let detail = v.get("detail").and_then(|d| d.as_str()).map(str::to_owned);
        self.conn.execute(
            "INSERT INTO state_changes (session_id, ts, state, detail) VALUES (?1, ?2, ?3, ?4)",
            params![session_key, ts, name, detail],
        )?;
        self.conn.execute(
            "UPDATE sessions SET last_state = ?2 WHERE id = ?1",
            params![session_key, name],
        )?;
        Ok(())
    }

    #[cfg(test)]
    pub fn session_count(&self) -> Result<u64> {
        Ok(self
            .conn
            .query_row("SELECT COUNT(*) FROM sessions", [], |r| r.get::<_, i64>(0))?
            as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sidepaw_core::{Host, Kind, Tool, ToolCategory};

    fn ev(kind: Kind) -> Event {
        Event {
            v: 1,
            host: Host::Dsh,
            session: "42".into(),
            cwd: Some("/p".into()),
            ts: "2026-10-25T00:00:00.000Z".into(),
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
    fn schema_applies_and_counters_update() {
        let mut s = Store::in_memory().unwrap();
        s.insert_event(&ev(Kind::SessionStart)).unwrap();
        s.insert_event(&ev(Kind::ToolStart)).unwrap();
        s.insert_event(&ev(Kind::ToolFail)).unwrap();
        s.insert_event(&ev(Kind::TurnEnd)).unwrap();
        assert_eq!(s.session_count().unwrap(), 1);

        let (turns, tools, errors): (i64, i64, i64) = s
            .conn
            .query_row(
                "SELECT turns, tool_calls, errors FROM sessions WHERE id = 'dsh:42'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!((turns, tools, errors), (1, 1, 1));

        s.insert_state_change("dsh:42", "t", State::Tool(ToolCategory::Shell))
            .unwrap();
        let detail: String = s
            .conn
            .query_row("SELECT detail FROM state_changes LIMIT 1", [], |r| r.get(0))
            .unwrap();
        assert_eq!(detail, "shell");
    }

    #[test]
    fn schema_is_idempotent() {
        let s = Store::in_memory().unwrap();
        s.conn.execute_batch(SCHEMA).unwrap();
        let v: i64 = s
            .conn
            .query_row("SELECT COUNT(*) FROM schema_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, 1);
    }
}
