//! Recently played games: Steam's own `LastPlayed` timestamps plus Kudos' launch journal.
//!
//! ADDITIVE feature (Session G). This module does not change `Game`, `LibrarySource`
//! or any existing command. It adds one command: `get_recent_games(limit)`.
//!
//! ## Steam
//! Steam writes a per-account play timestamp to
//! `<Steam>/userdata/<accountid>/config/localconfig.vdf`:
//!
//! ```text
//! UserLocalConfigStore -> Software -> Valve -> Steam -> apps -> <appid> -> LastPlayed
//! ```
//!
//! Detection of the Steam root and the active account is delegated to Session B's
//! helpers (`detect_steam_root`, `active_userdata_dir`) - never duplicated here.
//! Parsing is cached by file `(mtime, len)`, so a warm request touches no parser.
//!
//! ## Everything else
//! GOG and Epic keep no local play history, so Kudos records its own: every
//! successful `launch_game` appends `(id, unix)` to `%APPDATA%\Kudos\recent.json`
//! with an atomic replace. That journal is also the fallback for Steam titles that
//! have no `LastPlayed` entry yet.
//!
//! Merge rule (contract): Steam's `LastPlayed` wins when present; the Kudos journal
//! fills the rest. Both are ordered newest-first and filtered to games that are in
//! the current library.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::models::Game;
use crate::sources::steam::{
    active_userdata_dir, detect_steam_root, parse_vdf, read_text_file, Vdf,
};
use crate::state::AppState;

/// Newest N play events kept on disk. Small by design: this is a launcher, not a stats DB.
const HISTORY_LIMIT: usize = 100;
/// Server-side ceiling for one `get_recent_games` call.
const MAX_RECENT: u32 = 50;

/// One entry returned to the frontend. Additive wire shape:
/// `{ "id": "steam:730", "lastPlayedUnix": 1792855921, "source": "steam" }`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentGame {
    pub id: String,
    pub last_played_unix: u64,
    pub source: RecentSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RecentSource {
    /// Timestamp read from Steam's `localconfig.vdf`.
    Steam,
    /// Timestamp recorded by Kudos when it launched the game.
    Kudos,
}

// ---------------------------------------------------------------------------
// Steam LastPlayed parsing
// ---------------------------------------------------------------------------

/// Parses `localconfig.vdf` text and returns `(appid, last_played_unix)`.
///
/// Missing `apps` tables, non-numeric app ids, absent `LastPlayed` fields and
/// non-numeric timestamps are skipped; a corrupt file returns an empty list.
/// It never panics: Steam rewrites this file while running.
pub(crate) fn parse_last_played(text: &str) -> Vec<(u32, u64)> {
    let Ok(vdf) = parse_vdf(text) else {
        return Vec::new();
    };
    let Some(root) = vdf.as_table() else {
        return Vec::new();
    };

    let Some(apps) = nested_table(root, &["UserLocalConfigStore", "Software", "Valve", "Steam", "apps"])
    else {
        return Vec::new();
    };

    let mut found = Vec::new();
    for (key, value) in apps {
        let Ok(appid) = key.trim().parse::<u32>() else {
            continue;
        };
        if appid == 0 {
            continue;
        }
        let Some(table) = value.as_table() else {
            continue;
        };
        let Some(raw) = field(table, "LastPlayed").and_then(Vdf::as_str) else {
            continue;
        };
        let Ok(timestamp) = raw.trim().parse::<u64>() else {
            continue;
        };
        if timestamp > 0 {
            found.push((appid, timestamp));
        }
    }
    found
}

/// Case-insensitive nested table lookup. VDF keys are compared the way Steam's
/// own parser treats them; real files use `"Apps"` while older builds used `"apps"`.
fn nested_table<'a>(mut table: &'a [(String, Vdf)], path: &[&str]) -> Option<&'a [(String, Vdf)]> {
    for key in path {
        table = field(table, key)?.as_table()?;
    }
    Some(table)
}

fn field<'a>(table: &'a [(String, Vdf)], key: &str) -> Option<&'a Vdf> {
    table
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(key))
        .map(|(_, value)| value)
}

struct LastPlayedCache {
    stamp: String,
    entries: Vec<(u32, u64)>,
}

static LAST_PLAYED_CACHE: OnceLock<Mutex<LastPlayedCache>> = OnceLock::new();

fn cache_stamp(path: &Path) -> Option<String> {
    let metadata = fs::metadata(path).ok()?;
    let modified = metadata.modified().ok()?.duration_since(UNIX_EPOCH).ok()?;
    Some(format!(
        "{}:{}:{}",
        path.to_string_lossy(),
        metadata.len(),
        modified.as_nanos()
    ))
}

