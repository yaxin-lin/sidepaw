//! Pet pack manifest (`pet.json`). See `docs/contracts/pet-pack.md`.
//!
//! The four Codex fields are read as-is. Everything sidepaw adds
//! lives under `x-sidepaw` and is optional.

use serde::{Deserialize, Serialize};

pub const SHEET_WIDTH: u32 = 1536;
pub const SHEET_HEIGHT: u32 = 1872;
pub const CELL_WIDTH: u32 = 192;
pub const CELL_HEIGHT: u32 = 208;
pub const COLUMNS: u32 = 8;
pub const ROWS: u32 = 9;

/// Spritesheet rows in the Codex order.
pub const ROW_NAMES: [&str; ROWS as usize] = [
    "idle",
    "running-right",
    "running-left",
    "waving",
    "jumping",
    "failed",
    "waiting",
    "running",
    "review",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub id: String,
    pub display_name: String,
    #[serde(default)]
    pub description: String,
    pub spritesheet_path: String,
    #[serde(default, rename = "x-sidepaw", skip_serializing_if = "Option::is_none")]
    pub sidepaw: Option<Extension>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Extension {
    #[serde(default = "one")]
    pub version: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attribution: Vec<Attribution>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub persona: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub persona_text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lines: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub voice: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attribution {
    pub role: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

fn one() -> u8 {
    1
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ManifestError {
    #[error("id must be lowercase letters, digits and hyphens, got {0:?}")]
    BadId(String),
    #[error("spritesheetPath must be a bare file name, got {0:?}")]
    BadSpritesheetPath(String),
    #[error("x-sidepaw.version {0} is newer than this build understands")]
    UnsupportedVersion(u8),
}

impl Manifest {
    pub fn parse(json: &str) -> Result<Self, ParseError> {
        let manifest: Manifest = serde_json::from_str(json)?;
        manifest.validate()?;
        Ok(manifest)
    }

    pub fn validate(&self) -> Result<(), ManifestError> {
        let id_ok = !self.id.is_empty()
            && self
                .id
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
        if !id_ok {
            return Err(ManifestError::BadId(self.id.clone()));
        }
        let p = &self.spritesheet_path;
        if p.is_empty() || p.contains('/') || p.contains('\\') || p.starts_with('.') {
            return Err(ManifestError::BadSpritesheetPath(p.clone()));
        }
        if let Some(ext) = &self.sidepaw {
            if ext.version > 1 {
                return Err(ManifestError::UnsupportedVersion(ext.version));
            }
        }
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Invalid(#[from] ManifestError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_plain_codex_manifest() {
        let m = Manifest::parse(
            r#"{"id":"mochi","displayName":"Mochi","description":"a cat","spritesheetPath":"spritesheet.webp"}"#,
        )
        .unwrap();
        assert_eq!(m.id, "mochi");
        assert!(m.sidepaw.is_none());
    }

    #[test]
    fn reads_extension() {
        let m = Manifest::parse(
            r#"{
              "id": "whale-maid",
              "displayName": "Whale Maid",
              "spritesheetPath": "spritesheet.webp",
              "x-sidepaw": {
                "license": "CC-BY-NC-SA-4.0",
                "attribution": [{"role": "character", "name": "someone"}],
                "persona": "persona.json"
              }
            }"#,
        )
        .unwrap();
        let ext = m.sidepaw.unwrap();
        assert_eq!(ext.version, 1);
        assert_eq!(ext.license.as_deref(), Some("CC-BY-NC-SA-4.0"));
        assert_eq!(ext.attribution.len(), 1);
    }

    #[test]
    fn rejects_path_traversal() {
        let err =
            Manifest::parse(r#"{"id":"x","displayName":"X","spritesheetPath":"../etc/passwd"}"#)
                .unwrap_err();
        assert!(matches!(
            err,
            ParseError::Invalid(ManifestError::BadSpritesheetPath(_))
        ));
    }

    #[test]
    fn rejects_bad_id() {
        let err =
            Manifest::parse(r#"{"id":"My Cat","displayName":"X","spritesheetPath":"s.webp"}"#)
                .unwrap_err();
        assert!(matches!(err, ParseError::Invalid(ManifestError::BadId(_))));
    }

    #[test]
    fn sheet_geometry_is_consistent() {
        assert_eq!(COLUMNS * CELL_WIDTH, SHEET_WIDTH);
        assert_eq!(ROWS * CELL_HEIGHT, SHEET_HEIGHT);
        assert_eq!(ROW_NAMES.len() as u32, ROWS);
    }
}
