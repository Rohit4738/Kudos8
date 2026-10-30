//! SteamGridDB artwork support (Session M).
//!
//! Responsibilities:
//! - typed art kinds (`grid`, `hero`, `logo`, `icon`) shared by the commands,
//!   the cache and Steam's grid writer;
//! - a disk cache at `%APPDATA%\Kudos\art\<sanitized-game-id>-<kind>.<ext>`;
//! - a strict HTTPS downloader for SteamGridDB's own CDNs (8 MB cap, image
//!   content-type + magic-byte validation, one retry, per-session budget);
//! - `search_art` / `apply_art` / `get_art_status` helpers used by `commands.rs`.
//!
//! No plaintext API key is ever logged or returned. The key is read from
//! `secrets.rs` only inside the SteamGridDB client.

pub mod steamgriddb;

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{LazyLock, Mutex};

use serde::{Deserialize, Serialize};

use crate::models::{Game, Platform};

/// Hard limit for one downloaded image. SteamGridDB's largest assets are well
/// under this; anything bigger is a red flag.
pub const MAX_IMAGE_BYTES: usize = 8 * 1024 * 1024;
/// Per-process request budget. A full picker session (search + one art list +
/// a few applies) stays far below this, while a runaway caller cannot hammer the
/// API and burn the key's rate limit.
pub const SESSION_REQUEST_BUDGET: u32 = 60;

// ---------------------------------------------------------------------------
// Types (the frontend contract)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ArtKind {
    Grid,
    Hero,
    Logo,
    Icon,
}

impl ArtKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ArtKind::Grid => "grid",
            ArtKind::Hero => "hero",
            ArtKind::Logo => "logo",
            ArtKind::Icon => "icon",
        }
    }

    pub fn parse(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "grid" => Ok(ArtKind::Grid),
            "hero" => Ok(ArtKind::Hero),
            "logo" => Ok(ArtKind::Logo),
            "icon" => Ok(ArtKind::Icon),
            other => Err(format!(
                "Unknown art kind \"{other}\". Expected grid, hero, logo or icon."
            )),
        }
    }
}

/// One SteamGridDB candidate. `id` is the SteamGridDB asset id (string so the
/// frontend never has to deal with 64-bit JSON numbers).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtResult {
    pub id: String,
    pub kind: ArtKind,
    pub url: String,
    pub thumb_url: String,
    pub width: u32,
    pub height: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
}

/// Result of applying one candidate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyArtResult {
    /// Absolute path of the cached local override.
    pub local_path: String,
    /// True when the file was copied into Steam's `config/grid` folder.
    pub wrote_to_steam: bool,
    /// Populated when the local override succeeded but Steam could not be
    /// updated (no shortcut, Steam running, unreadable shortcuts.vdf, ...).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub steam_error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ArtSource {
    Steamgriddb,
    Local,
    None,
}

/// Current slots for one game. Missing slots are omitted on the wire.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtStatus {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hero: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logo: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    pub source: ArtSource,
    /// Last quiet auto-art failure (the picker itself reports its own errors).
    /// Never shown as a modal: the Settings card and detail page can mention it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

// ---------------------------------------------------------------------------
// Request budget
// ---------------------------------------------------------------------------

pub struct RequestBudget {
    used: AtomicU32,
    limit: u32,
}

impl RequestBudget {
    pub const fn new(limit: u32) -> Self {
        Self {
            used: AtomicU32::new(0),
            limit,
        }
    }

    /// Consume one request, or fail without consuming when the budget is gone.
    pub fn try_consume(&self) -> Result<(), String> {
        let mut current = self.used.load(Ordering::Relaxed);
        loop {
            if current >= self.limit {
                return Err(format!(
                    "SteamGridDB request budget reached for this session ({}). \
                     Restart Kudos to reset it.",
                    self.limit
                ));
            }
            match self.used.compare_exchange_weak(
                current,
                current + 1,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => return Ok(()),
                Err(actual) => current = actual,
            }
        }
    }

