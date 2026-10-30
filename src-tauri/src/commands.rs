//! Tauri command layer. Owned by Session A.
//!
//! These four commands are the entire frontend -> Rust API (see TARGET.md).
//! Sessions B/C implement sources; they must not need to touch this file.

use std::path::Path;

use tauri::{Manager, State};

use crate::art::{ArtKind, ArtResult, ArtStatus, ApplyArtResult};
use crate::error::{SourceError, SourceResult};
use crate::models::{AchievementSummary, Game, SteamStatus};
use crate::secrets::SecretStatus;
use crate::state::AppState;
use crate::steam_api::SteamApiStatus;
use crate::steam_owned::{SteamPlaytime, SteamSyncStatus};

/// Every game from every store, already merged and sorted.
///
/// Local cover files (Steam's `librarycache`, GOG's `GOG.ico`) are exposed to the
/// WebView through Tauri's asset protocol one file at a time. The static scope in
/// `tauri.conf.json` is intentionally tiny; this grants exactly the files the
/// library is about to display.
#[tauri::command]
pub fn list_games(app: tauri::AppHandle, state: State<'_, AppState>) -> Vec<Game> {
    let mut games = state.all_games();
    // Session M: a grid override chosen in the art picker wins over the store's
    // own cover, so the tile and detail page update without a rescan.
    for game in &mut games {
        if let Some(path) = crate::art::override_cover(&game.id) {
            game.cover_image = Some(path.to_string_lossy().into_owned());
        }
    }
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
    let Some(game) = state.find_game(&game_id) else {
        // Session H: owned-but-not-installed Steam catalog entries are not part
        // of any source's installed() scan. Route their install URI through the
        // same launch helper. Not journaled: this starts an install, not a play.
        if let Some(command) = crate::steam_owned::install_command(&state, &game_id) {
            return crate::launch::open(&command);
        }
        return Err(SourceError::game_not_found(&game_id));
    };

    let source = state
        .source_for(game.platform)
        .ok_or_else(|| SourceError::unavailable(format!("no source for {}", game.platform)))?;

    let result = source.launch(&game);
    // Session G: journal every successful launch for the Recently Played row.
    // Best-effort: record_launch never rejects and never affects the launch result.
    if result.is_ok() {
        crate::steam_recent::record_launch(&game.id);
    }
    result
}

/// Add a game to the Steam library as a non-Steam shortcut.
#[tauri::command]
pub fn add_to_steam(state: State<'_, AppState>, game_id: String) -> SourceResult<()> {
    let game = state
        .find_game(&game_id)
        .ok_or_else(|| SourceError::game_not_found(&game_id))?;

    let result = crate::sources::steam::add_to_steam_shortcut(&game);
    if result.is_ok() {
        // Session M: once the shortcut exists, quietly fetch SteamGridDB art.
        // This is intentionally fire-and-forget: a slow network must never delay
        // the success toast, and a missing key is a no-op.
        let game = game.clone();
        tauri::async_runtime::spawn_blocking(move || {
            if let Err(err) = crate::art::auto_art_for_game(&game) {
                eprintln!(
                    "[kudos] automatic artwork for '{}' was incomplete: {err}",
                    game.title
                );
            }
        });
    }
    result
}

/// Whether Steam is installed/running. Used by the Settings page.
#[tauri::command]
pub fn get_steam_status() -> SteamStatus {
    crate::sources::steam::steam_status()
}

/// Recently played games: Steam's `LastPlayed` merged with Kudos' launch journal.
///
/// Additive command (Session G). Never rejects; only ids present in the current
/// library are returned, newest first. `limit` is clamped to 1..=50.
#[tauri::command]
pub fn get_recent_games(state: State<'_, AppState>, limit: u32) -> Vec<crate::steam_recent::RecentGame> {
    crate::steam_recent::recent_games(&state, limit)
}

/// Local-only achievement count for one Steam game.
///
/// Never errors: unsupported platforms, missing files and unreadable caches all
/// come back as `supported: false`, so the detail page can render quietly.
#[tauri::command]
pub fn get_achievements(game_id: String) -> AchievementSummary {
    crate::achievements::summary_for(&game_id)
}

/// Every Steam store app this account has touched that is not currently
/// installed, as a normal `Game` with `steam://install/<appid>` as its command.
///
/// Additive command (Session H). Local-only: reads `localconfig.vdf` and the
/// optional name cache. Never rejects; an absent Steam install returns `[]`.
#[tauri::command]
pub fn get_steam_owned_games(app: tauri::AppHandle, state: State<'_, AppState>) -> Vec<Game> {
    let mut games = crate::steam_owned::owned_games(&state);
    for game in &mut games {
        if let Some(path) = crate::art::override_cover(&game.id) {
            game.cover_image = Some(path.to_string_lossy().into_owned());
        }
    }
    allow_local_covers(&app, &games);
    games
}

