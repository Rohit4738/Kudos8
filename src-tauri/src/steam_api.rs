//! Optional Steam Web API support: the account's COMPLETE owned library.
//!
//! ADDITIVE feature (Session L). Steam's local `localconfig.vdf` only knows the
//! apps this PC has seen. With a user-supplied Steam Web API key Kudos can ask
//! Valve for every game the account owns, including titles that were never
//! installed or launched here.
//!
//! ## Network
//! Exactly one endpoint is ever called:
//!
//! ```text
//! GET https://api.steampowered.com/IPlayerService/GetOwnedGames/v1/
//!     ?include_appinfo=1&include_played_free_games=1&key=<key>&steamid=<steamid64>
//! ```
//!
//! - HTTPS only, host pinned to `api.steampowered.com`.
//! - 10 s WinHTTP receive timeout, at most one retry.
//! - HTTP 429 is **not** retried: the error tells the user to wait.
//! - Never called at startup. Only `set_steam_api_key`, `sync_steam_owned` and
//!   the Settings card's stale-cache check can reach it.
//!
//! ## Key handling
//! The key lives in Session M's DPAPI vault (`secrets.rs`, provider `steam`).
//! This module never logs it, never returns it, and never writes it to a file.
//! [`SteamApiKey`] has a redacted `Debug` so it cannot leak through `{:?}`.
//!
//! ## Cache
//! `%APPDATA%\Kudos\steam-owned-api.json`:
//!
//! ```json
//! { "fetchedAt": 1793176000, "steamId": "76561199806792336",
//!   "games": [ { "appid": 730, "name": "Counter-Strike 2" } ] }
//! ```
//!
//! Written atomically (`tmp` + rename), valid for 6 hours. Any failure falls
//! back to the previous cache; a missing/corrupt cache simply yields no games.

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::models::{Game, Platform};
use crate::sources::steam::{active_userdata_dir, detect_steam_root, steam_cover_url};
use crate::state::AppState;

/// The only host this module talks to.
pub const STEAM_API_HOST: &str = "api.steampowered.com";
const OWNED_GAMES_PATH: &str = "/IPlayerService/GetOwnedGames/v1/";
/// Steam Web API keys are 32 hexadecimal characters.
const KEY_LEN: usize = 32;
/// The cache is reused for six hours unless the user presses "Test".
pub const CACHE_TTL_SECONDS: i64 = 6 * 60 * 60;
const CACHE_FILE: &str = "steam-owned-api.json";
/// GetOwnedGames for a 5000-game account is well under 2 MB.
const MAX_BODY_BYTES: usize = 16 * 1024 * 1024;
/// Steam client (7), Steam Cloud (760) and Steamworks redistributables (228980)
/// share the owned-games response but are not games.
const EXCLUDED_APPIDS: &[u32] = &[228_980, 7, 760];
/// Non-Steam shortcuts are stored above this bit; the Web API never returns
/// them, but a hand-edited cache must not smuggle one in.
const SHORTCUT_APPID_HIGH_BIT: u32 = 0x8000_0000;
/// Same constant as `sources::steam` (private there): SteamID64 = base + account id.
const STEAM_ID64_BASE: u64 = 76_561_197_960_265_728;
/// The DPAPI provider name owned by Session M's vault.
const PROVIDER: &str = "steam";

pub const INVALID_KEY_MESSAGE: &str =
    "A Steam Web API key is exactly 32 hexadecimal characters (0-9, A-F). Copy the whole key \
     from steamcommunity.com/dev/apikey and paste it again.";
pub const PRIVACY_MESSAGE: &str =
    "Steam returned no games for this account. In Steam, open your profile → Edit Profile → \
     Privacy Settings and set \"Game details\" to Public, then press Test again.";
pub const RATE_LIMIT_MESSAGE: &str =
    "Steam is rate-limiting requests (HTTP 429). Kudos will not retry automatically; wait a few \
     minutes, then press Test again.";
pub const REJECTED_KEY_MESSAGE: &str =
    "Steam rejected the API key (HTTP 401/403). Copy the 32-character key from \
     steamcommunity.com/dev/apikey again.";
pub const NO_KEY_MESSAGE: &str =
    "No Steam Web API key is saved. Add one to load your complete Steam library.";
