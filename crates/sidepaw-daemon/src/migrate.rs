//! Schema migrations. Each file in `migrations/` moves the database
//! one version up and runs once, in its own transaction, recorded in
//! `schema_version`. `docs/contracts/schema.sql` is the resulting
//! schema written out in full for readers; a test keeps the two equal.
//!
//! To change the schema: add the next numbered file here and to
//! `MIGRATIONS`, then update `schema.sql` to match. Never edit a
//! migration that has shipped.

use anyhow::{bail, Context, Result};
use rusqlite::{Connection, OptionalExtension};
use std::path::Path;

const MIGRATIONS: &[(u32, &str)] = &[
    (1, include_str!("../migrations/0001_initial.sql")),
    (2, include_str!("../migrations/0002_approvals.sql")),
];

/// The version this build brings a database to.
pub fn latest() -> u32 {
    MIGRATIONS.last().map_or(0, |(v, _)| *v)
}

/// 0 for a database nothing has been applied to yet.
pub fn version(conn: &Connection) -> Result<u32> {
    let has_table: Option<i64> = conn
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'schema_version'",
            [],
            |r| r.get(0),
        )
        .optional()?;
    if has_table.is_none() {
        return Ok(0);
    }
    let v: Option<u32> =
        conn.query_row("SELECT MAX(version) FROM schema_version", [], |r| r.get(0))?;
    Ok(v.unwrap_or(0))
}

/// Bring the database up to `latest()`. Before changing a database that
/// already has data, a copy is written next to `backup_of` as
/// `<name>.v<old>.bak`, so a failed upgrade loses nothing. A database
/// from a newer sidepaw is refused rather than guessed at.
pub fn run(conn: &mut Connection, backup_of: Option<&Path>) -> Result<u32> {
    let from = version(conn)?;
    let to = latest();
    if from > to {
        bail!("the database is at version {from}, newer than this sidepaw ({to}); update sidepaw");
    }
    if from == to {
        return Ok(to);
    }
    if let (true, Some(db)) = (from > 0, backup_of) {
        backup(conn, db, from)?;
    }
    for (v, sql) in MIGRATIONS.iter().filter(|(v, _)| *v > from) {
        let tx = conn.transaction()?;
        tx.execute_batch(sql)
            .with_context(|| format!("migration {v}"))?;
        tx.execute("DELETE FROM schema_version", [])?;
        tx.execute("INSERT INTO schema_version (version) VALUES (?1)", [v])?;
        tx.commit()?;
        tracing::info!("database migrated to version {v}");
    }
    Ok(to)
}

fn backup(conn: &Connection, db: &Path, from: u32) -> Result<()> {
    let name = db
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("sidepaw.sqlite");
    let target = db.with_file_name(format!("{name}.v{from}.bak"));
    // VACUUM INTO refuses to overwrite.
    let _ = std::fs::remove_file(&target);
    conn.execute("VACUUM INTO ?1", [target.to_string_lossy()])
        .with_context(|| format!("backing up to {}", target.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOCUMENTED: &str = include_str!("../../../docs/contracts/schema.sql");

    /// Table and index definitions, whitespace-insensitive, without the
    /// `IF NOT EXISTS` the documented file is free to carry.
    fn shape(conn: &Connection) -> Vec<(String, String)> {
        let mut stmt = conn
            .prepare("SELECT name, sql FROM sqlite_master WHERE sql IS NOT NULL ORDER BY name")
            .unwrap();
        stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
            .unwrap()
            .map(|row| {
                let (name, sql) = row.unwrap();
                let sql = sql.replace(" IF NOT EXISTS", "");
                (name, sql.split_whitespace().collect::<Vec<_>>().join(" "))
            })
            .collect()
    }

    #[test]
    fn migrations_build_the_documented_schema() {
        let mut migrated = Connection::open_in_memory().unwrap();
        assert_eq!(run(&mut migrated, None).unwrap(), latest());
        let documented = Connection::open_in_memory().unwrap();
        documented.execute_batch(DOCUMENTED).unwrap();
        assert_eq!(shape(&migrated), shape(&documented));
        assert_eq!(version(&migrated).unwrap(), latest());
    }

    #[test]
    fn versions_are_consecutive() {
        for (i, (v, _)) in MIGRATIONS.iter().enumerate() {
            assert_eq!(*v as usize, i + 1);
        }
    }

    #[test]
    fn a_version_1_database_gains_only_what_is_new_and_keeps_its_rows() {
        let dir = std::env::temp_dir().join(format!("sidepaw-migrate-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = dir.join("sidepaw.sqlite");
        let _ = std::fs::remove_file(&db);
        let mut conn = Connection::open(&db).unwrap();
        conn.execute_batch(MIGRATIONS[0].1).unwrap();
        conn.execute("INSERT INTO schema_version (version) VALUES (1)", [])
            .unwrap();
        conn.execute(
            "INSERT INTO memories (created_at, source, body) VALUES ('t', 'user', 'likes tea')",
            [],
        )
        .unwrap();

        assert_eq!(run(&mut conn, Some(&db)).unwrap(), 2);
        let body: String = conn
            .query_row("SELECT body FROM memories", [], |r| r.get(0))
            .unwrap();
        assert_eq!(body, "likes tea");
        conn.execute(
            "INSERT INTO approvals (ts, session_id, decision) VALUES ('t', 's', 'allow')",
            [],
        )
        .unwrap();
        let rows: i64 = conn
            .query_row("SELECT COUNT(*) FROM schema_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(rows, 1);

        let backup = Connection::open(dir.join("sidepaw.sqlite.v1.bak")).unwrap();
        assert_eq!(version(&backup).unwrap(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_database_from_the_future_is_refused() {
        let mut conn = Connection::open_in_memory().unwrap();
        run(&mut conn, None).unwrap();
        conn.execute("UPDATE schema_version SET version = 99", [])
            .unwrap();
        let err = run(&mut conn, None).unwrap_err().to_string();
        assert!(err.contains("newer"), "{err}");
    }
}
