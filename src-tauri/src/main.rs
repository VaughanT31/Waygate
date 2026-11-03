// No console window behind the app in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod addons;
mod characters;
mod games;
mod launch;
mod online;
mod settings;
mod week;

use serde::Serialize;
use std::path::Path;

#[tauri::command]
fn installed() -> games::Installed {
    games::scan()
}

/// Plays a game by its uid and version. Paths and launch codes are looked up
/// again here rather than taken from the page.
#[tauri::command]
fn play(uid: String, product: Option<String>) -> Result<(), String> {
    let found = games::scan();
    let battle_net = found.battle_net.ok_or("Battle.net isn't installed.")?;
    let game = found.games.iter().find(|g| g.uid == uid).ok_or("That game isn't installed.")?;
    let code = product
        .and_then(|p| game.flavors.iter().find(|f| f.product == p))
        .and_then(|f| f.launch.clone());
    launch::start(Path::new(&battle_net), code.as_deref())
}

fn open_in_explorer(target: &str) -> Result<(), String> {
    std::process::Command::new("explorer")
        .arg(target)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Couldn't open it: {e}"))
}

/// Opens a game's install folder in Explorer, looked up again here rather
/// than taken from the page. Only opens it; nothing in it is touched.
#[tauri::command]
fn open_folder(uid: String) -> Result<(), String> {
    let found = games::scan();
    let game = found.games.iter().find(|g| g.uid == uid).ok_or("That game isn't installed.")?;
    if !Path::new(&game.install).is_dir() {
        return Err("The game's folder wasn't found.".into());
    }
    open_in_explorer(&game.install)
}

/// Opens one of Waygate's own links in the default browser. Only these
/// addresses can be opened, whatever the page asks for.
#[tauri::command]
fn open_link(which: String) -> Result<(), String> {
    let url = match which.as_str() {
        "kofi" => "https://ko-fi.com/ctrlshiftzed",
        "patreon" => "https://www.patreon.com/cw/CtrlShiftZed",
        _ => return Err("Unknown link.".into()),
    };
    open_in_explorer(url)
}

/// Opens a news post: only pages on Blizzard's own forums.
#[tauri::command]
fn open_news(url: String) -> Result<(), String> {
    if !online::is_news_url(&url) {
        return Err("Only Blizzard forum links open from news.".into());
    }
    open_in_explorer(&url)
}

#[derive(Serialize)]
struct Update {
    uid: String,
    game: String,
    product: String,
    name: String,
    region: String,
    installed: String,
    latest: Option<String>,
    update: bool,
    error: Option<String>,
}

/// Is a newer version of each installed game out? Compares the game folder
/// with Blizzard's public patch server, version and build fingerprint.
#[tauri::command]
async fn updates() -> Vec<Update> {
    tauri::async_runtime::spawn_blocking(|| {
        let found = games::scan();
        let mut out = Vec::new();
        for game in &found.games {
            for f in &game.flavors {
                let (latest, update, error) = match online::latest_cached(&f.product, &f.region) {
                    Ok((version, build)) => {
                        let newer = if !build.is_empty() && !f.build.is_empty() { build != f.build } else { version != f.version };
                        (Some(version), newer, None)
                    }
                    Err(e) => (None, false, Some(e)),
                };
                out.push(Update {
                    uid: game.uid.clone(),
                    game: game.name.clone(),
                    product: f.product.clone(),
                    name: f.name.clone(),
                    region: f.region.clone(),
                    installed: f.version.clone(),
                    latest,
                    update,
                    error,
                });
            }
        }
        out
    })
    .await
    .unwrap_or_default()
}

/// Installed WoW addons, per game version.
#[tauri::command]
async fn addons() -> Vec<addons::Flavor> {
    tauri::async_runtime::spawn_blocking(|| {
        let found = games::scan();
        let Some(wow) = found.games.iter().find(|g| g.uid == "wow") else { return Vec::new() };
        let versions: Vec<(String, String)> = wow.flavors.iter().map(|f| (f.product.clone(), f.version.clone())).collect();
        addons::scan(Path::new(&wow.install), &versions)
    })
    .await
    .unwrap_or_default()
}

/// Blizzard news for the installed games, newest first.
#[tauri::command]
async fn news() -> Vec<online::NewsItem> {
    tauri::async_runtime::spawn_blocking(|| {
        let found = games::scan();
        let games: Vec<(String, String)> = found.games.iter()
            .map(|g| (g.uid.clone(), g.flavors.first().map(|f| f.region.clone()).unwrap_or_default()))
            .collect();
        online::news(&games)
    })
    .await
    .unwrap_or_default()
}

