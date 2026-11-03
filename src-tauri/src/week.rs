//! This week in World of Warcraft (scope.md phase 3), public data only:
//! Raider.IO's Mythic+ affixes and current season, which also give each
//! region's weekly reset (seasons start on a reset). No key, no account.

use serde::Serialize;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Raider.IO's id for Midnight, the expansion the season list comes from.
/// Bump with each expansion.
const EXPANSION_ID: u32 = 11;
const FRESH: Duration = Duration::from_secs(30 * 60);

#[derive(Serialize, Clone)]
pub struct Affix {
    pub name: String,
    pub description: String,
    /// Raider.IO's copy of the game icon, or None if it isn't on their CDN.
    pub icon: Option<String>,
}

#[derive(Serialize, Clone)]
pub struct Week {
    pub region: String,
    pub affixes: Vec<Affix>,
    /// The current season, like "MN Season 2", and when it started in this
    /// region (ISO, always on a weekly reset).
    pub season: Option<String>,
    pub season_start: Option<String>,
    /// The season's dungeons, in Raider.IO's order.
    pub dungeons: Vec<Dungeon>,
}

#[derive(Serialize, Clone)]
pub struct Dungeon {
    pub name: String,
    /// Raider.IO's short name, like "AOF", which runs refer to.
    pub short: String,
    pub icon: Option<String>,
}

/// The main season running now: name, start and dungeons.
struct Season {
    name: String,
    start: String,
    dungeons: Vec<Dungeon>,
}

/// Raider.IO's region for a .build.info region.
fn rio_region(region: &str) -> &'static str {
    match region.to_lowercase().as_str() {
        "eu" => "eu",
        "kr" => "kr",
        "tw" => "tw",
        "cn" => "cn",
        _ => "us",
    }
}

fn icon_url(url: &str) -> Option<String> {
    url.starts_with("https://cdn.raiderio.net/").then(|| url.to_string())
}

fn affixes(region: &str) -> Result<Vec<Affix>, String> {
    let url = format!("https://raider.io/api/v1/mythic-plus/affixes?region={region}&locale=en");
    let json: serde_json::Value = crate::online::agent().get(&url).call().map_err(|e| e.to_string())?.into_json().map_err(|e| e.to_string())?;
    let list = json.get("affix_details").and_then(|a| a.as_array()).cloned().unwrap_or_default();
    Ok(list.iter().filter_map(|a| {
        let name = a.get("name")?.as_str()?.trim().to_string();
        Some(Affix {
            description: a.get("description").and_then(|d| d.as_str()).unwrap_or("").trim().to_string(),
            icon: a.get("icon_url").and_then(|u| u.as_str()).and_then(icon_url),
            name,
        })
    }).collect())
}

/// The main season running now in a region.
fn season(region: &str) -> Result<Option<Season>, String> {
    let url = format!("https://raider.io/api/v1/mythic-plus/static-data?expansion_id={EXPANSION_ID}");
    let json: serde_json::Value = crate::online::agent().get(&url).call().map_err(|e| e.to_string())?.into_json().map_err(|e| e.to_string())?;
    let seasons = json.get("seasons").and_then(|s| s.as_array()).cloned().unwrap_or_default();
    let now = crate::online::now_iso();
    // ISO dates in the same format compare as text; the newest started season wins.
    let mut best: Option<Season> = None;
    for s in &seasons {
        if s.get("is_main_season").and_then(|m| m.as_bool()) != Some(true) {
            continue;
        }
        let start = s.pointer(&format!("/starts/{region}")).and_then(|v| v.as_str()).unwrap_or("");
        let end = s.pointer(&format!("/ends/{region}")).and_then(|v| v.as_str()).unwrap_or("");
        if start.is_empty() || start > now.as_str() || (!end.is_empty() && end <= now.as_str()) {
            continue;
        }
        if best.as_ref().map_or(true, |b| start > b.start.as_str()) {
            let dungeons = s.get("dungeons").and_then(|d| d.as_array()).cloned().unwrap_or_default().iter().filter_map(|d| Some(Dungeon {
                name: d.get("name")?.as_str()?.to_string(),
                short: d.get("short_name")?.as_str()?.to_string(),
                icon: d.get("icon_url").and_then(|u| u.as_str()).and_then(icon_url),
            })).collect();
            let name = s.get("name").and_then(|n| n.as_str()).unwrap_or("").to_string();
            best = Some(Season { name, start: start.to_string(), dungeons });
        }
    }
    Ok(best)
}

static CACHE: Mutex<Option<HashMap<String, (Instant, Week)>>> = Mutex::new(None);

