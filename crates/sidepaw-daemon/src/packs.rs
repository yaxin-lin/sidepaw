//! Pet packs the user installed, in `<data dir>/packs/<id>/`: from a
//! zip (a petdex download, a file someone shared), from a folder, or
//! copied over from Codex's or petdex's own pet folders. Only the files
//! a pack may contain are read, each with a size cap, and nothing is
//! written until the manifest and the spritesheet have been checked.
//! The user's persona and lines edits stay in `pets/<id>/` (`pets.rs`)
//! so reinstalling or removing a pack never touches them.

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use sidepaw_core::pack::{image_size, sheet_size_ok, Manifest};
use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};

/// The art may be large; petdex itself caps it at 20 MiB.
const SHEET_MAX: u64 = 20 * 1024 * 1024;
const TEXT_MAX: u64 = 1024 * 1024;
const ZIP_ENTRIES_MAX: usize = 64;

/// Optional files a pack may carry next to `pet.json` and its sheet.
fn optional(name: &str) -> bool {
    matches!(name, "persona.json" | "persona.md" | "lines.json")
        || (name.starts_with("lines.") && name.ends_with(".json") && name.len() < 32)
}

/// Where a pack came from, kept next to it as `source.json`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct Source {
    /// `petdex`, `codex`, `zip` or `folder`.
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slug: Option<String>,
    /// Who made or submitted it, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct PackInfo {
    pub id: String,
    pub display_name: String,
    pub description: String,
    pub source: Option<Source>,
    pub license: Option<String>,
}

/// A pack found in another app's pet folder, not installed here yet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct Found {
    pub id: String,
    pub display_name: String,
    /// `codex` or `petdex`.
    pub app: String,
    pub path: String,
}

pub fn dir(home: &Path) -> PathBuf {
    home.join("packs")
}

fn info(dir: &Path, manifest: &Manifest) -> PackInfo {
    let source = std::fs::read_to_string(dir.join("source.json"))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok());
    PackInfo {
        id: manifest.id.clone(),
        display_name: manifest.display_name.clone(),
        description: manifest.description.clone(),
        source,
        license: manifest.sidepaw.as_ref().and_then(|x| x.license.clone()),
    }
}

fn read_manifest(pack: &Path) -> Option<Manifest> {
    Manifest::parse(&std::fs::read_to_string(pack.join("pet.json")).ok()?).ok()
}

/// Installed packs, by display name.
pub fn list(home: &Path) -> Vec<PackInfo> {
    let mut out: Vec<PackInfo> = std::fs::read_dir(dir(home))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| read_manifest(&e.path()).map(|m| info(&e.path(), &m)))
        .collect();
    out.sort_by_key(|p| p.display_name.to_lowercase());
    out
}

