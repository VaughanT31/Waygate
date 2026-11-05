//! Installed addons, read only: each WoW version folder (`_retail_`,
//! `_classic_`...) names its product in `.flavor.info`, and every folder in
//! its `Interface\AddOns` has a `.toc` with the title, version and the
//! CurseForge and Wago project ids. Nothing in the game folder is written.

use serde::Serialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Serialize, Clone)]
pub struct Flavor {
    pub product: String,
    /// The game version's own folder name, like `_retail_`.
    pub folder: String,
    /// The interface number this version expects (12.1.5 is 120105), when known.
    pub interface: Option<u32>,
    /// The installed game version, like 12.1.5.70575, when known.
    pub game_version: Option<String>,
    pub addons: Vec<Addon>,
}

#[derive(Serialize, Clone)]
pub struct Addon {
    pub title: String,
    pub version: String,
    pub author: String,
    pub notes: String,
    /// Folders that make up this addon (an addon can ship several).
    pub folders: Vec<String>,
    pub interfaces: Vec<u32>,
    pub curse_id: Option<String>,
    pub wago_id: Option<String>,
    /// Built for an older version of the game than the one installed.
    pub out_of_date: bool,
}

/// 12.1.5.70575 gives 120105 (major, minor and patch as MMmmpp).
pub fn interface_of(version: &str) -> Option<u32> {
    let mut parts = version.split('.').map(|p| p.parse::<u32>().ok());
    let (major, minor, patch) = (parts.next()??, parts.next()??, parts.next()??);
    Some(major * 10000 + minor * 100 + patch)
}

/// Toc suffixes each version reads first, before the plain `<Name>.toc`.
fn toc_suffixes(product: &str) -> &'static [&'static str] {
    match product {
        "wow" | "wow_ptr" | "wowt" | "wow_beta" | "wowxptr" => &["Mainline"],
        "wow_classic" | "wow_classic_ptr" | "wow_classic_beta" => &["Mists", "Classic"],
        "wow_classic_era" | "wow_classic_era_ptr" => &["Vanilla", "Classic"],
        "wow_anniversary" => &["TBC", "BCC", "Classic"],
        _ => &[],
    }
}

/// `|cffff8000Name|r` and `|T...|t` icons out of a toc title.
fn plain(text: &str) -> String {
    let mut out = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '|' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('c') | Some('C') => {
                for _ in 0..8 {
                    chars.next();
                }
            }
            Some('r') | Some('R') => {}
            Some('T') | Some('t') | Some('A') | Some('a') => {
                for n in chars.by_ref() {
                    if n == '|' {
                        chars.next();
                        break;
                    }
                }
            }
            Some(other) => {
                out.push('|');
                out.push(other);
            }
            None => out.push('|'),
        }
    }
    out.trim().to_string()
}

fn read_toc(dir: &Path, name: &str, product: &str) -> Option<BTreeMap<String, String>> {
    let entries: Vec<String> = fs::read_dir(dir).ok()?.flatten()
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|f| f.to_lowercase().ends_with(".toc"))
        .collect();
    let find = |wanted: String| entries.iter().find(|f| f.to_lowercase() == wanted.to_lowercase()).cloned();
    let file = toc_suffixes(product).iter()
        .find_map(|s| find(format!("{name}_{s}.toc")).or_else(|| find(format!("{name}-{s}.toc"))))
        .or_else(|| find(format!("{name}.toc")))?;
    let bytes = fs::read(dir.join(file)).ok()?;
    let text = String::from_utf8_lossy(&bytes);
    let mut fields = BTreeMap::new();
    for line in text.lines() {
        let Some(rest) = line.trim_start_matches('\u{feff}').strip_prefix("##") else { continue };
        let Some((key, value)) = rest.split_once(':') else { continue };
        fields.entry(key.trim().to_lowercase()).or_insert_with(|| value.trim().to_string());
    }
    Some(fields)
}

