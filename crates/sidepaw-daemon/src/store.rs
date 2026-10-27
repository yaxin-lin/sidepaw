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

/// One remote model call, as `contracts/permissions.md` requires: what
/// went out is described by size and digest, never stored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct AuditRow {
    pub ts: String,
    pub provider: String,
    pub purpose: String,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub bytes: u64,
    pub sha256: String,
    pub level: u8,
}

/// One stored event as the brain reads it back. Rows hold only what
/// the permission level allowed at ingest, so reading them is safe at
/// any level; `project` is the folder name of the session's `cwd`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct EventRow {
    pub ts: String,
    pub host: String,
    pub project: Option<String>,
    pub kind: String,
    pub tool_name: Option<String>,
    pub text: Option<String>,
    pub error_type: Option<String>,
    pub error_msg: Option<String>,
}

/// Counters for one local calendar day.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct DayRow {
    /// `YYYY-MM-DD` in local time.
    pub day: String,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub sessions: u64,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub turns: u64,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub tool_calls: u64,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub errors: u64,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub tokens: u64,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub waiting_ms: u64,
}

/// Counters for one project over a range. `project` is the folder
/// name, absent for sessions stored below level 2 (no `cwd`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct ProjectRow {
    pub project: Option<String>,
    pub hosts: Vec<String>,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub sessions: u64,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub turns: u64,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub tool_calls: u64,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub errors: u64,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub tokens: u64,
    /// RFC 3339, UTC.
    pub last_active: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct History {
    pub since: String,
    /// Only days with activity, oldest first.
    pub days: Vec<DayRow>,
    /// Most tool calls first.
    pub projects: Vec<ProjectRow>,
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

    pub fn insert_approval(
        &mut self,
        session_key: &str,
        ts: &str,
        tool_name: Option<&str>,
        summary: Option<&str>,
        allow: bool,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO approvals (ts, session_id, tool_name, summary, decision)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                ts,
                session_key,
                tool_name,
                summary,
                if allow { "allow" } else { "deny" }
            ],
        )?;
        Ok(())
    }

    pub fn insert_audit(&mut self, row: &AuditRow) -> Result<()> {
        self.conn.execute(
            "INSERT INTO audit (ts, provider, purpose, bytes, sha256, level)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                row.ts,
                row.provider,
                row.purpose,
                row.bytes as i64,
                row.sha256,
                row.level
            ],
        )?;
        Ok(())
    }

    /// Newest first.
    pub fn recent_events(&self, limit: u32) -> Result<Vec<EventRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT e.ts, s.host, s.cwd, e.kind, e.tool_name, e.text, e.error_type, e.error_msg
             FROM events e JOIN sessions s ON s.id = e.session_id
             ORDER BY e.id DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit], |r| {
            let cwd: Option<String> = r.get(2)?;
            Ok(EventRow {
                ts: r.get(0)?,
                host: r.get(1)?,
                project: cwd.as_deref().and_then(project_name),
                kind: r.get(3)?,
                tool_name: r.get(4)?,
                text: r.get(5)?,
                error_type: r.get(6)?,
                error_msg: r.get(7)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    /// Most recent first.
    pub fn audit_recent(&self, limit: u32) -> Result<Vec<AuditRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT ts, provider, purpose, bytes, sha256, level FROM audit
             ORDER BY id DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit], |r| {
            Ok(AuditRow {
                ts: r.get(0)?,
                provider: r.get(1)?,
                purpose: r.get(2)?,
                bytes: r.get::<_, i64>(3)?.max(0) as u64,
                sha256: r.get(4)?,
                level: r.get(5)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    /// Day and project counters since `since` (RFC 3339, UTC). Days are
    /// local days at `offset_s` from UTC, the offset of today: across a
    /// daylight saving change an hour of events lands on the next day,
    /// which a usage chart can live with.
    pub fn history(&self, since: &str, offset_s: i32) -> Result<History> {
        let shift = format!("{offset_s:+} seconds");
        let mut stmt = self.conn.prepare(
            "SELECT date(ts, ?2) AS d, COUNT(DISTINCT session_id),
                    SUM(kind = 'turn_end'), SUM(kind = 'tool_start'),
                    SUM(kind IN ('tool_fail', 'error')),
                    COALESCE(SUM(tokens_in), 0) + COALESCE(SUM(tokens_out), 0)
             FROM events WHERE ts >= ?1 GROUP BY d ORDER BY d",
        )?;
        let mut days: Vec<DayRow> = stmt
            .query_map(params![since, shift], |r| {
                Ok(DayRow {
                    day: r.get(0)?,
                    sessions: r.get::<_, i64>(1)? as u64,
                    turns: r.get::<_, Option<i64>>(2)?.unwrap_or(0) as u64,
                    tool_calls: r.get::<_, Option<i64>>(3)?.unwrap_or(0) as u64,
                    errors: r.get::<_, Option<i64>>(4)?.unwrap_or(0) as u64,
                    tokens: r.get::<_, i64>(5)? as u64,
                    waiting_ms: 0,
                })
            })?
            .collect::<std::result::Result<_, _>>()?;
        // Same rule as `report_since`: a wait lasts until the session's
        // next state change, and counts on the day it began.
        let mut stmt = self.conn.prepare(
            "SELECT date(ts, ?2) AS d,
                    COALESCE(SUM((julianday(next_ts) - julianday(ts)) * 86400000.0), 0)
             FROM (SELECT ts, state,
                          LEAD(ts) OVER (PARTITION BY session_id ORDER BY id) AS next_ts
                   FROM state_changes WHERE ts >= ?1)
             WHERE state = 'waiting' AND next_ts IS NOT NULL GROUP BY d",
        )?;
        let waits = stmt.query_map(params![since, shift], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, f64>(1)?))
        })?;
        for w in waits {
            let (day, ms) = w?;
            if let Some(row) = days.iter_mut().find(|d| d.day == day) {
                row.waiting_ms = ms.max(0.0) as u64;
            }
        }

        // Grouped by full cwd in SQL, then merged by folder name, since
        // two checkouts of one repo are one project to a person.
        let mut stmt = self.conn.prepare(
            "SELECT s.cwd, s.host, COUNT(DISTINCT e.session_id),
                    SUM(e.kind = 'turn_end'), SUM(e.kind = 'tool_start'),
                    SUM(e.kind IN ('tool_fail', 'error')),
                    COALESCE(SUM(e.tokens_in), 0) + COALESCE(SUM(e.tokens_out), 0),
                    MAX(e.ts)
             FROM events e JOIN sessions s ON s.id = e.session_id
             WHERE e.ts >= ?1 GROUP BY s.cwd, s.host",
        )?;
        let rows = stmt.query_map(params![since], |r| {
            let cwd: Option<String> = r.get(0)?;
            Ok((
                cwd.as_deref().and_then(project_name),
                r.get::<_, String>(1)?,
                [
                    r.get::<_, i64>(2)?,
                    r.get::<_, Option<i64>>(3)?.unwrap_or(0),
                    r.get::<_, Option<i64>>(4)?.unwrap_or(0),
                    r.get::<_, Option<i64>>(5)?.unwrap_or(0),
                    r.get::<_, i64>(6)?,
                ],
                r.get::<_, String>(7)?,
            ))
        })?;
        let mut projects: Vec<ProjectRow> = Vec::new();
        for row in rows {
            let (project, host, n, last) = row?;
            let p = match projects.iter_mut().position(|p| p.project == project) {
                Some(i) => &mut projects[i],
                None => {
                    projects.push(ProjectRow {
                        project,
                        ..Default::default()
                    });
                    projects.last_mut().unwrap()
                }
            };
            if !p.hosts.contains(&host) {
                p.hosts.push(host);
                p.hosts.sort();
            }
            p.sessions += n[0] as u64;
            p.turns += n[1] as u64;
            p.tool_calls += n[2] as u64;
            p.errors += n[3] as u64;
            p.tokens += n[4] as u64;
            if last > p.last_active {
                p.last_active = last;
            }
        }
        projects.sort_by(|a, b| {
            b.tool_calls
                .cmp(&a.tool_calls)
                .then(b.last_active.cmp(&a.last_active))
        });
        Ok(History {
            since: since.to_owned(),
            days,
            projects,
        })
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
    pub fn conn_for_tests(&self) -> &Connection {
        &self.conn
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

// The last path segment, with either separator, so a Windows cwd reads
// the same as a POSIX one.
fn project_name(cwd: &str) -> Option<String> {
    cwd.trim_end_matches(['/', '\\'])
        .rsplit(['/', '\\'])
        .next()
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
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
    fn schema_is_idempotent_and_versioned() {
        let s = Store::in_memory().unwrap();
        s.conn.execute_batch(SCHEMA).unwrap();
        let (rows, version): (i64, i64) = s
            .conn
            .query_row(
                "SELECT COUNT(*), MAX(version) FROM schema_version",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!((rows, version), (1, 2));
    }

    #[test]
    fn approvals_are_recorded() {
        let mut s = Store::in_memory().unwrap();
        s.insert_approval(
            "claude-code:s",
            "t",
            Some("Bash"),
            Some("rm -rf build"),
            false,
        )
        .unwrap();
        let (tool, decision): (String, String) = s
            .conn
            .query_row("SELECT tool_name, decision FROM approvals", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        assert_eq!((tool.as_str(), decision.as_str()), ("Bash", "deny"));
    }

    #[test]
    fn audit_rows_come_back_newest_first() {
        let mut store = Store::in_memory().unwrap();
        for (i, purpose) in ["lines", "chat"].iter().enumerate() {
            store
                .insert_audit(&AuditRow {
                    ts: format!("2026-10-27T00:00:0{i}.000Z"),
                    provider: "deepseek".into(),
                    purpose: purpose.to_string(),
                    bytes: 100 + i as u64,
                    sha256: "ab".repeat(32),
                    level: 1,
                })
                .unwrap();
        }
        let rows = store.audit_recent(10).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].purpose, "chat");
        assert_eq!(rows[1].bytes, 100);
        assert_eq!(store.audit_recent(1).unwrap().len(), 1);
    }

    #[test]
    fn recent_events_read_back_newest_first_with_the_project_name() {
        let mut store = Store::in_memory().unwrap();
        let mut first = ev(Kind::ToolStart);
        first.text = Some("cargo test".into());
        store.insert_event(&first).unwrap();
        let mut second = ev(Kind::ToolFail);
        second.ts = "2026-10-25T00:00:01.000Z".into();
        store.insert_event(&second).unwrap();
        let rows = store.recent_events(5).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].kind, "tool_fail");
        assert_eq!(rows[1].text.as_deref(), Some("cargo test"));
        assert_eq!(rows[1].project.as_deref(), Some("p"));
        assert_eq!(rows[1].host, "dsh");
        assert_eq!(project_name("C:\\work\\app\\"), Some("app".into()));
        assert_eq!(project_name("/"), None);
    }

    #[test]
    fn history_groups_by_local_day_and_merges_projects_by_folder() {
        let mut store = Store::in_memory().unwrap();
        let mut put = |session: &str, cwd: Option<&str>, ts: &str, kind: Kind| {
            let mut e = ev(kind);
            e.session = session.into();
            e.cwd = cwd.map(str::to_owned);
            e.ts = ts.into();
            store.insert_event(&e).unwrap();
        };
        // 23:30 UTC is the next day at +08:00.
        put(
            "1",
            Some("/w/app"),
            "2026-10-25T10:00:00.000Z",
            Kind::ToolStart,
        );
        put(
            "1",
            Some("/w/app"),
            "2026-10-25T23:30:00.000Z",
            Kind::ToolStart,
        );
        put(
            "2",
            Some("/other/app"),
            "2026-10-26T01:00:00.000Z",
            Kind::TurnEnd,
        );
        put("3", None, "2026-10-26T02:00:00.000Z", Kind::ToolFail);
        let h = store.history("2026-10-01T00:00:00.000Z", 8 * 3600).unwrap();
        let days: Vec<(&str, u64)> = h
            .days
            .iter()
            .map(|d| (d.day.as_str(), d.tool_calls))
            .collect();
        assert_eq!(days, [("2026-10-25", 1), ("2026-10-26", 1)]);
        assert_eq!(h.days[1].sessions, 3);
        assert_eq!(h.days[1].errors, 1);
        assert_eq!(h.projects[0].project.as_deref(), Some("app"));
        assert_eq!(
            h.projects[0].sessions, 2,
            "two checkouts named app are one project"
        );
        assert_eq!(h.projects[0].tool_calls, 2);
        assert_eq!(h.projects[0].turns, 1);
        assert_eq!(h.projects[0].last_active, "2026-10-26T01:00:00.000Z");
        assert_eq!(h.projects[1].project, None);
        assert_eq!(h.projects[0].hosts, ["dsh"]);
        let utc = store.history("2026-10-01T00:00:00.000Z", 0).unwrap();
        assert_eq!(utc.days.len(), 2);
        assert_eq!(
            utc.days[0].tool_calls, 2,
            "both tool calls are on the 25th in UTC"
        );
    }
}