/// This week for a region, cached for 30 minutes. Errors only when nothing loaded.
pub fn this_week(region: &str) -> Result<Week, String> {
    let region = rio_region(region);
    if let Ok(cache) = CACHE.lock() {
        if let Some((at, w)) = cache.as_ref().and_then(|c| c.get(region)) {
            if at.elapsed() < FRESH {
                return Ok(w.clone());
            }
        }
    }
    let affixes = affixes(region);
    let season = season(region);
    if affixes.is_err() && season.is_err() {
        return Err(affixes.err().unwrap_or_default());
    }
    let season = season.ok().flatten();
    let week = Week {
        region: region.to_string(),
        affixes: affixes.unwrap_or_default(),
        season_start: season.as_ref().map(|s| s.start.clone()),
        dungeons: season.as_ref().map(|s| s.dungeons.clone()).unwrap_or_default(),
        season: season.map(|s| s.name),
    };
    if let Ok(mut cache) = CACHE.lock() {
        cache.get_or_insert_with(HashMap::new).insert(region.to_string(), (Instant::now(), week.clone()));
    }
    Ok(week)
}

/// The WoW Token price, from our own server (ctrlshiftzed.com/api/waygate.php),
/// which holds the Blizzard API secret so the app never does.
#[derive(Serialize, serde::Deserialize, Clone)]
pub struct Token {
    pub region: String,
    /// In gold.
    pub price: u64,
    /// When Blizzard last updated it (ISO).
    pub updated: String,
    /// Percent change over about a day, once the server has a day of prices.
    pub change: Option<f64>,
}

static TOKEN_CACHE: Mutex<Option<HashMap<String, (Instant, Token)>>> = Mutex::new(None);

pub fn token(region: &str) -> Result<Token, String> {
    let region = rio_region(region);
    if region == "cn" {
        return Err("The WoW Token price isn't available for China.".into());
    }
    if let Ok(cache) = TOKEN_CACHE.lock() {
        if let Some((at, t)) = cache.as_ref().and_then(|c| c.get(region)) {
            if at.elapsed() < Duration::from_secs(5 * 60) {
                return Ok(t.clone());
            }
        }
    }
    let url = format!("https://ctrlshiftzed.com/api/waygate.php?token={region}");
    let t: Token = match crate::online::agent().get(&url).call() {
        Ok(r) => r.into_json().map_err(|e| e.to_string())?,
        Err(ureq::Error::Status(503, _)) => return Err("Not set up on the server yet.".into()),
        Err(e) => return Err(e.to_string()),
    };
    if let Ok(mut cache) = TOKEN_CACHE.lock() {
        cache.get_or_insert_with(HashMap::new).insert(region.to_string(), (Instant::now(), t.clone()));
    }
    Ok(t)
}

/// Whether a realm is up and how busy it is, from our own server.
#[derive(Serialize, serde::Deserialize, Clone)]
pub struct RealmStatus {
    pub realm: String,
    pub up: bool,
    /// Blizzard's words, like "Up" and "High".
    pub status: String,
    pub population: String,
}

static REALM_CACHE: Mutex<Option<HashMap<String, (Instant, RealmStatus)>>> = Mutex::new(None);

pub fn realm_status(region: &str, realm: &str) -> Result<RealmStatus, String> {
    let region = rio_region(region);
    if region == "cn" {
        return Err("Realm status isn't available for China.".into());
    }
    let key = format!("{region}/{}", realm.to_lowercase());
    if let Ok(cache) = REALM_CACHE.lock() {
        if let Some((at, r)) = cache.as_ref().and_then(|c| c.get(&key)) {
            if at.elapsed() < Duration::from_secs(3 * 60) {
                return Ok(r.clone());
            }
        }
    }
    let url = format!("https://ctrlshiftzed.com/api/waygate.php?realm={}&region={region}", crate::characters::encode(realm));
    let r: RealmStatus = match crate::online::agent().get(&url).call() {
        Ok(r) => r.into_json().map_err(|e| e.to_string())?,
        Err(ureq::Error::Status(404, _)) => return Err(format!("Blizzard doesn't know {realm}.")),
        Err(ureq::Error::Status(400 | 503, _)) => return Err("Realm status isn't set up on the server yet.".into()),
        Err(e) => return Err(e.to_string()),
    };
    if let Ok(mut cache) = REALM_CACHE.lock() {
        cache.get_or_insert_with(HashMap::new).insert(key, (Instant::now(), r.clone()));
    }
    Ok(r)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_raiderio_icons() {
        assert!(icon_url("https://cdn.raiderio.net/images/wow/icons/large/a.jpg").is_some());
        assert!(icon_url("https://evil.example/a.jpg").is_none());
        assert!(icon_url("http://cdn.raiderio.net/a.jpg").is_none());
    }

    #[test]
    fn regions() {
        assert_eq!(rio_region("EU"), "eu");
        assert_eq!(rio_region("xx"), "us");
    }
}