    pub fn remaining(&self) -> u32 {
        self.limit.saturating_sub(self.used.load(Ordering::Relaxed))
    }
}

pub static BUDGET: RequestBudget = RequestBudget::new(SESSION_REQUEST_BUDGET);

// ---------------------------------------------------------------------------
// Cache paths
// ---------------------------------------------------------------------------

/// `%APPDATA%\Kudos\art`.
pub fn art_dir() -> Option<PathBuf> {
    crate::secrets::data_dir().map(|dir| dir.join("art"))
}

/// Windows-safe file stem for a namespaced game id. `gog:1207658924` becomes
/// `gog-1207658924`; characters outside `[A-Za-z0-9._-]` become `-`.
pub fn sanitize_game_id(game_id: &str) -> String {
    let mut out = String::with_capacity(game_id.len());
    for ch in game_id.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
            out.push(ch);
        } else {
            out.push('-');
        }
    }
    let trimmed = out.trim_matches(['-', '.', ' ']).to_string();
    if trimmed.is_empty() {
        "game".to_string()
    } else if trimmed.len() > 80 {
        trimmed[..80].to_string()
    } else {
        trimmed
    }
}

pub fn cache_stem(game_id: &str, kind: ArtKind) -> String {
    format!("{}-{}", sanitize_game_id(game_id), kind.as_str())
}

pub fn cache_path(dir: &Path, game_id: &str, kind: ArtKind, extension: &str) -> PathBuf {
    dir.join(format!(
        "{}.{}",
        cache_stem(game_id, kind),
        extension.trim_start_matches('.')
    ))
}

/// First cached extension for a slot. JPEG is preferred, then PNG.
pub fn cached_path(dir: &Path, game_id: &str, kind: ArtKind) -> Option<PathBuf> {
    ["jpg", "png"]
        .into_iter()
        .map(|extension| cache_path(dir, game_id, kind, extension))
        .find(|path| path.is_file())
}

/// Portrait cover override used by `list_games`.
pub fn override_cover(game_id: &str) -> Option<PathBuf> {
    let dir = art_dir()?;
    cached_path(&dir, game_id, ArtKind::Grid)
}

fn save_image(
    dir: &Path,
    game_id: &str,
    kind: ArtKind,
    bytes: &[u8],
    extension: &str,
) -> Result<PathBuf, String> {
    fs::create_dir_all(dir)
        .map_err(|err| format!("Could not create {}: {err}", dir.display()))?;
    let destination = cache_path(dir, game_id, kind, extension);
    let temp = with_suffix(&destination, ".kudos-tmp");
    fs::write(&temp, bytes).map_err(|err| format!("Could not write {}: {err}", temp.display()))?;
    if let Err(first_error) = fs::rename(&temp, &destination) {
        let _ = fs::remove_file(&destination);
        fs::rename(&temp, &destination).map_err(|second_error| {
            let _ = fs::remove_file(&temp);
            format!(
                "Could not replace {} ({first_error}; retry: {second_error})",
                destination.display()
            )
        })?;
    }
    Ok(destination)
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut os = path.as_os_str().to_os_string();
    os.push(suffix);
    PathBuf::from(os)
}

// ---------------------------------------------------------------------------
// URL + image validation
// ---------------------------------------------------------------------------

/// SteamGridDB serves art from `steamgriddb.com` and its subdomains. Restricting
/// the downloader to those hosts means `apply_art` cannot be turned into a local
/// SSRF probe by a compromised frontend.
pub fn split_art_url(url: &str) -> Result<(String, String), String> {
    let rest = url
        .trim()
        .strip_prefix("https://")
        .ok_or_else(|| "Artwork URLs must use HTTPS.".to_string())?;
    let (host, path) = match rest.find('/') {
        Some(index) => (&rest[..index], &rest[index..]),
        None => (rest, "/"),
    };
    if host.is_empty() || host.contains('@') || host.contains(':') {
        return Err("Artwork URL has an invalid host.".to_string());
    }
    let host = host.to_ascii_lowercase();
    if !is_steamgriddb_host(&host) {
        return Err(format!(
            "Refusing to download artwork from \"{host}\"; only SteamGridDB hosts are allowed."
        ));
    }
    Ok((host, path.to_string()))
}

