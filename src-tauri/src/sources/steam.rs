//! Steam source. OWNER: Session B.
//!
//! Stub only - Session A wires it into the registry so the frontend works today.
//! Replace the `TODO(Session B)` bodies; keep the trait signatures and the two
//! free functions (`add_to_steam_shortcut`, `steam_status`) intact - the command
//! layer in `commands.rs` calls them by name.

use crate::error::{SourceError, SourceResult};
use crate::launch;
use crate::models::{Game, Platform, SteamStatus};
use crate::sources::LibrarySource;

pub struct SteamSource;

impl SteamSource {
    pub fn new() -> Self {
        Self
    }
}

impl Default for SteamSource {
    fn default() -> Self {
        Self::new()
    }
}

impl LibrarySource for SteamSource {
    fn platform(&self) -> Platform {
        Platform::Steam
    }

    fn installed(&self) -> SourceResult<Vec<Game>> {
        // TODO(Session B): parse `libraryfolders.vdf` + `appmanifest_*.acf`
        // and return one `Game` per installed Steam app. Id format: `steam:<appid>`.
        Ok(Vec::new())
    }

    fn launch(&self, game: &Game) -> SourceResult<()> {
        // TODO(Session B): prefer `steam://rungameid/<appid>` so Steam Cloud,
        // overlay and playtime keep working; fall back to `game.exe_path`.
        if game.platform != Platform::Steam {
            return Err(SourceError::invalid(format!(
                "{} is not a Steam game",
                game.id
            )));
        }
        launch::open(&game.launch_command)
    }
}

/// Add `game` to the Steam library as a non-Steam shortcut.
///
/// TODO(Session B): write `shortcuts.vdf` under
/// `<steam>/userdata/<account>/config/`. Steam must be restarted to pick it up -
/// report that to the UI through the returned `Result`.
pub fn add_to_steam_shortcut(game: &Game) -> SourceResult<()> {
    Err(SourceError::not_implemented(format!(
        "adding '{}' to Steam",
        game.title
    )))
}

/// Detect the local Steam client.
///
/// TODO(Session B): read `HKCU\Software\Valve\Steam\SteamPath`, then check for a
/// running `steam.exe` process.
pub fn steam_status() -> SteamStatus {
    SteamStatus {
        installed: false,
        running: false,
        steam_path: None,
    }
}
