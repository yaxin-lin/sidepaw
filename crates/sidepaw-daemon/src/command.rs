//! The `sidepaw` command on the user's PATH. The desktop executable is
//! also the CLI (`cli`), so "installing" it is a link: a symlink in
//! `~/.local/bin` on macOS and Linux, a one-line `sidepaw.cmd` in the
//! per-user WindowsApps folder on Windows, which is on PATH by default.

use crate::home;
use anyhow::{Context, Result};
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct CommandLink {
    pub path: String,
    /// The link exists and points at this executable.
    pub installed: bool,
    /// Its folder is on PATH, so a new terminal finds `sidepaw`.
    pub on_path: bool,
}

fn link_path() -> Result<PathBuf> {
    #[cfg(windows)]
    {
        let base = std::env::var_os("LOCALAPPDATA").context("LOCALAPPDATA is not set")?;
        Ok(PathBuf::from(base)
            .join("Microsoft")
            .join("WindowsApps")
            .join("sidepaw.cmd"))
    }
    #[cfg(not(windows))]
    {
        Ok(home::user_home()
            .context("cannot find home directory")?
            .join(".local")
            .join("bin")
            .join("sidepaw"))
    }
}

fn on_path(dir: &Path) -> bool {
    std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).any(|d| d == dir))
        .unwrap_or(false)
}

#[cfg(windows)]
fn shim(exe: &Path) -> String {
    format!("@\"{}\" %*\r\n", exe.display())
}

fn points_at(link: &Path, exe: &Path) -> bool {
    #[cfg(windows)]
    {
        std::fs::read_to_string(link).is_ok_and(|s| s == shim(exe))
    }
    #[cfg(not(windows))]
    {
        std::fs::read_link(link).is_ok_and(|t| t == exe)
    }
}

pub fn status(exe: &Path) -> Result<CommandLink> {
    let link = link_path()?;
    Ok(CommandLink {
        installed: points_at(&link, exe),
        on_path: link.parent().is_some_and(on_path),
        path: link.display().to_string(),
    })
}

/// Creates or repoints the link. Anything else already at that path
/// that is not ours is left alone and reported.
pub fn install(exe: &Path) -> Result<CommandLink> {
    let link = link_path()?;
    if let Some(dir) = link.parent() {
        std::fs::create_dir_all(dir).with_context(|| dir.display().to_string())?;
    }
    if link.symlink_metadata().is_ok() && !points_at(&link, exe) && !is_ours(&link) {
        anyhow::bail!(
            "{} exists and is not sidepaw's; remove it first",
            link.display()
        );
    }
    let _ = std::fs::remove_file(&link);
    #[cfg(windows)]
    std::fs::write(&link, shim(exe)).with_context(|| link.display().to_string())?;
    #[cfg(not(windows))]
    std::os::unix::fs::symlink(exe, &link).with_context(|| link.display().to_string())?;
    status(exe)
}

pub fn remove(exe: &Path) -> Result<CommandLink> {
    let link = link_path()?;
    if is_ours(&link) {
        std::fs::remove_file(&link).with_context(|| link.display().to_string())?;
    }
    status(exe)
}

// An older install pointing at a since-moved app is still ours to
// replace: a symlink to something named sidepaw, or our shim.
fn is_ours(link: &Path) -> bool {
    #[cfg(windows)]
    {
        std::fs::read_to_string(link).is_ok_and(|s| s.starts_with("@\"") && s.ends_with("%*\r\n"))
    }
    #[cfg(not(windows))]
    {
        std::fs::read_link(link).is_ok_and(|t| {
            t.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("sidepaw"))
        })
    }
}