/// Steam `LastPlayed` values for the active account, cached until the file changes.
fn steam_last_played() -> Vec<(u32, u64)> {
    let Some(root) = detect_steam_root() else {
        return Vec::new();
    };
    let Some(profile) = active_userdata_dir(&root) else {
        return Vec::new();
    };
    let path = profile.join("config").join("localconfig.vdf");
    let Some(stamp) = cache_stamp(&path) else {
        return Vec::new();
    };

    let cache = LAST_PLAYED_CACHE.get_or_init(|| Mutex::new(LastPlayedCache {
        stamp: String::new(),
        entries: Vec::new(),
    }));
    if let Ok(guard) = cache.lock() {
        if guard.stamp == stamp {
            return guard.entries.clone();
        }
    }

    let Ok(text) = read_text_file(&path) else {
        return Vec::new();
    };
    let entries = parse_last_played(&text);
    if let Ok(mut guard) = cache.lock() {
        guard.stamp = stamp;
        guard.entries = entries.clone();
    }
    entries
}

// ---------------------------------------------------------------------------
// Kudos launch journal
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HistoryEntry {
    id: String,
    last_played_unix: u64,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HistoryFile {
    #[serde(default)]
    version: u32,
    #[serde(default)]
    entries: Vec<HistoryEntry>,
}

/// `%APPDATA%\Kudos\recent.json`, with LOCALAPPDATA/TEMP fallbacks.
fn history_path() -> Option<PathBuf> {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("LOCALAPPDATA").map(PathBuf::from))
        .or_else(|| std::env::var_os("TEMP").map(PathBuf::from))?;
    Some(base.join("Kudos").join("recent.json"))
}

fn read_history_from(path: &Path) -> Vec<HistoryEntry> {
    let Ok(bytes) = fs::read(path) else {
        return Vec::new();
    };
    let Ok(file) = serde_json::from_slice::<HistoryFile>(&bytes) else {
        // A corrupt journal must never break launch or the library. Ignore it.
        return Vec::new();
    };
    file.entries
        .into_iter()
        .filter(|entry| !entry.id.trim().is_empty() && entry.last_played_unix > 0)
        .collect()
}

fn write_history_to(path: &Path, entries: &[HistoryEntry]) {
    let Some(parent) = path.parent() else {
        return;
    };
    if fs::create_dir_all(parent).is_err() {
        return;
    }
    let file = HistoryFile {
        version: 1,
        entries: entries.to_vec(),
    };
    let Ok(json) = serde_json::to_vec(&file) else {
        return;
    };

    // Atomic replace so a crash mid-write cannot leave a truncated journal.
    let tmp = path.with_extension("json.kudos-tmp");
    if fs::write(&tmp, &json).is_err() {
        return;
    }
    if fs::rename(&tmp, path).is_err() {
        let _ = fs::remove_file(&tmp);
    }
}

fn upsert_history(entries: &mut Vec<HistoryEntry>, id: &str, timestamp: u64) {
    entries.retain(|entry| entry.id != id);
    entries.push(HistoryEntry {
        id: id.to_string(),
        last_played_unix: timestamp,
    });
    entries.sort_by(|a, b| b.last_played_unix.cmp(&a.last_played_unix));
    entries.truncate(HISTORY_LIMIT);
}

/// Records one successful launch. Best-effort: a failure only logs to stderr and
/// must never turn a successful launch into an error for the user.
pub fn record_launch(game_id: &str) {
    let Some(timestamp) = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_secs())
        .filter(|seconds| *seconds > 0)
    else {
        return;
    };
    let Some(path) = history_path() else {
        return;
    };
    let mut entries = read_history_from(&path);
    upsert_history(&mut entries, game_id, timestamp);
    write_history_to(&path, &entries);
}

// ---------------------------------------------------------------------------
// Merge + command
// ---------------------------------------------------------------------------

/// Merges Steam and Kudos timestamps against the current library.
///
/// Split out from the command so the merge rule is unit-tested without touching
/// this machine's Steam files.
fn merge_recent(
    games: &[Game],
    steam: Vec<(u32, u64)>,
    history: Vec<HistoryEntry>,
    limit: u32,
) -> Vec<RecentGame> {
    let limit = limit.clamp(1, MAX_RECENT) as usize;
    let library: HashMap<&str, &Game> = games.iter().map(|game| (game.id.as_str(), game)).collect();
    let mut merged: HashMap<String, RecentGame> = HashMap::new();

    // Steam wins when present.
    for (appid, timestamp) in steam {
        let id = format!("steam:{appid}");
        if !library.contains_key(id.as_str()) || timestamp == 0 {
            continue;
        }
        merged.insert(
            id.clone(),
            RecentGame {
                id,
                last_played_unix: timestamp,
                source: RecentSource::Steam,
            },
        );
    }

    // Kudos fills the rest. It never overwrites a Steam timestamp, per contract.
    for entry in history {
        if !library.contains_key(entry.id.as_str()) || entry.last_played_unix == 0 {
            continue;
        }
        merged.entry(entry.id.clone()).or_insert(RecentGame {
            id: entry.id,
            last_played_unix: entry.last_played_unix,
            source: RecentSource::Kudos,
        });
    }

    let mut recent: Vec<RecentGame> = merged.into_values().collect();
    recent.sort_by(|a, b| {
        b.last_played_unix
            .cmp(&a.last_played_unix)
            .then_with(|| a.id.cmp(&b.id))
    });
    recent.truncate(limit);
    recent
}