/// This week in World of Warcraft for the retail install's region: Mythic+
/// affixes and the current season, from Raider.IO.
#[tauri::command]
async fn this_week() -> Result<week::Week, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let found = games::scan();
        let region = found.games.iter().find(|g| g.uid == "wow")
            .and_then(|g| g.flavors.iter().find(|f| f.product == "wow").or(g.flavors.first()))
            .map(|f| f.region.clone())
            .ok_or("World of Warcraft isn't installed.")?;
        week::this_week(&region)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// The WoW Token price in the retail install's region, through our server.
#[tauri::command]
async fn wow_token() -> Result<week::Token, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let (_, region) = retail().ok_or("World of Warcraft isn't installed.")?;
        week::token(&region)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Whether a realm is up and how busy, in the retail install's region, through our server.
#[tauri::command]
async fn realm_status(realm: String) -> Result<week::RealmStatus, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let (_, region) = retail().ok_or("World of Warcraft isn't installed.")?;
        week::realm_status(&region, &realm)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// The retail install's folder and region, for character work.
fn retail() -> Option<(std::path::PathBuf, String)> {
    let found = games::scan();
    let wow = found.games.iter().find(|g| g.uid == "wow")?;
    let region = wow.flavors.iter().find(|f| f.product == "wow").or(wow.flavors.first())?.region.clone();
    let folder = addons::version_folders(Path::new(&wow.install)).into_iter().find(|(p, _, _)| p == "wow")?.2;
    Some((folder, region))
}

/// Characters with a folder in the game's WTF, most recently played first.
#[tauri::command]
async fn found_characters() -> Vec<characters::Found> {
    tauri::async_runtime::spawn_blocking(|| retail().map(|(folder, _)| characters::found(&folder)).unwrap_or_default())
        .await
        .unwrap_or_default()
}

/// One character from Raider.IO, in the retail install's region.
#[tauri::command]
async fn character(realm: String, name: String) -> Result<characters::Profile, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let (_, region) = retail().ok_or("World of Warcraft isn't installed.")?;
        characters::profile(&region, &realm, &name)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
fn load_settings() -> settings::Settings {
    settings::load()
}

#[tauri::command]
fn save_settings(settings: settings::Settings) -> Result<(), String> {
    settings::save(&settings)
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            installed, play, open_folder, open_link, open_news, updates, addons, news, this_week, found_characters, character, wow_token, realm_status, load_settings, save_settings
        ])
        .run(tauri::generate_context!())
        .expect("error while running Waygate");
}

#[cfg(test)]
mod live {
    use super::*;

    /// Real games and real servers: `cargo test live -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn live_check() {
        let found = games::scan();
        for g in &found.games {
            for f in &g.flavors {
                println!("{} {} {} installed {} build {}", g.name, f.product, f.region, f.version, f.build);
                println!("  latest {:?}", online::latest_cached(&f.product, &f.region));
            }
        }
        if let Some(wow) = found.games.iter().find(|g| g.uid == "wow") {
            let versions: Vec<(String, String)> = wow.flavors.iter().map(|f| (f.product.clone(), f.version.clone())).collect();
            for fl in addons::scan(Path::new(&wow.install), &versions) {
                let old = fl.addons.iter().filter(|a| a.out_of_date).count();
                println!("addons {} {} {:?}: {} addons, {} out of date", fl.product, fl.folder, fl.interface, fl.addons.len(), old);
                for a in fl.addons.iter().take(6) {
                    println!("  {} {} {:?} curse {:?} wago {:?} old {}", a.title, a.version, a.folders, a.curse_id, a.wago_id, a.out_of_date);
                }
            }
        }
        let games: Vec<(String, String)> = found.games.iter()
            .map(|g| (g.uid.clone(), g.flavors.first().map(|f| f.region.clone()).unwrap_or_default()))
            .collect();
        for (uid, region) in &games {
            println!("forum {uid} {region}: {:?}", online::forum_news(uid, region).map(|v| v.len()));
            println!("articles {uid} {region}: {:?}", online::articles(uid, region).map(|v| v.len()));
        }
        if let Some((folder, region)) = retail() {
            let found = characters::found(&folder);
            println!("characters {}: {} found", region, found.len());
            for f in found.iter().take(3) {
                println!("  {} {} played {:?} -> {:?}", f.name, f.realm, f.played,
                    characters::profile(&region, &f.realm, &f.name).map(|p| (p.class, p.spec, p.ilvl, p.score, p.color)));
            }
        }
        if let Some((_, region)) = games.iter().find(|(u, _)| u == "wow") {
            match week::this_week(region) {
                Ok(w) => println!("week {}: {:?} since {:?}, affixes {:?}", w.region, w.season, w.season_start,
                    w.affixes.iter().map(|a| a.name.as_str()).collect::<Vec<_>>()),
                Err(e) => println!("week: {e}"),
            }
        }
        for n in online::news(&games).iter().take(8) {
            println!("news {} {} | {} | img {:?}", n.uid, &n.date[..10.min(n.date.len())], n.title, n.image.as_deref().map(|i| &i[..40.min(i.len())]));
        }
    }
}
