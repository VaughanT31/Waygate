//! Characters (scope.md phase 4).
//!
//! Found on this PC: the game makes a folder per character in
//! `_retail_\WTF\Account\<account>\<Realm>\<Character>`. Only folder names
//! are read, plus the date `config-cache.wtf` last changed (WoW saves it on
//! logout, so it's when the character was last played). Nothing in WTF is
//! opened or written, and the account folder's name is never kept or shown.
//!
//! Details come from Raider.IO by realm and name: class, spec, item level and
//! Mythic+ score. No Blizzard login or Profile API.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, Instant, UNIX_EPOCH};

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Found {
    pub name: String,
    pub realm: String,
    /// When it was last logged out of (ISO), if the game saved it.
    pub played: Option<String>,
}

/// A character to show, as kept in settings.
#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct Tracked {
    pub name: String,
    pub realm: String,
}

fn iso_from_secs(secs: u64) -> String {
    crate::online::iso_from_ms(secs as i64 * 1000)
}

/// Characters in a WoW version folder's WTF, most recently played first.
pub fn found(version_folder: &Path) -> Vec<Found> {
    let accounts = version_folder.join("WTF").join("Account");
    let mut out: Vec<(u64, Found)> = Vec::new();
    for account in fs::read_dir(&accounts).into_iter().flatten().flatten() {
        if !account.path().is_dir() {
            continue;
        }
        for realm in fs::read_dir(account.path()).into_iter().flatten().flatten() {
            let Ok(realm_name) = realm.file_name().into_string() else { continue };
            if realm_name == "SavedVariables" || !realm.path().is_dir() {
                continue;
            }
            for character in fs::read_dir(realm.path()).into_iter().flatten().flatten() {
                let Ok(name) = character.file_name().into_string() else { continue };
                if !character.path().is_dir() {
                    continue;
                }
                let secs = fs::metadata(character.path().join("config-cache.wtf")).and_then(|m| m.modified()).ok()
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map(|d| d.as_secs()).unwrap_or(0);
                // The same character under two accounts: keep the newest.
                if let Some(seen) = out.iter_mut().find(|(_, f)| f.name == name && f.realm == realm_name) {
                    if secs > seen.0 {
                        seen.0 = secs;
                        seen.1.played = Some(iso_from_secs(secs));
                    }
                    continue;
                }
                out.push((secs, Found { played: (secs > 0).then(|| iso_from_secs(secs)), name, realm: realm_name.clone() }));
            }
        }
    }
    out.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.name.cmp(&b.1.name)));
    out.into_iter().map(|(_, f)| f).collect()
}

#[derive(Serialize, Clone)]
pub struct Profile {
    pub name: String,
    pub realm: String,
    pub class: String,
    pub spec: String,
    pub ilvl: Option<f64>,
    pub score: f64,
    /// Raider.IO's colour for the score, like "#ff8000".
    pub color: Option<String>,
    /// When Raider.IO last updated the character (ISO).
    pub crawled: Option<String>,
    /// Best timed (or not) key per dungeon this season.
    pub runs: Vec<Run>,
    /// Key levels done since the weekly reset, highest first (the vault's M+ row).
    pub week: Vec<u32>,
    /// The same for last week, for "ready to claim" on reset day.
    pub last_week: Vec<u32>,
    /// Raid progress, like ("The Venomous Abyss", "2/8 M").
    pub raids: Vec<Raid>,
}

#[derive(Serialize, Clone)]
pub struct Run {
    /// Raider.IO's dungeon short name, like "AOF".
    pub dungeon: String,
    pub level: u32,
    /// 0 when it ran out of time, else how many levels the key went up.
    pub upgrades: u32,
}

#[derive(Serialize, Clone)]
pub struct Raid {
    pub name: String,
    pub summary: String,
}

/// "the-venomous-abyss" as "The Venomous Abyss".
fn raid_name(slug: &str) -> String {
    slug.split('-').filter(|w| !w.is_empty()).map(|w| {
        let mut c = w.chars();
        c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
    }).collect::<Vec<_>>().join(" ")
}

fn levels(json: &serde_json::Value, key: &str) -> Vec<u32> {
    let mut out: Vec<u32> = json.get(key).and_then(|r| r.as_array()).into_iter().flatten()
        .filter_map(|r| r.get("mythic_level").and_then(|l| l.as_u64()).map(|l| l as u32)).collect();
    out.sort_unstable_by(|a, b| b.cmp(a));
    out
}

fn rio_region(region: &str) -> &'static str {
    match region.to_lowercase().as_str() {
        "eu" => "eu",
        "kr" => "kr",
        "tw" => "tw",
        "cn" => "cn",
        _ => "us",
    }
}

