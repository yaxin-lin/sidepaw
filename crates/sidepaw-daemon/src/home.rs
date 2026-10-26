//! The data directory (`~/.sidepaw` or `$SIDEPAW_HOME`) and the small
//! `state.json` the daemon writes so other processes can find it.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub fn user_home() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

pub fn dir() -> Result<PathBuf> {
    if let Some(p) = std::env::var_os("SIDEPAW_HOME") {
        return Ok(PathBuf::from(p));
    }
    Ok(user_home()
        .context("cannot find home directory")?
        .join(".sidepaw"))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonState {
    pub port: u16,
    pub token: String,
}

impl DaemonState {
    fn path() -> Result<PathBuf> {
        Ok(dir()?.join("state.json"))
    }

    pub fn load() -> Result<Self> {
        let path = Self::path()?;
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("daemon not running? no {}", path.display()))?;
        Ok(serde_json::from_str(&text)?)
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::path()?;
        std::fs::create_dir_all(path.parent().unwrap())?;
        std::fs::write(&path, serde_json::to_string_pretty(self)?)?;
        Ok(())
    }

    pub fn new_token() -> String {
        uuid::Uuid::new_v4().simple().to_string()
    }
}
