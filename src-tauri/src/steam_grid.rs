//! Write cached artwork into Steam's own `config/grid` folder (Session M).
//!
//! Steam stores custom art for a non-Steam shortcut next to `shortcuts.vdf`:
//!
//! | Kind   | File                  |
//! | ------ | --------------------- |
//! | grid   | `<appid>p.jpg/png`    |
//! | hero   | `<appid>_hero.jpg/png`|
//! | logo   | `<appid>_logo.png`    |
//! | icon   | `<appid>_icon.png`    |
//!
//! Safety rules:
//! - The shortcut must exist in `shortcuts.vdf`; a game that was never added to
//!   Steam is refused instead of writing orphan files.
//! - Existing user art is never replaced unless `replace == true` (the art
//!   picker passes `true` because the user explicitly chose a replacement;
//!   auto-art passes `false`).
//! - Writes are temp-file + rename, and no backup is created (Steam regenerates
//!   grid files and the originals are still in the user's library).
//!
//! The app id algorithm mirrors Session B's `generate_shortcut_appid`
//! (`CRC32(lowercased exe + title) | 0x8000_0000`). The authoritative value is
//! read back from `shortcuts.vdf`, so a collision-bumped id still resolves.

use std::fs;
use std::path::{Path, PathBuf};

use crate::art::ArtKind;
use crate::models::{Game, Platform};

const SHORTCUT_APPID_HIGH_BIT: u32 = 0x8000_0000;
const MAX_ART_BYTES: usize = 16 * 1024 * 1024;

const TAG_TABLE: u8 = 0x00;
const TAG_STRING: u8 = 0x01;
const TAG_INT32: u8 = 0x02;
const TAG_FLOAT32: u8 = 0x03;
const TAG_POINTER: u8 = 0x04;
const TAG_WIDESTRING: u8 = 0x05;
const TAG_COLOR: u8 = 0x06;
const TAG_UINT64: u8 = 0x07;
const TAG_END: u8 = 0x08;
const TAG_INT64: u8 = 0x0A;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GridWrite {
    pub wrote: bool,
    pub path: Option<String>,
    pub skipped_existing: bool,
    pub appid: u32,
}

/// Resolve the active account, verify the shortcut and copy one cached file into
/// Steam's grid folder. Used by `apply_art`.
pub fn apply_for_game(
    game: &Game,
    kind: ArtKind,
    source: &Path,
    replace: bool,
) -> Result<GridWrite, String> {
    if game.platform == Platform::Steam {
        return Err("Steam games use Steam's own artwork; only local overrides apply.".to_string());
    }

    let root = crate::sources::steam::detect_steam_root().ok_or_else(|| {
        "Steam is not installed, so there is no grid folder to write to.".to_string()
    })?;
    let userdata = crate::sources::steam::active_userdata_dir(&root).ok_or_else(|| {
        "No signed-in Steam account was found. Sign in to Steam once, then try again.".to_string()
    })?;
    let shortcuts = userdata.join("config").join("shortcuts.vdf");
    apply_to_userdata(&userdata, &shortcuts, game, kind, source, replace)
}

/// Testable core: `userdata` is `...\userdata\<accountid>`, `shortcuts` is its
/// `config\shortcuts.vdf`.
pub fn apply_to_userdata(
    userdata: &Path,
    shortcuts: &Path,
    game: &Game,
    kind: ArtKind,
    source: &Path,
    replace: bool,
) -> Result<GridWrite, String> {
    let exe = shortcut_exe(game);
    let appid = find_shortcut_appid(shortcuts, exe.as_deref(), &game.title)?
        .ok_or_else(|| {
            format!(
                "Steam has no non-Steam shortcut for \"{}\". Add it to Steam first, then change its artwork.",
                game.title
            )
        })?;

    let grid_dir = userdata.join("config").join("grid");
    write_cached_art(&grid_dir, appid, kind, source, replace)
}

