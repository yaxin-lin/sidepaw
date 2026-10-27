//! `sidepaw run -- <command>`: any command becomes something the pet
//! can watch. It posts unified events (host `wrapped`) around the
//! child: a shell tool while it runs, then done, failed or interrupted
//! by how it exited. The child inherits the terminal and its exit
//! status is passed through, so the wrapper can go in front of any
//! command line. Without a running app the command still runs.

use crate::clock::now_rfc3339;
use crate::hook;
use anyhow::{Context, Result};
use sidepaw_core::event::ErrorInfo;
use sidepaw_core::{Event, Host, Kind, Tool, ToolCategory};
use std::process::{Command, ExitStatus};

pub fn run(argv: &[String]) -> Result<i32> {
    let (program, args) = argv.split_first().context("no command given")?;
    let cwd = std::env::current_dir()
        .ok()
        .map(|p| p.display().to_string());
    let line = argv.join(" ");
    let session = session_id(cwd.as_deref().unwrap_or(""), program);
    let tool = Tool {
        name: basename(program).to_owned(),
        category: ToolCategory::Shell,
    };
    let base = |kind: Kind| Event {
        v: 1,
        host: Host::Wrapped,
        session: session.clone(),
        cwd: cwd.clone(),
        ts: now_rfc3339(),
        kind,
        tool: None,
        text: None,
        error: None,
        tokens: None,
        raw: None,
    };

    hook::post_event(&Event {
        tool: Some(tool.clone()),
        text: Some(line),
        ..base(Kind::ToolStart)
    });
    let status = Command::new(program)
        .args(args)
        .status()
        .with_context(|| format!("could not start {program}"));
    let status = match status {
        Ok(s) => s,
        Err(e) => {
            hook::post_event(&Event {
                tool: Some(tool),
                error: Some(ErrorInfo {
                    kind: "spawn".into(),
                    message: Some(format!("{e:#}")),
                }),
                ..base(Kind::ToolFail)
            });
            return Err(e);
        }
    };
    for event in finish(status, &tool, base) {
        hook::post_event(&event);
    }
    Ok(exit_code(status))
}

fn finish(status: ExitStatus, tool: &Tool, base: impl Fn(Kind) -> Event) -> Vec<Event> {
    if status.success() {
        return vec![
            Event {
                tool: Some(tool.clone()),
                ..base(Kind::ToolEnd)
            },
            base(Kind::TurnEnd),
        ];
    }
    if status.code().is_none() {
        // Killed by a signal, which from a terminal is nearly always ^C.
        return vec![base(Kind::Interrupt)];
    }
    vec![Event {
        tool: Some(tool.clone()),
        error: Some(ErrorInfo {
            kind: "exit".into(),
            message: Some(format!("exited with status {}", exit_code(status))),
        }),
        ..base(Kind::ToolFail)
    }]
}

// The shell convention: 128 plus the signal number for a killed child.
fn exit_code(status: ExitStatus) -> i32 {
    if let Some(code) = status.code() {
        return code;
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(sig) = status.signal() {
            return 128 + sig;
        }
    }
    1
}

fn basename(program: &str) -> &str {
    program.rsplit(['/', '\\']).next().unwrap_or(program)
}

/// Same command in the same folder is the same session, so running
/// `npm test` again updates one row in the tray instead of adding one.
/// FNV-1a, because it must not change between builds.
fn session_id(cwd: &str, program: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in cwd.bytes().chain([0]).chain(basename(program).bytes()) {
        hash ^= u64::from(b);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    format!("run-{hash:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base(kind: Kind) -> Event {
        Event {
            v: 1,
            host: Host::Wrapped,
            session: "s".into(),
            cwd: None,
            ts: "2026-10-27T00:00:00.000Z".into(),
            kind,
            tool: None,
            text: None,
            error: None,
            tokens: None,
            raw: None,
        }
    }

    fn status_of(script: &str) -> ExitStatus {
        if cfg!(windows) {
            Command::new("cmd").args(["/C", script]).status().unwrap()
        } else {
            Command::new("sh").args(["-c", script]).status().unwrap()
        }
    }

    #[test]
    fn exit_status_becomes_done_or_failed() {
        let tool = Tool {
            name: "npm".into(),
            category: ToolCategory::Shell,
        };
        let ok: Vec<Kind> = finish(status_of("exit 0"), &tool, base)
            .iter()
            .map(|e| e.kind)
            .collect();
        assert_eq!(ok, [Kind::ToolEnd, Kind::TurnEnd]);
        let failed = finish(status_of("exit 3"), &tool, base);
        assert_eq!(failed[0].kind, Kind::ToolFail);
        assert_eq!(
            failed[0].error.as_ref().unwrap().message.as_deref(),
            Some("exited with status 3")
        );
        assert_eq!(exit_code(status_of("exit 3")), 3);
    }

    #[cfg(unix)]
    #[test]
    fn a_signal_is_an_interrupt() {
        let s = status_of("kill -INT $$");
        assert_eq!(
            finish(
                s,
                &Tool {
                    name: "x".into(),
                    category: ToolCategory::Shell
                },
                base
            )[0]
            .kind,
            Kind::Interrupt
        );
        assert_eq!(exit_code(s), 130);
    }

    #[test]
    fn session_is_stable_per_folder_and_program() {
        assert_eq!(session_id("/a", "/usr/bin/npm"), session_id("/a", "npm"));
        assert_ne!(session_id("/a", "npm"), session_id("/b", "npm"));
        assert_ne!(session_id("/a", "npm"), session_id("/a", "cargo"));
        assert_eq!(session_id("", "x").len(), "run-".len() + 16);
    }
}
