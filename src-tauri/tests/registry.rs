//! Real-machine integration checks for the source registry.
//!
//! These deliberately tolerate a machine with no store clients installed: the
//! point is to prove that `AppState` wires every source, keeps ids stable, and
//! only ever returns usable local cover paths. CI (no Steam/GOG/Epic) still
//! passes; a developer machine exercises the real scanners.

use std::collections::HashSet;
use std::path::Path;

use kudos_lib::models::Platform;
use kudos_lib::state::AppState;

#[test]
fn registry_registers_every_platform_once() {
    let state = AppState::new();
    for platform in [Platform::Steam, Platform::Epic, Platform::Gog] {
        assert!(
            state.source_for(platform).is_some(),
            "no source registered for {platform}"
        );
    }
    assert_eq!(state.sources().len(), 3, "duplicate source registration");
}

#[test]
fn all_games_are_namespaced_unique_and_resolvable() {
    let state = AppState::new();
    let games = state.all_games();

    let mut ids = HashSet::new();
    for game in &games {
        let (prefix, store_id) = game
            .id
            .split_once(':')
            .unwrap_or_else(|| panic!("id '{}' is not namespaced", game.id));
        assert_eq!(prefix, game.platform.as_str(), "id/platform mismatch");
        assert!(!store_id.trim().is_empty(), "id '{}' has an empty store id", game.id);
        assert!(!game.title.trim().is_empty(), "{} has an empty title", game.id);
        assert!(
            !game.launch_command.trim().is_empty(),
            "{} has no launch command",
            game.id
        );
        assert!(ids.insert(game.id.clone()), "duplicate id {}", game.id);
    }

    // The same id must resolve through the registry to the same game, which is
    // what both `launch_game` and `add_to_steam` depend on.
    for game in &games {
        let found = state
            .find_game(&game.id)
            .unwrap_or_else(|| panic!("find_game could not resolve {}", game.id));
        assert_eq!(found.id, game.id);
        assert_eq!(found.platform, game.platform);
        assert_eq!(found.title, game.title);
    }
}

#[test]
fn local_cover_paths_are_absolute_and_exist() {
    let state = AppState::new();
    let games = state.all_games();
    let mut local_covers = 0usize;
    let mut remote_covers = 0usize;

    for game in &games {
        let Some(cover) = game.cover_image.as_deref() else {
            continue;
        };
        if cover.contains("://") {
            // CDN/http URLs are loaded by the WebView, not the asset protocol.
            remote_covers += 1;
            continue;
        }
        local_covers += 1;

        let path = Path::new(cover);
        assert!(
            path.is_absolute(),
            "{} has a relative local cover: {cover}",
            game.id
        );
        assert!(
            path.is_file(),
            "{} points at a missing local cover: {cover}",
            game.id
        );

        // Steam's local cover is the librarycache copy, never an arbitrary
        // user file (that would widen the asset-protocol attack surface).
        if game.platform == Platform::Steam {
            let normalised = cover.replace('/', "\\").to_ascii_lowercase();
            assert!(
                normalised.contains("\\appcache\\librarycache\\"),
                "{} points outside Steam's librarycache: {cover}",
                game.id
            );
        }
    }

    eprintln!(
        "[kudos] registry covers: {local_covers} local, {remote_covers} remote, {} games total",
        games.len()
    );
}