pub const NO_ACCOUNT_MESSAGE: &str =
    "Steam has no signed-in account on this PC, so Kudos cannot tell which library to request. \
     Start Steam once, then try again.";

// ---------------------------------------------------------------------------
// Wire types
// ---------------------------------------------------------------------------

/// Result of every Steam Web API command. The wire format is camelCase:
/// `{ "hasKey": true, "lastSync": 1793176000, "ownedCount": 412 }`.
///
/// Deliberately contains no key material. `error` is omitted when empty.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SteamApiStatus {
    pub has_key: bool,
    #[serde(default)]
    pub last_sync: Option<i64>,
    pub owned_count: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl SteamApiStatus {
    fn empty(has_key: bool) -> Self {
        Self {
            has_key,
            last_sync: None,
            owned_count: 0,
            error: None,
        }
    }
}

/// A validated Steam Web API key.
///
/// `Debug` is redacted on purpose; there is no `Serialize` impl, so the key can
/// never end up in a command response or a log line by accident.
#[derive(Clone, PartialEq, Eq)]
pub struct SteamApiKey(String);

impl SteamApiKey {
    /// Accepts exactly 32 hex characters, ignoring surrounding whitespace.
    /// Canonical form is uppercase (Steam's own keys are uppercase).
    pub fn parse(raw: &str) -> Option<Self> {
        let trimmed = raw.trim();
        if trimmed.len() != KEY_LEN || !trimmed.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return None;
        }
        Some(Self(trimmed.to_ascii_uppercase()))
    }

    /// The plaintext, for the one place that needs it: the HTTPS query string
    /// and the DPAPI vault. Never expose this to the frontend.
    pub(crate) fn expose(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for SteamApiKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SteamApiKey(redacted)")
    }
}

/// One app from `GetOwnedGames`. `name` is present because we request
/// `include_appinfo=1`; playtime and icon fields are ignored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiGame {
    pub appid: u32,
    pub name: String,
}

/// On-disk cache. `steamId` scopes the list to one account so switching Steam
/// accounts cannot show the previous library.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OwnedCache {
    #[serde(default)]
    pub fetched_at: Option<i64>,
    #[serde(default)]
    pub steam_id: Option<String>,
    #[serde(default)]
    pub games: Vec<ApiGame>,
}

// ---------------------------------------------------------------------------
// Response parsing
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct Envelope {
    response: Body,
}

#[derive(Deserialize, Default)]
struct Body {
    #[serde(default)]
    game_count: Option<u32>,
    #[serde(default)]
    games: Option<Vec<RawGame>>,
}

#[derive(Deserialize)]
struct RawGame {
    appid: u32,
    #[serde(default)]
    name: String,
}

/// Parses `IPlayerService/GetOwnedGames`. An empty/missing `games` array is the
/// privacy path: Steam answers `{"response":{}}` for a private profile.
pub(crate) fn parse_owned_response(bytes: &[u8]) -> Result<Vec<ApiGame>, String> {
    let envelope: Envelope = serde_json::from_slice(bytes)
        .map_err(|err| format!("Steam's owned-games response was not valid JSON: {err}"))?;
    let Some(raw) = envelope.response.games else {
        return Err(PRIVACY_MESSAGE.to_string());
    };
    if raw.is_empty() || envelope.response.game_count == Some(0) {
        return Err(PRIVACY_MESSAGE.to_string());
    }

    let mut by_appid: BTreeMap<u32, String> = BTreeMap::new();
    for game in raw {
        if !is_displayable_appid(game.appid) {
            continue;
        }
        let name = game.name.trim();
        if name.is_empty() {
            continue;
        }
        by_appid.insert(game.appid, name.to_string());
    }

    if by_appid.is_empty() {
        // Every row was a non-game or had no name: this is still "nothing to show".
        return Err(PRIVACY_MESSAGE.to_string());
    }
    Ok(by_appid
        .into_iter()
        .map(|(appid, name)| ApiGame { appid, name })
        .collect())
}

/// Store app ids only: no zero, no Steam-internal rows, no shortcuts.
fn is_displayable_appid(appid: u32) -> bool {
    appid != 0 && appid < SHORTCUT_APPID_HIGH_BIT && !EXCLUDED_APPIDS.contains(&appid)
}

// ---------------------------------------------------------------------------
// Cache
// ---------------------------------------------------------------------------

