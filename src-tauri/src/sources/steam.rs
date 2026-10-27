//! Steam source. OWNER: Session B.
//!
//! What this module does:
//! - Finds Steam via `HKCU\Software\Valve\Steam\SteamPath` (with HKLM + well-known
//!   directory fallbacks).
//! - Parses `steamapps/libraryfolders.vdf` (modern and pre-2021 formats) and every
//!   `appmanifest_*.acf` in every library.
//! - Returns BOTH installed games (`is_installed = true`) and games whose manifest
//!   exists but is not fully downloaded (`is_installed = false`). Steam handles the
//!   download/update when the URI is opened.
//! - Launches through `steam://rungameid/<appid>` so Steam Cloud, the overlay and
//!   playtime tracking keep working.
//! - Adds a non-Steam shortcut (e.g. a GOG game) to the active user's
//!   `userdata/<account>/config/shortcuts.vdf` using a full binary-VDF
//!   read-modify-write that preserves every existing entry and unknown field.
//!
//! Design notes:
//! - Cover art is a CDN URL; listing never performs network I/O.
//! - Manifests are cached by (mtime, size); unchanged `.acf` files are not reparsed.
//! - Binary VDF is parsed into an ordered tree and written back from that tree, so
//!   fields this build does not know about survive round-trips intact.
//! - We never overwrite a `shortcuts.vdf` we cannot parse. A backup named
//!   `shortcuts.vdf.kudos-backup` is written before any replacement.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::time::SystemTime;

use winreg::enums::KEY_READ;
use winreg::{RegKey, HKCU, HKLM};

use crate::error::{SourceError, SourceResult};
use crate::launch;
use crate::models::{Game, Platform, SteamStatus};
use crate::sources::LibrarySource;

/// Steamworks Common Redistributables is not a game.
const REDISTRIBUTABLE_APPID: u32 = 228_980;
/// SteamID64 = this + 32-bit account id.
const STEAM_ID64_BASE: u64 = 76_561_197_960_265_728;
const STEAM_CDN: &str = "https://cdn.cloudflare.steamstatic.com/steam/apps";
/// `StateFlags & 4` means "fully installed" in appmanifest files.
const STATE_FULLY_INSTALLED: u32 = 4;
/// Non-Steam shortcuts use app ids with the high bit set (2.1B..4.29B).
const SHORTCUT_APPID_HIGH_BIT: u32 = 0x8000_0000;
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

// ---------------------------------------------------------------------------
// Public source
// ---------------------------------------------------------------------------

pub struct SteamSource {
    cache: Mutex<ScanCache>,
}

impl SteamSource {
    pub fn new() -> Self {
        Self {
            cache: Mutex::new(ScanCache::default()),
        }
    }

    fn cache(&self) -> std::sync::MutexGuard<'_, ScanCache> {
        // A panic in another caller must not take the library down with it.
        self.cache
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
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
        let mut cache = self.cache();
        let Some(root) = cached_steam_root(&mut cache) else {
            // Steam is not installed. That is not an error.
            return Ok(Vec::new());
        };
        Ok(scan_libraries(&root, &mut cache))
    }

    fn launch(&self, game: &Game) -> SourceResult<()> {
        if game.platform != Platform::Steam {
            return Err(SourceError::invalid(format!(
                "{} is not a Steam game",
                game.id
            )));
        }

        // The appid from the stable id is authoritative: it keeps the Steam URI
        // correct even if a caller stored a stale launch_command.
        if let Some(appid) = steam_appid_from_id(&game.id) {
            return launch::open(&format!("steam://rungameid/{appid}"));
        }

        // Fallbacks for hand-built Game values.
        if let Some(exe) = game.exe_path.as_deref() {
            if !exe.trim().is_empty() {
                return launch::open(exe);
            }
        }
        if !game.launch_command.trim().is_empty() {
            return launch::open(&game.launch_command);
        }

        Err(SourceError::not_launchable(format!(
            "Steam game {} has no launch command",
            game.id
        )))
    }
}

fn steam_appid_from_id(id: &str) -> Option<u32> {
    id.strip_prefix("steam:")?.trim().parse::<u32>().ok()
}

// ---------------------------------------------------------------------------
// Scanning + cache
// ---------------------------------------------------------------------------

#[derive(Default)]
struct ScanCache {
    /// Registry lookup is a process spawn; remember a successful result.
    root: Option<PathBuf>,
    library_folders: Option<(FileStamp, Vec<PathBuf>)>,
    manifests: HashMap<String, CachedManifest>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FileStamp {
    modified: Option<SystemTime>,
    len: u64,
}

impl FileStamp {
    fn from_metadata(metadata: &fs::Metadata) -> Self {
        Self {
            modified: metadata.modified().ok(),
            len: metadata.len(),
        }
    }
}

struct CachedManifest {
    stamp: FileStamp,
    manifest: AppManifest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AppManifest {
    appid: u32,
    name: String,
    size_bytes: u64,
    state_flags: u32,
}

impl AppManifest {
    fn is_installed(&self) -> bool {
        self.state_flags & STATE_FULLY_INSTALLED != 0
    }

