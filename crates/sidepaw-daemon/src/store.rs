//! SQLite store. Schema lives in `docs/contracts/schema.sql` and is
//! embedded at build time so the binary and the document cannot drift.

use anyhow::Result;
use rusqlite::{params, Connection};
use serde::Serialize;
use sidepaw_core::{Event, State};
use std::path::Path;

/// Counters for one day, computed from stored events alone, so it works
/// at every permission level and without a model.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct DayReport {
    /// RFC 3339 start of the day the report covers.
    pub since: String,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub sessions: u64,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub turns: u64,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub tool_calls: u64,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub errors: u64,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub tokens_in: u64,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub tokens_out: u64,
    /// Time spent waiting for the user to answer a prompt, in ms.
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub waiting_ms: u64,
    /// Tool calls per tool name, most used first. Empty at level 0,
    /// where names are not stored.
    pub tools: Vec<(String, u64)>,
}

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

    /// Counters since `since` (RFC 3339, UTC). Waiting time is the gap
    /// between each `waiting` state change and the next change in the
    /// same session; an open wait is not counted until it closes.
    pub fn report_since(&self, since: &str) -> Result<DayReport> {
        let mut r = DayReport {
            since: since.to_owned(),
            ..Default::default()
        };
        let row = self.conn.query_row(
            "SELECT COUNT(DISTINCT session_id),
                    SUM(kind = 'turn_end'), SUM(kind = 'tool_start'),
                    SUM(kind IN ('tool_fail', 'error')),
                    COALESCE(SUM(tokens_in), 0), COALESCE(SUM(tokens_out), 0)
             FROM events WHERE ts >= ?1",
            params![since],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, Option<i64>>(1)?.unwrap_or(0),
                    row.get::<_, Option<i64>>(2)?.unwrap_or(0),
                    row.get::<_, Option<i64>>(3)?.unwrap_or(0),
                    row.get::<_, i64>(4)?,
                    row.get::<_, i64>(5)?,
                ))
            },
        )?;
        r.sessions = row.0 as u64;
        r.turns = row.1 as u64;
        r.tool_calls = row.2 as u64;
        r.errors = row.3 as u64;
        r.tokens_in = row.4 as u64;
        r.tokens_out = row.5 as u64;

        let waiting: f64 = self.conn.query_row(
            "SELECT COALESCE(SUM((julianday(next_ts) - julianday(ts)) * 86400000.0), 0)
             FROM (SELECT ts, state,
                          LEAD(ts) OVER (PARTITION BY session_id ORDER BY id) AS next_ts
                   FROM state_changes WHERE ts >= ?1)
             WHERE state = 'waiting' AND next_ts IS NOT NULL",
            params![since],
            |row| row.get(0),
        )?;
        r.waiting_ms = waiting.max(0.0) as u64;

        let mut stmt = self.conn.prepare(
            "SELECT tool_name, COUNT(*) AS n FROM events
             WHERE ts >= ?1 AND kind = 'tool_start' AND tool_name IS NOT NULL AND tool_name != ''
             GROUP BY tool_name ORDER BY n DESC, tool_name LIMIT 20",
        )?;
        r.tools = stmt
            .query_map(params![since], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as u64))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(r)
    }

    #[cfg(test)]
    pub fn last_event_text(&self) -> Result<Option<String>> {
        Ok(self.conn.query_row(
            "SELECT text FROM events ORDER BY id DESC LIMIT 1",
            [],
            |r| r.get::<_, Option<String>>(0),
        )?)
    }

    #[cfg(test)]
    pub fn last_event_raw(&self) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row("SELECT raw FROM events ORDER BY id DESC LIMIT 1", [], |r| {
                r.get::<_, Option<String>>(0)
            })?)
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
    fn report_counts_today_only_and_sums_closed_waits() {
        let mut s = Store::in_memory().unwrap();
        let mut at = |ts: &str, kind: Kind, tool: &str| {
            let mut e = ev(kind);
            e.ts = ts.into();
            e.tool = Some(Tool {
                name: tool.into(),
                category: ToolCategory::Shell,
            });
            s.insert_event(&e).unwrap();
        };
        at("2026-10-25T23:00:00.000Z", Kind::ToolStart, "Bash"); // yesterday
        at("2026-10-26T09:00:00.000Z", Kind::ToolStart, "Bash");
        at("2026-10-26T09:00:01.000Z", Kind::ToolStart, "Read");
        at("2026-10-26T09:00:02.000Z", Kind::ToolStart, "Bash");
        at("2026-10-26T09:00:03.000Z", Kind::ToolFail, "Bash");
        at("2026-10-26T09:00:04.000Z", Kind::TurnEnd, "");
        s.insert_state_change("dsh:42", "2026-10-26T09:00:00.000Z", State::Waiting)
            .unwrap();
        s.insert_state_change("dsh:42", "2026-10-26T09:00:30.000Z", State::Thinking)
            .unwrap();
        s.insert_state_change("dsh:42", "2026-10-26T09:01:00.000Z", State::Waiting)
            .unwrap(); // still open

        let r = s.report_since("2026-10-26T00:00:00.000Z").unwrap();
        assert_eq!(r.sessions, 1);
        assert_eq!(r.tool_calls, 3, "yesterday's call is excluded");
        assert_eq!(r.errors, 1);
        assert_eq!(r.turns, 1);
        assert!(
            (r.waiting_ms as i64 - 30_000).abs() < 50,
            "waited {} ms, expected about 30000",
            r.waiting_ms
        );
        assert_eq!(
            r.tools,
            vec![("Bash".to_string(), 2), ("Read".to_string(), 1)]
        );
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