fn appdata_dir() -> Option<PathBuf> {
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("LOCALAPPDATA").map(PathBuf::from))
        .or_else(|| std::env::var_os("TEMP").map(PathBuf::from))
}

fn cache_path() -> Option<PathBuf> {
    Some(appdata_dir()?.join("Kudos").join(CACHE_FILE))
}

fn now_unix() -> Option<i64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_secs() as i64)
        .filter(|seconds| *seconds > 0)
}

/// Six-hour TTL with a small future-clock tolerance.
pub fn cache_is_fresh(fetched_at: Option<i64>, now: i64) -> bool {
    let Some(fetched_at) = fetched_at.filter(|seconds| *seconds > 0) else {
        return false;
    };
    if fetched_at > now {
        // A clock jump can leave a timestamp slightly in the future; do not
        // hammer the API because of it.
        return fetched_at - now <= 300;
    }
    now - fetched_at < CACHE_TTL_SECONDS
}

fn cache_matches_account(cache: &OwnedCache, steam_id: Option<&str>) -> bool {
    match (cache.steam_id.as_deref(), steam_id) {
        (Some(cached), Some(current)) => cached == current,
        // A cache without an account (older build) is still usable; a current
        // account we cannot determine keeps whatever cache exists.
        _ => true,
    }
}

fn read_cache_from(path: &Path) -> Option<OwnedCache> {
    let bytes = fs::read(path).ok()?;
    let mut cache = serde_json::from_slice::<OwnedCache>(&bytes).ok()?;
    cache.fetched_at = cache.fetched_at.filter(|seconds| *seconds > 0);
    let mut seen = HashSet::new();
    cache.games.retain(|game| {
        is_displayable_appid(game.appid)
            && !game.name.trim().is_empty()
            && seen.insert(game.appid)
    });
    for game in &mut cache.games {
        let trimmed = game.name.trim().to_string();
        if trimmed != game.name {
            game.name = trimmed;
        }
    }
    Some(cache)
}

fn write_cache_to(path: &Path, cache: &OwnedCache) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("{} has no parent directory", path.display()))?;
    fs::create_dir_all(parent)
        .map_err(|err| format!("could not create {}: {err}", parent.display()))?;
    let json =
        serde_json::to_vec(cache).map_err(|err| format!("could not encode the Steam cache: {err}"))?;

    let tmp = with_suffix(path, ".kudos-tmp");
    fs::write(&tmp, &json).map_err(|err| format!("could not write {}: {err}", tmp.display()))?;
    if let Err(first) = fs::rename(&tmp, path) {
        // Windows normally replaces the destination, but be defensive.
        let _ = fs::remove_file(path);
        if let Err(second) = fs::rename(&tmp, path) {
            let _ = fs::remove_file(&tmp);
            return Err(format!(
                "could not replace {} ({first}; retry: {second})",
                path.display()
            ));
        }
    }
    Ok(())
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut os = path.as_os_str().to_os_string();
    os.push(suffix);
    PathBuf::from(os)
}

fn current_cache() -> Option<OwnedCache> {
    let cache = cache_path().and_then(|path| read_cache_from(&path))?;
    let steam_id = current_steamid64();
    cache_matches_account(&cache, steam_id.as_deref()).then_some(cache)
}

/// The active account's SteamID64, derived from the same `userdata` directory
/// Session B/H already resolve (no second loginusers parser).
pub(crate) fn current_steamid64() -> Option<String> {
    let root = detect_steam_root()?;
    let profile = active_userdata_dir(&root)?;
    let account_id = profile.file_name()?.to_str()?.trim().parse::<u32>().ok()?;
    if account_id == 0 {
        return None;
    }
    Some((STEAM_ID64_BASE + u64::from(account_id)).to_string())
}

// ---------------------------------------------------------------------------
// Game mapping
// ---------------------------------------------------------------------------