    fn to_game(&self) -> Game {
        let mut game = Game::new(
            format!("steam:{}", self.appid),
            Platform::Steam,
            self.name.clone(),
            format!("steam://rungameid/{}", self.appid),
        );
        game.is_installed = self.is_installed();
        if self.size_bytes > 0 {
            game.size_bytes = Some(self.size_bytes);
        }
        game.cover_image = Some(steam_cover_url(self.appid));
        game
    }
}

fn steam_cover_url(appid: u32) -> String {
    format!("{STEAM_CDN}/{appid}/library_600x900.jpg")
}

fn cached_steam_root(cache: &mut ScanCache) -> Option<PathBuf> {
    if let Some(root) = &cache.root {
        if root.is_dir() {
            return Some(root.clone());
        }
    }
    let root = detect_steam_root();
    cache.root = root.clone();
    root
}

fn scan_libraries(root: &Path, cache: &mut ScanCache) -> Vec<Game> {
    let libraries = cached_libraries(root, cache);
    let mut games = Vec::new();
    let mut seen_appids = HashSet::new();
    let mut live_manifest_keys = HashSet::new();

    for library in &libraries {
        let steamapps = library.join("steamapps");
        let entries = match fs::read_dir(&steamapps) {
            Ok(entries) => entries,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            if !file_name.starts_with("appmanifest_") || !file_name.ends_with(".acf") {
                continue;
            }
            let Ok(metadata) = entry.metadata() else {
                continue;
            };
            if !metadata.is_file() {
                continue;
            }

            let key = path_key(&path);
            live_manifest_keys.insert(key.clone());
            let stamp = FileStamp::from_metadata(&metadata);

            let cached = cache
                .manifests
                .get(&key)
                .filter(|cached| cached.stamp == stamp)
                .map(|cached| cached.manifest.clone());

            let manifest = match cached {
                Some(manifest) => manifest,
                None => match read_manifest(&path) {
                    Ok(manifest) => {
                        cache.manifests.insert(
                            key,
                            CachedManifest {
                                stamp,
                                manifest: manifest.clone(),
                            },
                        );
                        manifest
                    }
                    Err(err) => {
                        // A corrupt manifest must not hide the rest of the library.
                        eprintln!("[kudos] steam: skipping {}: {err}", path.display());
                        continue;
                    }
                },
            };

            if manifest.appid == 0 || manifest.appid == REDISTRIBUTABLE_APPID {
                continue;
            }
            if !seen_appids.insert(manifest.appid) {
                // The same app manifest in two libraries after a move/leftover.
                continue;
            }
            games.push(manifest.to_game());
        }
    }

    // Do not keep entries for manifests that were removed.
    cache
        .manifests
        .retain(|key, _| live_manifest_keys.contains(key));

    games.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
    games
}

fn cached_libraries(root: &Path, cache: &mut ScanCache) -> Vec<PathBuf> {
    let file = root.join("steamapps").join("libraryfolders.vdf");
    let stamp = fs::metadata(&file)
        .ok()
        .map(|metadata| FileStamp::from_metadata(&metadata));

    if let (Some(stamp), Some((cached_stamp, libraries))) =
        (stamp, cache.library_folders.as_ref())
    {
        if *cached_stamp == stamp {
            return libraries.clone();
        }
    }

    let mut libraries = vec![root.to_path_buf()];
    if let Ok(text) = read_text_file(&file) {
        match parse_vdf(&text) {
            Ok(vdf) => libraries.extend(library_paths(&vdf)),
            Err(err) => eprintln!("[kudos] steam: cannot parse {}: {err}", file.display()),
        }
    }
    dedup_libraries(&mut libraries);
    cache.library_folders = stamp.map(|stamp| (stamp, libraries.clone()));
    libraries
}

fn read_manifest(path: &Path) -> Result<AppManifest, String> {
    let text = read_text_file(path).map_err(|err| err.to_string())?;
    parse_manifest(&text, path)
}

fn parse_manifest(text: &str, path: &Path) -> Result<AppManifest, String> {
    let vdf = parse_vdf(text)?;
    let root = vdf
        .as_table()
        .ok_or_else(|| "manifest is not a table".to_string())?;
    let app_state = table_get(root, "AppState")
        .and_then(Vdf::as_table)
        .ok_or_else(|| "manifest has no AppState table".to_string())?;

    let appid = match table_get_str(app_state, "appid")
        .and_then(|value| value.trim().parse::<u32>().ok())
        .or_else(|| appid_from_manifest_name(path))
    {
        Some(appid) if appid > 0 => appid,
        _ => return Err("manifest has no usable appid".to_string()),
    };

    let name = table_get_str(app_state, "name")
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .ok_or_else(|| "manifest has no name".to_string())?
        .to_string();

    let state_flags = table_get_str(app_state, "StateFlags")
        .and_then(|value| value.trim().parse::<u32>().ok())
        .unwrap_or(STATE_FULLY_INSTALLED);
    let size_bytes = table_get_str(app_state, "SizeOnDisk")
        .and_then(|value| value.trim().parse::<u64>().ok())
        .unwrap_or(0);

    Ok(AppManifest {
        appid,
        name,
        size_bytes,
        state_flags,
    })
}

fn appid_from_manifest_name(path: &Path) -> Option<u32> {
    let name = path.file_name()?.to_str()?;
    name.strip_prefix("appmanifest_")?
        .strip_suffix(".acf")?
        .parse::<u32>()
        .ok()
}

fn dedup_libraries(libraries: &mut Vec<PathBuf>) {
    let mut seen = HashSet::new();
    libraries.retain(|library| {
        if library.as_os_str().is_empty() {
            return false;
        }
        seen.insert(path_key(library))
    });
}

fn path_key(path: &Path) -> String {
    path.to_string_lossy().to_ascii_lowercase()
}

fn read_text_file(path: &Path) -> std::io::Result<String> {
    let bytes = fs::read(path)?;
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(&bytes);
    Ok(String::from_utf8_lossy(bytes).into_owned())
}

// ---------------------------------------------------------------------------
// Steam installation / process detection
// ---------------------------------------------------------------------------

fn detect_steam_root() -> Option<PathBuf> {
    for (hive, key, value) in [
        (&HKCU, r"Software\Valve\Steam", "SteamPath"),
        (
            &HKLM,
            r"SOFTWARE\WOW6432Node\Valve\Steam",
            "InstallPath",
        ),
        (&HKLM, r"SOFTWARE\Valve\Steam", "InstallPath"),
    ] {
        if let Some(found) = registry_value(hive, key, value) {
            let path = PathBuf::from(found.trim().trim_matches('"').replace('/', "\\"));
            if steam_root_looks_valid(&path) {
                return Some(path);
            }
        }
    }

    // Last resort: the default install locations, even when the registry is odd.
    for candidate in [r"C:\Program Files (x86)\Steam", r"C:\Program Files\Steam"] {
        let path = PathBuf::from(candidate);
        if steam_root_looks_valid(&path) {
            return Some(path);
        }
    }
    None
}

fn steam_root_looks_valid(path: &Path) -> bool {
    if !path.is_dir() {
        return false;
    }
    path.join("steamapps").is_dir()
        || path.join("steam.exe").is_file()
        || path.join("Steam.exe").is_file()
}

fn registry_value(hive: &RegKey, key: &str, value: &str) -> Option<String> {
    let key = hive.open_subkey_with_flags(key, KEY_READ).ok()?;
    let value = key.get_value::<String, _>(value).ok()?;
    let value = value.trim().trim_matches('"');
    (!value.is_empty()).then(|| value.to_string())
}

fn is_steam_running() -> bool {
    let mut command = Command::new("tasklist");
    command.args(["/FI", "IMAGENAME eq steam.exe", "/NH", "/FO", "CSV"]);
    let Ok(output) = command_output(&mut command) else {
        return false;
    };
    String::from_utf8_lossy(&output.stdout)
        .to_ascii_lowercase()
        .lines()
        .any(|line| {
            let line = line.trim_start();
            line.starts_with("\"steam.exe\"") || line.starts_with("steam.exe")
        })
}

fn command_output(command: &mut Command) -> std::io::Result<std::process::Output> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command.output()
}

// ---------------------------------------------------------------------------
// Text VDF (libraryfolders.vdf, appmanifest .acf, loginusers.vdf)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum Vdf {
    Str(String),
    Table(Vec<(String, Vdf)>),
}

impl Vdf {
    fn as_str(&self) -> Option<&str> {
        match self {
            Vdf::Str(value) => Some(value),
            Vdf::Table(_) => None,
        }
    }

    fn as_table(&self) -> Option<&[(String, Vdf)]> {
        match self {
            Vdf::Table(entries) => Some(entries),
            Vdf::Str(_) => None,
        }
    }
}

fn table_get<'a>(table: &'a [(String, Vdf)], key: &str) -> Option<&'a Vdf> {
    table
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(key))
        .map(|(_, value)| value)
}

fn table_get_str<'a>(table: &'a [(String, Vdf)], key: &str) -> Option<&'a str> {
    table_get(table, key).and_then(Vdf::as_str)
}

fn parse_vdf(text: &str) -> Result<Vdf, String> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut parser = VdfParser {
        bytes: text.as_bytes(),
        pos: 0,
    };
    Ok(Vdf::Table(parser.parse_entries(true)?))
}

