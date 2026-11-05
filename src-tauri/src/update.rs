//! One-click addon updates (addons step 2). The only place Waygate writes in a
//! game folder, and only inside that version's `Interface\AddOns`, after the
//! player has said yes once (settings `addon_updates`).
//!
//! 1. The zip comes through our server (waygate.php?cf=download), which holds
//!    the CurseForge key and refuses addons whose authors don't allow other
//!    apps to download them.
//! 2. Every name in the zip is checked: plain top-level folders only, no paths
//!    that climb out, and at least one folder with a .toc.
//! 3. The addon's current folders are copied to
//!    %LOCALAPPDATA%\Waygate\backups\<product>\<id> first (one backup per
//!    addon, the latest), so Undo can put them back.
//! 4. If anything fails while replacing, the backup is put back at once.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs;
use std::io::{Cursor, Read};
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_ZIP: u64 = 200 * 1024 * 1024;
const MAX_UNPACKED: u64 = 1024 * 1024 * 1024;
const MAX_ENTRIES: usize = 30_000;

#[derive(Serialize, Deserialize, Clone)]
pub struct Backup {
    pub product: String,
    pub id: String,
    pub title: String,
    /// The version that was replaced (what Undo brings back).
    pub version: String,
    /// Folders that were in AddOns before and are kept in the backup.
    pub saved: Vec<String>,
    /// Folders the update put in AddOns (Undo removes them first).
    pub added: Vec<String>,
    /// Seconds since 1970.
    pub at: u64,
}

/// A folder name that can only ever mean a folder directly inside AddOns.
pub fn safe_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 120
        && !name.starts_with('.')
        && !name.ends_with(['.', ' '])
        && !name.contains(['/', '\\', ':', '*', '?', '"', '<', '>', '|'])
        && !name.chars().any(char::is_control)
}

fn backups_root() -> Result<PathBuf, String> {
    let base = std::env::var_os("LOCALAPPDATA").ok_or("No LOCALAPPDATA folder.")?;
    Ok(PathBuf::from(base).join("Waygate").join("backups"))
}

fn backup_dir(product: &str, id: &str) -> Result<PathBuf, String> {
    if !safe_name(product) || !id.chars().all(|c| c.is_ascii_digit()) || id.is_empty() {
        return Err("Unknown addon.".into());
    }
    Ok(backups_root()?.join(product).join(id))
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

fn remove_dir(path: &Path) -> std::io::Result<()> {
    match fs::remove_dir_all(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        other => other,
    }
}

fn download(id: &str, file_id: u64) -> Result<Vec<u8>, String> {
    let url = format!("https://ctrlshiftzed.com/api/waygate.php?cf=download&mod={id}&file={file_id}");
    let agent = ureq::AgentBuilder::new()
        .tls_connector(std::sync::Arc::new(native_tls::TlsConnector::new().map_err(|e| e.to_string())?))
        .timeout(std::time::Duration::from_secs(180))
        .user_agent(concat!("Waygate/", env!("CARGO_PKG_VERSION")))
        .build();
    let response = match agent.get(&url).call() {
        Ok(r) => r,
        Err(ureq::Error::Status(403, _)) => return Err("The author only allows downloads from CurseForge itself.".into()),
        Err(ureq::Error::Status(429, _)) => return Err("Too many downloads; try again in a few minutes.".into()),
        Err(ureq::Error::Status(503, _)) => return Err("Updates aren't set up on the server yet.".into()),
        Err(ureq::Error::Status(code, _)) => return Err(format!("The download failed (server said {code}).")),
        Err(e) => return Err(format!("The download failed: {e}")),
    };
    let mut bytes = Vec::new();
    response.into_reader().take(MAX_ZIP + 1).read_to_end(&mut bytes).map_err(|e| format!("The download broke off: {e}"))?;
    if bytes.len() as u64 > MAX_ZIP {
        return Err("That download is too big.".into());
    }
    Ok(bytes)
}

/// Unpacks into `stage` and returns the top-level folders. Refuses anything
/// that isn't plain folders of files.
pub fn unpack(bytes: &[u8], stage: &Path) -> Result<Vec<String>, String> {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|_| "The download isn't a valid zip.".to_string())?;
    if zip.len() > MAX_ENTRIES {
        return Err("The zip has too many files.".into());
    }
    let mut tops = BTreeSet::new();
    let mut total: u64 = 0;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|e| e.to_string())?;
        let Some(rel) = entry.enclosed_name() else { return Err("The zip has an unsafe file path.".into()) };
        let parts: Vec<String> = rel.components().map(|c| match c {
            Component::Normal(p) => Ok(p.to_string_lossy().to_string()),
            _ => Err("The zip has an unsafe file path.".to_string()),
        }).collect::<Result<_, _>>()?;
        if parts.iter().any(|p| !safe_name(p)) {
            return Err(format!("The zip has a file name Waygate won't write: {}", rel.display()));
        }
        // Files loose at the top of the zip (a readme) aren't addons: skipped.
        if parts.len() < 2 && !entry.is_dir() {
            continue;
        }
        tops.insert(parts[0].clone());
        let target = stage.join(&rel);
        if entry.is_dir() {
            fs::create_dir_all(&target).map_err(|e| e.to_string())?;
            continue;
        }
        total += entry.size();
        if total > MAX_UNPACKED {
            return Err("The zip unpacks too big.".into());
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut out = fs::File::create(&target).map_err(|e| e.to_string())?;
        std::io::copy(&mut entry, &mut out).map_err(|e| e.to_string())?;
    }
    let tops: Vec<String> = tops.into_iter().collect();
    let has_toc = tops.iter().any(|t| {
        fs::read_dir(stage.join(t)).map(|d| d.flatten().any(|e| e.file_name().to_string_lossy().to_lowercase().ends_with(".toc"))).unwrap_or(false)
    });
    if tops.is_empty() || !has_toc {
        return Err("The download doesn't look like a World of Warcraft addon.".into());
    }
    Ok(tops)
}