/// Maps a cache entry to the public `Game` shape.
///
/// `installed` is the set of ids already returned by `list_games`; the API list
/// is the superset, so an installed game keeps `steam://run/<appid>` and a
/// missing one gets `steam://install/<appid>`.
pub(crate) fn build_games(cache: &OwnedCache, installed: &HashSet<String>) -> Vec<Game> {
    let mut games: Vec<Game> = cache
        .games
        .iter()
        .filter(|game| is_displayable_appid(game.appid))
        .filter_map(|game| {
            let name = game.name.trim();
            if name.is_empty() {
                return None;
            }
            let id = format!("steam:{}", game.appid);
            let is_installed = installed.contains(&id);
            let launch = if is_installed {
                format!("steam://run/{}", game.appid)
            } else {
                format!("steam://install/{}", game.appid)
            };
            let mut mapped = Game::new(id, Platform::Steam, name, launch);
            mapped.is_installed = is_installed;
            mapped.cover_image = Some(steam_cover_url(game.appid));
            Some(mapped)
        })
        .collect();

    games.sort_by(|a, b| {
        a.title
            .to_lowercase()
            .cmp(&b.title.to_lowercase())
            .then_with(|| a.id.cmp(&b.id))
    });
    games.dedup_by(|a, b| a.id == b.id);
    games
}

fn installed_ids(state: &AppState) -> HashSet<String> {
    state
        .all_games()
        .into_iter()
        .map(|game| game.id)
        .collect()
}

/// `get_steam_owned_api_games`: reads the cache only. Never touches the network.
pub fn owned_api_games(state: &AppState) -> Vec<Game> {
    let Some(cache) = current_cache() else {
        return Vec::new();
    };
    build_games(&cache, &installed_ids(state))
}

// ---------------------------------------------------------------------------
// HTTP
// ---------------------------------------------------------------------------

fn owned_games_path(key: &SteamApiKey, steamid: &str) -> String {
    format!(
        "{OWNED_GAMES_PATH}?include_appinfo=1&include_played_free_games=1&key={}&steamid={steamid}",
        key.expose()
    )
}

/// One request with at most one retry. 429/401/403 are final: retrying cannot
/// help and the user gets a specific, actionable message.
fn fetch_owned_games(key: &SteamApiKey, steamid: &str) -> Result<Vec<ApiGame>, String> {
    let path = owned_games_path(key, steamid);
    let mut last_error = String::new();

    for attempt in 0..2 {
        match crate::steam_owned::http_get(STEAM_API_HOST, &path) {
            Ok(body) => {
                if body.len() > MAX_BODY_BYTES {
                    return Err("Steam's owned-games response was unexpectedly large.".to_string());
                }
                return parse_owned_response(&body);
            }
            Err(err) => {
                if err.starts_with("HTTP 429") {
                    return Err(RATE_LIMIT_MESSAGE.to_string());
                }
                if err.starts_with("HTTP 401") || err.starts_with("HTTP 403") {
                    return Err(REJECTED_KEY_MESSAGE.to_string());
                }
                last_error = err;
                if attempt == 0 {
                    // One short pause, then one retry. Never a loop.
                    std::thread::sleep(Duration::from_millis(400));
                }
            }
        }
    }

    Err(format!(
        "Couldn't reach Steam after two attempts. Check your connection and try again. ({last_error})"
    ))
}

// ---------------------------------------------------------------------------
// Status + commands
// ---------------------------------------------------------------------------

static LAST_ERROR: OnceLock<Mutex<Option<String>>> = OnceLock::new();

fn last_error_slot() -> &'static Mutex<Option<String>> {
    LAST_ERROR.get_or_init(|| Mutex::new(None))
}

fn set_last_error(error: Option<String>) {
    if let Ok(mut slot) = last_error_slot().lock() {
        *slot = error;
    }
}

fn remembered_error() -> Option<String> {
    last_error_slot().lock().ok().and_then(|slot| slot.clone())
}

fn status_from_cache(has_key: bool, cache: Option<&OwnedCache>) -> SteamApiStatus {
    SteamApiStatus {
        has_key,
        last_sync: cache.and_then(|cache| cache.fetched_at),
        owned_count: cache
            .map(|cache| cache.games.len().try_into().unwrap_or(u32::MAX))
            .unwrap_or(0),
        error: remembered_error(),
    }
}

fn with_error(mut status: SteamApiStatus, error: impl Into<String>) -> SteamApiStatus {
    status.error = Some(error.into());
    status
}

/// A safe status for command-layer failures (for example a panicked worker):
/// keeps the saved-key/cache view and adds a readable error. No key material.
pub fn error_status(error: impl Into<String>) -> SteamApiStatus {
    with_error(status(), error)
}