/// `get_recent_games`: additive command. Never rejects; a broken Steam install
/// simply yields the Kudos journal (or an empty list).
pub fn recent_games(state: &AppState, limit: u32) -> Vec<RecentGame> {
    let games = state.all_games();
    let history = history_path()
        .map(|path| read_history_from(&path))
        .unwrap_or_default();
    merge_recent(&games, steam_last_played(), history, limit)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Platform;

    fn fixture() -> &'static str {
        include_str!("../tests/fixtures/steam/localconfig.vdf")
    }

    fn game(id: &str, platform: Platform, title: &str) -> Game {
        Game::new(id, platform, title, "cmd")
    }

    fn library() -> Vec<Game> {
        vec![
            game("steam:730", Platform::Steam, "Counter-Strike 2"),
            game("steam:413150", Platform::Steam, "Stardew Valley"),
            game("gog:1207658924", Platform::Gog, "The Witcher 3"),
            game("epic:Fortnite", Platform::Epic, "Fortnite"),
        ]
    }

    #[test]
    fn parses_real_localconfig_shape() {
        let entries = parse_last_played(fixture());
        assert_eq!(
            entries,
            vec![(730, 1_792_855_921), (413150, 1_782_486_344)]
        );
    }

    #[test]
    fn ignores_missing_invalid_and_non_app_entries() {
        let entries = parse_last_played(fixture());
        let ids: Vec<u32> = entries.iter().map(|(id, _)| *id).collect();
        // No LastPlayed, non-numeric appid, non-numeric timestamp, zero timestamp.
        assert!(!ids.contains(&999));
        assert!(!ids.contains(&480));
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn is_case_insensitive_and_tolerates_garbage() {
        let text = r#"
            "userlocalconfigstore" {
              "software" { "valve" { "steam" { "APPS" {
                "10" { "lastplayed" "1700000000" }
              } } } }
            }
        "#;
        assert_eq!(parse_last_played(text), vec![(10, 1_700_000_000)]);
        assert!(parse_last_played("not a vdf at all").is_empty());
        assert!(parse_last_played("").is_empty());
    }

    #[test]
    fn steam_wins_and_kudos_fills_the_rest() {
        let steam = vec![(730, 1_700_000_500)];
        let history = vec![
            // Older than the Steam value: Steam must still win.
            HistoryEntry {
                id: "steam:730".into(),
                last_played_unix: 1_700_000_100,
            },
            HistoryEntry {
                id: "gog:1207658924".into(),
                last_played_unix: 1_700_000_900,
            },
            HistoryEntry {
                id: "epic:Fortnite".into(),
                last_played_unix: 1_700_000_300,
            },
        ];

        let recent = merge_recent(&library(), steam, history, 10);
        assert_eq!(
            recent,
            vec![
                RecentGame {
                    id: "gog:1207658924".into(),
                    last_played_unix: 1_700_000_900,
                    source: RecentSource::Kudos,
                },
                RecentGame {
                    id: "steam:730".into(),
                    last_played_unix: 1_700_000_500,
                    source: RecentSource::Steam,
                },
                RecentGame {
                    id: "epic:Fortnite".into(),
                    last_played_unix: 1_700_000_300,
                    source: RecentSource::Kudos,
                },
            ]
        );
    }

    #[test]
    fn filters_unknown_games_and_respects_limit() {
        let steam = vec![(730, 100), (999_999, 900)];
        let history = vec![HistoryEntry {
            id: "steam:413150".into(),
            last_played_unix: 200,
        }];
        let recent = merge_recent(&library(), steam, history, 1);
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].id, "steam:413150");

        let zero = merge_recent(&library(), Vec::new(), Vec::new(), 0);
        assert!(zero.is_empty());
    }

    #[test]
    fn history_roundtrip_is_atomic_and_upserts() {
        let unique = format!(
            "kudos-recent-test-{}-{}.json",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let path = std::env::temp_dir().join(unique);
        let _ = fs::remove_file(&path);

        let mut entries = Vec::new();
        upsert_history(&mut entries, "gog:1", 100);
        upsert_history(&mut entries, "steam:2", 300);
        upsert_history(&mut entries, "gog:1", 400);
        write_history_to(&path, &entries);

        let loaded = read_history_from(&path);
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].id, "gog:1");
        assert_eq!(loaded[0].last_played_unix, 400);
        assert_eq!(loaded[1].id, "steam:2");
        assert!(!path.with_extension("json.kudos-tmp").exists());

        fs::write(&path, b"not json").unwrap();
        assert!(read_history_from(&path).is_empty());
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn journal_is_capped() {
        let mut entries = Vec::new();
        for index in 0..(HISTORY_LIMIT + 25) {
            upsert_history(&mut entries, &format!("steam:{index}"), index as u64 + 1);
        }
        assert_eq!(entries.len(), HISTORY_LIMIT);
        assert_eq!(entries[0].last_played_unix, HISTORY_LIMIT as u64 + 25);
    }
}
