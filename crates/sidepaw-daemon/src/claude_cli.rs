//! The user's own Claude Code as a model provider (ADR 10): the pet
//! runs the `claude` program they are signed in to, in print mode, so
//! chat and lines draw on their subscription instead of an API key.
//! Every run is a blank slate for Claude Code: no settings (so none of
//! our own hooks fire back at the pet), no CLAUDE.md, no auto memory,
//! no tools, no MCP servers, and nothing saved to its session list.

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Mutex;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;

/// Where installers put the program, under the home directory. A GUI
/// app's PATH has none of these.
const HOME_BINS: [&str; 6] = [
    ".local/bin",
    ".claude/local",
    ".local/npm-global/bin",
    ".npm-global/bin",
    ".volta/bin",
    ".bun/bin",
];
/// Version managers: one `bin` per Node version under these.
const NODE_VERSIONS: [&str; 1] = [".nvm/versions/node"];
const SYSTEM_BINS: [&str; 2] = ["/opt/homebrew/bin", "/usr/local/bin"];

const AUTH_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, Clone, Default, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct CliStatus {
    /// The program that will run, when one was found.
    pub path: Option<String>,
    pub logged_in: bool,
    /// As `claude auth status` reports it: `claude.ai` for a
    /// subscription, `api_key`, `none`.
    pub auth: String,
    /// `pro`, `max`, ... when signed in with a subscription.
    pub plan: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct CliRequest {
    pub system: String,
    pub prompt: String,
    /// A name or alias Claude Code accepts: `haiku`, `sonnet`, a full id.
    pub model: String,
    pub timeout_s: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct CliReply {
    pub text: String,
    /// Share of the subscription's five-hour window used, 0 to 1, when
    /// Claude Code reported it.
    pub five_hour: Option<f64>,
    pub seven_day: Option<f64>,
}

fn names() -> &'static [&'static str] {
    if cfg!(windows) {
        &["claude.exe", "claude.cmd"]
    } else {
        &["claude"]
    }
}

fn candidate_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default();
    if let Some(home) = crate::home::user_home() {
        dirs.extend(HOME_BINS.iter().map(|d| home.join(d)));
        for base in NODE_VERSIONS {
            if let Ok(versions) = std::fs::read_dir(home.join(base)) {
                dirs.extend(versions.flatten().map(|v| v.path().join("bin")));
            }
        }
    }
    dirs.extend(SYSTEM_BINS.iter().map(PathBuf::from));
    if let Some(appdata) = std::env::var_os("APPDATA") {
        dirs.push(PathBuf::from(appdata).join("npm"));
    }
    dirs
}

fn first_in(dirs: &[PathBuf], names: &[&str]) -> Option<PathBuf> {
    dirs.iter()
        .flat_map(|d| names.iter().map(move |n| d.join(n)))
        .find(|p| p.is_file())
}

/// The login shell knows PATH entries a GUI app does not (set in
/// `.zshrc` and friends).
#[cfg(unix)]
async fn ask_shell() -> Option<PathBuf> {
    const SHELL_TIMEOUT: Duration = Duration::from_secs(5);
    let shell = std::env::var_os("SHELL")?;
    let out = tokio::time::timeout(
        SHELL_TIMEOUT,
        Command::new(shell)
            .args(["-lc", "command -v claude"])
            .stdin(Stdio::null())
            .output(),
    )
    .await
    .ok()?
    .ok()?;
    let line = String::from_utf8_lossy(&out.stdout);
    let path = PathBuf::from(line.lines().last()?.trim());
    path.is_file().then_some(path)
}

#[cfg(not(unix))]
async fn ask_shell() -> Option<PathBuf> {
    None
}

static FOUND: Mutex<Option<PathBuf>> = Mutex::new(None);

/// Looks every time; `cached` is for the hot path.
pub async fn find() -> Option<PathBuf> {
    let found = match first_in(&candidate_dirs(), names()) {
        Some(p) => Some(p),
        None => ask_shell().await,
    };
    *FOUND.lock().unwrap_or_else(|e| e.into_inner()) = found.clone();
    found
}

async fn cached() -> Option<PathBuf> {
    let known = FOUND.lock().unwrap_or_else(|e| e.into_inner()).clone();
    match known {
        Some(p) if p.is_file() => Some(p),
        _ => find().await,
    }
}

