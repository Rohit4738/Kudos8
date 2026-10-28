//! Tauri command layer. Owned by Session A.
//!
//! These four commands are the entire frontend -> Rust API (see TARGET.md).
//! Sessions B/C implement sources; they must not need to touch this file.

use std::path::Path;

use tauri::{Manager, State};

use crate::error::{SourceError, SourceResult};
use crate::models::{Game, SteamStatus};
use crate::state::AppState;

/// Every game from every store, already merged and sorted.
///
/// Local cover files (Steam's `librarycache`, GOG's `GOG.ico`) are exposed to the
/// WebView through Tauri's asset protocol one file at a time. The static scope in
/// `tauri.conf.json` is intentionally tiny; this grants exactly the files the
/// library is about to display.
#[tauri::command]
pub fn list_games(app: tauri::AppHandle, state: State<'_, AppState>) -> Vec<Game> {
    let games = state.all_games();
    allow_local_covers(&app, &games);
    games
}

fn allow_local_covers(app: &tauri::AppHandle, games: &[Game]) {
    for game in games {
        let Some(path) = game.cover_image.as_deref() else {
            continue;
        };
        let path = path.trim();
        if path.is_empty() || path.contains("://") {
            continue;
        }
        let path = Path::new(path);
        if path.is_file() {
            if let Err(err) = app.asset_protocol_scope().allow_file(path) {
                eprintln!(
                    "[kudos] could not expose cover {} to the asset protocol: {err}",
                    path.display()
                );
            }
        }
    }
}

/// Launch one game by id. The registry resolves the owning source.
#[tauri::command]
pub fn launch_game(state: State<'_, AppState>, game_id: String) -> SourceResult<()> {
    let game = state
        .find_game(&game_id)
        .ok_or_else(|| SourceError::game_not_found(&game_id))?;

    let source = state
        .source_for(game.platform)
        .ok_or_else(|| SourceError::unavailable(format!("no source for {}", game.platform)))?;

    source.launch(&game)
}

/// Add a game to the Steam library as a non-Steam shortcut.
#[tauri::command]
pub fn add_to_steam(state: State<'_, AppState>, game_id: String) -> SourceResult<()> {
    let game = state
        .find_game(&game_id)
        .ok_or_else(|| SourceError::game_not_found(&game_id))?;

    crate::sources::steam::add_to_steam_shortcut(&game)
}

/// Whether Steam is installed/running. Used by the Settings page.
#[tauri::command]
pub fn get_steam_status() -> SteamStatus {
    crate::sources::steam::steam_status()
}
