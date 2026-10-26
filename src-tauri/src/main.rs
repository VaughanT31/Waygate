// No console window behind the app in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod games;
mod launch;
mod settings;

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

/// Opens one of Waygate's own links in the default browser. Only these
/// addresses can be opened, whatever the page asks for.
#[tauri::command]
fn open_link(which: String) -> Result<(), String> {
    let url = match which.as_str() {
        "kofi" => "https://ko-fi.com/ctrlshiftzed",
        "patreon" => "https://www.patreon.com/cw/CtrlShiftZed",
        _ => return Err("Unknown link.".into()),
    };
    std::process::Command::new("explorer")
        .arg(url)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Couldn't open the browser: {e}"))
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
        .invoke_handler(tauri::generate_handler![installed, play, open_link, load_settings, save_settings])
        .run(tauri::generate_context!())
        .expect("error while running Waygate");
}