/// `get_steam_api_status`: local-only (DPAPI read + cache read). No network.
pub fn status() -> SteamApiStatus {
    let has_key = stored_key().is_some();
    let cache = current_cache();
    status_from_cache(has_key, cache.as_ref())
}

/// Persists a validated key, then attempts exactly one sync. An invalid key
/// never replaces a previously saved one.
pub fn save_key(raw: &str) -> SteamApiStatus {
    let Some(key) = SteamApiKey::parse(raw) else {
        let status = status();
        set_last_error(Some(INVALID_KEY_MESSAGE.to_string()));
        return with_error(status, INVALID_KEY_MESSAGE);
    };

    if let Err(err) = store_key(&key) {
        let status = status();
        set_last_error(Some(err.clone()));
        return with_error(status, err);
    }
    sync_with_key(&key)
}

/// `clear_steam_api_key`: forgets the key and removes the cached library so the
/// next account cannot inherit it.
pub fn clear_key() -> SteamApiStatus {
    let mut error = None;
    if let Err(err) = forget_key() {
        error = Some(err);
    }
    if let Some(path) = cache_path() {
        let _ = fs::remove_file(&path);
    }
    set_last_error(error.clone());
    let mut status = SteamApiStatus::empty(false);
    status.error = error;
    status
}

/// `sync_steam_owned`: bypasses the TTL and refreshes the cache.
pub fn sync_now() -> SteamApiStatus {
    let Some(key) = stored_key() else {
        set_last_error(Some(NO_KEY_MESSAGE.to_string()));
        return with_error(status_from_cache(false, current_cache().as_ref()), NO_KEY_MESSAGE);
    };
    sync_with_key(&key)
}

fn sync_with_key(key: &SteamApiKey) -> SteamApiStatus {
    let Some(steamid) = current_steamid64() else {
        set_last_error(Some(NO_ACCOUNT_MESSAGE.to_string()));
        let status = status_from_cache(true, current_cache().as_ref());
        return with_error(status, NO_ACCOUNT_MESSAGE);
    };

    match fetch_owned_games(key, &steamid) {
        Ok(games) => {
            let cache = OwnedCache {
                fetched_at: now_unix(),
                steam_id: Some(steamid),
                games,
            };
            if let Some(path) = cache_path() {
                if let Err(err) = write_cache_to(&path, &cache) {
                    set_last_error(Some(err.clone()));
                    let mut status = status_from_cache(true, Some(&cache));
                    status.error = Some(err);
                    return status;
                }
            }
            set_last_error(None);
            status_from_cache(true, Some(&cache))
        }
        Err(err) => {
            // Keep the previous cache untouched; the UI keeps showing it.
            set_last_error(Some(err.clone()));
            let status = status_from_cache(true, current_cache().as_ref());
            with_error(status, err)
        }
    }
}

// ---------------------------------------------------------------------------
// Secret store adapter (Session M: src-tauri/src/secrets.rs)
// ---------------------------------------------------------------------------
//
// These three functions are the only coupling to the vault. The key is stored
// under provider "steam" and never touches a plaintext file.

fn stored_key() -> Option<SteamApiKey> {
    let raw = crate::secrets::get(PROVIDER)?;
    SteamApiKey::parse(&raw)
}

fn store_key(key: &SteamApiKey) -> Result<(), String> {
    crate::secrets::set(PROVIDER, key.expose())
        .map(|_| ())
        .map_err(|err| format!("Couldn't save the Steam API key: {err}"))
}

