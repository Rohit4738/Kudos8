//! Steam owned-games catalog: playtime, install links and the opt-in name cache.
//!
//! ADDITIVE feature (Session H). This module changes no existing shape and adds
//! four commands:
//!
//! - `get_steam_owned_games()` - every store app Steam remembers for the active
//!   account that is not installed, as a normal `Game` with
//!   `launch_command = "steam://install/<appid>"`.
//! - `get_steam_playtime()` - per-app minutes, last two weeks and `LastPlayed`.
//! - `get_steam_sync_status()` - how many names are cached, and how many app ids
//!   Steam knows locally.
//! - `sync_steam_metadata()` - the only network call in Kudos, user-triggered.
//!
//! ## Local source
//! Steam writes the per-account catalog to
//! `<Steam>/userdata/<accountid>/config/localconfig.vdf`:
//!
//! ```text
//! UserLocalConfigStore -> Software -> Valve -> Steam -> Apps -> <appid> -> { Playtime, Playtime2wks, LastPlayed }
//! ```
//!
//! Parsing is cached by `(path, len, mtime)`; a warm command does no file I/O
//! beyond one `metadata()` call. No network request is ever made while listing.
//!
//! Only store app ids are accepted (1 .. 0x8000_0000). Non-Steam shortcuts are
//! stored under the high bit (or even as negative keys); they are local library
//! entries, not owned Steam apps, and cannot be installed through Steam. Three
//! Steam-internal ids (7, 760 and the redistributables 228980) are also skipped:
//! they share the `Apps` table but are not games.
//!
//! ## Name cache
//! `%APPDATA%\Kudos\steam-apps.json`:
//!
//! ```json
//! { "fetchedAt": 1793176000, "apps": { "730": "Counter-Strike 2" } }
//! ```
//!
//! The cache is filtered to app ids we actually know about (owned + installed),
//! kept small, and replaced atomically (`tmp` + rename). Titles without a cached
//! name fall back to `Steam App <appid>`; covers are only returned when a name is
//! known, so no CDN request is made for an unknown app.

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::models::{Game, Platform};
use crate::sources::steam::{
    active_userdata_dir, detect_steam_root, parse_vdf, read_text_file, steam_cover_url, Vdf,
    REDISTRIBUTABLE_APPID,
};
use crate::state::AppState;

/// Steam publishes non-Steam shortcuts with this bit set. Store app ids are below it.
const SHORTCUT_APPID_HIGH_BIT: u64 = 0x8000_0000;
/// Entries that live in the same `Apps` table but are not games: the Steam
/// client itself (7), Steam Cloud (760) and Steamworks Common Redistributables
/// (228980, excluded by the contract). They have no playtime and no cover.
const NON_GAME_APPIDS: &[u32] = &[7, 760, REDISTRIBUTABLE_APPID];
/// Steam's keyless app-list endpoint, as frozen in the Session H contract.
const APP_LIST_HOST: &str = "api.steampowered.com";
const APP_LIST_PATH: &str = "/ISteamApps/GetAppList/v2/";
/// Fallback used only when `GetAppList` fails. Valve retired the keyless
/// `GetAppList` endpoint (HTTP 404 as of 2026); the store's own keyless batch
/// endpoint still returns official names. Same host, same user-triggered action.
const STORE_BROWSE_PATH: &str = "/IStoreBrowseService/GetItems/v1/";
/// The store endpoint accepts at least 100 ids per request.
const STORE_BATCH: usize = 100;
/// Hard cap for the downloaded list; the real payload is a few megabytes.
const MAX_APP_LIST_BYTES: usize = 64 * 1024 * 1024;
/// `%APPDATA%\Kudos\steam-apps.json` keeps only the ids Kudos can display.
const NAME_CACHE_FILE: &str = "steam-apps.json";

/// One row of `get_steam_playtime`. Wire format is camelCase:
/// `{ "appId": "steam:730", "minutes": 2044, "twoWeeksMinutes": 0, "lastPlayed": 1793162961 }`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SteamPlaytime {
    pub app_id: String,
    pub minutes: u64,
    pub two_weeks_minutes: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_played: Option<i64>,
}

/// Result of `get_steam_sync_status` and `sync_steam_metadata`. Wire format:
/// `{ "cached": true, "appCount": 96, "fetchedAt": 1793176000, "ownedAppCount": 123 }`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SteamSyncStatus {
    /// A non-empty name cache exists on disk.
    pub cached: bool,
    /// Number of `appid -> name` pairs in the cache.
    pub app_count: u32,
    /// Unix seconds when the cache was written, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fetched_at: Option<i64>,
    /// Distinct store app ids Steam knows for the active account (installed included).
    pub owned_app_count: u32,
}

/// Parsed playtime fields for one app id.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct AppUsage {
    pub minutes: u64,
    pub two_weeks_minutes: u64,
    pub last_played: Option<i64>,
}

