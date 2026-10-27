//! Per-pet files the user edits or the app generates: `persona.json`,
//! `persona.md` and `lines.json` under `<data dir>/pets/<id>/`. They
//! override the same files in the bundled pack, so a pack ships with
//! defaults and the user's edits survive an app update.

use crate::home;
use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};

const NAMES: [&str; 3] = ["persona.json", "persona.md", "lines.json"];

fn path(dir: &Path, id: &str, name: &str) -> Result<PathBuf> {
    if id.is_empty()
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        bail!("pet id {id:?} is not a plain slug");
    }
    if !NAMES.contains(&name) {
        bail!("{name} is not a per-pet file");
    }
    Ok(dir.join("pets").join(id).join(name))
}

/// `None` when the user has no override for that file.
pub fn read(id: &str, name: &str) -> Result<Option<String>> {
    read_in(&home::dir()?, id, name)
}

/// An empty text removes the override instead of writing an empty file.
pub fn write(id: &str, name: &str, text: &str) -> Result<()> {
    write_in(&home::dir()?, id, name, text)
}

pub fn read_in(dir: &Path, id: &str, name: &str) -> Result<Option<String>> {
    let path = path(dir, id, name)?;
    match std::fs::read_to_string(&path) {
        Ok(text) => Ok(Some(text)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e).with_context(|| path.display().to_string()),
    }
}

pub fn write_in(dir: &Path, id: &str, name: &str, text: &str) -> Result<()> {
    let path = path(dir, id, name)?;
    if text.trim().is_empty() {
        return match std::fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e).with_context(|| path.display().to_string()),
        };
    }
    std::fs::create_dir_all(path.parent().unwrap())?;
    std::fs::write(&path, text).with_context(|| path.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_paths_and_unknown_names() {
        let dir = Path::new("/x");
        assert!(path(dir, "../x", "lines.json").is_err());
        assert!(path(dir, "cat", "pet.json").is_err());
        assert!(path(dir, "", "lines.json").is_err());
        assert!(path(dir, "my-cat_2", "persona.md").is_ok());
    }

    #[test]
    fn round_trip_and_remove() {
        let dir = std::env::temp_dir().join(format!("sidepaw-pets-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(read_in(&dir, "cat", "lines.json").unwrap(), None);
        write_in(&dir, "cat", "lines.json", "{\"version\":1}").unwrap();
        assert_eq!(
            read_in(&dir, "cat", "lines.json").unwrap().as_deref(),
            Some("{\"version\":1}")
        );
        write_in(&dir, "cat", "lines.json", "  ").unwrap();
        assert_eq!(read_in(&dir, "cat", "lines.json").unwrap(), None);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