struct VdfParser<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> VdfParser<'a> {
    fn eof(&self) -> bool {
        self.pos >= self.bytes.len()
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn bump(&mut self) -> Option<u8> {
        let byte = self.peek()?;
        self.pos += 1;
        Some(byte)
    }

    fn skip_trivia(&mut self) {
        loop {
            while self.peek().is_some_and(|byte| byte.is_ascii_whitespace()) {
                self.pos += 1;
            }
            if self.peek() == Some(b'/') && self.bytes.get(self.pos + 1) == Some(&b'/') {
                while let Some(byte) = self.bump() {
                    if byte == b'\n' {
                        break;
                    }
                }
                continue;
            }
            break;
        }
    }

    fn parse_entries(&mut self, top_level: bool) -> Result<Vec<(String, Vdf)>, String> {
        let mut entries = Vec::new();
        loop {
            self.skip_trivia();
            if self.eof() {
                if top_level {
                    return Ok(entries);
                }
                return Err("unexpected end of file inside a table".to_string());
            }
            match self.peek() {
                Some(b'}') => {
                    self.pos += 1;
                    if top_level {
                        return Err("unexpected '}' at the top level".to_string());
                    }
                    return Ok(entries);
                }
                Some(b'[') => {
                    // Valve conditionals like [$WIN32]; irrelevant for our files.
                    while let Some(byte) = self.bump() {
                        if byte == b']' {
                            break;
                        }
                    }
                    continue;
                }
                _ => {}
            }

            let key = self.parse_token()?;
            self.skip_trivia();
            if self.peek() == Some(b'{') {
                self.pos += 1;
                let children = self.parse_entries(false)?;
                entries.push((key, Vdf::Table(children)));
            } else {
                let value = self.parse_token()?;
                entries.push((key, Vdf::Str(value)));
            }
        }
    }

    fn parse_token(&mut self) -> Result<String, String> {
        self.skip_trivia();
        match self.peek() {
            None => Err("unexpected end of file, expected a token".to_string()),
            Some(b'"') => self.parse_quoted(),
            Some(b'{') | Some(b'}') => Err("unexpected brace, expected a token".to_string()),
            Some(_) => {
                let start = self.pos;
                while let Some(byte) = self.peek() {
                    if byte.is_ascii_whitespace()
                        || byte == b'{'
                        || byte == b'}'
                        || byte == b'"'
                        || (byte == b'/' && self.bytes.get(self.pos + 1) == Some(&b'/'))
                    {
                        break;
                    }
                    self.pos += 1;
                }
                if self.pos == start {
                    return Err("empty token".to_string());
                }
                Ok(String::from_utf8_lossy(&self.bytes[start..self.pos]).into_owned())
            }
        }
    }

    fn parse_quoted(&mut self) -> Result<String, String> {
        debug_assert_eq!(self.peek(), Some(b'"'));
        self.pos += 1;
        let mut out = Vec::new();
        while let Some(byte) = self.bump() {
            match byte {
                b'"' => return Ok(String::from_utf8_lossy(&out).into_owned()),
                b'\\' => {
                    let Some(escaped) = self.bump() else {
                        return Err("unterminated escape sequence".to_string());
                    };
                    match escaped {
                        b'\\' => out.push(b'\\'),
                        b'"' => out.push(b'"'),
                        b'n' => out.push(b'\n'),
                        b't' => out.push(b'\t'),
                        b'r' => out.push(b'\r'),
                        other => {
                            // Keep unknown escapes byte-for-byte.
                            out.push(b'\\');
                            out.push(other);
                        }
                    }
                }
                other => out.push(other),
            }
        }
        Err("unterminated quoted string".to_string())
    }
}

/// Paths from both `libraryfolders.vdf` layouts:
/// - modern: `"0" { "path" "D:\\SteamLibrary" ... }`
/// - legacy: `"1" "D:\\SteamLibrary"`
fn library_paths(vdf: &Vdf) -> Vec<PathBuf> {
    let Some(root) = vdf.as_table() else {
        return Vec::new();
    };
    let mut out = Vec::new();

    let mut found_container = false;
    for (key, value) in root {
        if key.eq_ignore_ascii_case("libraryfolders") {
            if let Some(table) = value.as_table() {
                found_container = true;
                collect_library_entries(table, &mut out);
            }
        }
    }
    if !found_container {
        // Tolerate a caller passing the inner container or a differently cased root.
        for (_, value) in root {
            if let Some(table) = value.as_table() {
                collect_library_entries(table, &mut out);
            }
        }
    }
    if out.is_empty() {
        collect_library_entries(root, &mut out);
    }
    out
}

fn collect_library_entries(table: &[(String, Vdf)], out: &mut Vec<PathBuf>) {
    if let Some(path) = table_get_str(table, "path") {
        if !path.trim().is_empty() {
            out.push(PathBuf::from(path));
        }
    }
    for (key, value) in table {
        match value {
            Vdf::Str(path) if key.parse::<u32>().is_ok() && looks_like_path(path) => {
                out.push(PathBuf::from(path));
            }
            Vdf::Table(child) if key.parse::<u32>().is_ok() => {
                collect_library_entries(child, out);
            }
            _ => {}
        }
    }
}

fn looks_like_path(value: &str) -> bool {
    value.contains('\\') || value.contains('/') || value.as_bytes().get(1) == Some(&b':')
}

// ---------------------------------------------------------------------------
// Launch input resolution + status
// ---------------------------------------------------------------------------

/// Whether Steam is installed/running. Used by the Settings page.
pub fn steam_status() -> SteamStatus {
    let root = detect_steam_root();
    let installed = root
        .as_ref()
        .is_some_and(|path| steam_root_looks_valid(path));
    SteamStatus {
        installed,
        running: is_steam_running(),
        steam_path: root.map(|path| path.display().to_string()),
    }
}

// ---------------------------------------------------------------------------
// Active Steam user
// ---------------------------------------------------------------------------

fn active_userdata_dir(root: &Path) -> Option<PathBuf> {
    let userdata = root.join("userdata");

    if let Some((account_id, _)) = most_recent_login(root) {
        let candidate = userdata.join(account_id.to_string());
        if candidate.is_dir() {
            return Some(candidate);
        }
    }

    // Fallback for clients that do not write loginusers.vdf: the account whose
    // config was touched most recently (that is the profile Steam is using).
    let entries = fs::read_dir(&userdata).ok()?;
    let mut best: Option<(SystemTime, u32, PathBuf)> = None;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        let Ok(account_id) = name.parse::<u32>() else {
            continue;
        };
        if account_id == 0 {
            // userdata/0 is the anonymous/offline profile.
            continue;
        }
        let dir = entry.path();
        if !dir.is_dir() {
            continue;
        }
        let stamp = fs::metadata(dir.join("config").join("shortcuts.vdf"))
            .and_then(|metadata| metadata.modified())
            .or_else(|_| fs::metadata(&dir).and_then(|metadata| metadata.modified()))
            .unwrap_or(SystemTime::UNIX_EPOCH);
        let better = match &best {
            None => true,
            Some((best_stamp, best_id, _)) => {
                stamp > *best_stamp || (stamp == *best_stamp && account_id > *best_id)
            }
        };
        if better {
            best = Some((stamp, account_id, dir));
        }
    }
    best.map(|(_, _, dir)| dir)
}

