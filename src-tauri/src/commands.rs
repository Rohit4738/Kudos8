//! Tauri command layer. Owned by Session A.
//!
//! These four commands are the entire frontend -> Rust API (see TARGET.md).
//! Sessions B/C implement sources; they must not need to touch this file.

use tauri::State;

use crate::error::{SourceError, SourceResult};
use crate::models::{Game, SteamStatus};
use crate::state::AppState;

/// Every game from every store, already merged and sorted.
#[tauri::command]
pub fn list_games(state: State<'_, AppState>) -> Vec<Game> {
    state.all_games()
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