/// Copy one cached image to `<grid_dir>/<steam name>`. Does not check for a
/// shortcut; callers that need that guarantee use `apply_to_userdata`.
pub fn write_cached_art(
    grid_dir: &Path,
    appid: u32,
    kind: ArtKind,
    source: &Path,
    replace: bool,
) -> Result<GridWrite, String> {
    if !source.is_file() {
        return Err(format!(
            "The cached artwork file is missing: {}",
            source.display()
        ));
    }
    let extension = source
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let destinations: Vec<PathBuf> = steam_art_names(appid, kind, &extension)?
        .into_iter()
        .map(|name| grid_dir.join(name))
        .collect();

    // Never overwrite the user's own art. Grid writes two names (`<id>p` and
    // `<id>`); if either already exists, leave both untouched.
    if !replace {
        if let Some(existing) = destinations.iter().find(|path| path.is_file()) {
            return Ok(GridWrite {
                wrote: false,
                path: Some(existing.to_string_lossy().into_owned()),
                skipped_existing: true,
                appid,
            });
        }
    }

    let metadata = fs::metadata(source)
        .map_err(|err| format!("Could not read {}: {err}", source.display()))?;
    if metadata.len() as usize > MAX_ART_BYTES {
        return Err(format!(
            "The artwork file is too large ({} bytes; the limit is {MAX_ART_BYTES}).",
            metadata.len()
        ));
    }
    let bytes =
        fs::read(source).map_err(|err| format!("Could not read {}: {err}", source.display()))?;

    fs::create_dir_all(grid_dir)
        .map_err(|err| format!("Could not create {}: {err}", grid_dir.display()))?;
    for destination in &destinations {
        write_atomic(destination, &bytes)?;
    }

    Ok(GridWrite {
        wrote: true,
        path: destinations
            .first()
            .map(|path| path.to_string_lossy().into_owned()),
        skipped_existing: false,
        appid,
    })
}

fn write_atomic(destination: &Path, bytes: &[u8]) -> Result<(), String> {
    let temp = with_suffix(destination, ".kudos-tmp");
    fs::write(&temp, bytes).map_err(|err| format!("Could not write {}: {err}", temp.display()))?;
    if let Err(first_error) = fs::rename(&temp, destination) {
        let _ = fs::remove_file(destination);
        fs::rename(&temp, destination).map_err(|second_error| {
            let _ = fs::remove_file(&temp);
            format!(
                "Could not replace {} ({first_error}; retry: {second_error})",
                destination.display()
            )
        })?;
    }
    Ok(())
}

/// Steam's exact file names. Grid art is written under both names Steam reads
/// (`<id>p` for the portrait tile and `<id>` for the legacy fallback); logos and
/// icons are PNG-only in Steam's grid folder.
pub fn steam_art_names(appid: u32, kind: ArtKind, extension: &str) -> Result<Vec<String>, String> {
    let extension = extension.trim_start_matches('.').to_ascii_lowercase();
    if !matches!(extension.as_str(), "jpg" | "png") {
        return Err(format!(
            "Unsupported artwork format \"{extension}\"; expected jpg or png."
        ));
    }
    match kind {
        ArtKind::Grid => Ok(vec![
            format!("{appid}p.{extension}"),
            format!("{appid}.{extension}"),
        ]),
        ArtKind::Hero => Ok(vec![format!("{appid}_hero.{extension}")]),
        ArtKind::Logo if extension == "png" => Ok(vec![format!("{appid}_logo.png")]),
        ArtKind::Icon if extension == "png" => Ok(vec![format!("{appid}_icon.png")]),
        ArtKind::Logo | ArtKind::Icon => Err(format!(
            "Steam requires PNG for {} art.",
            kind.as_str()
        )),
    }
}

/// The primary (first) file name for a kind.
pub fn steam_art_name(appid: u32, kind: ArtKind, extension: &str) -> Result<String, String> {
    steam_art_names(appid, kind, extension)?
        .into_iter()
        .next()
        .ok_or_else(|| "Steam has no file name for this artwork kind.".to_string())
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut os = path.as_os_str().to_os_string();
    os.push(suffix);
    PathBuf::from(os)
}