fn most_recent_login(root: &Path) -> Option<(u32, u64)> {
    let text = read_text_file(&root.join("config").join("loginusers.vdf")).ok()?;
    let vdf = parse_vdf(&text).ok()?;
    let users = table_get(vdf.as_table()?, "users")?.as_table()?;

    let mut best: Option<(bool, u64, u32)> = None;
    for (steamid, value) in users {
        let Some(table) = value.as_table() else {
            continue;
        };
        let Ok(steamid64) = steamid.parse::<u64>() else {
            continue;
        };
        let Some(account_id) = steamid64_to_account_id(steamid64) else {
            continue;
        };
        let timestamp = table_get_str(table, "timestamp")
            .and_then(|value| value.trim().parse::<u64>().ok())
            .unwrap_or(0);
        let most_recent = table_get_str(table, "MostRecent")
            .map(|value| value.trim() == "1")
            .unwrap_or(false);

        let better = match best {
            None => true,
            Some((best_recent, best_timestamp, _)) => {
                (most_recent && !best_recent)
                    || (most_recent == best_recent && timestamp > best_timestamp)
            }
        };
        if better {
            best = Some((most_recent, timestamp, account_id));
        }
    }
    best.map(|(_, timestamp, account_id)| (account_id, timestamp))
}

fn steamid64_to_account_id(steamid64: u64) -> Option<u32> {
    let account_id = steamid64.checked_sub(STEAM_ID64_BASE)?;
    u32::try_from(account_id).ok()
}

// ---------------------------------------------------------------------------
// Add to Steam (non-Steam shortcut)
// ---------------------------------------------------------------------------

/// Add `game` to the Steam library as a non-Steam shortcut.
///
/// Works for any non-Steam game with a local executable (notably GOG, which is
/// the feature this method exists for). Steam must be closed: it rewrites
/// `shortcuts.vdf` from memory when it exits, which would discard our change.
pub fn add_to_steam_shortcut(game: &Game) -> SourceResult<()> {
    if game.platform == Platform::Steam {
        return Err(SourceError::invalid(format!(
            "'{}' is already a Steam game; no shortcut is needed",
            game.title
        )));
    }

    let root = detect_steam_root().ok_or_else(|| {
        SourceError::unavailable("Steam is not installed (SteamPath was not found in the registry)")
    })?;

    if is_steam_running() {
        return Err(SourceError::invalid(
            "Steam is running. Close Steam completely, then try again - Steam rewrites \
             shortcuts.vdf when it exits and would discard the new shortcut.",
        ));
    }

    let exe = resolve_shortcut_exe(game)?;
    let user_dir = active_userdata_dir(&root).ok_or_else(|| {
        SourceError::unavailable(
            "No signed-in Steam account found. Sign in to Steam once, close it, then try again.",
        )
    })?;
    let shortcuts_path = user_dir.join("config").join("shortcuts.vdf");

    add_shortcut_to_path(&shortcuts_path, game, &exe)?;
    eprintln!(
        "[kudos] Added '{}' to Steam. Restart Steam to see it in the library.",
        game.title
    );
    Ok(())
}

fn resolve_shortcut_exe(game: &Game) -> SourceResult<String> {
    let mut candidates = Vec::new();
    if let Some(exe) = game.exe_path.as_deref() {
        candidates.push(exe);
    }
    candidates.push(&game.launch_command);

    for candidate in candidates {
        let candidate = candidate.trim().trim_matches('"');
        if candidate.is_empty() || candidate.contains("://") {
            continue;
        }
        let path = Path::new(candidate);
        if !path.is_file() {
            continue;
        }
        let absolute = if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir()
                .map(|cwd| cwd.join(path))
                .unwrap_or_else(|_| path.to_path_buf())
        };
        return Ok(absolute.to_string_lossy().into_owned());
    }

    Err(SourceError::not_launchable(format!(
        "'{}' has no local .exe path, and Steam shortcuts require one. \
         Install the game or rescan it first.",
        game.title
    )))
}

fn add_shortcut_to_path(path: &Path, game: &Game, exe: &str) -> SourceResult<()> {
    let existing = if path.exists() {
        fs::read(path).map_err(|err| {
            SourceError::io(format!("could not read {}: {err}", path.display()))
        })?
    } else {
        Vec::new()
    };

    // Refuse to touch a file we do not understand. Replacing it would lose the
    // user's existing shortcuts.
    let mut root = if existing.is_empty() {
        Vec::new()
    } else {
        parse_binary_vdf(&existing).map_err(|err| {
            SourceError::invalid(format!(
                "{} is not a valid binary VDF file ({err}); refusing to overwrite it",
                path.display()
            ))
        })?
    };

    add_shortcut_to_tree(&mut root, game, exe);
    let bytes = write_binary_vdf(&root);

    // Extra safety: never write a file our own parser cannot read back.
    parse_binary_vdf(&bytes).map_err(|err| {
        SourceError::io(format!("generated shortcuts.vdf failed validation: {err}"))
    })?;

    let parent = path.parent().ok_or_else(|| {
        SourceError::invalid(format!("{} has no parent directory", path.display()))
    })?;
    fs::create_dir_all(parent).map_err(|err| {
        SourceError::io(format!("could not create {}: {err}", parent.display()))
    })?;

    let backup = with_suffix(path, ".kudos-backup");
    if path.exists() {
        fs::copy(path, &backup).map_err(|err| {
            SourceError::io(format!(
                "could not back up {} to {}: {err}",
                path.display(),
                backup.display()
            ))
        })?;
    }

    let temp = with_suffix(path, ".kudos-tmp");
    fs::write(&temp, &bytes).map_err(|err| {
        SourceError::io(format!("could not write {}: {err}", temp.display()))
    })?;
    if let Err(first_error) = fs::rename(&temp, path) {
        // Windows normally replaces the destination atomically, but be
        // defensive about filesystems that do not.
        let _ = fs::remove_file(path);
        fs::rename(&temp, path).map_err(|second_error| {
            let _ = fs::remove_file(&temp);
            SourceError::io(format!(
                "could not replace {} ({first_error}; retry: {second_error})",
                path.display()
            ))
        })?;
    }
    Ok(())
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut os = path.as_os_str().to_os_string();
    os.push(suffix);
    PathBuf::from(os)
}

fn add_shortcut_to_tree(root: &mut Vec<BEntry>, game: &Game, exe: &str) {
    let shortcuts = shortcuts_table_mut(root);
    let taken = shortcut_appids(shortcuts);
    let appid = generate_shortcut_appid(exe, &game.title, &taken);
    let exe_quoted = quote_exe(exe);
    let start_dir = start_dir_for(exe);

    if let Some(index) = shortcuts
        .iter()
        .position(|entry| shortcut_matches(entry, appid, &game.title, &exe_quoted))
    {
        // Re-adding an existing shortcut updates only the core fields. Icons,
        // launch options, tags and unknown fields are left exactly as they were.
        if let BValue::Table(fields) = &mut shortcuts[index].value {
            apply_shortcut_fields(fields, appid, &game.title, &exe_quoted, &start_dir);
        }
        return;
    }

    let key = next_shortcut_key(shortcuts);
    let fields = shortcut_fields(appid, &game.title, &exe_quoted, &start_dir)
        .into_iter()
        .map(|(key, value)| BEntry {
            key: key.into_bytes(),
            value,
        })
        .collect();
    shortcuts.push(BEntry {
        key: key.into_bytes(),
        value: BValue::Table(fields),
    });
}

