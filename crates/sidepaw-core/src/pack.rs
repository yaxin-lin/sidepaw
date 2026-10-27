//! Pet pack manifest (`pet.json`). See `docs/contracts/pet-pack.md`.
//!
//! The four Codex fields are read as-is. Everything sidepaw adds
//! lives under `x-sidepaw` and is optional.

use serde::{Deserialize, Serialize};

pub const SHEET_WIDTH: u32 = 1536;
pub const SHEET_HEIGHT: u32 = 1872;
/// petdex's v2 atlas: the same nine rows plus two the renderer ignores.
pub const SHEET_HEIGHT_V2: u32 = 2288;
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
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
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
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
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
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
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

/// Whether a spritesheet of this size has the Codex layout (or the v2
/// layout, whose first nine rows are the same).
pub fn sheet_size_ok(width: u32, height: u32) -> bool {
    width == SHEET_WIDTH && (height == SHEET_HEIGHT || height == SHEET_HEIGHT_V2)
}

/// Width and height from a PNG or WebP header, without decoding the
/// image. `None` for anything else, so a pack is checked before any of
/// it is written.
pub fn image_size(bytes: &[u8]) -> Option<(u32, u32)> {
    let be = |b: &[u8]| u32::from_be_bytes([b[0], b[1], b[2], b[3]]);
    let le24 = |b: &[u8]| u32::from(b[0]) | u32::from(b[1]) << 8 | u32::from(b[2]) << 16;
    if bytes.len() >= 24 && bytes.starts_with(b"\x89PNG\r\n\x1a\n") && &bytes[12..16] == b"IHDR" {
        return Some((be(&bytes[16..20]), be(&bytes[20..24])));
    }
    if bytes.len() < 30 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WEBP" {
        return None;
    }
    match &bytes[12..16] {
        b"VP8X" => Some((le24(&bytes[24..27]) + 1, le24(&bytes[27..30]) + 1)),
        b"VP8L" if bytes[20] == 0x2f => {
            let b = &bytes[21..25];
            let w = (u32::from(b[0]) | (u32::from(b[1]) & 0x3f) << 8) + 1;
            let h =
                ((u32::from(b[1]) >> 6) | u32::from(b[2]) << 2 | (u32::from(b[3]) & 0x0f) << 10)
                    + 1;
            Some((w, h))
        }
        b"VP8 " if bytes[23..26] == [0x9d, 0x01, 0x2a] => {
            let w = u32::from(u16::from_le_bytes([bytes[26], bytes[27]]) & 0x3fff);
            let h = u32::from(u16::from_le_bytes([bytes[28], bytes[29]]) & 0x3fff);
            Some((w, h))
        }
        _ => None,
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

    fn hex(s: &str) -> Vec<u8> {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }

    #[test]
    fn reads_png_and_webp_sizes() {
        let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        png.extend_from_slice(&1536u32.to_be_bytes());
        png.extend_from_slice(&1872u32.to_be_bytes());
        assert_eq!(image_size(&png), Some((1536, 1872)));
        // VP8X stores width-1 and height-1 as 24-bit little endian.
        let mut x = b"RIFF\0\0\0\0WEBPVP8X\0\0\0\0\0\0\0\0".to_vec();
        x.extend_from_slice(&[0xff, 0x05, 0x00, 0xef, 0x08, 0x00]);
        assert_eq!(image_size(&x), Some((1536, 2288)));
        // The first bytes of two real petdex sheets (lossless WebP).
        let v1 = hex("52494646e21c2100574542505650384cd51c21002fffc5d311118c8d2009");
        let v2 = hex("52494646d2732300574542505650384cc67323002fffc53b128d48902439");
        assert_eq!(image_size(&v1), Some((1536, 1872)));
        assert_eq!(image_size(&v2), Some((1536, 2288)));
        assert_eq!(image_size(b"GIF89a................................"), None);
        assert!(sheet_size_ok(1536, 2288));
        assert!(!sheet_size_ok(1536, 2000));
    }

    #[test]
    fn sheet_geometry_is_consistent() {
        assert_eq!(COLUMNS * CELL_WIDTH, SHEET_WIDTH);
        assert_eq!(ROWS * CELL_HEIGHT, SHEET_HEIGHT);
        assert_eq!(ROW_NAMES.len() as u32, ROWS);
    }
}