fn is_steamgriddb_host(host: &str) -> bool {
    host == "steamgriddb.com" || host.ends_with(".steamgriddb.com")
}

/// Validate both the declared content type and the file signature. Returns the
/// canonical extension (`jpg` or `png`).
pub fn image_extension(content_type: &str, bytes: &[u8]) -> Result<&'static str, String> {
    let declared = content_type
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    let by_type = match declared.as_str() {
        "image/jpeg" | "image/jpg" => "jpg",
        "image/png" => "png",
        other => {
            return Err(format!(
                "SteamGridDB returned \"{}\" instead of a JPEG or PNG image.",
                if other.is_empty() { "no content type" } else { other }
            ))
        }
    };
    let by_magic = if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("jpg")
    } else if bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]) {
        Some("png")
    } else {
        None
    };
    match by_magic {
        Some(magic) if magic == by_type => Ok(by_type),
        Some(_) => Err("The image data does not match its declared content type.".to_string()),
        None => Err("The downloaded file is not a valid JPEG or PNG image.".to_string()),
    }
}

// ---------------------------------------------------------------------------
// Search / apply / status
// ---------------------------------------------------------------------------

pub fn search_for_game(game: &Game, kind: ArtKind) -> Result<Vec<ArtResult>, String> {
    steamgriddb::search(&game.title, kind, &BUDGET)
}

pub fn apply_for_game(
    game: &Game,
    url: &str,
    kind: ArtKind,
    replace: bool,
) -> Result<ApplyArtResult, String> {
    let dir = art_dir().ok_or_else(|| {
        "Kudos could not find %APPDATA%. Artwork cannot be cached.".to_string()
    })?;

    let (host, path) = split_art_url(url)?;
    let response = get_with_retry(&host, &path, None, MAX_IMAGE_BYTES, &BUDGET)
        .map_err(|err| format!("Could not download artwork: {err}"))?;
    let extension = image_extension(
        response.content_type.as_deref().unwrap_or_default(),
        &response.body,
    )?;
    let local_path = save_image(&dir, &game.id, kind, &response.body, extension)?;

    let mut wrote_to_steam = false;
    let mut steam_error = None;
    if game.platform != Platform::Steam {
        match crate::steam_grid::apply_for_game(game, kind, &local_path, replace) {
            Ok(write) => wrote_to_steam = write.wrote,
            Err(err) => steam_error = Some(err),
        }
    }

    Ok(ApplyArtResult {
        local_path: local_path.to_string_lossy().into_owned(),
        wrote_to_steam,
        steam_error,
    })
}

pub fn status_for_game(game: &Game) -> ArtStatus {
    let dir = art_dir();
    let slot = |kind: ArtKind| -> Option<String> {
        dir.as_deref()
            .and_then(|dir| cached_path(dir, &game.id, kind))
            .map(|path| path.to_string_lossy().into_owned())
    };

    let grid = slot(ArtKind::Grid);
    let hero = slot(ArtKind::Hero);
    let logo = slot(ArtKind::Logo);
    let icon = slot(ArtKind::Icon);
    let source = if grid.is_some() || hero.is_some() || logo.is_some() || icon.is_some() {
        ArtSource::Steamgriddb
    } else if game.cover_image.is_some() {
        ArtSource::Local
    } else {
        ArtSource::None
    };

    ArtStatus {
        grid,
        hero,
        logo,
        icon,
        source,
        error: auto_error(&game.id),
    }
}

// ---------------------------------------------------------------------------
// Automatic artwork (after "Add to Steam")
// ---------------------------------------------------------------------------

/// Quiet auto-art failures, keyed by game id. Kept in memory: a restart is a
/// clean slate, and the user can always retry from the art picker.
static AUTO_ERRORS: LazyLock<Mutex<HashMap<String, String>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn auto_errors() -> std::sync::MutexGuard<'static, HashMap<String, String>> {
    AUTO_ERRORS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