fn shortcut_exe(game: &Game) -> Option<String> {
    [game.exe_path.as_deref(), Some(game.launch_command.as_str())]
        .into_iter()
        .flatten()
        .map(|value| value.trim().trim_matches('"').trim())
        .find(|value| !value.is_empty() && !value.contains("://"))
        .map(|value| value.replace('/', "\\"))
}

fn normalize_exe(value: &str) -> String {
    value
        .trim()
        .trim_matches('"')
        .replace('/', "\\")
        .to_ascii_lowercase()
}

// ---------------------------------------------------------------------------
// shortcuts.vdf lookup (read-only)
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct ShortcutEntry {
    appid: Option<u32>,
    app_name: Option<String>,
    exe: Option<String>,
}

/// Whether any shortcut in `shortcuts.vdf` has this app id.
pub fn shortcut_exists(shortcuts: &Path, appid: u32) -> Result<bool, String> {
    Ok(read_shortcuts(shortcuts)?
        .iter()
        .any(|entry| entry.appid == Some(appid)))
}

/// Find the real app id for a shortcut by `AppName` + `Exe` (case-insensitive),
/// falling back to the deterministic id Session B uses. Returns `Ok(None)` when
/// the file exists but has no matching shortcut.
pub fn find_shortcut_appid(
    shortcuts: &Path,
    exe: Option<&str>,
    title: &str,
) -> Result<Option<u32>, String> {
    let entries = read_shortcuts(shortcuts)?;
    let wanted_title = title.trim().to_ascii_lowercase();
    let wanted_exe = exe.map(normalize_exe);

    for entry in &entries {
        let name_matches = entry
            .app_name
            .as_deref()
            .map(|name| name.trim().to_ascii_lowercase() == wanted_title)
            .unwrap_or(false);
        let exe_matches = match (&wanted_exe, entry.exe.as_deref()) {
            (Some(wanted), Some(actual)) => normalize_exe(actual) == *wanted,
            _ => false,
        };
        if name_matches && (wanted_exe.is_none() || exe_matches) {
            return Ok(entry.appid);
        }
    }

    if let Some(exe) = wanted_exe {
        let candidate = deterministic_shortcut_appid(&exe, title);
        if entries.iter().any(|entry| entry.appid == Some(candidate)) {
            return Ok(Some(candidate));
        }
    }
    Ok(None)
}

fn read_shortcuts(path: &Path) -> Result<Vec<ShortcutEntry>, String> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => {
            return Err(format!(
                "Could not read {}: {err}",
                path.display()
            ))
        }
    };
    if bytes.is_empty() {
        return Ok(Vec::new());
    }
    parse_shortcuts(&bytes)
        .map_err(|err| format!("{} is not a valid shortcuts.vdf: {err}", path.display()))
}

fn parse_shortcuts(bytes: &[u8]) -> Result<Vec<ShortcutEntry>, String> {
    let mut cursor = Cursor {
        data: bytes,
        pos: 0,
    };
    let mut entries = Vec::new();

    while !cursor.eof() {
        let tag = cursor.byte()?;
        if tag == TAG_END {
            break;
        }
        let key = cursor.cstring()?;
        if tag == TAG_TABLE {
            if key.eq_ignore_ascii_case("shortcuts") {
                read_shortcuts_table(&mut cursor, &mut entries)?;
            } else {
                cursor.skip_table()?;
            }
        } else {
            cursor.skip_value(tag)?;
        }
    }
    Ok(entries)
}

fn read_shortcuts_table(
    cursor: &mut Cursor<'_>,
    entries: &mut Vec<ShortcutEntry>,
) -> Result<(), String> {
    loop {
        let tag = cursor.byte()?;
        if tag == TAG_END {
            return Ok(());
        }
        let _key = cursor.cstring()?;
        if tag == TAG_TABLE {
            entries.push(read_shortcut_entry(cursor)?);
        } else {
            cursor.skip_value(tag)?;
        }
    }
}

