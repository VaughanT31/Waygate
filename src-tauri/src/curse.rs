//! Addon update checks with CurseForge, step 1 (check only, nothing is
//! downloaded or written). Every call goes through our own server
//! (ctrlshiftzed.com/api/waygate.php?cf=check), which holds the CurseForge key;
//! the app never has it. CurseForge's terms don't allow keeping their data, so
//! nothing here is cached: each check asks again.

use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
struct Answer {
    mods: Vec<Mod>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Mod {
    id: u64,
    name: String,
    url: String,
    allow_distribution: bool,
    latest: Option<Latest>,
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Latest {
    pub file_id: u64,
    pub file_name: String,
    pub display_name: String,
    pub date: String,
    pub game_version: String,
}

/// One installed addon the app asks about: its CurseForge id and toc version.
#[derive(Deserialize)]
pub struct Installed {
    pub id: String,
    pub version: String,
}

#[derive(Serialize, Clone, PartialEq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum State {
    /// The installed version is CurseForge's newest for this game version.
    Current,
    /// CurseForge has a newer file.
    Update,
    /// CurseForge has nothing for this version of the game.
    NoFile,
    /// The names don't say which version is newer.
    Unknown,
}

#[derive(Serialize, Clone)]
pub struct Status {
    pub id: String,
    pub state: State,
    pub name: String,
    /// The addon's CurseForge page (only ever a curseforge.com link).
    pub url: Option<String>,
    /// The author allows other apps to download it (step 2 needs this).
    pub allow_distribution: bool,
    pub latest: Option<Latest>,
}

/// Version-looking runs like 12.0.29 or 3.4 (at least one dot), as numbers.
fn versions_in(text: &str) -> Vec<Vec<u64>> {
    let mut out = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i].is_ascii_digit() && (i == 0 || !(chars[i - 1].is_ascii_digit() || chars[i - 1] == '.')) {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_digit() || (chars[i] == '.' && i + 1 < chars.len() && chars[i + 1].is_ascii_digit())) {
                i += 1;
            }
            let run: String = chars[start..i].iter().collect();
            if run.contains('.') {
                out.push(run.split('.').filter_map(|p| p.parse().ok()).collect());
            }
        } else {
            i += 1;
        }
    }
    out
}

/// `text` holds `needle` as a whole version, not the start of a longer one (1.2 isn't in 1.2.3).
fn has_version(text: &str, needle: &str) -> bool {
    let mut from = 0;
    while let Some(at) = text[from..].find(needle) {
        let start = from + at;
        let end = start + needle.len();
        let before = text[..start].chars().last();
        let mut after = text[end..].chars();
        let a1 = after.next();
        let a2 = after.next();
        let before_ok = !before.is_some_and(|c| c.is_ascii_digit() || c == '.');
        let after_ok = !(a1.is_some_and(|c| c.is_ascii_digit()) || (a1 == Some('.') && a2.is_some_and(|c| c.is_ascii_digit())));
        if before_ok && after_ok {
            return true;
        }
        from = start + 1;
    }
    false
}

/// Whether CurseForge's newest file is newer than what's installed, from the
/// toc's version and the file's names (which nearly always carry the version).
pub fn compare(installed: &str, latest: &Latest) -> State {
    let names = format!("{} {}", latest.display_name, latest.file_name).to_lowercase();
    let mine = installed.trim().trim_start_matches(['v', 'V']).to_lowercase();
    // Unbuilt tocs say @project-version@; nothing to compare.
    if mine.is_empty() || mine.contains('@') {
        return State::Unknown;
    }
    if has_version(&names, &mine) {
        return State::Current;
    }
    let theirs = versions_in(&names);
    let Some(ours) = versions_in(&mine).into_iter().next() else {
        // A version like "r123" or a date: a different name means a different file.
        return if theirs.is_empty() { State::Unknown } else { State::Update };
    };
    match theirs.iter().max() {
        Some(newest) if *newest > ours => State::Update,
        Some(_) => State::Current, // installed is the same or newer (a beta, say)
        None => State::Unknown,
    }
}

fn curseforge_url(url: &str) -> Option<String> {
    (url.starts_with("https://www.curseforge.com/") || url.starts_with("https://curseforge.com/")).then(|| url.to_string())
}

/// `game` is the installed game version (12.1.5.70575 is sent as 12.1.5).
pub fn check(game: &str, addons: &[Installed]) -> Result<Vec<Status>, String> {
    let game: String = game.split('.').take(3).collect::<Vec<_>>().join(".");
    let mut ids: Vec<&str> = addons.iter().map(|a| a.id.trim()).filter(|id| !id.is_empty() && id.chars().all(|c| c.is_ascii_digit())).collect();
    ids.sort();
    ids.dedup();
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for chunk in ids.chunks(200) {
        let url = format!("https://ctrlshiftzed.com/api/waygate.php?cf=check&game={game}&ids={}", chunk.join(","));
        let answer: Answer = match crate::online::agent().get(&url).call() {
            Ok(r) => r.into_json().map_err(|e| e.to_string())?,
            Err(ureq::Error::Status(503, _)) => return Err("Update checks aren't set up on the server yet.".into()),
            Err(ureq::Error::Status(429, _)) => return Err("Too many update checks; try again in a few minutes.".into()),
            Err(ureq::Error::Status(502, _)) => return Err("CurseForge couldn't be reached.".into()),
            Err(e) => return Err(e.to_string()),
        };
        for m in answer.mods {
            let id = m.id.to_string();
            let Some(mine) = addons.iter().find(|a| a.id.trim() == id) else { continue };
            let state = match &m.latest {
                Some(l) => compare(&mine.version, l),
                None => State::NoFile,
            };
            out.push(Status { id, state, name: m.name, url: curseforge_url(&m.url), allow_distribution: m.allow_distribution, latest: m.latest });
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(display: &str, file: &str) -> Latest {
        Latest { file_id: 1, file_name: file.into(), display_name: display.into(), date: String::new(), game_version: "12.1.5".into() }
    }

    #[test]
    fn same_version_is_current() {
        assert_eq!(compare("12.0.29", &file("DBM-Core 12.0.29", "DBM-Core-12.0.29.zip")), State::Current);
        assert_eq!(compare("v1.2.3", &file("", "MyAddon-v1.2.3.zip")), State::Current);
    }

    #[test]
    fn newer_file_is_an_update() {
        assert_eq!(compare("12.0.28", &file("DBM-Core 12.0.29", "DBM-Core-12.0.29.zip")), State::Update);
        // 1.2 is not "in" 1.2.3.
        assert_eq!(compare("1.2", &file("1.2.3", "a-1.2.3.zip")), State::Update);
        assert_eq!(compare("r120", &file("r121", "a-r121.zip")), State::Unknown);
    }

    #[test]
    fn installed_newer_is_not_an_update() {
        assert_eq!(compare("2.1.0", &file("2.0.9", "a-2.0.9.zip")), State::Current);
    }

    #[test]
    fn unbuilt_or_unnamed_is_unknown() {
        assert_eq!(compare("@project-version@", &file("1.0.0", "a.zip")), State::Unknown);
        assert_eq!(compare("1.0.0", &file("Release", "addon.zip")), State::Unknown);
    }

    #[test]
    fn only_curseforge_links() {
        assert!(curseforge_url("https://www.curseforge.com/wow/addons/dbm").is_some());
        assert!(curseforge_url("https://evil.example/").is_none());
    }
}
