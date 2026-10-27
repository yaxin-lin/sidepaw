//! `~/.sidepaw/config.toml`. Every key has a default and a missing file
//! is valid; see `docs/config.md`. Only the parts the server acts on
//! are modelled here. Keys for parts that are not built yet (quiet
//! hours, chattiness) are parsed so a user's file round-trips, but
//! nothing reads them.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sidepaw_core::{Host, Level};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Clone, Deserialize, Default, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub ui: Ui,
    pub keys: Keys,
    pub model: ModelConfig,
    pub pet: Pet,
    pub quiet: Quiet,
    pub hosts: BTreeMap<String, HostConfig>,
    pub projects: BTreeMap<String, ProjectConfig>,
    pub daemon: Daemon,
}

/// `[keys]`: global shortcuts for answering a held permission prompt
/// (level 4 with `act`). They are registered only while a prompt waits,
/// so they never shadow another app's shortcut otherwise. Empty turns
/// one off.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Keys {
    pub allow: String,
    pub deny: String,
}

impl Default for Keys {
    fn default() -> Self {
        Keys {
            allow: "Control+Alt+Y".into(),
            deny: "Control+Alt+N".into(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Default, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Ui {
    pub language: UiLanguage,
}

/// The language of everything sidepaw itself says: windows, tray,
/// notifications. The pet's lines follow its persona instead.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum UiLanguage {
    #[default]
    #[serde(rename = "system")]
    System,
    #[serde(rename = "en")]
    En,
    #[serde(rename = "zh-CN")]
    ZhCn,
}

impl UiLanguage {
    pub fn as_str(self) -> &'static str {
        match self {
            UiLanguage::System => "system",
            UiLanguage::En => "en",
            UiLanguage::ZhCn => "zh-CN",
        }
    }
}

/// `[model]`: the one provider the brain talks to (ADR 5). The API key
/// is not here, see `secret`.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default, deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct ModelConfig {
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
    pub size: PetSize,
    /// Do not disturb: no notifications and no unprompted lines.
    pub dnd: bool,
    /// Parked at the screen edge, sliding out when the cursor comes.
    pub mini: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum PetSize {
    S,
    #[default]
    M,
    L,
}

impl PetSize {
    /// Window and web view scale. Kept to quarters so the hit mask's
    /// cell stays a whole number of pixels (`hit.rs`).
    pub fn zoom(self) -> f64 {
        match self {
            PetSize::S => 0.75,
            PetSize::M => 1.0,
            PetSize::L => 1.25,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            PetSize::S => "s",
            PetSize::M => "m",
            PetSize::L => "l",
        }
    }
}

/// The parts of `[pet]` the tray toggles and the pet window follows.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct PetPrefs {
    pub size: PetSize,
    pub dnd: bool,
    pub mini: bool,
}

impl Default for Pet {
    fn default() -> Self {
        Pet {
            active: "placeholder".into(),
            level: Level::DEFAULT,
            chatty: 2,
            size: PetSize::M,
            dnd: false,
            mini: false,
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
    /// Let the pet answer this host's permission prompts (level 4 only).
    pub act: bool,
    /// Seconds the hook is held open for the user to click Allow or Deny
    /// before the host's own prompt takes over.
    pub act_window_s: u64,
}

impl Default for HostConfig {
    fn default() -> Self {
        HostConfig {
            enabled: true,
            level: None,
            act: false,
            act_window_s: 15,
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

    /// Set `[pet] level` in `<dir>/config.toml`, creating the file if
    /// needed and keeping everything else in it, comments included.
    pub fn write_level(dir: &Path, level: Level) -> Result<()> {
        edit(dir, |doc| {
            if !doc.contains_table("pet") {
                doc["pet"] = toml_edit::table();
            }
            doc["pet"]["level"] = toml_edit::value(i64::from(u8::from(level)));
        })
    }

    /// Set `[model]` the same way `write_level` does. An empty `model`
    /// means the provider's default and is written as such so the file
    /// shows the user where to put a choice.
    pub fn write_model(dir: &Path, model: &ModelConfig) -> Result<()> {
        edit(dir, |doc| {
            if !doc.contains_table("model") {
                doc["model"] = toml_edit::table();
            }
            doc["model"]["provider"] = toml_edit::value(model.provider.as_str());
            doc["model"]["model"] = toml_edit::value(model.model.as_str());
            match model
                .base_url
                .as_deref()
                .map(str::trim)
                .filter(|u| !u.is_empty())
            {
                Some(url) => doc["model"]["base_url"] = toml_edit::value(url),
                None => {
                    if let Some(t) = doc["model"].as_table_mut() {
                        t.remove("base_url");
                    }
                }
            }
        })
    }

    /// Set `[pet] size`, `dnd` and `mini` the same way `write_level` does.
    pub fn write_pet_prefs(dir: &Path, prefs: PetPrefs) -> Result<()> {
        edit(dir, |doc| {
            if !doc.contains_table("pet") {
                doc["pet"] = toml_edit::table();
            }
            doc["pet"]["size"] = toml_edit::value(prefs.size.as_str());
            doc["pet"]["dnd"] = toml_edit::value(prefs.dnd);
            doc["pet"]["mini"] = toml_edit::value(prefs.mini);
        })
    }

    pub fn pet_prefs(&self) -> PetPrefs {
        PetPrefs {
            size: self.pet.size,
            dnd: self.pet.dnd,
            mini: self.pet.mini,
        }
    }

    /// Set `[ui] language` the same way `write_level` does.
    pub fn write_ui_language(dir: &Path, language: UiLanguage) -> Result<()> {
        edit(dir, |doc| {
            if !doc.contains_table("ui") {
                doc["ui"] = toml_edit::table();
            }
            doc["ui"]["language"] = toml_edit::value(language.as_str());
        })
    }

    /// Set `[hosts.<host>] act` the same way `write_level` does.
    pub fn write_host_act(dir: &Path, host: Host, act: bool) -> Result<()> {
        edit(dir, |doc| {
            if !doc.contains_table("hosts") {
                doc["hosts"] = toml_edit::table();
                if let Some(t) = doc["hosts"].as_table_mut() {
                    t.set_implicit(true);
                }
            }
            if !doc["hosts"]
                .as_table()
                .is_some_and(|t| t.contains_table(host.as_str()))
            {
                doc["hosts"][host.as_str()] = toml_edit::table();
            }
            doc["hosts"][host.as_str()]["act"] = toml_edit::value(act);
        })
    }

    pub fn host(&self, host: Host) -> HostConfig {
        self.hosts.get(host.as_str()).cloned().unwrap_or_default()
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

// Read, change and write back `config.toml` keeping comments and the
// user's key order; a missing file starts empty.
fn edit(dir: &Path, change: impl FnOnce(&mut toml_edit::DocumentMut)) -> Result<()> {
    let path = dir.join("config.toml");
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e).with_context(|| path.display().to_string()),
    };
    let mut doc: toml_edit::DocumentMut = text
        .parse()
        .with_context(|| format!("{} is not valid TOML", path.display()))?;
    change(&mut doc);
    std::fs::create_dir_all(dir)?;
    std::fs::write(&path, doc.to_string()).with_context(|| path.display().to_string())
}

// `root` contains `cwd` when they are equal or `cwd` is below it.
// Compared as strings on purpose: no filesystem access, works for paths
// that no longer exist, and both sides come from the same host.
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

    /// A fresh directory per test, so tests can run in parallel.
    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("sidepaw-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

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
    fn write_level_keeps_the_rest_of_the_file() {
        let dir = scratch("config");
        Config::write_level(&dir, Level::Notify).unwrap();
        assert_eq!(Config::load(&dir).unwrap().pet.level, Level::Notify);

        std::fs::write(
            dir.join("config.toml"),
            "# my notes\n[pet]\nactive = \"cat\" # keep\nlevel = 1\n\n[daemon]\nport = 4242\n",
        )
        .unwrap();
        Config::write_level(&dir, Level::Observe).unwrap();
        let text = std::fs::read_to_string(dir.join("config.toml")).unwrap();
        assert!(text.contains("# my notes"), "{text}");
        assert!(text.contains("active = \"cat\" # keep"), "{text}");
        assert!(text.contains("level = 0"), "{text}");
        let c = Config::load(&dir).unwrap();
        assert_eq!(c.daemon.port, 4242);
        assert_eq!(c.pet.level, Level::Observe);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_host_act_creates_the_host_table() {
        let dir = scratch("act");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("config.toml"), "[pet]\nlevel = 4\n").unwrap();
        Config::write_host_act(&dir, Host::ClaudeCode, true).unwrap();
        let c = Config::load(&dir).unwrap();
        assert!(c.host(Host::ClaudeCode).act);
        assert_eq!(c.host(Host::ClaudeCode).act_window_s, 15);
        assert_eq!(c.pet.level, Level::Act, "other keys untouched");
        Config::write_host_act(&dir, Host::ClaudeCode, false).unwrap();
        assert!(!Config::load(&dir).unwrap().host(Host::ClaudeCode).act);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_model_replaces_the_table_and_drops_an_empty_base_url() {
        let dir = scratch("model");
        let m = ModelConfig {
            provider: "ollama".into(),
            model: "qwen3:8b".into(),
            base_url: Some("http://127.0.0.1:11434".into()),
        };
        Config::write_model(&dir, &m).unwrap();
        assert_eq!(Config::load(&dir).unwrap().model, m);
        let m = ModelConfig {
            provider: "deepseek".into(),
            model: String::new(),
            base_url: Some("  ".into()),
        };
        Config::write_model(&dir, &m).unwrap();
        let text = std::fs::read_to_string(dir.join("config.toml")).unwrap();
        assert!(!text.contains("base_url"), "{text}");
        assert_eq!(Config::load(&dir).unwrap().model.provider, "deepseek");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn ui_language_round_trips_and_rejects_unknown_tags() {
        let dir = scratch("ui");
        Config::write_ui_language(&dir, UiLanguage::ZhCn).unwrap();
        assert_eq!(Config::load(&dir).unwrap().ui.language, UiLanguage::ZhCn);
        let text = std::fs::read_to_string(dir.join("config.toml")).unwrap();
        assert!(text.contains("language = \"zh-CN\""), "{text}");
        assert_eq!(Config::default().ui.language, UiLanguage::System);
        assert!(Config::parse("[ui]\nlanguage = \"fr\"\n").is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn pet_prefs_round_trip_next_to_other_pet_keys() {
        let dir = scratch("prefs");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("config.toml"), "[pet]\nlevel = 3 # mine\n").unwrap();
        let prefs = PetPrefs {
            size: PetSize::L,
            dnd: true,
            mini: false,
        };
        Config::write_pet_prefs(&dir, prefs).unwrap();
        let c = Config::load(&dir).unwrap();
        assert_eq!(c.pet_prefs(), prefs);
        assert_eq!(c.pet.level, Level::Notify);
        assert!(std::fs::read_to_string(dir.join("config.toml"))
            .unwrap()
            .contains("# mine"));
        assert!(Config::parse("[pet]\nsize = \"xl\"\n").is_err());
        assert_eq!(PetSize::S.zoom() * 4.0, 3.0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn keys_default_and_can_be_turned_off() {
        assert_eq!(Config::default().keys.allow, "Control+Alt+Y");
        let c = Config::parse("[keys]\ndeny = \"\"\n").unwrap();
        assert_eq!(c.keys.deny, "");
        assert_eq!(c.keys.allow, "Control+Alt+Y");
        assert!(Config::parse("[keys]\napprove = \"x\"\n").is_err());
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