fn forget_key() -> Result<(), String> {
    crate::secrets::clear(PROVIDER)
        .map(|_| ())
        .map_err(|err| format!("Couldn't remove the Steam API key: {err}"))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    fn fixture() -> &'static [u8] {
        include_bytes!("../tests/fixtures/steam/owned-api.json")
    }

    fn private_fixture() -> &'static [u8] {
        include_bytes!("../tests/fixtures/steam/owned-api-private.json")
    }

    fn temp_path(tag: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "kudos-steam-api-{}-{unique}-{tag}.json",
            std::process::id()
        ))
    }

    fn installed(ids: &[&str]) -> HashSet<String> {
        ids.iter().map(|id| (*id).to_string()).collect()
    }

    #[test]
    fn parses_appinfo_names_and_skips_non_games() {
        let games = parse_owned_response(fixture()).unwrap();
        let ids: Vec<u32> = games.iter().map(|game| game.appid).collect();

        assert_eq!(ids, vec![620, 391540, 413150, 1091500, 1245620]);
        assert!(!ids.contains(&228_980), "redistributables leaked");
        assert!(!ids.contains(&7), "Steam client leaked");
        assert!(!ids.contains(&760), "Steam Cloud leaked");
        assert_eq!(
            games.iter().find(|game| game.appid == 413150).unwrap().name,
            "Stardew Valley",
            "surrounding whitespace must be trimmed"
        );
    }

    #[test]
    fn empty_or_missing_games_is_the_privacy_path() {
        let error = parse_owned_response(private_fixture()).unwrap_err();
        assert_eq!(error, PRIVACY_MESSAGE);
        assert!(error.contains("Game details"), "must name the setting: {error}");

        let empty = br#"{"response":{"game_count":0,"games":[]}}"#;
        assert_eq!(parse_owned_response(empty).unwrap_err(), PRIVACY_MESSAGE);
    }

    #[test]
    fn malformed_json_is_a_readable_error() {
        let error = parse_owned_response(b"<html>nope</html>").unwrap_err();
        assert!(error.contains("not valid JSON"), "{error}");
        assert!(!error.contains("key="), "an error must never echo the request URL");
    }

    #[test]
    fn key_format_is_strict_and_canonical() {
        let lower = "0123456789abcdef0123456789abcdef";
        let key = SteamApiKey::parse(&format!("  {lower}  ")).expect("valid key");
        assert_eq!(key.expose(), lower.to_ascii_uppercase());

        assert!(SteamApiKey::parse("0123456789abcdef0123456789abcde").is_none());
        assert!(SteamApiKey::parse("0123456789abcdef0123456789abcdef0").is_none());
        assert!(SteamApiKey::parse("0123456789abcdef0123456789abcdeg").is_none());
        assert!(SteamApiKey::parse("0123 456789abcdef0123456789abcdef").is_none());
        assert!(SteamApiKey::parse("").is_none());
        assert!(SteamApiKey::parse("not-a-steam-key").is_none());
    }

    #[test]
    fn debug_output_never_contains_the_key() {
        let secret = "0123456789ABCDEF0123456789ABCDEF";
        let key = SteamApiKey::parse(secret).unwrap();
        let debug = format!("{key:?}");
        assert!(!debug.contains(secret));
        assert!(debug.contains("redacted"));

        let status = SteamApiStatus {
            has_key: true,
            last_sync: Some(1_793_176_000),
            owned_count: 412,
            error: None,
        };
        let serialized = serde_json::to_string(&status).unwrap();
        assert!(!serialized.contains(secret));
        assert!(!serialized.contains("0123456789"));
        assert!(serialized.contains("\"hasKey\":true"));
        assert!(!serialized.contains("\"error\""));
    }

    #[test]
    fn ttl_is_six_hours_with_clock_skew_tolerance() {
        let now = 1_793_176_000;
        assert!(!cache_is_fresh(None, now));
        assert!(cache_is_fresh(Some(now), now));
        assert!(cache_is_fresh(Some(now - CACHE_TTL_SECONDS + 1), now));
        assert!(!cache_is_fresh(Some(now - CACHE_TTL_SECONDS), now));
        assert!(!cache_is_fresh(Some(0), now));
        assert!(cache_is_fresh(Some(now + 120), now), "small skew is tolerated");
        assert!(!cache_is_fresh(Some(now + 3_600), now), "a far-future stamp is stale");
    }

    #[test]
    fn cache_roundtrip_is_atomic_and_filters_junk() {
        let path = temp_path("roundtrip");
        let cache = OwnedCache {
            fetched_at: Some(1_793_176_000),
            steam_id: Some("76561199806792336".to_string()),
            games: vec![
                ApiGame {
                    appid: 730,
                    name: " Counter-Strike 2 ".to_string(),
                },
                ApiGame {
                    appid: 730,
                    name: "Duplicate".to_string(),
                },
                ApiGame {
                    appid: 7,
                    name: "Steam Client".to_string(),
                },
                ApiGame {
                    appid: 228_980,
                    name: "Redistributables".to_string(),
                },
                ApiGame {
                    appid: 3_000_000_000,
                    name: "Shortcut".to_string(),
                },
                ApiGame {
                    appid: 620,
                    name: "   ".to_string(),
                },
            ],
        };

        write_cache_to(&path, &cache).unwrap();
        assert!(!path.with_extension("json.kudos-tmp").exists());
        let read = read_cache_from(&path).unwrap();
        assert_eq!(read.fetched_at, Some(1_793_176_000));
        assert_eq!(read.steam_id.as_deref(), Some("76561199806792336"));
        assert_eq!(
            read.games,
            vec![ApiGame {
                appid: 730,
                name: "Counter-Strike 2".to_string()
            }]
        );

        fs::write(&path, b"{ not json").unwrap();
        assert!(read_cache_from(&path).is_none(), "a corrupt cache means no games");
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn cache_is_scoped_to_one_steam_account() {
        let cache = OwnedCache {
            fetched_at: Some(1_793_176_000),
            steam_id: Some("76561199806792336".to_string()),
            games: vec![],
        };
        assert!(cache_matches_account(&cache, Some("76561199806792336")));
        assert!(!cache_matches_account(&cache, Some("76561198717205789")));
        assert!(cache_matches_account(&cache, None), "unknown account keeps the cache");
    }

    #[test]
    fn games_mark_installed_and_choose_run_vs_install() {
        let games = parse_owned_response(fixture()).unwrap();
        let cache = OwnedCache {
            fetched_at: Some(1_793_176_000),
            steam_id: None,
            games,
        };
        let mapped = build_games(&cache, &installed(&["steam:1091500", "steam:620"]));
        let by_id: BTreeMap<&str, &Game> =
            mapped.iter().map(|game| (game.id.as_str(), game)).collect();

        let cyberpunk = by_id.get("steam:1091500").unwrap();
        assert!(cyberpunk.is_installed);
        assert_eq!(cyberpunk.launch_command, "steam://run/1091500");
        assert_eq!(cyberpunk.platform, Platform::Steam);

        let portal = by_id.get("steam:620").unwrap();
        assert!(portal.is_installed);
        assert_eq!(portal.launch_command, "steam://run/620");

        let stardew = by_id.get("steam:413150").unwrap();
        assert!(!stardew.is_installed);
        assert_eq!(stardew.launch_command, "steam://install/413150");
        assert_eq!(
            stardew.cover_image.as_deref(),
            Some("https://cdn.cloudflare.steamstatic.com/steam/apps/413150/library_600x900.jpg")
        );

        assert!(!by_id.contains_key("steam:228980"));
        assert_eq!(mapped.len(), 5);
        assert_eq!(
            mapped.first().map(|game| game.title.as_str()),
            Some("Cyberpunk 2077"),
            "title-sorted"
        );
    }

    #[test]
    fn status_omits_error_when_healthy_and_counts_the_cache() {
        let cache = OwnedCache {
            fetched_at: Some(1_793_176_000),
            steam_id: None,
            games: vec![ApiGame {
                appid: 730,
                name: "Counter-Strike 2".to_string(),
            }],
        };
        let status = status_from_cache(true, Some(&cache));
        assert_eq!(status.last_sync, Some(1_793_176_000));
        assert_eq!(status.owned_count, 1);
        assert_eq!(status.error, None);
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(
            json,
            r#"{"hasKey":true,"lastSync":1793176000,"ownedCount":1}"#
        );
    }

    #[test]
    fn status_never_contains_key_material_even_with_an_error() {
        let status = SteamApiStatus {
            has_key: true,
            last_sync: None,
            owned_count: 0,
            error: Some(PRIVACY_MESSAGE.to_string()),
        };
        let json = serde_json::to_string(&status).unwrap();
        assert!(!json.to_lowercase().contains("\"key\""));
        assert!(json.contains("Game details"));
    }

    #[test]
    fn real_machine_account_lookup_is_read_only_and_fast() {
        // No network, no key needed: only reads Steam's loginusers/userdata.
        let started = Instant::now();
        let steamid = current_steamid64();
        assert!(
            started.elapsed() < Duration::from_millis(150),
            "account lookup took {:?}",
            started.elapsed()
        );
        if let Some(steamid) = steamid {
            assert_eq!(steamid.len(), 17, "SteamID64 should be 17 digits");
            assert!(steamid.starts_with("7656119"));
        }
    }
}