fn shortcut_fields(
    appid: u32,
    title: &str,
    exe_quoted: &str,
    start_dir: &str,
) -> Vec<(String, BValue)> {
    vec![
        ("appid".to_string(), BValue::Int32(appid as i32)),
        (
            "AppName".to_string(),
            BValue::String(title.as_bytes().to_vec()),
        ),
        (
            "Exe".to_string(),
            BValue::String(exe_quoted.as_bytes().to_vec()),
        ),
        (
            "StartDir".to_string(),
            BValue::String(start_dir.as_bytes().to_vec()),
        ),
        ("icon".to_string(), BValue::String(Vec::new())),
        ("ShortcutPath".to_string(), BValue::String(Vec::new())),
        ("LaunchOptions".to_string(), BValue::String(Vec::new())),
        ("IsHidden".to_string(), BValue::Int32(0)),
        ("AllowDesktopConfig".to_string(), BValue::Int32(1)),
        ("AllowOverlay".to_string(), BValue::Int32(1)),
        ("OpenVR".to_string(), BValue::Int32(0)),
        ("Devkit".to_string(), BValue::Int32(0)),
        ("DevkitGameID".to_string(), BValue::String(Vec::new())),
        ("DevkitOverrideAppID".to_string(), BValue::Int32(0)),
        ("LastPlayTime".to_string(), BValue::Int32(0)),
        ("FlatpakAppID".to_string(), BValue::String(Vec::new())),
        ("tags".to_string(), BValue::Table(Vec::new())),
    ]
}

fn apply_shortcut_fields(
    fields: &mut Vec<BEntry>,
    appid: u32,
    title: &str,
    exe_quoted: &str,
    start_dir: &str,
) {
    set_string_field(fields, "AppName", title);
    set_string_field(fields, "Exe", exe_quoted);
    set_string_field(fields, "StartDir", start_dir);
    set_int_field(fields, "appid", appid as i32);

    // Add any standard field the entry is missing (older Steam formats are
    // shorter); never overwrite a value the user or Steam already set.
    for (key, value) in shortcut_fields(appid, title, exe_quoted, start_dir) {
        if btable_get(fields, &key).is_none() {
            fields.push(BEntry {
                key: key.into_bytes(),
                value,
            });
        }
    }
}

fn shortcut_matches(entry: &BEntry, appid: u32, title: &str, exe_quoted: &str) -> bool {
    let BValue::Table(fields) = &entry.value else {
        return false;
    };
    if shortcut_appid(fields) == Some(appid) {
        return true;
    }
    let same_name = btable_str(fields, "AppName")
        .map(|name| name.eq_ignore_ascii_case(title))
        .unwrap_or(false);
    let same_exe = btable_str(fields, "Exe")
        .map(|exe| {
            exe.trim_matches('"')
                .eq_ignore_ascii_case(exe_quoted.trim_matches('"'))
        })
        .unwrap_or(false);
    same_name && same_exe
}

fn shortcut_appids(table: &[BEntry]) -> HashSet<u32> {
    table
        .iter()
        .filter_map(|entry| {
            let BValue::Table(fields) = &entry.value else {
                return None;
            };
            shortcut_appid(fields)
        })
        .collect()
}

fn shortcut_appid(fields: &[BEntry]) -> Option<u32> {
    match btable_get(fields, "appid")? {
        // Standard format.
        BValue::Int32(value) => Some(*value as u32),
        // Some third-party tools store it as a string; accept that on read.
        BValue::String(bytes) => std::str::from_utf8(bytes)
            .ok()?
            .trim()
            .parse::<i64>()
            .ok()
            .map(|value| value as u32),
        _ => None,
    }
}

fn next_shortcut_key(table: &[BEntry]) -> String {
    let mut next = 0u32;
    for entry in table {
        if let Ok(index) = std::str::from_utf8(&entry.key)
            .unwrap_or("")
            .parse::<u32>()
        {
            next = next.max(index.saturating_add(1));
        }
    }
    loop {
        let candidate = next.to_string();
        if !table
            .iter()
            .any(|entry| entry.key.as_slice() == candidate.as_bytes())
        {
            return candidate;
        }
        next = next.saturating_add(1);
    }
}

fn shortcuts_table_mut(root: &mut Vec<BEntry>) -> &mut Vec<BEntry> {
    let index = match root
        .iter()
        .position(|entry| entry.key.eq_ignore_ascii_case(b"shortcuts"))
    {
        Some(index) => index,
        None => {
            root.push(BEntry {
                key: b"shortcuts".to_vec(),
                value: BValue::Table(Vec::new()),
            });
            root.len() - 1
        }
    };
    if !matches!(root[index].value, BValue::Table(_)) {
        root[index].value = BValue::Table(Vec::new());
    }
    match &mut root[index].value {
        BValue::Table(table) => table,
        BValue::String(_)
        | BValue::Int32(_)
        | BValue::Float32(_)
        | BValue::Pointer(_)
        | BValue::WideString(_)
        | BValue::Color(_)
        | BValue::UInt64(_)
        | BValue::Int64(_) => unreachable!("value was just replaced with a table"),
    }
}

fn quote_exe(exe: &str) -> String {
    format!(
        "\"{}\"",
        exe.trim().trim_matches('"').replace('/', "\\")
    )
}

fn start_dir_for(exe: &str) -> String {
    match Path::new(exe).parent() {
        Some(parent) if !parent.as_os_str().is_empty() => {
            let mut dir = parent.to_string_lossy().replace('/', "\\");
            if !dir.ends_with('\\') {
                dir.push('\\');
            }
            dir
        }
        _ => String::new(),
    }
}