/// Lowercase letters, digits and hyphens, as the manifest requires;
/// petdex and Codex ids are mostly that already.
fn slug(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars().flat_map(char::to_lowercase) {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    out.trim_end_matches('-').chars().take(48).collect()
}

/// Checks a set of files read from a zip or folder and writes them as
/// pack `<id>`. `files` maps bare file names to contents.
fn install(home: &Path, mut files: BTreeMap<String, Vec<u8>>, source: Source) -> Result<PackInfo> {
    let raw = files
        .remove("pet.json")
        .context("no pet.json in the pack")?;
    let mut json: serde_json::Value =
        serde_json::from_slice(&raw).context("pet.json is not valid JSON")?;
    let id = json.get("id").and_then(|v| v.as_str()).unwrap_or_default();
    let fixed = slug(if id.is_empty() {
        source.slug.as_deref().unwrap_or("")
    } else {
        id
    });
    if fixed.is_empty() {
        bail!("the pack has no usable id");
    }
    json["id"] = fixed.clone().into();
    let text = serde_json::to_string_pretty(&json)?;
    let manifest = Manifest::parse(&text).context("pet.json")?;
    let sheet = files
        .remove(&manifest.spritesheet_path)
        .with_context(|| format!("the spritesheet {} is missing", manifest.spritesheet_path))?;
    let (w, h) = image_size(&sheet).context("the spritesheet is not a PNG or WebP image")?;
    if !sheet_size_ok(w, h) {
        bail!("the spritesheet is {w}x{h}; a Codex pet sheet is 1536x1872 (or 1536x2288)");
    }
    for (name, body) in &files {
        if name.ends_with(".json") && serde_json::from_slice::<serde_json::Value>(body).is_err() {
            bail!("{name} is not valid JSON");
        }
    }

    // Write next to the final folder and swap it in, so a failure halfway
    // leaves the previous version of the pack in place.
    let root = dir(home);
    std::fs::create_dir_all(&root)?;
    let target = root.join(&manifest.id);
    let staging = root.join(format!(".{}.new", manifest.id));
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::create_dir_all(&staging)?;
    std::fs::write(staging.join("pet.json"), text)?;
    std::fs::write(staging.join(&manifest.spritesheet_path), sheet)?;
    for (name, body) in files {
        std::fs::write(staging.join(name), body)?;
    }
    std::fs::write(
        staging.join("source.json"),
        serde_json::to_string_pretty(&source)?,
    )?;
    let _ = std::fs::remove_dir_all(&target);
    std::fs::rename(&staging, &target).with_context(|| target.display().to_string())?;
    Ok(info(&target, &manifest))
}

/// Whether a file inside a pack is one sidepaw keeps; `sheet` is the
/// manifest's spritesheet name once known.
fn wanted(name: &str, sheet: Option<&str>) -> bool {
    name == "pet.json"
        || optional(name)
        || sheet == Some(name)
        || (sheet.is_none() && (name.ends_with(".webp") || name.ends_with(".png")))
}

fn cap(name: &str) -> u64 {
    if name.ends_with(".json") || name.ends_with(".md") {
        TEXT_MAX
    } else {
        SHEET_MAX
    }
}

/// A zip whose files sit at the top or inside one folder, as petdex,
/// Codex exports and hand-made archives all do. Entries are read into
/// memory by bare name only; paths, links and everything else in the
/// archive are ignored, so nothing can land outside the pack folder.
pub fn install_zip(home: &Path, bytes: &[u8], source: Source) -> Result<PackInfo> {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).context("not a zip file")?;
    if zip.len() > ZIP_ENTRIES_MAX {
        bail!(
            "the zip has {} entries; a pet pack has a handful",
            zip.len()
        );
    }
    let mut files = BTreeMap::new();
    for i in 0..zip.len() {
        let entry = zip.by_index(i)?;
        if entry.is_dir() || entry.is_symlink() {
            continue;
        }
        let Some(path) = entry.enclosed_name() else {
            continue;
        };
        if path.components().count() > 2 {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()).map(str::to_owned) else {
            continue;
        };
        if name.starts_with('.') || !wanted(&name, None) {
            continue;
        }
        let limit = cap(&name);
        if entry.size() > limit {
            bail!("{name} is larger than {} MiB", limit / 1024 / 1024);
        }
        let mut body = Vec::new();
        entry.take(limit + 1).read_to_end(&mut body)?;
        if body.len() as u64 > limit {
            bail!("{name} is larger than {} MiB", limit / 1024 / 1024);
        }
        files.insert(name, body);
    }
    install(home, files, source)
}

pub fn install_folder(home: &Path, folder: &Path, source: Source) -> Result<PackInfo> {
    let manifest = std::fs::read(folder.join("pet.json"))
        .with_context(|| format!("no pet.json in {}", folder.display()))?;
    let sheet: Option<String> = serde_json::from_slice::<serde_json::Value>(&manifest)
        .ok()
        .and_then(|v| v.get("spritesheetPath")?.as_str().map(str::to_owned));
    let mut files = BTreeMap::new();
    for entry in std::fs::read_dir(folder)?.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let meta = entry.metadata()?;
        if !meta.is_file() || !wanted(&name, sheet.as_deref()) {
            continue;
        }
        if meta.len() > cap(&name) {
            bail!("{name} is too large");
        }
        files.insert(name, std::fs::read(entry.path())?);
    }
    install(home, files, source)
}

