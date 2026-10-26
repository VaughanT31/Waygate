//! Installed Blizzard games.
//!
//! Read only, and only from two places (scope.md section 4):
//! - the Windows uninstall list (HKLM ...\Uninstall), where Battle.net registers
//!   each game with its product id in the UninstallString (`--uid=wow`);
//! - the game's own `.build.info` in its install folder, for the installed
//!   versions (retail, classic...) and their region.
//! Nothing that belongs to Battle.net itself is ever read.

use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Serialize, Clone)]
pub struct Installed {
    /// Where Battle.net.exe is, if it's installed.
    pub battle_net: Option<String>,
    pub games: Vec<Game>,
}

#[derive(Serialize, Clone)]
pub struct Game {
    /// Battle.net's product id from the uninstall entry, like "wow".
    pub uid: String,
    pub name: String,
    /// A short label for the game rail.
    pub short: String,
    pub install: String,
    pub flavors: Vec<Flavor>,
}

#[derive(Serialize, Clone)]
pub struct Flavor {
    /// The product in .build.info, like "wow" or "wow_classic_era".
    pub product: String,
    pub name: String,
    pub version: String,
    pub region: String,
    /// The build's fingerprint (Build Key), compared with the patch server's
    /// BuildConfig so a rebuild under the same version still counts as new.
    pub build: String,
    /// Battle.net's launch code for `--exec="launch <code>"`. Only codes
    /// checked on a real PC are listed; the rest open Battle.net instead.
    pub launch: Option<String>,
}

/// Games Waygate knows: Battle.net uid, rail label.
const GAMES: &[(&str, &str)] = &[("wow", "W"), ("prometheus", "OW")];

/// .build.info products: name, launch code once confirmed (scope.md section 2).
fn flavor_info(product: &str) -> (&'static str, Option<&'static str>) {
    match product {
        "wow" => ("Retail", Some("WoW")),
        "wow_classic" => ("Mists of Pandaria Classic", None),
        "wow_anniversary" => ("Burning Crusade Anniversary", None),
        "wow_classic_era" => ("Classic Era", None),
        "wow_ptr" | "wowt" => ("Public Test Realm", None),
        "wow_beta" | "wowxptr" => ("Beta", None),
        "pro" => ("Overwatch", None),
        _ => ("", None),
    }
}

struct Entry {
    name: String,
    install: String,
    icon: String,
    uninstall: String,
}

#[cfg(windows)]
fn uninstall_entries() -> Vec<Entry> {
    use winreg::enums::{HKEY_LOCAL_MACHINE, KEY_READ};
    use winreg::RegKey;

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let mut out = Vec::new();
    for path in [
        r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall",
        r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
    ] {
        let Ok(root) = hklm.open_subkey_with_flags(path, KEY_READ) else { continue };
        for name in root.enum_keys().flatten() {
            let Ok(key) = root.open_subkey_with_flags(&name, KEY_READ) else { continue };
            let get = |value: &str| key.get_value::<String, _>(value).unwrap_or_default();
            if !get("Publisher").contains("Blizzard") {
                continue;
            }
            out.push(Entry {
                name: get("DisplayName"),
                install: get("InstallLocation"),
                icon: get("DisplayIcon"),
                uninstall: get("UninstallString"),
            });
        }
    }
    out
}

#[cfg(not(windows))]
fn uninstall_entries() -> Vec<Entry> {
    Vec::new()
}

/// `... --uid=wow --displayname=...` gives "wow".
fn uid_of(uninstall: &str) -> Option<String> {
    let rest = &uninstall[uninstall.find("--uid=")? + "--uid=".len()..];
    let uid: String = rest.chars().take_while(|c| !c.is_whitespace() && *c != '"').collect();
    (!uid.is_empty()).then_some(uid)
}