pub fn record_auto_error(game_id: &str, error: &str) {
    auto_errors().insert(game_id.to_string(), error.to_string());
}

pub fn clear_auto_error(game_id: &str) {
    auto_errors().remove(game_id);
}

pub fn auto_error(game_id: &str) -> Option<String> {
    auto_errors().get(game_id).cloned()
}

/// Best-effort artwork for a freshly added non-Steam shortcut.
///
/// Runs only when a SteamGridDB key is saved. Grid, hero and logo are fetched
/// in that order and written with `replace == false`, so art the user already
/// chose is never overwritten. Failures are recorded for `get_art_status`
/// instead of surfacing a modal: adding a shortcut still succeeded.
pub fn auto_art_for_game(game: &Game) -> Result<(), String> {
    if game.platform == Platform::Steam {
        return Ok(());
    }
    if crate::secrets::get(crate::secrets::PROVIDER_STEAMGRIDDB).is_none() {
        return Ok(());
    }

    let mut errors: Vec<String> = Vec::new();
    for kind in [ArtKind::Grid, ArtKind::Hero, ArtKind::Logo] {
        let candidates = match search_for_game(game, kind) {
            Ok(candidates) => candidates,
            Err(err) => {
                errors.push(format!("{}: {err}", kind.as_str()));
                continue;
            }
        };
        let chosen = candidates.iter().find(|candidate| match kind {
            // Steam only accepts PNG logos.
            ArtKind::Logo => candidate.url.to_ascii_lowercase().ends_with(".png"),
            _ => true,
        });
        let Some(candidate) = chosen else {
            continue;
        };
        match apply_for_game(game, &candidate.url, kind, false) {
            Ok(result) => {
                if let Some(err) = result.steam_error {
                    errors.push(format!("{}: {err}", kind.as_str()));
                }
            }
            Err(err) => errors.push(format!("{}: {err}", kind.as_str())),
        }
    }

    if errors.is_empty() {
        clear_auto_error(&game.id);
        Ok(())
    } else {
        let message = errors.join("; ");
        record_auto_error(&game.id, &message);
        Err(message)
    }
}

// ---------------------------------------------------------------------------
// Minimal blocking HTTPS client (WinHTTP, no extra crate)
// ---------------------------------------------------------------------------

pub(crate) struct HttpResponse {
    pub content_type: Option<String>,
    pub body: Vec<u8>,
}

#[cfg(windows)]
mod http {
    use std::ffi::c_void;
    use std::ptr::null;

    use super::{HttpResponse, RequestBudget};

    type InternetHandle = *mut c_void;

    const WINHTTP_ACCESS_TYPE_DEFAULT_PROXY: u32 = 0;
    const WINHTTP_FLAG_SECURE: u32 = 0x0080_0000;
    const WINHTTP_QUERY_CONTENT_TYPE: u32 = 1;
    const WINHTTP_QUERY_CONTENT_LENGTH: u32 = 5;
    const WINHTTP_QUERY_STATUS_CODE: u32 = 19;
    const WINHTTP_QUERY_FLAG_NUMBER: u32 = 0x2000_0000;

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

    fn header_string(request: InternetHandle, level: u32) -> Option<String> {
        let mut length = 0u32;
        unsafe {
            WinHttpQueryHeaders(
                request,
                level,
                null(),
                std::ptr::null_mut(),
                &mut length,
                std::ptr::null_mut(),
            );
        }
        if length == 0 {
            return None;
        }
        let mut buffer = vec![0u16; (length as usize / 2) + 1];
        let ok = unsafe {
            WinHttpQueryHeaders(
                request,
                level,
                null(),
                buffer.as_mut_ptr().cast(),
                &mut length,
                std::ptr::null_mut(),
            )
        };
        if ok == 0 {
            return None;
        }
        let units = (length as usize / 2).saturating_sub(1);
        Some(String::from_utf16_lossy(&buffer[..units]))
    }