// ---------------------------------------------------------------------------
// localconfig.vdf parsing
// ---------------------------------------------------------------------------

/// Parses `UserLocalConfigStore -> Software -> Valve -> Steam -> Apps`.
///
/// Missing tables, non-numeric app ids, non-store (shortcut) ids and malformed
/// numbers are skipped. A corrupt file returns an empty map: this function is
/// called while Steam may be rewriting the file, so it must never panic.
pub(crate) fn parse_app_usage(text: &str) -> BTreeMap<u32, AppUsage> {
    let mut usage = BTreeMap::new();
    let Ok(vdf) = parse_vdf(text) else {
        return usage;
    };
    let Some(root) = vdf.as_table() else {
        return usage;
    };
    let Some(apps) = nested_table(
        root,
        &["UserLocalConfigStore", "Software", "Valve", "Steam", "Apps"],
    ) else {
        return usage;
    };

    for (key, value) in apps {
        let Some(appid) = store_appid(key) else {
            continue;
        };
        let Some(table) = value.as_table() else {
            continue;
        };
        let last_played = field_u64(table, "LastPlayed")
            .filter(|seconds| *seconds > 0)
            .and_then(|seconds| i64::try_from(seconds).ok());
        usage.insert(
            appid,
            AppUsage {
                minutes: field_u64(table, "Playtime").unwrap_or(0),
                two_weeks_minutes: field_u64(table, "Playtime2wks").unwrap_or(0),
                last_played,
            },
        );
    }
    usage
}

/// Accepts only real Steam store app ids: positive and below the shortcut bit.
fn store_appid(key: &str) -> Option<u32> {
    let value = key.trim().parse::<u64>().ok()?;
    if value == 0 || value >= SHORTCUT_APPID_HIGH_BIT {
        return None;
    }
    u32::try_from(value).ok()
}

/// Case-insensitive nested table lookup, matching Steam's own key handling.
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

fn field_u64(table: &[(String, Vdf)], key: &str) -> Option<u64> {
    field(table, key)?.as_str()?.trim().parse::<u64>().ok()
}

// ---------------------------------------------------------------------------
// Cached local catalog
// ---------------------------------------------------------------------------

struct UsageCache {
    stamp: String,
    apps: BTreeMap<u32, AppUsage>,
}

static USAGE_CACHE: OnceLock<Mutex<UsageCache>> = OnceLock::new();

fn cache_stamp(path: &Path) -> Option<String> {
    let metadata = fs::metadata(path).ok()?;
    let modified = metadata.modified().ok()?.duration_since(UNIX_EPOCH).ok()?;
    Some(format!(
        "{}:{}:{}",
        path.display(),
        metadata.len(),
        modified.as_nanos()
    ))
}

fn local_config_path() -> Option<PathBuf> {
    let root = detect_steam_root()?;
    let profile = active_userdata_dir(&root)?;
    Some(profile.join("config").join("localconfig.vdf"))
}

/// The active account's catalog, cached until `localconfig.vdf` changes.
fn load_usage() -> BTreeMap<u32, AppUsage> {
    let Some(path) = local_config_path() else {
        return BTreeMap::new();
    };
    let Some(stamp) = cache_stamp(&path) else {
        return BTreeMap::new();
    };

    let cache = USAGE_CACHE.get_or_init(|| {
        Mutex::new(UsageCache {
            stamp: String::new(),
            apps: BTreeMap::new(),
        })
    });
    if let Ok(guard) = cache.lock() {
        if guard.stamp == stamp {
            return guard.apps.clone();
        }
    }

    let apps = read_text_file(&path)
        .map(|text| parse_app_usage(&text))
        .unwrap_or_default();
    if let Ok(mut guard) = cache.lock() {
        guard.stamp = stamp;
        guard.apps = apps.clone();
    }
    apps
}

fn known_owned_count() -> u32 {
    load_usage()
        .keys()
        .filter(|appid| !NON_GAME_APPIDS.contains(appid))
        .count()
        .try_into()
        .unwrap_or(u32::MAX)
}

// ---------------------------------------------------------------------------
// Name cache
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NameCacheFile {
    #[serde(default)]
    fetched_at: Option<i64>,
    #[serde(default)]
    apps: BTreeMap<String, String>,
}

fn appdata_dir() -> Option<PathBuf> {
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("LOCALAPPDATA").map(PathBuf::from))
        .or_else(|| std::env::var_os("TEMP").map(PathBuf::from))
}

fn name_cache_path() -> Option<PathBuf> {
    Some(appdata_dir()?.join("Kudos").join(NAME_CACHE_FILE))
}