/// The installed versions from the game folder's .build.info: a `|` separated
/// table whose header names each column as `Name!TYPE:size`.
pub fn read_build_info(install: &Path) -> Vec<Flavor> {
    let Ok(text) = fs::read_to_string(install.join(".build.info")) else { return Vec::new() };
    let mut lines = text.lines();
    let Some(header) = lines.next() else { return Vec::new() };
    let cols: Vec<&str> = header.split('|').map(|c| c.split('!').next().unwrap_or("")).collect();
    let col = |name: &str| cols.iter().position(|c| *c == name);
    let (Some(product_col), Some(version_col)) = (col("Product"), col("Version")) else { return Vec::new() };
    let (branch_col, active_col, build_col) = (col("Branch"), col("Active"), col("Build Key"));

    let mut out: Vec<Flavor> = Vec::new();
    for line in lines.filter(|l| !l.trim().is_empty()) {
        let fields: Vec<&str> = line.split('|').collect();
        let field = |i: Option<usize>| i.and_then(|i| fields.get(i)).map(|s| s.trim()).unwrap_or("");
        if field(active_col) == "0" {
            continue;
        }
        let product = field(Some(product_col)).to_string();
        if product.is_empty() || out.iter().any(|f| f.product == product) {
            continue;
        }
        let (name, launch) = flavor_info(&product);
        out.push(Flavor {
            name: if name.is_empty() { product.clone() } else { name.to_string() },
            version: field(Some(version_col)).to_string(),
            region: field(branch_col).to_uppercase(),
            build: field(build_col).to_lowercase(),
            launch: launch.map(str::to_string),
            product,
        });
    }
    out
}

fn battle_net_exe(entry: &Entry) -> PathBuf {
    if entry.icon.to_lowercase().ends_with("battle.net.exe") {
        PathBuf::from(&entry.icon)
    } else {
        Path::new(&entry.install).join("Battle.net.exe")
    }
}

pub fn scan() -> Installed {
    let mut battle_net = None;
    let mut games = Vec::new();
    for entry in uninstall_entries() {
        let Some(uid) = uid_of(&entry.uninstall) else { continue };
        if uid == "battle.net" {
            let exe = battle_net_exe(&entry);
            if exe.is_file() {
                battle_net = Some(exe.to_string_lossy().into_owned());
            }
            continue;
        }
        if games.iter().any(|g: &Game| g.uid == uid) {
            continue;
        }
        let short = GAMES.iter().find(|(id, _)| *id == uid).map(|(_, s)| s.to_string())
            .unwrap_or_else(|| entry.name.chars().filter(|c| c.is_uppercase()).take(2).collect());
        games.push(Game {
            flavors: read_build_info(Path::new(&entry.install)),
            uid,
            name: entry.name,
            short,
            install: entry.install,
        });
    }
    // Known games first, in the order above, then the rest by name.
    let rank = |uid: &str| GAMES.iter().position(|(id, _)| *id == uid).unwrap_or(GAMES.len());
    games.sort_by(|a, b| rank(&a.uid).cmp(&rank(&b.uid)).then(a.name.cmp(&b.name)));
    Installed { battle_net, games }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uid_from_uninstall_string() {
        let s = r#""C:\ProgramData\Battle.net\Agent\Blizzard Uninstaller.exe" --lang=enUS --uid=wow --displayname="World of Warcraft""#;
        assert_eq!(uid_of(s).as_deref(), Some("wow"));
        assert_eq!(uid_of("no id here"), None);
    }

    #[test]
    fn build_info_rows() {
        let dir = std::env::temp_dir().join("waygate-test-build-info");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join(".build.info"),
            "Branch!STRING:0|Active!DEC:1|Version!STRING:0|Product!STRING:0\n\
             eu|1|12.1.5.70529|wow\n\
             eu|1|1.15.9.66000|wow_classic_era\n\
             eu|0|0.0.1|wow_beta\n",
        )
        .unwrap();
        let flavors = read_build_info(&dir);
        assert_eq!(flavors.len(), 2);
        assert_eq!((flavors[0].name.as_str(), flavors[0].version.as_str(), flavors[0].region.as_str()), ("Retail", "12.1.5.70529", "EU"));
        assert_eq!(flavors[0].launch.as_deref(), Some("WoW"));
        assert_eq!(flavors[1].name, "Classic Era");
        assert_eq!(flavors[1].launch, None);
        fs::remove_dir_all(&dir).ok();
    }
}