/// Query text for a URL (names can have accents and realms apostrophes).
pub fn encode(s: &str) -> String {
    s.bytes().map(|b| match b {
        b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' => (b as char).to_string(),
        _ => format!("%{b:02X}"),
    }).collect()
}

fn lookup(region: &str, realm: &str, name: &str) -> Result<Profile, String> {
    let url = format!(
        "https://raider.io/api/v1/characters/profile?region={}&realm={}&name={}&fields=gear,mythic_plus_scores_by_season:current,mythic_plus_best_runs,mythic_plus_weekly_highest_level_runs,mythic_plus_previous_weekly_highest_level_runs,raid_progression",
        rio_region(region), encode(realm), encode(name)
    );
    let json: serde_json::Value = match crate::online::agent().get(&url).call() {
        Ok(r) => r.into_json().map_err(|e| e.to_string())?,
        Err(ureq::Error::Status(400 | 404, _)) => return Err("Raider.IO doesn't know this character yet.".into()),
        Err(e) => return Err(e.to_string()),
    };
    let text = |k: &str| json.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
    let season = json.pointer("/mythic_plus_scores_by_season/0");
    Ok(Profile {
        name: text("name"),
        realm: text("realm"),
        class: text("class"),
        spec: text("active_spec_name"),
        ilvl: json.pointer("/gear/item_level_equipped").and_then(|v| v.as_f64()),
        score: season.and_then(|s| s.pointer("/scores/all")).and_then(|v| v.as_f64()).unwrap_or(0.0),
        color: season.and_then(|s| s.pointer("/segments/all/color")).and_then(|v| v.as_str()).map(str::to_string),
        crawled: json.get("last_crawled_at").and_then(|v| v.as_str()).map(str::to_string),
        runs: json.get("mythic_plus_best_runs").and_then(|r| r.as_array()).into_iter().flatten().filter_map(|r| Some(Run {
            dungeon: r.get("short_name")?.as_str()?.to_string(),
            level: r.get("mythic_level")?.as_u64()? as u32,
            upgrades: r.get("num_keystone_upgrades").and_then(|u| u.as_u64()).unwrap_or(0) as u32,
        })).collect(),
        week: levels(&json, "mythic_plus_weekly_highest_level_runs"),
        last_week: levels(&json, "mythic_plus_previous_weekly_highest_level_runs"),
        raids: json.get("raid_progression").and_then(|r| r.as_object()).into_iter().flatten().filter_map(|(slug, r)| Some(Raid {
            name: raid_name(slug),
            summary: r.get("summary")?.as_str()?.to_string(),
        })).collect(),
    })
}

static CACHE: Mutex<Option<HashMap<String, (Instant, Result<Profile, String>)>>> = Mutex::new(None);
const FRESH: Duration = Duration::from_secs(15 * 60);

/// A character from Raider.IO, cached for 15 minutes (misses too).
pub fn profile(region: &str, realm: &str, name: &str) -> Result<Profile, String> {
    let key = format!("{}/{}/{}", rio_region(region), realm.to_lowercase(), name.to_lowercase());
    if let Ok(cache) = CACHE.lock() {
        if let Some((at, p)) = cache.as_ref().and_then(|c| c.get(&key)) {
            if at.elapsed() < FRESH {
                return p.clone();
            }
        }
    }
    let p = lookup(region, realm, name);
    if let Ok(mut cache) = CACHE.lock() {
        cache.get_or_insert_with(HashMap::new).insert(key, (Instant::now(), p.clone()));
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raid_names() {
        assert_eq!(raid_name("the-venomous-abyss"), "The Venomous Abyss");
    }

    #[test]
    fn query_encoding() {
        assert_eq!(encode("Quel'Thalas"), "Quel%27Thalas");
        assert_eq!(encode("Argent Dawn"), "Argent%20Dawn");
        assert_eq!(encode("Mägnumos"), "M%C3%A4gnumos");
    }

    #[test]
    fn found_reads_folder_names_only() {
        let root = std::env::temp_dir().join(format!("waygate-chars-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let acc = root.join("WTF").join("Account");
        fs::create_dir_all(acc.join("12345#1").join("SavedVariables")).unwrap();
        fs::create_dir_all(acc.join("12345#1").join("Draenor").join("Alpha")).unwrap();
        fs::create_dir_all(acc.join("12345#1").join("Quel'Thalas").join("Beta")).unwrap();
        fs::write(acc.join("12345#1").join("Quel'Thalas").join("Beta").join("config-cache.wtf"), "").unwrap();
        let found = found(&root);
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].name, "Beta");
        assert!(found[0].played.is_some());
        assert_eq!(found[1].played, None);
        assert!(found.iter().all(|f| !f.realm.contains('#')));
        fs::remove_dir_all(&root).unwrap();
    }
}