/// `claude.cmd` is a batch file: only a shell can run it.
fn command(bin: &Path) -> Command {
    if bin
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("cmd"))
    {
        let mut c = Command::new("cmd");
        c.arg("/C").arg(bin);
        c
    } else {
        Command::new(bin)
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AuthStatus {
    #[serde(default)]
    logged_in: bool,
    #[serde(default)]
    auth_method: String,
    subscription_type: Option<String>,
}

pub async fn status() -> CliStatus {
    let Some(bin) = find().await else {
        return CliStatus::default();
    };
    let mut st = CliStatus {
        path: Some(bin.display().to_string()),
        ..Default::default()
    };
    let out = tokio::time::timeout(
        AUTH_TIMEOUT,
        command(&bin)
            .args(["auth", "status"])
            .stdin(Stdio::null())
            .output(),
    )
    .await;
    if let Ok(Ok(out)) = out {
        if let Ok(a) = serde_json::from_slice::<AuthStatus>(&out.stdout) {
            st.logged_in = a.logged_in;
            st.auth = a.auth_method;
            st.plan = a.subscription_type;
        }
    }
    st
}

/// One line of `--output-format stream-json`, reduced to what the pet
/// needs. `Done` carries the reply, or why there is none.
#[derive(Debug, PartialEq)]
enum Line {
    Delta(String),
    Limits {
        five_hour: Option<f64>,
        seven_day: Option<f64>,
    },
    Done(std::result::Result<String, String>),
    Other,
}

fn parse(line: &str) -> Line {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
        return Line::Other;
    };
    match v.get("type").and_then(|t| t.as_str()) {
        Some("stream_event") => {
            let delta = v.pointer("/event/delta");
            match delta.and_then(|d| d.get("type")).and_then(|t| t.as_str()) {
                Some("text_delta") => Line::Delta(
                    delta
                        .and_then(|d| d.get("text"))
                        .and_then(|t| t.as_str())
                        .unwrap_or_default()
                        .to_string(),
                ),
                _ => Line::Other,
            }
        }
        Some("rate_limit_event") => {
            let util = |w: &str| {
                v.pointer(&format!("/rate_limit_info/unifiedWindows/{w}/utilization"))
                    .and_then(|u| u.as_f64())
            };
            Line::Limits {
                five_hour: util("five_hour"),
                seven_day: util("seven_day"),
            }
        }
        Some("result") => {
            let text = v.get("result").and_then(|r| r.as_str()).unwrap_or_default();
            let failed = v.get("is_error").and_then(|e| e.as_bool()).unwrap_or(false)
                || v.get("subtype").and_then(|s| s.as_str()) != Some("success");
            if failed {
                let why = if text.is_empty() {
                    v.get("subtype")
                        .and_then(|s| s.as_str())
                        .unwrap_or("no reply")
                        .to_string()
                } else {
                    text.to_string()
                };
                Line::Done(Err(why))
            } else if text.is_empty() {
                Line::Done(Err("the reply had no text".into()))
            } else {
                Line::Done(Ok(text.to_string()))
            }
        }
        _ => Line::Other,
    }
}