fn read_name_cache_from(path: &Path) -> NameCacheFile {
    let Ok(bytes) = fs::read(path) else {
        return NameCacheFile::default();
    };
    let Ok(mut cache) = serde_json::from_slice::<NameCacheFile>(&bytes) else {
        // A corrupt cache only means "no names"; never break the library over it.
        return NameCacheFile::default();
    };
    cache.apps.retain(|key, name| {
        key.trim().parse::<u32>().is_ok() && !name.trim().is_empty()
    });
    for name in cache.apps.values_mut() {
        let trimmed = name.trim().to_string();
        if trimmed.len() != name.len() {
            *name = trimmed;
        }
    }
    cache.fetched_at = cache.fetched_at.filter(|seconds| *seconds > 0);
    cache
}

fn write_name_cache_to(path: &Path, cache: &NameCacheFile) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("{} has no parent directory", path.display()))?;
    fs::create_dir_all(parent)
        .map_err(|err| format!("could not create {}: {err}", parent.display()))?;
    let json = serde_json::to_vec(cache).map_err(|err| format!("could not encode the name cache: {err}"))?;

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

fn cached_names() -> BTreeMap<String, String> {
    name_cache_path()
        .map(|path| read_name_cache_from(&path).apps)
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Commands (pure builders + entry points)
// ---------------------------------------------------------------------------

/// Builds the uninstalled catalog. Split out so filtering is unit-tested.
pub(crate) fn build_owned_games(
    usage: &BTreeMap<u32, AppUsage>,
    names: &BTreeMap<String, String>,
    installed_ids: &HashSet<String>,
) -> Vec<Game> {
    let mut games: Vec<Game> = usage
        .keys()
        .filter_map(|appid| {
            if NON_GAME_APPIDS.contains(appid) {
                return None;
            }
            let id = format!("steam:{appid}");
            if installed_ids.contains(&id) {
                return None;
            }
            let known_title = names
                .get(&appid.to_string())
                .map(|name| name.trim())
                .filter(|name| !name.is_empty());

            let title = known_title
                .map(str::to_string)
                .unwrap_or_else(|| format!("Steam App {appid}"));
            let mut game = Game::new(
                id,
                Platform::Steam,
                title,
                format!("steam://install/{appid}"),
            );
            game.is_installed = false;
            if known_title.is_some() {
                // Only advertise a CDN cover when we know this is a real app.
                game.cover_image = Some(steam_cover_url(*appid));
            }
            Some(game)
        })
        .collect();

    games.sort_by(|a, b| {
        a.title
            .to_lowercase()
            .cmp(&b.title.to_lowercase())
            .then_with(|| a.id.cmp(&b.id))
    });
    games
}

fn installed_ids(state: &AppState) -> HashSet<String> {
    state
        .all_games()
        .into_iter()
        .map(|game| game.id)
        .collect()
}

/// `get_steam_owned_games`: never rejects, never touches the network.
pub fn owned_games(state: &AppState) -> Vec<Game> {
    let installed = installed_ids(state);
    build_owned_games(&load_usage(), &cached_names(), &installed)
}

/// The `steam://install/<appid>` command for one owned, uninstalled app.
///
/// `launch_game` needs this because the catalog is not part of any source's
/// `installed()` scan. Returns `None` for installed games (they launch normally),
/// unknown ids and non-game app ids.
pub fn install_command(state: &AppState, game_id: &str) -> Option<String> {
    let appid = game_id.strip_prefix("steam:")?.trim().parse::<u32>().ok()?;
    if NON_GAME_APPIDS.contains(&appid) || !load_usage().contains_key(&appid) {
        return None;
    }
    if state.all_games().iter().any(|game| game.id == game_id) {
        return None;
    }
    Some(format!("steam://install/{appid}"))
}

/// `get_steam_playtime`: never rejects. Sorted by minutes, then app id.
pub fn playtime() -> Vec<SteamPlaytime> {
    let mut rows: Vec<SteamPlaytime> = load_usage()
        .into_iter()
        .map(|(appid, usage)| SteamPlaytime {
            app_id: format!("steam:{appid}"),
            minutes: usage.minutes,
            two_weeks_minutes: usage.two_weeks_minutes,
            last_played: usage.last_played,
        })
        .collect();
    rows.sort_by(|a, b| {
        b.minutes
            .cmp(&a.minutes)
            .then_with(|| a.app_id.cmp(&b.app_id))
    });
    rows
}

fn status_from_cache(cache: &NameCacheFile, owned: u32) -> SteamSyncStatus {
    SteamSyncStatus {
        cached: !cache.apps.is_empty(),
        app_count: cache.apps.len().try_into().unwrap_or(u32::MAX),
        fetched_at: cache.fetched_at,
        owned_app_count: owned,
    }
}

/// `get_steam_sync_status`: local-only, no state and no network needed.
pub fn sync_status() -> SteamSyncStatus {
    let cache = name_cache_path()
        .map(|path| read_name_cache_from(&path))
        .unwrap_or_default();
    status_from_cache(&cache, known_owned_count())
}

/// App ids the name cache should cover: everything localconfig knows plus every
/// installed Steam game (even if localconfig has no row for it yet).
pub fn known_appids(state: &AppState) -> HashSet<u32> {
    let mut known: HashSet<u32> = load_usage()
        .keys()
        .copied()
        .filter(|appid| !NON_GAME_APPIDS.contains(appid))
        .collect();
    for game in state.all_games() {
        if game.platform != Platform::Steam {
            continue;
        }
        if let Some(appid) = game
            .id
            .strip_prefix("steam:")
            .and_then(|id| id.trim().parse::<u32>().ok())
        {
            known.insert(appid);
        }
    }
    known
}

// ---------------------------------------------------------------------------
// Opt-in metadata sync (the only network call)
// ---------------------------------------------------------------------------

/// `sync_steam_metadata`: download Steam's public app list, keep only the names
/// for ids we know, and atomically replace the cache. Two attempts per request.
///
/// `GetAppList` v2 is the contract's primary source. Valve retired its keyless
/// form in 2026 (HTTP 404), so on failure we ask the store's own keyless batch
/// endpoint (`IStoreBrowseService/GetItems`) for the same ids. Still one
/// user-triggered action, still local cache + atomic replace afterwards.
pub fn sync_metadata(known: HashSet<u32>) -> Result<SteamSyncStatus, String> {
    if known.is_empty() {
        return Err(
            "Steam has no known app ids on this machine. Start Steam once, then try again."
                .to_string(),
        );
    }

    let names = fetch_names(&known)?;
    if names.is_empty() {
        return Err(
            "Steam returned no names for this account's app ids. The cached library was left \
             unchanged."
                .to_string(),
        );
    }

    let cache = NameCacheFile {
        fetched_at: now_unix(),
        apps: names,
    };
    let path = name_cache_path()
        .ok_or_else(|| "could not resolve %APPDATA% for the Steam name cache".to_string())?;
    write_name_cache_to(&path, &cache)?;
    Ok(status_from_cache(&cache, known_owned_count()))
}

/// Tries the frozen `GetAppList` path first, then the store fallback.
fn fetch_names(known: &HashSet<u32>) -> Result<BTreeMap<String, String>, String> {
    match fetch_app_list_with_retry().and_then(|body| parse_app_list(&body, known)) {
        Ok(names) if !names.is_empty() => Ok(names),
        Ok(_) => {
            eprintln!("[kudos] GetAppList returned no names for known apps; trying the store fallback");
            fetch_store_names(known)
        }
        Err(primary_error) => {
            eprintln!("[kudos] GetAppList failed ({primary_error}); trying the store fallback");
            fetch_store_names(known).map_err(|fallback_error| {
                format!("{primary_error} The store fallback also failed: {fallback_error}")
            })
        }
    }
}

/// Keeps only names for known ids; unknown/blank names are dropped.
pub(crate) fn parse_app_list(
    bytes: &[u8],
    known: &HashSet<u32>,
) -> Result<BTreeMap<String, String>, String> {
    #[derive(Deserialize)]
    struct Response {
        applist: AppList,
    }
    #[derive(Deserialize)]
    struct AppList {
        apps: Vec<Entry>,
    }
    #[derive(Deserialize)]
    struct Entry {
        appid: u32,
        #[serde(default)]
        name: String,
    }

    let response: Response = serde_json::from_slice(bytes)
        .map_err(|err| format!("Steam's app list was not valid JSON: {err}"))?;
    let mut names = BTreeMap::new();
    for entry in response.applist.apps {
        if !known.contains(&entry.appid) {
            continue;
        }
        let name = entry.name.trim();
        if !name.is_empty() {
            names.insert(entry.appid.to_string(), name.to_string());
        }
    }
    Ok(names)
}

/// Parses `IStoreBrowseService/GetItems` and keeps known, non-empty names.
pub(crate) fn parse_store_items(
    bytes: &[u8],
    known: &HashSet<u32>,
) -> Result<BTreeMap<String, String>, String> {
    #[derive(Deserialize)]
    struct Response {
        response: Body,
    }
    #[derive(Deserialize)]
    struct Body {
        #[serde(default)]
        store_items: Vec<Item>,
    }
    #[derive(Deserialize)]
    struct Item {
        id: u32,
        #[serde(default)]
        name: String,
    }

    let response: Response = serde_json::from_slice(bytes)
        .map_err(|err| format!("Steam's store item response was not valid JSON: {err}"))?;
    let mut names = BTreeMap::new();
    for item in response.response.store_items {
        if !known.contains(&item.id) {
            continue;
        }
        let name = item.name.trim();
        if !name.is_empty() {
            names.insert(item.id.to_string(), name.to_string());
        }
    }
    Ok(names)
}

/// Requests names in batches of [`STORE_BATCH`] from the keyless store endpoint.
fn fetch_store_names(known: &HashSet<u32>) -> Result<BTreeMap<String, String>, String> {
    let mut ids: Vec<u32> = known.iter().copied().collect();
    ids.sort_unstable();
    let mut names = BTreeMap::new();
    for chunk in ids.chunks(STORE_BATCH) {
        let path = store_browse_path(chunk);
        let body = http_with_retry(APP_LIST_HOST, &path, "Steam store item request")?;
        names.extend(parse_store_items(&body, known)?);
    }
    Ok(names)
}

/// Builds the percent-encoded `input_json` query for a batch of app ids.
fn store_browse_path(appids: &[u32]) -> String {
    let mut json = String::from("{\"ids\":[");
    for (index, appid) in appids.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        json.push_str(&format!("{{\"appid\":{appid}}}"));
    }
    json.push_str(
        "],\"context\":{\"language\":\"english\",\"country_code\":\"US\"},\
         \"data_request\":{\"include_basic_info\":false}}",
    );
    format!("{STORE_BROWSE_PATH}?input_json={}", percent_encode(&json))
}