/// Steam playtime for the active account, minutes + last two weeks + LastPlayed.
///
/// Additive command (Session H). Local-only and cached by `localconfig.vdf`
/// mtime; never rejects.
#[tauri::command]
pub fn get_steam_playtime() -> Vec<SteamPlaytime> {
    crate::steam_owned::playtime()
}

/// Name-cache status for the Settings/library sync affordance.
///
/// Additive command (Session H). Local-only, no network.
#[tauri::command]
pub fn get_steam_sync_status() -> SteamSyncStatus {
    crate::steam_owned::sync_status()
}

/// Download Steam's public app list once, filter it to this account's app ids
/// and atomically replace `%APPDATA%\Kudos\steam-apps.json`.
///
/// Additive command (Session H). This is the **only** network request in Kudos
/// and it is only reachable from an explicit user action. Runs on a blocking
/// worker so a slow response cannot freeze the WebView. Errors are plain strings.
#[tauri::command]
pub async fn sync_steam_metadata(state: State<'_, AppState>) -> Result<SteamSyncStatus, String> {
    let known = crate::steam_owned::known_appids(&state);
    drop(state);
    tauri::async_runtime::spawn_blocking(move || crate::steam_owned::sync_metadata(known))
        .await
        .map_err(|err| format!("Steam metadata sync task failed: {err}"))?
}

// ---------------------------------------------------------------------------
// Secure secrets (Session M + L). Generic by provider ("steam", "steamgriddb").
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn get_secret_status(provider: String) -> SecretStatus {
    crate::secrets::status(&provider)
}

#[tauri::command]
pub fn set_secret(provider: String, value: String) -> Result<SecretStatus, String> {
    crate::secrets::set(&provider, &value)
}

#[tauri::command]
pub fn clear_secret(provider: String) -> Result<SecretStatus, String> {
    crate::secrets::clear(&provider)
}

// ---------------------------------------------------------------------------
// SteamGridDB artwork (Session M). All network calls are user-triggered.
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn search_art(state: State<'_, AppState>, game_id: String, kind: String) -> Result<Vec<ArtResult>, String> {
    let game = state
        .find_game(&game_id)
        .ok_or_else(|| SourceError::game_not_found(&game_id).to_string())?;
    let kind = ArtKind::parse(&kind)?;
    crate::art::search_for_game(&game, kind)
}

#[tauri::command]
pub fn apply_art(
    state: State<'_, AppState>,
    game_id: String,
    url: String,
    kind: String,
    replace: Option<bool>,
) -> Result<ApplyArtResult, String> {
    let game = state
        .find_game(&game_id)
        .ok_or_else(|| SourceError::game_not_found(&game_id).to_string())?;
    let kind = ArtKind::parse(&kind)?;
    crate::art::apply_for_game(&game, &url, kind, replace.unwrap_or(false))
}

#[tauri::command]
pub fn get_art_status(state: State<'_, AppState>, game_id: String) -> ArtStatus {
    match state.find_game(&game_id) {
        Some(game) => crate::art::status_for_game(&game),
        None => ArtStatus {
            grid: None,
            hero: None,
            logo: None,
            icon: None,
            source: crate::art::ArtSource::None,
            error: None,
        },
    }
}

// ---------------------------------------------------------------------------
// Steam Web API (Session L). Optional key -> complete owned library.
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn get_steam_api_status() -> SteamApiStatus {
    crate::steam_api::status()
}

#[tauri::command]
pub async fn set_steam_api_key(value: String) -> SteamApiStatus {
    // The key check is one bounded HTTPS request; never block the UI thread.
    match tauri::async_runtime::spawn_blocking(move || crate::steam_api::save_key(&value)).await {
        Ok(status) => status,
        Err(err) => crate::steam_api::error_status(format!("Steam key save task failed: {err}")),
    }
}

#[tauri::command]
pub fn clear_steam_api_key() -> SteamApiStatus {
    crate::steam_api::clear_key()
}

#[tauri::command]
pub async fn sync_steam_owned() -> SteamApiStatus {
    match tauri::async_runtime::spawn_blocking(crate::steam_api::sync_now).await {
        Ok(status) => status,
        Err(err) => crate::steam_api::error_status(format!("Steam sync task failed: {err}")),
    }
}

#[tauri::command]
pub fn get_steam_owned_api_games(state: State<'_, AppState>) -> Vec<Game> {
    crate::steam_api::owned_api_games(&state)
}