    fn content_length(request: InternetHandle) -> Option<usize> {
        let mut length = 0u32;
        let mut size = std::mem::size_of::<u32>() as u32;
        let ok = unsafe {
            WinHttpQueryHeaders(
                request,
                WINHTTP_QUERY_CONTENT_LENGTH | WINHTTP_QUERY_FLAG_NUMBER,
                null(),
                (&mut length as *mut u32).cast(),
                &mut size,
                std::ptr::null_mut(),
            )
        };
        (ok != 0).then_some(length as usize)
    }

    /// One blocking HTTPS GET. The caller supplies the request budget so a
    /// cached result costs nothing and a retry costs a second request.
    pub(super) fn get_once(
        host: &str,
        path: &str,
        headers: Option<&str>,
        max_bytes: usize,
        budget: &RequestBudget,
    ) -> Result<HttpResponse, String> {
        budget.try_consume()?;

        let agent = wide("Kudos/0.1 (SteamGridDB artwork)");
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
        unsafe {
            // Resolve/connect/send 5 s each; 10 s for the whole receive phase.
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

        let header_wide = headers.map(wide);
        let (header_ptr, header_len) = match header_wide.as_ref() {
            Some(value) => (value.as_ptr(), u32::MAX),
            None => (null(), 0),
        };
        if unsafe {
            WinHttpSendRequest(request.0, header_ptr, header_len, null(), 0, 0, 0)
        } == 0
        {
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
        if status == 401 || status == 403 {
            return Err(format!(
                "SteamGridDB rejected the API key (HTTP {status}). Check the key in Settings."
            ));
        }
        if status == 429 {
            return Err(
                "SteamGridDB rate limit reached (HTTP 429). Try again in a minute.".to_string(),
            );
        }
        if !(200..300).contains(&status) {
            return Err(format!("SteamGridDB returned HTTP {status}."));
        }

        if let Some(declared) = content_length(request.0) {
            if declared > max_bytes {
                return Err(format!(
                    "The response is too large ({declared} bytes; the limit is {max_bytes})."
                ));
            }
        }

        let content_type = header_string(request.0, WINHTTP_QUERY_CONTENT_TYPE);
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
            if body.len() > max_bytes {
                return Err(format!(
                    "The response exceeded the {max_bytes}-byte limit and was stopped."
                ));
            }
        }

        Ok(HttpResponse {
            content_type,
            body,
        })
    }
}

/// Retry once for transport failures and 5xx responses. 4xx (bad key, rate
/// limit) is never retried: retrying cannot fix it and wastes the budget.
pub(crate) fn get_with_retry(
    host: &str,
    path: &str,
    headers: Option<&str>,
    max_bytes: usize,
    budget: &RequestBudget,
) -> Result<HttpResponse, String> {
    let mut last_error = String::new();
    for attempt in 0..2 {
        match http::get_once(host, path, headers, max_bytes, budget) {
            Ok(response) => return Ok(response),
            Err(err) => {
                let retryable = err.contains("WinHttp")
                    || err.contains("timed out")
                    || err.starts_with("SteamGridDB returned HTTP 5");
                last_error = err;
                if attempt == 0 && !retryable {
                    break;
                }
            }
        }
    }
    Err(last_error)
}

#[cfg(not(windows))]
mod http {
    use super::{HttpResponse, RequestBudget};