/// `DBM-Core` and `DBM-GUI` share the prefix `dbm`.
fn prefix(folder: &str) -> String {
    folder.split(['-', '_']).next().unwrap_or(folder).to_lowercase()
}

fn scan_addons(addons_dir: &Path, product: &str, interface: Option<u32>) -> Vec<Addon> {
    let Ok(dirs) = fs::read_dir(addons_dir) else { return Vec::new() };
    let mut found: Vec<(String, BTreeMap<String, String>)> = dirs.flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| {
            let folder = e.file_name().into_string().ok()?;
            let toc = read_toc(&e.path(), &folder, product)?;
            Some((folder, toc))
        })
        .collect();
    found.sort_by_key(|(f, _)| f.to_lowercase());

    // Folders of one addon are grouped: by the CurseForge or Wago id they share,
    // or, without one, by needing another folder with the same name prefix
    // (DBM-GUI needs DBM-Core).
    let id_key = |toc: &BTreeMap<String, String>| {
        toc.get("x-curse-project-id").filter(|v| !v.is_empty()).map(|c| format!("c:{c}"))
            .or_else(|| toc.get("x-wago-id").filter(|v| !v.is_empty()).map(|w| format!("w:{w}")))
    };
    let own_key: BTreeMap<String, String> = found.iter()
        .map(|(f, toc)| (f.to_lowercase(), id_key(toc).unwrap_or_else(|| format!("f:{}", f.to_lowercase()))))
        .collect();
    let group_of = |folder: &str, toc: &BTreeMap<String, String>| -> String {
        if let Some(key) = id_key(toc) {
            return key;
        }
        let deps = ["dependencies", "requireddeps", "dep", "reqdeps"].iter()
            .filter_map(|k| toc.get(*k)).flat_map(|d| d.split(',')).map(|d| d.trim().to_lowercase());
        for dep in deps {
            if dep != folder.to_lowercase() && prefix(&dep) == prefix(folder) {
                if let Some(key) = own_key.get(&dep) {
                    return key.clone();
                }
            }
        }
        format!("f:{}", folder.to_lowercase())
    };

    let mut by_key: BTreeMap<String, Addon> = BTreeMap::new();
    let mut roots: BTreeMap<String, (bool, usize)> = BTreeMap::new();
    for (folder, toc) in &found {
        let get = |k: &str| toc.get(k).cloned().unwrap_or_default();
        let interfaces: Vec<u32> = get("interface").split(',').filter_map(|v| v.trim().parse().ok()).collect();
        let title = plain(&get("title"));
        let key = group_of(folder, toc);
        let addon = by_key.entry(key.clone()).or_insert_with(|| Addon {
            title: if title.is_empty() { folder.clone() } else { title.clone() },
            version: get("version"),
            author: plain(&get("author")),
            notes: plain(&get("notes")),
            folders: Vec::new(),
            interfaces: interfaces.clone(),
            curse_id: None,
            wago_id: None,
            out_of_date: false,
        });
        // The addon itself names the group: the folder carrying the project id
        // with the shortest name (DBM-Core, not DBM-Raids-Midnight).
        let rank = (id_key(toc).is_none(), folder.len());
        if rank < roots.get(&key).copied().unwrap_or((true, usize::MAX)) {
            roots.insert(key.clone(), rank);
            addon.title = if title.is_empty() { folder.clone() } else { title };
            addon.version = get("version");
            addon.interfaces = interfaces;
        }
        addon.curse_id = addon.curse_id.take().or_else(|| toc.get("x-curse-project-id").filter(|v| !v.is_empty()).cloned());
        addon.wago_id = addon.wago_id.take().or_else(|| toc.get("x-wago-id").filter(|v| !v.is_empty()).cloned());
        addon.folders.push(folder.clone());
    }
    let mut out: Vec<Addon> = by_key.into_values().map(|mut a| {
        if let Some(current) = interface {
            // Only numbers for this game's major version count (a toc can list Classic too).
            let best = a.interfaces.iter().copied().filter(|v| v / 10000 == current / 10000).max();
            a.out_of_date = best.is_none_or(|v| v < current);
        }
        a
    }).collect();
    out.sort_by_key(|a| a.title.to_lowercase());
    out
}