fn percent_encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len() * 2);
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

fn fetch_app_list_with_retry() -> Result<Vec<u8>, String> {
    http_with_retry(APP_LIST_HOST, APP_LIST_PATH, "Steam app list request").map_err(|err| {
        if err.contains("HTTP 404") {
            "Steam's public GetAppList v2 endpoint returned HTTP 404 (Valve appears to have \
             retired the keyless endpoint)."
                .to_string()
        } else {
            err
        }
    })
}

fn http_with_retry(host: &str, path: &str, label: &str) -> Result<Vec<u8>, String> {
    let mut last_error = String::new();
    for attempt in 0..2 {
        match http_get(host, path) {
            Ok(body) => return Ok(body),
            Err(err) => {
                last_error = err;
                if attempt == 0 {
                    eprintln!("[kudos] {label} failed ({last_error}); retrying once");
                }
            }
        }
    }
    Err(format!("{label} failed: {last_error}"))
}

fn now_unix() -> Option<i64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_secs() as i64)
        .filter(|seconds| *seconds > 0)
}

#[cfg(windows)]
mod http {
    use std::ffi::c_void;
    use std::ptr::null;

    type InternetHandle = *mut c_void;

    const WINHTTP_ACCESS_TYPE_DEFAULT_PROXY: u32 = 0;
    const WINHTTP_FLAG_SECURE: u32 = 0x0080_0000;
    const WINHTTP_QUERY_STATUS_CODE: u32 = 19;
    const WINHTTP_QUERY_FLAG_NUMBER: u32 = 0x2000_0000;
    const MAX_BODY: usize = super::MAX_APP_LIST_BYTES;