/// A file of pack `id`: `pet.json`, its spritesheet or an optional one.
pub fn read(home: &Path, id: &str, name: &str) -> Result<Option<Vec<u8>>> {
    if slug(id) != id || id.is_empty() {
        bail!("pack id {id:?} is not a plain slug");
    }
    let pack = dir(home).join(id);
    let Some(manifest) = read_manifest(&pack) else {
        return Ok(None);
    };
    if !(wanted(name, Some(&manifest.spritesheet_path)) && !name.contains(['/', '\\'])) {
        bail!("{name} is not a pack file");
    }
    match std::fs::read(pack.join(name)) {
        Ok(b) => Ok(Some(b)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub fn remove(home: &Path, id: &str) -> Result<()> {
    if slug(id) != id || id.is_empty() {
        bail!("pack id {id:?} is not a plain slug");
    }
    let pack = dir(home).join(id);
    if read_manifest(&pack).is_none() {
        bail!("no installed pack {id}");
    }
    std::fs::remove_dir_all(&pack).with_context(|| pack.display().to_string())
}

/// Pets in Codex's and petdex's own folders that are not installed
/// here, so they can be brought over with one click.
pub fn discover(home: &Path, user_home: &Path) -> Vec<Found> {
    let installed: Vec<String> = list(home).into_iter().map(|p| p.id).collect();
    let mut out = Vec::new();
    for (app, folder) in [("codex", ".codex/pets"), ("petdex", ".petdex/pets")] {
        for entry in std::fs::read_dir(user_home.join(folder))
            .into_iter()
            .flatten()
            .flatten()
        {
            let path = entry.path();
            let Ok(text) = std::fs::read_to_string(path.join("pet.json")) else {
                continue;
            };
            let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else {
                continue;
            };
            let folder_name = entry.file_name().to_string_lossy().into_owned();
            let raw_id = v.get("id").and_then(|x| x.as_str()).unwrap_or_default();
            let id = slug(if raw_id.is_empty() {
                &folder_name
            } else {
                raw_id
            });
            if id.is_empty() || installed.contains(&id) || out.iter().any(|f: &Found| f.id == id) {
                continue;
            }
            out.push(Found {
                display_name: v
                    .get("displayName")
                    .and_then(|x| x.as_str())
                    .unwrap_or(&id)
                    .to_owned(),
                id,
                app: app.to_owned(),
                path: path.display().to_string(),
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn png(w: u32, h: u32) -> Vec<u8> {
        let mut b = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        b.extend_from_slice(&w.to_be_bytes());
        b.extend_from_slice(&h.to_be_bytes());
        b.extend_from_slice(&[0; 16]);
        b
    }

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("sidepaw-packs-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    fn zip_of(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut out = std::io::Cursor::new(Vec::new());
        let mut z = zip::ZipWriter::new(&mut out);
        for (name, body) in files {
            z.start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            z.write_all(body).unwrap();
        }
        z.finish().unwrap();
        out.into_inner()
    }

    #[test]
    fn installs_a_petdex_style_zip_and_ignores_everything_else() {
        let home = scratch("zip");
        let bytes = zip_of(&[
            (
                "boba/pet.json",
                br#"{"id":"Boba Tea","displayName":"Boba","spritesheetPath":"spritesheet.png"}"#,
            ),
            ("boba/spritesheet.png", &png(1536, 2288)),
            ("boba/lines.zh-CN.json", br#"{"version":1,"idle":["hi"]}"#),
            ("boba/run.sh", b"rm -rf ~"),
            ("../evil.json", b"{}"),
        ]);
        let source = Source {
            kind: "petdex".into(),
            slug: Some("boba".into()),
            author: Some("someone".into()),
            url: None,
        };
        let p = install_zip(&home, &bytes, source).unwrap();
        assert_eq!(p.id, "boba-tea");
        assert_eq!(
            p.source.as_ref().unwrap().author.as_deref(),
            Some("someone")
        );
        let folder = dir(&home).join("boba-tea");
        let mut names: Vec<String> = std::fs::read_dir(&folder)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into())
            .collect();
        names.sort();
        assert_eq!(
            names,
            [
                "lines.zh-CN.json",
                "pet.json",
                "source.json",
                "spritesheet.png"
            ]
        );
        assert!(!home.join("evil.json").exists());
        assert_eq!(list(&home).len(), 1);
        assert!(read(&home, "boba-tea", "spritesheet.png")
            .unwrap()
            .is_some());
        assert!(
            read(&home, "boba-tea", "source.json").is_err(),
            "not a pack file"
        );
        assert!(read(&home, "../x", "pet.json").is_err());
        remove(&home, "boba-tea").unwrap();
        assert!(list(&home).is_empty());
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn rejects_a_wrong_sheet_without_touching_the_installed_pack() {
        let home = scratch("bad");
        let good = zip_of(&[
            (
                "pet.json",
                br#"{"id":"cat","displayName":"Cat","spritesheetPath":"s.png"}"#,
            ),
            ("s.png", &png(1536, 1872)),
        ]);
        install_zip(&home, &good, Source::default()).unwrap();
        let bad = zip_of(&[
            (
                "pet.json",
                br#"{"id":"cat","displayName":"Cat 2","spritesheetPath":"s.png"}"#,
            ),
            ("s.png", &png(800, 600)),
        ]);
        let err = install_zip(&home, &bad, Source::default())
            .unwrap_err()
            .to_string();
        assert!(err.contains("800x600"), "{err}");
        assert_eq!(list(&home)[0].display_name, "Cat");
        assert!(install_zip(&home, b"not a zip", Source::default()).is_err());
        let no_sheet = zip_of(&[(
            "pet.json",
            br#"{"id":"dog","displayName":"Dog","spritesheetPath":"s.png"}"#,
        )]);
        assert!(install_zip(&home, &no_sheet, Source::default())
            .unwrap_err()
            .to_string()
            .contains("missing"));
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn imports_a_folder_and_finds_codex_pets() {
        let home = scratch("folder");
        let user = scratch("user");
        let codex = user.join(".codex/pets/mochi");
        std::fs::create_dir_all(&codex).unwrap();
        std::fs::write(
            codex.join("pet.json"),
            br#"{"id":"mochi","displayName":"Mochi","spritesheetPath":"sheet.webp"}"#,
        )
        .unwrap();
        let mut webp = b"RIFF\0\0\0\0WEBPVP8X\0\0\0\0\0\0\0\0".to_vec();
        webp.extend_from_slice(&[0xff, 0x05, 0x00, 0x4f, 0x07, 0x00]);
        std::fs::write(codex.join("sheet.webp"), webp).unwrap();
        let found = discover(&home, &user);
        assert_eq!(found.len(), 1);
        assert_eq!(
            (found[0].id.as_str(), found[0].app.as_str()),
            ("mochi", "codex")
        );
        install_folder(
            &home,
            Path::new(&found[0].path),
            Source {
                kind: "codex".into(),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(
            discover(&home, &user).is_empty(),
            "installed ones are not offered again"
        );
        let _ = std::fs::remove_dir_all(&home);
        let _ = std::fs::remove_dir_all(&user);
    }

    #[test]
    fn slugs() {
        assert_eq!(slug("Boba Tea!"), "boba-tea");
        assert_eq!(slug("机甲小白"), "");
        assert_eq!(slug("mecha-xiaobai"), "mecha-xiaobai");
    }
}
