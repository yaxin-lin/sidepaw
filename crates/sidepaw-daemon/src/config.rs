//! `~/.sidepaw/config.toml`. Every key has a default and a missing file
//! is valid; see `docs/config.md`. Only the parts the server acts on
//! are modelled here. Keys for parts that are not built yet (model,
//! quiet hours, chattiness) are parsed so a user's file round-trips,
//! but nothing reads them.

use anyhow::{Context, Result};
use serde::Deserialize;
use sidepaw_core::{Host, Level};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Clone, Deserialize, Default, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub model: Model,
    pub pet: Pet,
    pub quiet: Quiet,
    pub hosts: BTreeMap<String, HostConfig>,
    pub projects: BTreeMap<String, ProjectConfig>,
    pub daemon: Daemon,
}

#[derive(Debug, Clone, Deserialize, Default, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Model {
    pub provider: String,
    pub model: String,
    pub base_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Pet {
    pub active: String,
    pub level: Level,
    pub chatty: u8,
}

impl Default for Pet {
    fn default() -> Self {
        Pet {
            active: "placeholder".into(),
            level: Level::DEFAULT,
            chatty: 2,
        }
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Quiet {
    pub from: String,
    pub to: String,
}

impl Default for Quiet {
    fn default() -> Self {
        Quiet {
            from: "23:00".into(),
            to: "08:00".into(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct HostConfig {
    pub enabled: bool,
    pub level: Option<Level>,
}

impl Default for HostConfig {
    fn default() -> Self {
        HostConfig {
            enabled: true,
            level: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Default, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct ProjectConfig {
    pub level: Option<Level>,
}

#[derive(Debug, Clone, Deserialize, Default, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Daemon {
    pub port: u16,
}

impl Config {
    pub fn parse(text: &str) -> Result<Self> {
        toml::from_str(text).context("config.toml")
    }

    /// Read `<dir>/config.toml`; a missing file is the default config.
    pub fn load(dir: &Path) -> Result<Self> {
        let path = dir.join("config.toml");
        match std::fs::read_to_string(&path) {
            Ok(text) => Self::parse(&text).with_context(|| path.display().to_string()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e).with_context(|| path.display().to_string()),
        }
    }

    pub fn host_enabled(&self, host: Host) -> bool {
        self.hosts
            .get(host.as_str())
            .map(|h| h.enabled)
            .unwrap_or(true)
    }

    /// The lowest of the global level, the host's override and the
    /// override of the longest project path that contains `cwd`. Must
    /// be computed from the unfiltered event: the filter itself removes
    /// `cwd` below level 2.
    pub fn effective_level(&self, host: Host, cwd: Option<&str>) -> Level {
        let mut level = self.pet.level;
        if let Some(l) = self.hosts.get(host.as_str()).and_then(|h| h.level) {
            level = level.min(l);
        }
        if let Some(cwd) = cwd {
            let project = self
                .projects
                .iter()
                .filter(|(path, _)| path_contains(path, cwd))
                .max_by_key(|(path, _)| path.len());
            if let Some((_, p)) = project {
                if let Some(l) = p.level {
                    level = level.min(l);
                }
            }
        }
        level
    }
}

/// `root` contains `cwd` when they are equal or `cwd` is below it.
/// Compared as strings on purpose: no filesystem access, works for
/// paths that no longer exist, and both sides come from the same host.
fn path_contains(root: &str, cwd: &str) -> bool {
    let root = root.trim_end_matches(['/', '\\']);
    if root.is_empty() {
        return false;
    }
    cwd == root
        || cwd
            .strip_prefix(root)
            .is_some_and(|rest| rest.starts_with('/') || rest.starts_with('\\'))
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXAMPLE: &str = r#"
[model]
provider = "deepseek"

[pet]
active = "whale-maid"
level  = 3
chatty = 4

[hosts.codex]
level = 1

[hosts.kimi]
enabled = false

[projects."/work/company-repo"]
level = 0

[projects."/work"]
level = 2

[daemon]
port = 4242
"#;

    #[test]
    fn missing_file_is_default() {
        let c = Config::load(Path::new("/definitely/not/here")).unwrap();
        assert_eq!(c, Config::default());
        assert_eq!(c.pet.level, Level::Record);
        assert!(c.host_enabled(Host::Codex));
    }

    #[test]
    fn parses_example_and_rejects_typos() {
        let c = Config::parse(EXAMPLE).unwrap();
        assert_eq!(c.daemon.port, 4242);
        assert_eq!(c.pet.chatty, 4);
        assert!(!c.host_enabled(Host::Kimi));
        assert!(Config::parse("[pet]\nlevl = 1\n").is_err());
        assert!(Config::parse("[pet]\nlevel = 9\n").is_err());
    }

    #[test]
    fn lowest_applicable_level_wins() {
        let c = Config::parse(EXAMPLE).unwrap();
        assert_eq!(c.effective_level(Host::ClaudeCode, None), Level::Notify);
        assert_eq!(c.effective_level(Host::Codex, None), Level::Record);
        assert_eq!(
            c.effective_level(Host::ClaudeCode, Some("/work/other")),
            Level::Read,
            "the /work rule applies"
        );
        assert_eq!(
            c.effective_level(Host::ClaudeCode, Some("/work/company-repo/src")),
            Level::Observe,
            "the longer, more specific rule applies"
        );
        assert_eq!(
            c.effective_level(Host::ClaudeCode, Some("/work/company-repo-fork")),
            Level::Read,
            "prefix match is on path segments, not characters"
        );
        assert_eq!(
            c.effective_level(Host::Codex, Some("/work/other")),
            Level::Record,
            "host override still caps it"
        );
    }

    #[test]
    fn path_containment() {
        assert!(path_contains("/a/b", "/a/b"));
        assert!(path_contains("/a/b/", "/a/b/c"));
        assert!(!path_contains("/a/b", "/a/bc"));
        assert!(!path_contains("", "/a"));
        assert!(path_contains("C:\\work", "C:\\work\\x"));
    }
}