fn read_shortcut_entry(cursor: &mut Cursor<'_>) -> Result<ShortcutEntry, String> {
    let mut entry = ShortcutEntry::default();
    loop {
        let tag = cursor.byte()?;
        if tag == TAG_END {
            return Ok(entry);
        }
        let key = cursor.cstring()?;
        match (tag, key.to_ascii_lowercase().as_str()) {
            (TAG_STRING, "appname") => entry.app_name = Some(cursor.cstring()?),
            (TAG_STRING, "exe") => entry.exe = Some(cursor.cstring()?),
            (TAG_INT32, "appid") => entry.appid = Some(cursor.i32()? as u32),
            (TAG_INT64, "appid") => entry.appid = Some(cursor.i64()? as u32),
            (TAG_STRING, "appid") => {
                let text = cursor.cstring()?;
                entry.appid = text.trim().parse::<i64>().ok().map(|value| value as u32);
            }
            (TAG_TABLE, _) => cursor.skip_table()?,
            _ => cursor.skip_value(tag)?,
        }
    }
}

struct Cursor<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
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

    fn cstring_bytes(&mut self) -> Result<Vec<u8>, String> {
        let start = self.pos;
        while self.pos < self.data.len() && self.data[self.pos] != 0 {
            self.pos += 1;
        }
        if self.pos >= self.data.len() {
            return Err("unterminated string in binary VDF".to_string());
        }
        let value = self.data[start..self.pos].to_vec();
        self.pos += 1;
        Ok(value)
    }

    fn cstring(&mut self) -> Result<String, String> {
        Ok(String::from_utf8_lossy(&self.cstring_bytes()?).into_owned())
    }

    fn i32(&mut self) -> Result<i32, String> {
        let bytes = self.take(4)?;
        Ok(i32::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3],
        ]))
    }

    fn i64(&mut self) -> Result<i64, String> {
        let bytes = self.take(8)?;
        Ok(i64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]))
    }

    fn u16(&mut self) -> Result<u16, String> {
        let bytes = self.take(2)?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    fn skip_value(&mut self, tag: u8) -> Result<(), String> {
        match tag {
            TAG_TABLE => self.skip_table(),
            TAG_STRING => {
                self.cstring_bytes()?;
                Ok(())
            }
            TAG_INT32 | TAG_FLOAT32 | TAG_POINTER | TAG_COLOR => {
                self.take(4)?;
                Ok(())
            }
            TAG_WIDESTRING => {
                while self.u16()? != 0 {}
                Ok(())
            }
            TAG_UINT64 | TAG_INT64 => {
                self.take(8)?;
                Ok(())
            }
            other => Err(format!("unsupported binary VDF type 0x{other:02x}")),
        }
    }

    fn skip_table(&mut self) -> Result<(), String> {
        loop {
            let tag = self.byte()?;
            if tag == TAG_END {
                return Ok(());
            }
            self.cstring_bytes()?;
            self.skip_value(tag)?;
        }
    }
}