    #[link(name = "winhttp")]
    extern "system" {
        fn WinHttpOpen(
            agent: *const u16,
            access_type: u32,
            proxy: *const u16,
            proxy_bypass: *const u16,
            flags: u32,
        ) -> InternetHandle;
        fn WinHttpConnect(
            session: InternetHandle,
            server: *const u16,
            port: u16,
            reserved: u32,
        ) -> InternetHandle;
        fn WinHttpOpenRequest(
            connect: InternetHandle,
            verb: *const u16,
            object: *const u16,
            version: *const u16,
            referrer: *const u16,
            accept_types: *const *const u16,
            flags: u32,
        ) -> InternetHandle;
        fn WinHttpSendRequest(
            request: InternetHandle,
            headers: *const u16,
            headers_len: u32,
            optional: *const c_void,
            optional_len: u32,
            total_len: u32,
            context: usize,
        ) -> i32;
        fn WinHttpReceiveResponse(request: InternetHandle, reserved: *mut c_void) -> i32;
        fn WinHttpQueryHeaders(
            request: InternetHandle,
            info_level: u32,
            name: *const u16,
            buffer: *mut c_void,
            buffer_len: *mut u32,
            index: *mut u32,
        ) -> i32;
        fn WinHttpReadData(
            request: InternetHandle,
            buffer: *mut c_void,
            to_read: u32,
            read: *mut u32,
        ) -> i32;
        fn WinHttpSetTimeouts(
            handle: InternetHandle,
            resolve: i32,
            connect: i32,
            send: i32,
            receive: i32,
        ) -> i32;
        fn WinHttpCloseHandle(handle: InternetHandle) -> i32;
    }

    struct Handle(InternetHandle);