fn generate_shortcut_appid(exe: &str, title: &str, taken: &HashSet<u32>) -> u32 {
    // Steam/Algolia-style deterministic id: CRC32 of the exe + name with the high
    // bit set. Deterministic means re-adding the same game is idempotent.
    let mut seed = String::with_capacity(exe.len() + title.len());
    seed.push_str(&exe.replace('/', "\\").to_ascii_lowercase());
    seed.push_str(title);

    let mut appid = crc32(seed.as_bytes()) | SHORTCUT_APPID_HIGH_BIT;
    while taken.contains(&appid) || appid == u32::MAX {
        appid = appid.wrapping_add(1);
        if appid < SHORTCUT_APPID_HIGH_BIT {
            appid = SHORTCUT_APPID_HIGH_BIT;
        }
    }
    appid
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &byte in bytes {
        crc ^= byte as u32;
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

// ---------------------------------------------------------------------------
// Binary VDF (shortcuts.vdf)
// ---------------------------------------------------------------------------

const BINARY_TABLE: u8 = 0x00;
const BINARY_STRING: u8 = 0x01;
const BINARY_INT32: u8 = 0x02;
const BINARY_FLOAT32: u8 = 0x03;
const BINARY_POINTER: u8 = 0x04;
const BINARY_WIDESTRING: u8 = 0x05;
const BINARY_COLOR: u8 = 0x06;
const BINARY_UINT64: u8 = 0x07;
const BINARY_END: u8 = 0x08;
const BINARY_INT64: u8 = 0x0A;

#[derive(Debug, Clone, PartialEq)]
enum BValue {
    String(Vec<u8>),
    Int32(i32),
    Float32(f32),
    Pointer(u32),
    WideString(Vec<u8>),
    Color(u32),
    UInt64(u64),
    Int64(i64),
    Table(Vec<BEntry>),
}

impl BValue {
    #[cfg(test)]
    fn as_table(&self) -> Option<&[BEntry]> {
        match self {
            BValue::Table(entries) => Some(entries),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct BEntry {
    key: Vec<u8>,
    value: BValue,
}

fn btable_get<'a>(table: &'a [BEntry], key: &str) -> Option<&'a BValue> {
    table
        .iter()
        .find(|entry| entry.key.eq_ignore_ascii_case(key.as_bytes()))
        .map(|entry| &entry.value)
}

fn btable_str<'a>(table: &'a [BEntry], key: &str) -> Option<&'a str> {
    match btable_get(table, key)? {
        BValue::String(bytes) => std::str::from_utf8(bytes).ok(),
        _ => None,
    }
}

#[cfg(test)]
fn btable_i32(table: &[BEntry], key: &str) -> Option<i32> {
    match btable_get(table, key)? {
        BValue::Int32(value) => Some(*value),
        BValue::String(bytes) => std::str::from_utf8(bytes).ok()?.trim().parse().ok(),
        _ => None,
    }
}

fn set_string_field(table: &mut Vec<BEntry>, key: &str, value: &str) {
    set_field(
        table,
        key,
        BValue::String(value.as_bytes().to_vec()),
    );
}

fn set_int_field(table: &mut Vec<BEntry>, key: &str, value: i32) {
    set_field(table, key, BValue::Int32(value));
}

fn set_field(table: &mut Vec<BEntry>, key: &str, value: BValue) {
    if let Some(entry) = table
        .iter_mut()
        .find(|entry| entry.key.eq_ignore_ascii_case(key.as_bytes()))
    {
        entry.value = value;
    } else {
        table.push(BEntry {
            key: key.as_bytes().to_vec(),
            value,
        });
    }
}

fn parse_binary_vdf(data: &[u8]) -> Result<Vec<BEntry>, String> {
    let mut parser = BinaryVdfParser { data, pos: 0 };
    let mut root = Vec::new();
    while !parser.eof() {
        let tag = parser.byte()?;
        if tag == BINARY_END {
            return Ok(root);
        }
        let key = parser.cstring()?;
        let value = parser.value(tag)?;
        root.push(BEntry { key, value });
    }
    Ok(root)
}

struct BinaryVdfParser<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> BinaryVdfParser<'a> {
    fn eof(&self) -> bool {
        self.pos >= self.data.len()
    }

    fn byte(&mut self) -> Result<u8, String> {
        let byte = self
            .data
            .get(self.pos)
            .copied()
            .ok_or_else(|| "unexpected end of binary VDF".to_string())?;
        self.pos += 1;
        Ok(byte)
    }

    fn take(&mut self, len: usize) -> Result<&'a [u8], String> {
        let end = self
            .pos
            .checked_add(len)
            .filter(|end| *end <= self.data.len())
            .ok_or_else(|| "truncated binary VDF".to_string())?;
        let bytes = &self.data[self.pos..end];
        self.pos = end;
        Ok(bytes)
    }

    fn cstring(&mut self) -> Result<Vec<u8>, String> {
        let start = self.pos;
        while self.pos < self.data.len() && self.data[self.pos] != 0 {
            self.pos += 1;
        }
        if self.pos >= self.data.len() {
            return Err("unterminated string in binary VDF".to_string());
        }
        let value = self.data[start..self.pos].to_vec();
        self.pos += 1; // NUL
        Ok(value)
    }

    fn u16(&mut self) -> Result<u16, String> {
        let bytes = self.take(2)?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    fn u32(&mut self) -> Result<u32, String> {
        let bytes = self.take(4)?;
        Ok(u32::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3],
        ]))
    }

    fn u64(&mut self) -> Result<u64, String> {
        let bytes = self.take(8)?;
        Ok(u64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]))
    }

    fn i64(&mut self) -> Result<i64, String> {
        Ok(self.u64()? as i64)
    }

    fn value(&mut self, tag: u8) -> Result<BValue, String> {
        match tag {
            BINARY_TABLE => {
                let mut entries = Vec::new();
                loop {
                    if self.eof() {
                        return Err("unterminated table in binary VDF".to_string());
                    }
                    let tag = self.byte()?;
                    if tag == BINARY_END {
                        break;
                    }
                    let key = self.cstring()?;
                    let value = self.value(tag)?;
                    entries.push(BEntry { key, value });
                }
                Ok(BValue::Table(entries))
            }
            BINARY_STRING => Ok(BValue::String(self.cstring()?)),
            BINARY_INT32 => Ok(BValue::Int32(self.u32()? as i32)),
            BINARY_FLOAT32 => Ok(BValue::Float32(f32::from_bits(self.u32()?))),
            BINARY_POINTER => Ok(BValue::Pointer(self.u32()?)),
            BINARY_WIDESTRING => {
                let mut bytes = Vec::new();
                loop {
                    let unit = self.u16()?;
                    if unit == 0 {
                        break;
                    }
                    bytes.extend_from_slice(&unit.to_le_bytes());
                }
                Ok(BValue::WideString(bytes))
            }
            BINARY_COLOR => Ok(BValue::Color(self.u32()?)),
            BINARY_UINT64 => Ok(BValue::UInt64(self.u64()?)),
            BINARY_INT64 => Ok(BValue::Int64(self.i64()?)),
            other => Err(format!("unsupported binary VDF type 0x{other:02x}")),
        }
    }
}

fn write_binary_vdf(root: &[BEntry]) -> Vec<u8> {
    let mut out = Vec::new();
    write_binary_entries(&mut out, root);
    out.push(BINARY_END);
    out
}

fn write_binary_entries(out: &mut Vec<u8>, entries: &[BEntry]) {
    for entry in entries {
        match &entry.value {
            BValue::Table(children) => {
                out.push(BINARY_TABLE);
                push_cstring(out, &entry.key);
                write_binary_entries(out, children);
                out.push(BINARY_END);
            }
            BValue::String(bytes) => {
                out.push(BINARY_STRING);
                push_cstring(out, &entry.key);
                out.extend_from_slice(bytes);
                out.push(0);
            }
            BValue::Int32(value) => {
                out.push(BINARY_INT32);
                push_cstring(out, &entry.key);
                out.extend_from_slice(&value.to_le_bytes());
            }
            BValue::Float32(value) => {
                out.push(BINARY_FLOAT32);
                push_cstring(out, &entry.key);
                out.extend_from_slice(&value.to_le_bytes());
            }
            BValue::Pointer(value) => {
                out.push(BINARY_POINTER);
                push_cstring(out, &entry.key);
                out.extend_from_slice(&value.to_le_bytes());
            }
            BValue::WideString(bytes) => {
                out.push(BINARY_WIDESTRING);
                push_cstring(out, &entry.key);
                out.extend_from_slice(bytes);
                out.extend_from_slice(&0u16.to_le_bytes());
            }
            BValue::Color(value) => {
                out.push(BINARY_COLOR);
                push_cstring(out, &entry.key);
                out.extend_from_slice(&value.to_le_bytes());
            }
            BValue::UInt64(value) => {
                out.push(BINARY_UINT64);
                push_cstring(out, &entry.key);
                out.extend_from_slice(&value.to_le_bytes());
            }
            BValue::Int64(value) => {
                out.push(BINARY_INT64);
                push_cstring(out, &entry.key);
                out.extend_from_slice(&value.to_le_bytes());
            }
        }
    }
}