/// A folder Claude Code has never seen: no CLAUDE.md above it worth
/// reading and no memory for it. Removed after the run.
fn scratch_dir() -> Result<PathBuf> {
    use std::sync::atomic::{AtomicU32, Ordering};
    static N: AtomicU32 = AtomicU32::new(0);
    let dir = std::env::temp_dir().join(format!(
        "sidepaw-claude-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).context("cannot create a working folder for claude")?;
    Ok(dir)
}

pub async fn run(req: &CliRequest, mut on_delta: impl FnMut(String)) -> Result<CliReply> {
    let bin = cached()
        .await
        .context("claude was not found; install Claude Code or put `claude` on PATH")?;
    let cwd = scratch_dir()?;
    let result = run_in(&bin, &cwd, req, &mut on_delta).await;
    let _ = std::fs::remove_dir_all(&cwd);
    result
}

async fn run_in(
    bin: &Path,
    cwd: &Path,
    req: &CliRequest,
    on_delta: &mut impl FnMut(String),
) -> Result<CliReply> {
    let mut child = command(bin)
        .args([
            "-p",
            "--model",
            &req.model,
            "--effort",
            "low",
            "--max-turns",
            "1",
        ])
        .args([
            "--output-format",
            "stream-json",
            "--verbose",
            "--include-partial-messages",
        ])
        .args([
            "--tools",
            "",
            "--strict-mcp-config",
            "--no-session-persistence",
        ])
        .args(["--setting-sources", "", "--system-prompt", &req.system])
        .env("CLAUDE_CODE_DISABLE_AUTO_MEMORY", "1")
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .with_context(|| format!("cannot start {}", bin.display()))?;
    let mut stdin = child.stdin.take().context("no stdin")?;
    let stdout = child.stdout.take().context("no stdout")?;
    let mut stderr = child.stderr.take().context("no stderr")?;
    let prompt = req.prompt.clone();
    tokio::spawn(async move {
        let _ = stdin.write_all(prompt.as_bytes()).await;
        // Dropping closes it, which is the end of the prompt.
    });
    let err_task = tokio::spawn(async move {
        let mut s = String::new();
        let _ = stderr.read_to_string(&mut s).await;
        s
    });
    let mut reply = CliReply::default();
    let mut done: Option<std::result::Result<String, String>> = None;
    let read = async {
        let mut lines = BufReader::new(stdout).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            match parse(&line) {
                Line::Delta(d) => on_delta(d),
                Line::Limits {
                    five_hour,
                    seven_day,
                } => {
                    reply.five_hour = five_hour;
                    reply.seven_day = seven_day;
                }
                Line::Done(r) => done = Some(r),
                Line::Other => {}
            }
        }
    };
    if tokio::time::timeout(Duration::from_secs(u64::from(req.timeout_s)), read)
        .await
        .is_err()
    {
        let _ = child.kill().await;
        bail!("claude took longer than {} s", req.timeout_s);
    }
    let status = child.wait().await?;
    let stderr = err_task.await.unwrap_or_default();
    match done {
        Some(Ok(text)) => {
            reply.text = text;
            Ok(reply)
        }
        Some(Err(why)) => bail!("claude: {why}"),
        None => {
            let tail: String = stderr.lines().rev().take(3).collect::<Vec<_>>().join(" ");
            bail!(
                "claude exited with {} before answering{}",
                status,
                if tail.is_empty() {
                    String::new()
                } else {
                    format!(": {tail}")
                }
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_deltas_and_limits_are_read_from_the_stream() {
        let delta = r#"{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":"one "}}}"#;
        assert_eq!(parse(delta), Line::Delta("one ".into()));
        let other = r#"{"type":"stream_event","event":{"type":"message_start"}}"#;
        assert_eq!(parse(other), Line::Other);
        let limits = r#"{"type":"rate_limit_event","rate_limit_info":{"unifiedWindows":{"five_hour":{"utilization":0.07},"seven_day":{"utilization":0.05}}}}"#;
        assert_eq!(
            parse(limits),
            Line::Limits {
                five_hour: Some(0.07),
                seven_day: Some(0.05)
            }
        );
        assert_eq!(parse("not json"), Line::Other);
    }

    #[test]
    fn the_result_line_decides() {
        let ok = r#"{"type":"result","subtype":"success","is_error":false,"result":"hi"}"#;
        assert_eq!(parse(ok), Line::Done(Ok("hi".into())));
        let failed = r#"{"type":"result","subtype":"error_during_execution","is_error":true,"result":"Not logged in"}"#;
        assert_eq!(parse(failed), Line::Done(Err("Not logged in".into())));
        let silent = r#"{"type":"result","subtype":"error_max_turns","is_error":true}"#;
        assert_eq!(parse(silent), Line::Done(Err("error_max_turns".into())));
        let empty = r#"{"type":"result","subtype":"success","is_error":false,"result":""}"#;
        assert_eq!(
            parse(empty),
            Line::Done(Err("the reply had no text".into()))
        );
    }

    #[test]
    fn the_first_dir_holding_the_program_wins() {
        let root = std::env::temp_dir().join(format!("sidepaw-claude-find-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let (a, b) = (root.join("a"), root.join("b"));
        std::fs::create_dir_all(&a).unwrap();
        std::fs::create_dir_all(&b).unwrap();
        std::fs::write(b.join("claude"), "").unwrap();
        assert_eq!(
            first_in(&[a.clone(), b.clone()], &["claude"]),
            Some(b.join("claude"))
        );
        assert_eq!(first_in(std::slice::from_ref(&a), &["claude"]), None);
        assert_eq!(first_in(&[root.join("missing")], &["claude"]), None);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Needs Claude Code signed in: `cargo test -p sidepaw-daemon -- --ignored real_claude`.
    #[tokio::test]
    #[ignore]
    async fn real_claude_answers_and_reports_limits() {
        let st = status().await;
        assert!(st.logged_in, "{st:?}");
        let req = CliRequest {
            system: "You are a terse pet.".into(),
            prompt: "Reply with exactly: ok".into(),
            model: "haiku".into(),
            timeout_s: 60,
        };
        let mut deltas = String::new();
        let reply = run(&req, |d| deltas.push_str(&d)).await.unwrap();
        assert_eq!(reply.text.trim(), "ok");
        assert_eq!(deltas.trim(), "ok", "the stream carried the same text");
        assert!(reply.five_hour.is_some(), "{reply:?}");
    }

    #[test]
    fn a_batch_file_runs_through_cmd() {
        let c = command(Path::new(r"C:\npm\claude.cmd"));
        assert_eq!(c.as_std().get_program(), "cmd");
        let c = command(Path::new("/opt/homebrew/bin/claude"));
        assert_eq!(c.as_std().get_program(), "/opt/homebrew/bin/claude");
    }
}