/// Copies `saved` folders back from the backup and removes `added`, inside `addons`.
fn put_back(addons: &Path, dir: &Path, saved: &[String], added: &[String]) -> Result<(), String> {
    for name in added.iter().filter(|n| safe_name(n)) {
        remove_dir(&addons.join(name)).map_err(|e| format!("Couldn't remove {name}: {e}"))?;
    }
    for name in saved.iter().filter(|n| safe_name(n)) {
        let target = addons.join(name);
        remove_dir(&target).map_err(|e| format!("Couldn't clear {name}: {e}"))?;
        copy_dir(&dir.join("folders").join(name), &target).map_err(|e| format!("Couldn't restore {name}: {e}"))?;
    }
    Ok(())
}

/// Replaces an addon's folders in `addons` with the ones in `bytes`.
pub fn install(addons: &Path, product: &str, id: &str, title: &str, version: &str, old_folders: &[String], bytes: &[u8]) -> Result<Backup, String> {
    if !addons.is_dir() || !addons.ends_with(Path::new("Interface").join("AddOns")) {
        return Err("The AddOns folder wasn't found.".into());
    }
    let stage = std::env::temp_dir().join(format!("waygate-stage-{id}-{}", now()));
    remove_dir(&stage).ok();
    let result = (|| {
        let new_folders = unpack(bytes, &stage)?;

        // Back up everything this update touches: the addon's folders and any
        // folder of the same name as one in the zip.
        let dir = backup_dir(product, id)?;
        remove_dir(&dir).map_err(|e| e.to_string())?;
        fs::create_dir_all(dir.join("folders")).map_err(|e| e.to_string())?;
        let touched: BTreeSet<String> = old_folders.iter().chain(new_folders.iter()).filter(|n| safe_name(n)).cloned().collect();
        let mut saved = Vec::new();
        for name in &touched {
            let src = addons.join(name);
            if src.is_dir() {
                copy_dir(&src, &dir.join("folders").join(name)).map_err(|e| format!("Couldn't back up {name}: {e}"))?;
                saved.push(name.clone());
            }
        }
        let backup = Backup { product: product.into(), id: id.into(), title: title.into(), version: version.into(), saved, added: new_folders.clone(), at: now() };
        fs::write(dir.join("backup.json"), serde_json::to_string_pretty(&backup).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;

        // Replace: folders the addon no longer ships go, the new ones come in.
        let replace = || -> Result<(), String> {
            for name in &touched {
                remove_dir(&addons.join(name)).map_err(|e| format!("Couldn't replace {name}: {e}"))?;
            }
            for name in &new_folders {
                copy_dir(&stage.join(name), &addons.join(name)).map_err(|e| format!("Couldn't write {name}: {e}"))?;
            }
            Ok(())
        };
        if let Err(e) = replace() {
            let rolled = put_back(addons, &dir, &backup.saved, &backup.added);
            remove_dir(&dir).ok();
            return Err(match rolled {
                Ok(()) => format!("{e}. Nothing was changed: the old version is back."),
                Err(r) => format!("{e}. Putting the old version back also failed ({r}); it is saved in {}.", dir.display()),
            });
        }
        Ok(backup)
    })();
    remove_dir(&stage).ok();
    result
}

/// Puts the version from before the last update back.
pub fn undo(addons: &Path, product: &str, id: &str) -> Result<Backup, String> {
    let dir = backup_dir(product, id)?;
    let text = fs::read_to_string(dir.join("backup.json")).map_err(|_| "There's nothing to undo for this addon.".to_string())?;
    let backup: Backup = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    if !addons.ends_with(Path::new("Interface").join("AddOns")) {
        return Err("The AddOns folder wasn't found.".into());
    }
    put_back(addons, &dir, &backup.saved, &backup.added)?;
    remove_dir(&dir).ok();
    Ok(backup)
}

/// The backups kept for one game version, newest first.
pub fn backups(product: &str) -> Vec<Backup> {
    let Ok(root) = backups_root() else { return Vec::new() };
    if !safe_name(product) {
        return Vec::new();
    }
    let mut out: Vec<Backup> = fs::read_dir(root.join(product)).map(|d| d.flatten()
        .filter_map(|e| fs::read_to_string(e.path().join("backup.json")).ok())
        .filter_map(|t| serde_json::from_str(&t).ok())
        .collect()).unwrap_or_default();
    out.sort_by(|a, b| b.at.cmp(&a.at));
    out
}

pub fn update(addons: &Path, product: &str, id: &str, title: &str, version: &str, old_folders: &[String], file_id: u64) -> Result<Backup, String> {
    let bytes = download(id, file_id)?;
    install(addons, product, id, title, version, old_folders, &bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn zip_of(files: &[(&str, &str)]) -> Vec<u8> {
        let mut buf = Cursor::new(Vec::new());
        {
            let mut z = zip::ZipWriter::new(&mut buf);
            let opts = zip::write::SimpleFileOptions::default();
            for (name, body) in files {
                z.start_file(*name, opts).unwrap();
                z.write_all(body.as_bytes()).unwrap();
            }
            z.finish().unwrap();
        }
        buf.into_inner()
    }

    fn setup(name: &str) -> (PathBuf, PathBuf) {
        let root = std::env::temp_dir().join(format!("waygate-update-test-{name}"));
        let _ = fs::remove_dir_all(&root);
        let addons = root.join("_retail_").join("Interface").join("AddOns");
        fs::create_dir_all(addons.join("MyAddon")).unwrap();
        fs::write(addons.join("MyAddon").join("MyAddon.toc"), "## Version: 1.0\n").unwrap();
        fs::create_dir_all(addons.join("MyAddon_Old")).unwrap();
        fs::write(addons.join("MyAddon_Old").join("x.lua"), "old").unwrap();
        fs::create_dir_all(addons.join("Other")).unwrap();
        (root, addons)
    }

    #[test]
    fn names_that_could_escape_are_refused() {
        assert!(safe_name("DBM-Core"));
        assert!(!safe_name(".."));
        assert!(!safe_name("a\\b"));
        assert!(!safe_name("C:"));
        assert!(!safe_name(""));
    }

    #[test]
    fn bad_zips_are_refused_before_anything_is_written() {
        let stage = std::env::temp_dir().join("waygate-update-test-stage");
        let _ = fs::remove_dir_all(&stage);
        assert!(unpack(b"not a zip", &stage).is_err());
        assert!(unpack(&zip_of(&[("../evil/x.lua", "x")]), &stage).is_err());
        assert!(unpack(&zip_of(&[("Folder/readme.txt", "no toc")]), &stage).is_err());
        let _ = fs::remove_dir_all(&stage);
    }

    #[test]
    fn update_then_undo_round_trip() {
        let (root, addons) = setup("roundtrip");
        std::env::set_var("LOCALAPPDATA", root.join("local"));
        let zip = zip_of(&[("MyAddon/MyAddon.toc", "## Version: 2.0\n"), ("MyAddon_New/y.lua", "new"), ("README.md", "loose")]);
        let old = vec!["MyAddon".to_string(), "MyAddon_Old".to_string()];

        let b = install(&addons, "wow", "42", "My Addon", "1.0", &old, &zip).unwrap();
        assert_eq!(fs::read_to_string(addons.join("MyAddon").join("MyAddon.toc")).unwrap(), "## Version: 2.0\n");
        assert!(addons.join("MyAddon_New").is_dir());
        assert!(!addons.join("MyAddon_Old").exists(), "a folder the addon dropped is removed");
        assert!(!addons.join("README.md").exists());
        assert!(addons.join("Other").is_dir(), "other addons are untouched");
        assert_eq!(b.saved, vec!["MyAddon", "MyAddon_Old"]);
        assert_eq!(backups("wow").len(), 1);

        undo(&addons, "wow", "42").unwrap();
        assert_eq!(fs::read_to_string(addons.join("MyAddon").join("MyAddon.toc")).unwrap(), "## Version: 1.0\n");
        assert!(addons.join("MyAddon_Old").is_dir());
        assert!(!addons.join("MyAddon_New").exists());
        assert!(backups("wow").is_empty());
        assert!(undo(&addons, "wow", "42").is_err(), "nothing left to undo");
        let _ = fs::remove_dir_all(&root);
    }
}
