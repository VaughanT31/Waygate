//! Starting a game: Battle.net's own documented command and nothing else
//! (scope.md section 4). `Battle.net.exe --exec="launch WoW"` starts the game
//! if Battle.net is logged in; if not, Battle.net opens and the player logs
//! in there, as always. Without a launch code Battle.net just opens.

use std::path::Path;
use std::process::Command;

pub fn start(battle_net: &Path, code: Option<&str>) -> Result<(), String> {
    let mut cmd = Command::new(battle_net);
    if let Some(code) = code {
        if !code.chars().all(|c| c.is_ascii_alphanumeric()) {
            return Err(format!("Unknown launch code {code}"));
        }
        // Written exactly as Battle.net documents it, quotes and all.
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.raw_arg(format!("--exec=\"launch {code}\""));
        }
        #[cfg(not(windows))]
        cmd.arg(format!("--exec=launch {code}"));
    }
    cmd.spawn().map(|_| ()).map_err(|e| format!("Couldn't start Battle.net: {e}"))
}