/// Every WoW version folder in the install: (product, folder name, path).
pub fn version_folders(install: &Path) -> Vec<(String, String, PathBuf)> {
    let Ok(dirs) = fs::read_dir(install) else { return Vec::new() };
    let mut out = Vec::new();
    for entry in dirs.flatten() {
        let path = entry.path();
        let Ok(folder) = entry.file_name().into_string() else { continue };
        if !folder.starts_with('_') || !path.is_dir() {
            continue;
        }
        let Ok(info) = fs::read_to_string(path.join(".flavor.info")) else { continue };
        let Some(product) = info.lines().map(str::trim).find(|l| !l.is_empty() && !l.contains('!')) else { continue };
        out.push((product.to_string(), folder, path));
    }
    out
}

/// Every WoW version folder in the install, with its addons.
pub fn scan(install: &Path, versions: &[(String, String)]) -> Vec<Flavor> {
    let mut out = Vec::new();
    for (product, folder, path) in version_folders(install) {
        let game_version = versions.iter().find(|(p, _)| *p == product).map(|(_, v)| v.clone());
        let interface = game_version.as_deref().and_then(interface_of);
        out.push(Flavor {
            game_version,
            addons: scan_addons(&path.join("Interface").join("AddOns"), &product, interface),
            product,
            folder,
            interface,
        });
    }
    out.sort_by_key(|f| (f.product != "wow", f.product.clone()));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interface_numbers() {
        assert_eq!(interface_of("12.1.5.70575"), Some(120105));
        assert_eq!(interface_of("1.15.9.70003"), Some(11509));
        assert_eq!(interface_of("bad"), None);
    }

    #[test]
    fn title_codes_removed() {
        assert_eq!(plain("|cff33ff99Auction|r Coach |TInterface\\Icons\\x:16|t"), "Auction Coach");
    }

    #[test]
    fn addons_grouped_and_dated() {
        let root = std::env::temp_dir().join("waygate-test-addons");
        let _ = fs::remove_dir_all(&root);
        let addons = root.join("_retail_").join("Interface").join("AddOns");
        for (folder, toc) in [
            ("BigAddon", "## Title: Big Addon\n## Version: 2.0\n## Interface: 120105\n## X-Curse-Project-ID: 42\n"),
            ("BigAddon_Module", "## Title: Big Addon Module\n## Interface: 120105\n## X-Curse-Project-ID: 42\n"),
            ("OldOne", "## Title: Old One\n## Interface: 110207, 11509\n"),
            ("BigAddon-Options", "## Title: Big Addon Options\n## Interface: 120105\n## Dependencies: BigAddon\n"),
        ] {
            fs::create_dir_all(addons.join(folder)).unwrap();
            fs::write(addons.join(folder).join(format!("{folder}.toc")), toc).unwrap();
        }
        // A Mainline toc wins over the plain one on retail.
        fs::write(addons.join("OldOne").join("OldOne_Mainline.toc"), "## Title: Old One\n## Interface: 120105\n").unwrap();
        fs::write(root.join("_retail_").join(".flavor.info"), "Product Flavor!STRING:0\nwow\n").unwrap();

        let flavors = scan(&root, &[("wow".into(), "12.1.5.70575".into())]);
        assert_eq!(flavors.len(), 1);
        let list = &flavors[0].addons;
        assert_eq!(list.len(), 2);
        assert_eq!((list[0].title.as_str(), list[0].folders.len(), list[0].out_of_date), ("Big Addon", 3, false));
        assert_eq!((list[1].title.as_str(), list[1].out_of_date), ("Old One", false));
        fs::remove_dir_all(&root).ok();
    }
}