    impl Drop for Handle {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe {
                    WinHttpCloseHandle(self.0);
                }
            }
        }
    }

    fn wide(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn failure(what: &str) -> String {
        format!("{what} failed: {}", std::io::Error::last_os_error())
    }

    /// A blocking HTTPS GET with 10 s receive timeout. Called from a worker thread.
    pub(super) fn get(host: &str, path: &str) -> Result<Vec<u8>, String> {
        let agent = wide("Kudos/0.1 (local Steam metadata sync)");
        let session = Handle(unsafe {
            WinHttpOpen(
                agent.as_ptr(),
                WINHTTP_ACCESS_TYPE_DEFAULT_PROXY,
                null(),
                null(),
                0,
            )
        });
        if session.0.is_null() {
            return Err(failure("WinHttpOpen"));
        }
        // Resolve/connect/send bounded by 5 s; the whole receive phase gets 10 s.
        unsafe {
            WinHttpSetTimeouts(session.0, 5_000, 5_000, 5_000, 10_000);
        }

        let host_wide = wide(host);
        let connect = Handle(unsafe { WinHttpConnect(session.0, host_wide.as_ptr(), 443, 0) });
        if connect.0.is_null() {
            return Err(failure("WinHttpConnect"));
        }

        let verb = wide("GET");
        let object = wide(path);
        let request = Handle(unsafe {
            WinHttpOpenRequest(
                connect.0,
                verb.as_ptr(),
                object.as_ptr(),
                null(),
                null(),
                null(),
                WINHTTP_FLAG_SECURE,
            )
        });
        if request.0.is_null() {
            return Err(failure("WinHttpOpenRequest"));
        }

        if unsafe { WinHttpSendRequest(request.0, null(), 0, null(), 0, 0, 0) } == 0 {
            return Err(failure("WinHttpSendRequest"));
        }
        if unsafe { WinHttpReceiveResponse(request.0, std::ptr::null_mut()) } == 0 {
            return Err(failure("WinHttpReceiveResponse"));
        }

        let mut status = 0u32;
        let mut status_len = std::mem::size_of::<u32>() as u32;
        let status_ok = unsafe {
            WinHttpQueryHeaders(
                request.0,
                WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
                null(),
                (&mut status as *mut u32).cast(),
                &mut status_len,
                std::ptr::null_mut(),
            )
        };
        if status_ok == 0 {
            return Err(failure("WinHttpQueryHeaders"));
        }
        if !(200..300).contains(&status) {
            return Err(format!("HTTP {status}"));
        }

        let mut body = Vec::new();
        let mut buffer = vec![0u8; 64 * 1024];
        loop {
            let mut read = 0u32;
            if unsafe {
                WinHttpReadData(
                    request.0,
                    buffer.as_mut_ptr().cast(),
                    buffer.len() as u32,
                    &mut read,
                )
            } == 0
            {
                return Err(failure("WinHttpReadData"));
            }
            if read == 0 {
                break;
            }
            body.extend_from_slice(&buffer[..read as usize]);
            if body.len() > MAX_BODY {
                return Err("Steam's app list response was unexpectedly large".to_string());
            }
        }
        Ok(body)
    }
}

#[cfg(windows)]
fn http_get(host: &str, path: &str) -> Result<Vec<u8>, String> {
    http::get(host, path)
}