fn push_cstring(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(bytes);
    out.push(0);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::Instant;

    fn fixture(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("steam")
            .join(name)
    }

    fn fixture_text(name: &str) -> String {
        read_text_file(&fixture(name)).expect("fixture should be readable")
    }

    struct TempDir {
        path: PathBuf,
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn temp_dir(tag: &str) -> TempDir {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let unique = NEXT.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "kudos-steam-{}-{}-{}",
            std::process::id(),
            unique,
            tag
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("temp dir");
        TempDir { path }
    }

    fn dummy_exe(dir: &Path, name: &str) -> PathBuf {
        let path = dir.join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("exe dir");
        }
        fs::write(&path, b"MZ").expect("dummy exe");
        path
    }

    fn gog_game(exe: &Path) -> Game {
        let mut game = Game::new(
            "gog:1207658924",
            Platform::Gog,
            "A GOG Game",
            exe.to_string_lossy().into_owned(),
        );
        game.exe_path = Some(exe.to_string_lossy().into_owned());
        game
    }

    fn shortcuts_of<'a>(root: &'a [BEntry]) -> &'a [BEntry] {
        btable_get(root, "shortcuts")
            .and_then(BValue::as_table)
            .expect("shortcuts table")
    }

    fn shortcut_at<'a>(root: &'a [BEntry], key: &str) -> &'a [BEntry] {
        shortcuts_of(root)
            .iter()
            .find(|entry| entry.key == key.as_bytes())
            .and_then(|entry| entry.value.as_table())
            .expect("shortcut entry")
    }

    #[test]
    fn parses_modern_libraryfolders() {
        let vdf = parse_vdf(&fixture_text("libraryfolders-modern.vdf")).unwrap();
        let paths = library_paths(&vdf);
        assert_eq!(
            paths,
            vec![
                PathBuf::from(r"C:\Program Files (x86)\Steam"),
                PathBuf::from(r"D:\SteamLibrary"),
                PathBuf::from(r"E:\Games\Steam Library"),
            ]
        );
    }

    #[test]
    fn parses_legacy_libraryfolders_and_ignores_stats() {
        let vdf = parse_vdf(&fixture_text("libraryfolders-legacy.vdf")).unwrap();
        let paths = library_paths(&vdf);
        assert_eq!(
            paths,
            vec![
                PathBuf::from(r"D:\SteamLibrary"),
                PathBuf::from(r"E:\Games\Steam Library"),
            ]
        );
    }

    #[test]
    fn parses_installed_appmanifest() {
        let manifest = parse_manifest(
            &fixture_text("appmanifest-730.acf"),
            Path::new("appmanifest_730.acf"),
        )
        .unwrap();

        assert_eq!(manifest.appid, 730);
        assert_eq!(manifest.name, "Counter-Strike 2");
        assert_eq!(manifest.size_bytes, 73_942_306_339);
        assert!(manifest.is_installed());

        let game = manifest.to_game();
        assert_eq!(game.id, "steam:730");
        assert_eq!(game.platform, Platform::Steam);
        assert_eq!(game.title, "Counter-Strike 2");
        assert_eq!(game.launch_command, "steam://rungameid/730");
        assert_eq!(game.size_bytes, Some(73_942_306_339));
        assert!(game.is_installed);
        assert_eq!(
            game.cover_image.as_deref(),
            Some("https://cdn.cloudflare.steamstatic.com/steam/apps/730/library_600x900.jpg")
        );
    }

    #[test]
    fn keeps_not_downloaded_games_with_launch_uri() {
        let manifest = parse_manifest(
            &fixture_text("appmanifest-not-downloaded.acf"),
            Path::new("appmanifest_999999.acf"),
        )
        .unwrap();

        assert_eq!(manifest.appid, 999_999);
        assert!(!manifest.is_installed());
        assert_eq!(manifest.size_bytes, 0);

        let game = manifest.to_game();
        assert!(!game.is_installed);
        assert_eq!(game.size_bytes, None);
        assert_eq!(game.launch_command, "steam://rungameid/999999");
        assert!(game.cover_image.is_some());
    }

    #[test]
    fn appid_falls_back_to_manifest_filename() {
        let text = r#"
            "AppState"
            {
                "name" "No Appid Here"
                "StateFlags" "4"
            }
        "#;
        let manifest = parse_manifest(text, Path::new("appmanifest_424242.acf")).unwrap();
        assert_eq!(manifest.appid, 424_242);
        assert_eq!(manifest.name, "No Appid Here");
    }

    #[test]
    fn scans_a_library_and_skips_redistributables() {
        let dir = temp_dir("scan");
        let steamapps = dir.path.join("steamapps");
        fs::create_dir_all(&steamapps).unwrap();
        for (fixture_name, file_name) in [
            ("appmanifest-730.acf", "appmanifest_730.acf"),
            (
                "appmanifest-not-downloaded.acf",
                "appmanifest_999999.acf",
            ),
            (
                "appmanifest-redistributable.acf",
                "appmanifest_228980.acf",
            ),
        ] {
            fs::copy(fixture(fixture_name), steamapps.join(file_name)).unwrap();
        }

        let mut cache = ScanCache::default();
        let first = scan_libraries(&dir.path, &mut cache);
        assert_eq!(first.len(), 2, "redistributables must be hidden");
        assert!(first.iter().all(|game| game.id != "steam:228980"));
        assert_eq!(first.iter().filter(|game| game.is_installed).count(), 1);
        assert!(first
            .iter()
            .any(|game| game.id == "steam:999999" && !game.is_installed));

        // Second pass uses the manifest cache and must agree with the first.
        let second = scan_libraries(&dir.path, &mut cache);
        assert_eq!(second.len(), 2);
        assert_eq!(cache.manifests.len(), 3);

        // A changed manifest (different length and mtime) must be reparsed.
        let changed_path = steamapps.join("appmanifest_999999.acf");
        let mut changed = fs::read_to_string(&changed_path).unwrap();
        changed = changed.replace("Queued Indie Game", "Queued Indie Game Renamed");
        changed.push_str("\n// cache-busting padding\n");
        fs::write(&changed_path, changed).unwrap();

        let third = scan_libraries(&dir.path, &mut cache);
        let renamed = third
            .iter()
            .find(|game| game.id == "steam:999999")
            .expect("renamed game");
        assert_eq!(renamed.title, "Queued Indie Game Renamed");
    }

    #[test]
    fn empty_shortcuts_fixture_round_trips_exactly() {
        let original = fs::read(fixture("shortcuts-empty.vdf")).unwrap();
        assert_eq!(original.len(), 13);
        let root = parse_binary_vdf(&original).unwrap();
        assert!(shortcuts_of(&root).is_empty());
        assert_eq!(write_binary_vdf(&root), original);
    }

    #[test]
    fn existing_shortcuts_fixture_round_trips_exactly() {
        let original = fs::read(fixture("shortcuts-existing.vdf")).unwrap();
        let root = parse_binary_vdf(&original).unwrap();

        let shortcuts = shortcuts_of(&root);
        assert_eq!(shortcuts.len(), 2);
        assert_eq!(
            btable_str(shortcut_at(&root, "0"), "AppName"),
            Some("Old Game")
        );
        assert_eq!(
            btable_str(shortcut_at(&root, "0"), "FutureField"),
            Some("kept-intact")
        );
        assert_eq!(write_binary_vdf(&root), original);
    }

    #[test]
    fn adding_a_shortcut_preserves_existing_entries_and_unknown_fields() {
        let dir = temp_dir("preserve");
        let exe = dummy_exe(&dir.path.join("new"), "new.exe");
        let game = gog_game(&exe);

        let original = fs::read(fixture("shortcuts-existing.vdf")).unwrap();
        let mut root = parse_binary_vdf(&original).unwrap();
        add_shortcut_to_tree(&mut root, &game, &game.exe_path.clone().unwrap());

        let bytes = write_binary_vdf(&root);
        let parsed = parse_binary_vdf(&bytes).unwrap();
        assert_eq!(shortcuts_of(&parsed).len(), 3);

        // Existing entry, including fields Kudos does not know, is untouched.
        let old = shortcut_at(&parsed, "0");
        assert_eq!(btable_str(old, "AppName"), Some("Old Game"));
        assert_eq!(btable_str(old, "LaunchOptions"), Some("-windowed"));
        assert_eq!(btable_str(old, "FutureField"), Some("kept-intact"));
        assert_eq!(btable_i32(old, "FutureFlag"), Some(7));
        assert!(matches!(btable_get(old, "tags"), Some(BValue::Table(tags)) if tags.is_empty()));

        // Second entry is untouched too.
        assert_eq!(
            btable_str(shortcut_at(&parsed, "1"), "AppName"),
            Some("Second Game")
        );

        // New entry uses the next index and Steam's normal field set.
        let new = shortcut_at(&parsed, "2");
        assert_eq!(btable_str(new, "AppName"), Some("A GOG Game"));
        assert_eq!(
            btable_str(new, "Exe"),
            Some(format!("\"{}\"", exe.display()).as_str())
        );
        let start_dir = btable_str(new, "StartDir").unwrap();
        assert!(start_dir.ends_with('\\'));
        assert_eq!(btable_i32(new, "AllowDesktopConfig"), Some(1));
        assert_eq!(btable_i32(new, "AllowOverlay"), Some(1));
        assert_eq!(btable_i32(new, "IsHidden"), Some(0));
        assert!(matches!(btable_get(new, "tags"), Some(BValue::Table(tags)) if tags.is_empty()));

        let appid = shortcut_appid(new).unwrap();
        assert!(appid >= SHORTCUT_APPID_HIGH_BIT);

        // Re-adding the same game is idempotent.
        add_shortcut_to_tree(&mut root, &game, &game.exe_path.clone().unwrap());
        let reparsed = parse_binary_vdf(&write_binary_vdf(&root)).unwrap();
        assert_eq!(shortcuts_of(&reparsed).len(), 3);
    }

    #[test]
    fn shortcut_appids_are_high_bit_and_unique() {
        let exe = r"C:\Games\Foo\foo.exe";
        let title = "Foo";
        let first = generate_shortcut_appid(exe, title, &HashSet::new());
        assert!(first >= SHORTCUT_APPID_HIGH_BIT);
        assert_eq!(first, generate_shortcut_appid(exe, title, &HashSet::new()));

        let mut taken = HashSet::new();
        taken.insert(first);
        let second = generate_shortcut_appid(exe, title, &taken);
        assert_ne!(first, second);
        assert!(second >= SHORTCUT_APPID_HIGH_BIT);
    }

    #[test]
    fn crc32_matches_known_vector() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }

    #[test]
    fn loginusers_picks_the_most_recent_account() {
        let dir = temp_dir("loginusers");
        let config = dir.path.join("config");
        fs::create_dir_all(&config).unwrap();
        fs::copy(fixture("loginusers.vdf"), config.join("loginusers.vdf")).unwrap();

        let userdata = dir.path.join("userdata");
        for account in [1_846_526_608u32, 756_940_061] {
            fs::create_dir_all(userdata.join(account.to_string()).join("config")).unwrap();
        }

        let (account, timestamp) = most_recent_login(&dir.path).unwrap();
        assert_eq!(account, 1_846_526_608);
        assert_eq!(timestamp, 1_793_107_333);
        assert_eq!(
            active_userdata_dir(&dir.path),
            Some(userdata.join("1846526608"))
        );
    }

    #[test]
    fn writing_creates_backs_up_and_is_idempotent() {
        let dir = temp_dir("write");
        let exe = dummy_exe(&dir.path.join("games"), "game.exe");
        let game = gog_game(&exe);
        let path = dir
            .path
            .join("userdata")
            .join("42")
            .join("config")
            .join("shortcuts.vdf");
        let backup = with_suffix(&path, ".kudos-backup");

        add_shortcut_to_path(&path, &game, game.exe_path.as_deref().unwrap()).unwrap();
        assert!(path.is_file());
        assert!(!backup.exists(), "nothing existed to back up yet");

        let root = parse_binary_vdf(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(shortcuts_of(&root).len(), 1);

        // Re-adding the same GOG game updates in place and leaves a backup.
        add_shortcut_to_path(&path, &game, game.exe_path.as_deref().unwrap()).unwrap();
        assert!(backup.is_file());
        let root = parse_binary_vdf(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(shortcuts_of(&root).len(), 1);

        // A different game appends a second entry.
        let other_exe = dummy_exe(&dir.path.join("games"), "other.exe");
        let mut other = gog_game(&other_exe);
        other.title = "Another GOG Game".to_string();
        add_shortcut_to_path(&path, &other, other.exe_path.as_deref().unwrap()).unwrap();
        let root = parse_binary_vdf(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(shortcuts_of(&root).len(), 2);
    }

    #[test]
    fn refuses_to_overwrite_an_unparseable_shortcuts_file() {
        let dir = temp_dir("corrupt");
        let exe = dummy_exe(&dir.path, "game.exe");
        let game = gog_game(&exe);
        let path = dir.path.join("config").join("shortcuts.vdf");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, b"this is not a binary vdf").unwrap();
        let before = fs::read(&path).unwrap();

        let error = add_shortcut_to_path(&path, &game, game.exe_path.as_deref().unwrap())
            .expect_err("must refuse to overwrite");
        assert!(matches!(error, SourceError::Invalid(_)));
        assert_eq!(fs::read(&path).unwrap(), before);
    }

    #[test]
    fn refuses_to_add_while_steam_is_running() {
        if !is_steam_running() {
            eprintln!("[kudos] running-refusal test skipped: Steam is not running");
            return;
        }

        // The running check happens before any path resolution or file write, so
        // this can never touch the real shortcuts.vdf.
        let game = Game::new(
            "gog:1",
            Platform::Gog,
            "Test Game",
            r"C:\does-not-exist\test.exe",
        );
        let error = add_to_steam_shortcut(&game).expect_err("must refuse while Steam runs");
        assert!(matches!(error, SourceError::Invalid(_)));
        assert!(
            error.to_string().contains("Close Steam"),
            "error should tell the user what to do: {error}"
        );
    }

    #[test]
    fn real_steam_scan_smoke() {
        let Some(root) = detect_steam_root() else {
            eprintln!("[kudos] steam smoke test skipped: Steam not installed");
            return;
        };

        let source = SteamSource::new();
        let start = Instant::now();
        let first = source.installed().expect("scan should succeed");
        let cold = start.elapsed();
        let start = Instant::now();
        let second = source.installed().expect("second scan should succeed");
        let warm = start.elapsed();

        assert_eq!(first.len(), second.len());
        for game in &first {
            assert!(game.id.starts_with("steam:"));
            assert!(game.launch_command.starts_with("steam://rungameid/"));
            assert!(game.cover_image.is_some());
        }
        eprintln!(
            "[kudos] steam scan: {} games from {}; cold {:?}, warm {:?}",
            first.len(),
            root.display(),
            cold,
            warm
        );
    }

    #[test]
    fn steam_status_smoke() {
        let status = steam_status();
        if status.installed {
            assert!(status.steam_path.is_some());
        }
        eprintln!(
            "[kudos] steam status: installed={} running={} path={:?}",
            status.installed,
            status.running,
            status.steam_path
        );
    }
}
