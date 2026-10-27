//! The single registry of library sources. Owned by Session A.
//!
//! The frontend never learns which source answered - commands aggregate here.

use crate::error::SourceResult;
use crate::models::{Game, Platform};
use crate::sources::{epic::EpicSource, gog::GogSource, steam::SteamSource, LibrarySource};

pub struct AppState {
    sources: Vec<Box<dyn LibrarySource>>,
}

impl AppState {
    pub fn new() -> Self {
        // Register every platform exactly once, in display order.
        Self {
            sources: vec![
                Box::new(SteamSource::new()),
                Box::new(EpicSource::new()),
                Box::new(GogSource::new()),
            ],
        }
    }

    pub fn sources(&self) -> &[Box<dyn LibrarySource>] {
        &self.sources
    }

    pub fn source_for(&self, platform: Platform) -> Option<&dyn LibrarySource> {
        self.sources
            .iter()
            .find(|s| s.platform() == platform)
            .map(|s| s.as_ref())
    }

    /// All games from all sources, de-duplicated and title-sorted.
    /// A broken source never breaks the library: its error is logged and skipped.
    pub fn all_games(&self) -> Vec<Game> {
        let mut games: Vec<Game> = Vec::new();
        for source in &self.sources {
            match source.installed() {
                Ok(found) => games.extend(found),
                Err(err) => eprintln!("[kudos] source {} failed: {err}", source.platform()),
            }
        }
        games.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
        games.dedup_by(|a, b| a.id == b.id);
        games
    }

    /// Find one game by its namespaced id across every source.
    pub fn find_game(&self, game_id: &str) -> Option<Game> {
        for source in &self.sources {
            if let Ok(games) = source.installed() {
                if let Some(game) = games.into_iter().find(|g| g.id == game_id) {
                    return Some(game);
                }
            }
        }
        None
    }

    /// Aggregate availability of every source, for the Settings page.
    pub fn source_health(&self) -> Vec<(Platform, SourceResult<usize>)> {
        self.sources
            .iter()
            .map(|s| (s.platform(), s.installed().map(|g| g.len())))
            .collect()
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