#[cfg(not(windows))]
fn http_get(_host: &str, _path: &str) -> Result<Vec<u8>, String> {
    Err("Steam metadata sync is only supported on Windows".to_string())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    fn fixture() -> &'static str {
        include_str!("../tests/fixtures/steam/localconfig-owned.vdf")
    }

    fn temp_path(tag: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "kudos-owned-{}-{unique}-{tag}",
            std::process::id()
        ))
    }

    fn installed(ids: &[&str]) -> HashSet<String> {
        ids.iter().map(|id| (*id).to_string()).collect()
    }

    #[test]
    fn parses_playtime_two_weeks_and_last_played_from_quoted_keys() {
        let usage = parse_app_usage(fixture());

        assert_eq!(
            usage.get(&730),
            Some(&AppUsage {
                minutes: 555,
                two_weeks_minutes: 0,
                last_played: Some(1_792_855_921),
            })
        );
        assert_eq!(
            usage.get(&413150),
            Some(&AppUsage {
                minutes: 842,
                two_weeks_minutes: 120,
                last_played: Some(1_782_486_344),
            })
        );
        // Quoted key with surrounding spaces is still a valid app id.
        assert_eq!(usage.get(&480).map(|row| row.minutes), Some(60));
        // No Playtime / LastPlayed fields at all.
        assert_eq!(
            usage.get(&3564740),
            Some(&AppUsage {
                minutes: 0,
                two_weeks_minutes: 0,
                last_played: None,
            })
        );
        // Redistributables are parsed, then filtered from the owned catalog.
        assert!(usage.contains_key(&REDISTRIBUTABLE_APPID));
    }

    #[test]
    fn ignores_shortcuts_junk_and_the_second_apps_table() {
        let usage = parse_app_usage(fixture());
        assert!(!usage.contains_key(&2798833905), "high-bit shortcut leaked");
        assert!(!usage.contains_key(&7), "top-level apps table leaked");
        assert!(!usage.contains_key(&760), "top-level apps table leaked");
        assert!(!usage.contains_key(&0));
        // 999 is a plain unknown app, abc and the negative shortcut key are junk.
        assert_eq!(usage.len(), 6);
        assert!(parse_app_usage("not a vdf").is_empty());
        assert!(parse_app_usage("").is_empty());
    }

    #[test]
    fn owned_games_exclude_installed_and_redistributables() {
        let mut usage = parse_app_usage(fixture());
        // Steam-internal rows that share the Apps table but are not games.
        usage.insert(7, AppUsage::default());
        usage.insert(760, AppUsage::default());
        let mut names = BTreeMap::new();
        names.insert("730".to_string(), "Counter-Strike 2".to_string());
        names.insert("413150".to_string(), "Stardew Valley".to_string());
        names.insert(
            REDISTRIBUTABLE_APPID.to_string(),
            "Steamworks Common Redistributables".to_string(),
        );

        let games = build_owned_games(&usage, &names, &installed(&["steam:730"]));
        let ids: Vec<&str> = games.iter().map(|game| game.id.as_str()).collect();
        assert_eq!(
            ids,
            vec!["steam:413150", "steam:3564740", "steam:480", "steam:999"]
        );

        assert!(!ids.contains(&"steam:730"), "installed game leaked");
        assert!(!ids.contains(&"steam:228980"), "redistributable leaked");
        assert!(!ids.contains(&"steam:2798833905"));
        assert!(!ids.contains(&"steam:7"), "Steam client row leaked");
        assert!(!ids.contains(&"steam:760"), "Steam Cloud row leaked");

        let stardew = games.iter().find(|game| game.id == "steam:413150").unwrap();
        assert_eq!(stardew.platform, Platform::Steam);
        assert!(!stardew.is_installed);
        assert_eq!(stardew.launch_command, "steam://install/413150");
        assert_eq!(
            stardew.cover_image.as_deref(),
            Some("https://cdn.cloudflare.steamstatic.com/steam/apps/413150/library_600x900.jpg")
        );

        let unknown = games.iter().find(|game| game.id == "steam:999").unwrap();
        assert_eq!(unknown.title, "Steam App 999");
        assert_eq!(unknown.cover_image, None);
    }

    #[test]
    fn name_cache_roundtrip_is_atomic_and_filters_junk() {
        let path = temp_path("steam-apps.json");
        let _ = fs::remove_file(&path);

        let cache = NameCacheFile {
            fetched_at: Some(1_793_176_000),
            apps: BTreeMap::from([
                ("730".to_string(), "Counter-Strike 2".to_string()),
                ("413150".to_string(), "Stardew Valley".to_string()),
            ]),
        };
        write_name_cache_to(&path, &cache).unwrap();

        let loaded = read_name_cache_from(&path);
        assert_eq!(loaded, cache);
        assert!(!with_suffix(&path, ".kudos-tmp").exists());

        // Corrupt and junk entries degrade to an empty/clean cache, never panic.
        fs::write(&path, b"not json").unwrap();
        assert_eq!(read_name_cache_from(&path), NameCacheFile::default());
        fs::write(
            &path,
            br#"{"fetchedAt":0,"apps":{"730":"  Counter-Strike 2  ","abc":"Nope","999":"   "}}"#,
        )
        .unwrap();
        let cleaned = read_name_cache_from(&path);
        assert_eq!(cleaned.fetched_at, None);
        assert_eq!(cleaned.apps.len(), 1);
        assert_eq!(cleaned.apps.get("730").map(String::as_str), Some("Counter-Strike 2"));
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn app_list_filter_keeps_only_known_non_empty_names() {
        let json = br#"
        {
          "applist": {
            "apps": [
              { "appid": 730, "name": "Counter-Strike 2" },
              { "appid": 413150, "name": "Stardew Valley" },
              { "appid": 1, "name": "Unknown To This Machine" },
              { "appid": 480, "name": "   " }
            ]
          }
        }
        "#;
        let known: HashSet<u32> = [730, 413150, 1, 480].into_iter().collect();
        let names = parse_app_list(json, &known).unwrap();
        assert_eq!(names.len(), 3);
        assert_eq!(names.get("730").map(String::as_str), Some("Counter-Strike 2"));
        assert_eq!(
            names.get("1").map(String::as_str),
            Some("Unknown To This Machine")
        );
        assert!(!names.contains_key("480"));

        let known: HashSet<u32> = [730, 413150].into_iter().collect();
        let filtered = parse_app_list(json, &known).unwrap();
        assert_eq!(filtered.len(), 2);
        assert!(parse_app_list(b"{}", &known).is_err());
    }

    #[test]
    fn store_fallback_keeps_known_names_and_builds_batches() {
        let json = br#"
        {
          "response": {
            "store_items": [
              { "id": 730, "success": 1, "visible": true, "name": "Counter-Strike 2" },
              { "id": 413150, "success": 15, "visible": false, "name": "" },
              { "id": 999999, "success": 1, "visible": true, "name": "Not Known" },
              { "id": 480, "success": 1, "visible": true, "name": "Spacewar" }
            ]
          }
        }
        "#;
        let known: HashSet<u32> = [730, 413150, 480].into_iter().collect();
        let names = parse_store_items(json, &known).unwrap();
        assert_eq!(names.len(), 2);
        assert_eq!(names.get("730").map(String::as_str), Some("Counter-Strike 2"));
        assert_eq!(names.get("480").map(String::as_str), Some("Spacewar"));

        let path = store_browse_path(&[730, 413150]);
        assert!(path.starts_with("/IStoreBrowseService/GetItems/v1/?input_json="));
        assert!(path.contains("appid%22%3A730"));
        assert!(!path.contains(' '));
    }

    #[test]
    fn wire_shapes_are_camel_case() {
        let playtime = SteamPlaytime {
            app_id: "steam:730".to_string(),
            minutes: 2044,
            two_weeks_minutes: 845,
            last_played: Some(1_793_162_961),
        };
        assert_eq!(
            serde_json::to_string(&playtime).unwrap(),
            r#"{"appId":"steam:730","minutes":2044,"twoWeeksMinutes":845,"lastPlayed":1793162961}"#
        );
        let none = SteamPlaytime {
            last_played: None,
            ..playtime
        };
        assert_eq!(
            serde_json::to_string(&none).unwrap(),
            r#"{"appId":"steam:730","minutes":2044,"twoWeeksMinutes":845}"#
        );

        let status = SteamSyncStatus {
            cached: true,
            app_count: 109,
            fetched_at: Some(1_793_176_743),
            owned_app_count: 117,
        };
        assert_eq!(
            serde_json::to_string(&status).unwrap(),
            r#"{"cached":true,"appCount":109,"fetchedAt":1793176743,"ownedAppCount":117}"#
        );
    }

    /// Reads this machine's Steam files read-only and prints evidence for the
    /// session report. Skips when Steam is absent.
    #[test]
    fn real_machine_owned_smoke() {
        if detect_steam_root().is_none() {
            eprintln!("[kudos] no Steam install on this machine; skipping");
            return;
        }
        let state = AppState::new();
        let started = Instant::now();
        let games = owned_games(&state);
        let cold = started.elapsed();
        let started = Instant::now();
        let again = owned_games(&state);
        let warm = started.elapsed();
        assert_eq!(games.len(), again.len());

        let library: HashSet<String> = state.all_games().into_iter().map(|game| game.id).collect();
        for game in &games {
            assert!(game.id.starts_with("steam:"));
            assert!(!game.is_installed);
            assert!(!library.contains(&game.id));
            assert_eq!(game.launch_command, format!("steam://install/{}", &game.id[6..]));
            assert!(!game.title.trim().is_empty());
        }
        assert!(!games.iter().any(|game| game.id == "steam:228980"));
        if let Some(first) = games.first() {
            assert_eq!(
                install_command(&state, &first.id).as_deref(),
                Some(first.launch_command.as_str()),
                "install path must mirror the owned Game command"
            );
        }
        assert!(install_command(&state, "steam:999999999").is_none());
        assert!(install_command(&state, "epic:Fortnite").is_none());

        let rows = playtime();
        let with_playtime = rows.iter().filter(|row| row.minutes > 0).count();
        let total_minutes: u64 = rows.iter().map(|row| row.minutes).sum();
        let names = cached_names();
        let top: Vec<String> = rows
            .iter()
            .filter(|row| row.minutes > 0)
            .take(5)
            .map(|row| {
                let title = names
                    .get(row.app_id.trim_start_matches("steam:"))
                    .map(String::as_str)
                    .unwrap_or("(unnamed)");
                format!("{} {} ({} min)", row.app_id, title, row.minutes)
            })
            .collect();

        eprintln!(
            "[kudos] steam owned catalog: {} uninstalled apps, {} playtime rows ({} with minutes), \
             {:.1} total hours; cold {:.1} ms, warm {:.1} ms; {} cached names",
            games.len(),
            rows.len(),
            with_playtime,
            total_minutes as f64 / 60.0,
            cold.as_secs_f64() * 1000.0,
            warm.as_secs_f64() * 1000.0,
            names.len()
        );
        eprintln!("[kudos] top 5: {}", top.join(" | "));

        assert!(
            warm.as_millis() < 100,
            "warm owned-games lookup took {warm:?}"
        );
    }

    /// Manual verification of the only network call. Ignored so normal test runs
    /// and app startup never touch the network.
    #[test]
    #[ignore = "performs a real HTTPS request; run explicitly with --ignored"]
    fn real_machine_metadata_sync_opt_in() {
        if detect_steam_root().is_none() {
            eprintln!("[kudos] no Steam install on this machine; skipping");
            return;
        }
        let state = AppState::new();
        let known = known_appids(&state);
        eprintln!("[kudos] sync target: {} known app ids", known.len());
        match sync_metadata(known) {
            Ok(status) => eprintln!(
                "[kudos] sync ok: cached={} appCount={} fetchedAt={:?} ownedAppCount={}",
                status.cached, status.app_count, status.fetched_at, status.owned_app_count
            ),
            Err(err) => eprintln!("[kudos] sync failed: {err}"),
        }
    }
}