fn deterministic_shortcut_appid(exe: &str, title: &str) -> u32 {
    let mut seed = String::with_capacity(exe.len() + title.len());
    seed.push_str(&normalize_exe(exe));
    seed.push_str(title);
    crc32(seed.as_bytes()) | SHORTCUT_APPID_HIGH_BIT
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
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn fixture(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("steam")
            .join(name)
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
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir().join(format!(
            "kudos-grid-{}-{}-{}-{}",
            std::process::id(),
            stamp,
            unique,
            tag
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("temp dir");
        TempDir { path }
    }

    const PNG: &[u8] = &[
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D,
    ];

    fn gog_game(exe: &str) -> Game {
        let mut game = Game::new(
            "gog:1207658924",
            Platform::Gog,
            "Old Game",
            "goggalaxy://openGameView/1207658924",
        );
        game.exe_path = Some(exe.to_string());
        game
    }

    fn png_file(dir: &Path, name: &str) -> PathBuf {
        let path = dir.join(name);
        fs::write(&path, PNG).expect("png");
        path
    }

    #[test]
    fn steam_art_names_match_the_grid_folder_conventions() {
        assert_eq!(steam_art_name(123, ArtKind::Grid, "jpg").unwrap(), "123p.jpg");
        assert_eq!(steam_art_name(123, ArtKind::Grid, "png").unwrap(), "123p.png");
        assert_eq!(
            steam_art_names(123, ArtKind::Grid, "jpg").unwrap(),
            vec!["123p.jpg".to_string(), "123.jpg".to_string()]
        );
        assert_eq!(
            steam_art_name(123, ArtKind::Hero, "jpg").unwrap(),
            "123_hero.jpg"
        );
        assert_eq!(
            steam_art_name(123, ArtKind::Logo, "png").unwrap(),
            "123_logo.png"
        );
        assert_eq!(
            steam_art_name(123, ArtKind::Icon, "png").unwrap(),
            "123_icon.png"
        );
        assert!(steam_art_name(123, ArtKind::Logo, "jpg").is_err());
        assert!(steam_art_name(123, ArtKind::Icon, "gif").is_err());
    }

    #[test]
    fn crc32_matches_the_standard_check_value() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        let appid = deterministic_shortcut_appid(r"C:\Games\Old\old.exe", "Old Game");
        assert!(appid >= SHORTCUT_APPID_HIGH_BIT);
        // Deterministic across slash styles and case.
        assert_eq!(
            appid,
            deterministic_shortcut_appid(r"c:/games/old/OLD.EXE", "Old Game")
        );
    }

    #[test]
    fn reads_the_committed_shortcuts_fixture() {
        let path = fixture("shortcuts-existing.vdf");
        let entries = read_shortcuts(&path).expect("parse fixture");
        assert_eq!(entries.len(), 2);
        assert!(entries.iter().any(|entry| entry.appid == Some(0x8123_4567)));
        assert!(entries.iter().any(|entry| entry.appid == Some(0x8234_5678)));
        assert!(shortcut_exists(&path, 0x8123_4567).unwrap());
        assert!(!shortcut_exists(&path, 0x1234_5678).unwrap());
    }

    #[test]
    fn finds_a_shortcut_by_name_and_exe_then_by_deterministic_id() {
        let path = fixture("shortcuts-existing.vdf");
        let by_pair = find_shortcut_appid(
            &path,
            Some(r"C:/Games/Old/old.exe"),
            "old game",
        )
        .unwrap();
        assert_eq!(by_pair, Some(0x8123_4567));

        // Name-only still resolves (useful when a source has no exe path).
        assert_eq!(
            find_shortcut_appid(&path, None, "Old Game").unwrap(),
            Some(0x8123_4567)
        );

        // A different game does not match.
        assert_eq!(
            find_shortcut_appid(&path, Some(r"C:\Elsewhere\new.exe"), "New Game").unwrap(),
            None
        );
    }

    #[test]
    fn empty_shortcuts_file_has_no_entries() {
        let path = fixture("shortcuts-empty.vdf");
        assert!(read_shortcuts(&path).unwrap().is_empty());
        assert!(!shortcut_exists(&path, 0x8123_4567).unwrap());
        assert_eq!(
            find_shortcut_appid(&path, Some(r"C:\Games\Old\old.exe"), "Old Game").unwrap(),
            None
        );
    }

    #[test]
    fn refuses_to_write_when_the_shortcut_is_absent() {
        let dir = temp_dir("absent");
        let userdata = dir.path.join("userdata").join("12345");
        let config = userdata.join("config");
        fs::create_dir_all(&config).unwrap();
        fs::copy(fixture("shortcuts-empty.vdf"), config.join("shortcuts.vdf")).unwrap();
        let source = png_file(&dir.path, "grid.png");

        let error = apply_to_userdata(
            &userdata,
            &config.join("shortcuts.vdf"),
            &gog_game(r"C:\Games\Old\old.exe"),
            ArtKind::Grid,
            &source,
            true,
        )
        .unwrap_err();

        assert!(error.contains("no non-Steam shortcut"), "got: {error}");
        assert!(!config.join("grid").exists());
    }

    #[test]
    fn refuses_when_shortcuts_vdf_is_corrupt() {
        let dir = temp_dir("corrupt");
        let userdata = dir.path.join("userdata").join("12345");
        let config = userdata.join("config");
        fs::create_dir_all(&config).unwrap();
        fs::write(config.join("shortcuts.vdf"), b"\x00shortcuts\x00\x01broken").unwrap();
        let source = png_file(&dir.path, "grid.png");

        let error = apply_to_userdata(
            &userdata,
            &config.join("shortcuts.vdf"),
            &gog_game(r"C:\Games\Old\old.exe"),
            ArtKind::Grid,
            &source,
            true,
        )
        .unwrap_err();
        assert!(error.contains("not a valid shortcuts.vdf"), "got: {error}");
    }

    #[test]
    fn writes_art_for_an_existing_shortcut() {
        let dir = temp_dir("write");
        let userdata = dir.path.join("userdata").join("12345");
        let config = userdata.join("config");
        fs::create_dir_all(&config).unwrap();
        fs::copy(fixture("shortcuts-existing.vdf"), config.join("shortcuts.vdf")).unwrap();
        let source = png_file(&dir.path, "grid.png");

        let write = apply_to_userdata(
            &userdata,
            &config.join("shortcuts.vdf"),
            &gog_game(r"C:\Games\Old\old.exe"),
            ArtKind::Grid,
            &source,
            true,
        )
        .expect("write");

        assert!(write.wrote);
        assert_eq!(write.appid, 0x8123_4567);
        let destination = config.join("grid").join("2166572391p.png");
        let legacy = config.join("grid").join("2166572391.png");
        assert_eq!(
            write.path.as_deref(),
            Some(destination.to_string_lossy().as_ref())
        );
        assert_eq!(fs::read(&destination).unwrap(), PNG);
        assert_eq!(fs::read(&legacy).unwrap(), PNG);
        assert!(!with_suffix(&destination, ".kudos-tmp").exists());
        assert!(!with_suffix(&legacy, ".kudos-tmp").exists());
    }

    #[test]
    fn never_overwrites_user_art_unless_explicitly_asked() {
        let dir = temp_dir("replace");
        let userdata = dir.path.join("userdata").join("12345");
        let config = userdata.join("config");
        let grid = config.join("grid");
        fs::create_dir_all(&grid).unwrap();
        fs::copy(fixture("shortcuts-existing.vdf"), config.join("shortcuts.vdf")).unwrap();
        let source = png_file(&dir.path, "grid.png");
        let destination = grid.join("2166572391p.png");
        let legacy = grid.join("2166572391.png");
        fs::write(&destination, b"user art").unwrap();

        let skipped = apply_to_userdata(
            &userdata,
            &config.join("shortcuts.vdf"),
            &gog_game(r"C:\Games\Old\old.exe"),
            ArtKind::Grid,
            &source,
            false,
        )
        .expect("skip");
        assert!(!skipped.wrote);
        assert!(skipped.skipped_existing);
        assert_eq!(fs::read(&destination).unwrap(), b"user art");
        assert!(!legacy.exists());

        let replaced = apply_to_userdata(
            &userdata,
            &config.join("shortcuts.vdf"),
            &gog_game(r"C:\Games\Old\old.exe"),
            ArtKind::Grid,
            &source,
            true,
        )
        .expect("replace");
        assert!(replaced.wrote);
        assert!(!replaced.skipped_existing);
        assert_eq!(fs::read(&destination).unwrap(), PNG);
        assert_eq!(fs::read(&legacy).unwrap(), PNG);
    }

    #[test]
    fn rejects_a_steam_game_and_unsupported_source_formats() {
        let dir = temp_dir("reject");
        let source = png_file(&dir.path, "grid.png");
        let mut steam = Game::new(
            "steam:730",
            Platform::Steam,
            "Counter-Strike 2",
            "steam://rungameid/730",
        );
        steam.exe_path = Some(r"C:\Games\cs2.exe".to_string());
        let error = apply_for_game(&steam, ArtKind::Grid, &source, true).unwrap_err();
        assert!(error.contains("Steam games"));

        let text = dir.path.join("grid.txt");
        fs::write(&text, b"nope").unwrap();
        let error = write_cached_art(&dir.path.join("grid"), 1, ArtKind::Grid, &text, true)
            .unwrap_err();
        assert!(error.contains("expected jpg or png"));
    }
}