    pub(super) fn get_once(
        _host: &str,
        _path: &str,
        _headers: Option<&str>,
        _max_bytes: usize,
        _budget: &RequestBudget,
    ) -> Result<HttpResponse, String> {
        Err("Artwork downloads are only supported on Windows.".to_string())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn art_kinds_round_trip_and_reject_junk() {
        for kind in [ArtKind::Grid, ArtKind::Hero, ArtKind::Logo, ArtKind::Icon] {
            assert_eq!(ArtKind::parse(kind.as_str()).unwrap(), kind);
        }
        assert_eq!(ArtKind::parse(" GRID ").unwrap(), ArtKind::Grid);
        assert!(ArtKind::parse("banner").is_err());
    }

    #[test]
    fn cache_paths_are_windows_safe_and_stable() {
        assert_eq!(sanitize_game_id("gog:1207658924"), "gog-1207658924");
        assert_eq!(
            sanitize_game_id("epic:Control/Ultimate Edition"),
            "epic-Control-Ultimate-Edition"
        );
        assert_eq!(sanitize_game_id(":::"), "game");
        assert_eq!(
            cache_stem("gog:1207658924", ArtKind::Grid),
            "gog-1207658924-grid"
        );

        let dir = Path::new(r"C:\Users\me\AppData\Roaming\Kudos\art");
        assert_eq!(
            cache_path(dir, "gog:1207658924", ArtKind::Hero, "jpg"),
            dir.join("gog-1207658924-hero.jpg")
        );
        assert_eq!(
            cache_path(dir, "steam:730", ArtKind::Logo, ".png"),
            dir.join("steam-730-logo.png")
        );
    }

    #[test]
    fn only_steamgriddb_https_urls_are_allowed() {
        assert_eq!(
            split_art_url("https://cdn2.steamgriddb.com/file/sgdb-storage/abc.jpg").unwrap(),
            (
                "cdn2.steamgriddb.com".to_string(),
                "/file/sgdb-storage/abc.jpg".to_string()
            )
        );
        assert!(split_art_url("http://cdn2.steamgriddb.com/a.jpg").is_err());
        assert!(split_art_url("https://evil.example.com/a.jpg").is_err());
        assert!(split_art_url("https://steamgriddb.com.evil.example/a.jpg").is_err());
        assert!(split_art_url("https://user@steamgriddb.com/a.jpg").is_err());
        assert!(split_art_url("https://cdn2.steamgriddb.com:8443/a.jpg").is_err());
    }

    #[test]
    fn image_validation_checks_type_and_magic() {
        let jpeg = [0xFF, 0xD8, 0xFF, 0xE0, 0, 0];
        let png = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0, 0];
        assert_eq!(image_extension("image/jpeg", &jpeg).unwrap(), "jpg");
        assert_eq!(image_extension("image/png; charset=binary", &png).unwrap(), "png");
        assert!(image_extension("text/html", &jpeg).is_err());
        assert!(image_extension("image/jpeg", &png).is_err());
        assert!(image_extension("image/jpeg", b"not an image").is_err());
        assert!(image_extension("", &jpeg).is_err());
    }

    #[test]
    fn request_budget_stops_at_the_limit() {
        let budget = RequestBudget::new(2);
        assert_eq!(budget.remaining(), 2);
        budget.try_consume().unwrap();
        budget.try_consume().unwrap();
        assert_eq!(budget.remaining(), 0);
        let error = budget.try_consume().unwrap_err();
        assert!(error.contains("budget reached"));
        assert_eq!(budget.remaining(), 0);
    }

    #[test]
    fn art_status_is_camel_case_and_omits_empty_slots() {
        let status = ArtStatus {
            grid: Some(r"C:\art\grid.jpg".to_string()),
            hero: None,
            logo: None,
            icon: None,
            source: ArtSource::Steamgriddb,
            error: None,
        };
        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("\"grid\":\"C:\\\\art\\\\grid.jpg\""));
        assert!(json.contains("\"source\":\"steamgriddb\""));
        assert!(!json.contains("hero"));
        assert!(!json.contains("error"));
    }

    #[test]
    fn quiet_auto_art_errors_are_recorded_per_game_and_cleared() {
        let id = "gog:test-auto-error";
        assert_eq!(auto_error(id), None);

        record_auto_error(id, "grid: Steam has no non-Steam shortcut");
        assert_eq!(
            auto_error(id).as_deref(),
            Some("grid: Steam has no non-Steam shortcut")
        );

        clear_auto_error(id);
        assert_eq!(auto_error(id), None);
    }
}
