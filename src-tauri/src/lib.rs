//! Project Kudos backend entry point.
//!
//! Contract summary (full details in TARGET.md):
//! - `Game` is the only shape the frontend ever sees.
//! - Every platform is a `sources::LibrarySource` behind a single registry.
//! - Frontend commands: `list_games`, `launch_game`, `add_to_steam`, `get_steam_status`.

pub mod commands;
pub mod error;
pub mod launch;
pub mod models;
pub mod sources;
pub mod state;

use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            commands::list_games,
            commands::launch_game,
            commands::add_to_steam,
            commands::get_steam_status,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Project Kudos");
}
