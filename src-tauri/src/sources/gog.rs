//! GOG source. OWNER: Session C.
//!
//! Installed GOG games are enumerated from the registry keys written by GOG
//! Galaxy / GOG installers. Each subkey describes one game: id, name, install
//! directory, executable and optional launch parameters.
//!
//! `HKLM\SOFTWARE\WOW6432Node\GOG.com\Games` is the canonical location on
//! 64-bit Windows. We also try the 64-bit view and `HKCU\Software\GOG.com\Games`
//! as fallbacks, because restricted machines and per-user installs may only
//! expose one of them.
//!
//! Games are launched directly (no Galaxy requirement) from their install
//! directory, honouring the stored `launchParam` arguments.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use winreg::enums::KEY_READ;
use winreg::{RegKey, HKCU, HKLM};

use crate::error::{SourceError, SourceResult};
use crate::models::{Game, Platform};
use crate::sources::LibrarySource;

/// How long a successful registry scan stays valid. Registry reads are cheap;
/// this mainly avoids re-scanning on `list_games` + `launch_game` double calls.
const CACHE_TTL: Duration = Duration::from_secs(5);

const GOG_ICO: &str = "GOG.ico";

/// One game as described by the registry, before conversion to [`Game`].
#[derive(Debug, Clone, Default)]
struct GogRecord {
    /// Bare store id, without the `gog:` prefix.
    id: String,
    title: String,
    install_dir: PathBuf,
    exe: Option<PathBuf>,
    working_dir: Option<PathBuf>,
    launch_param: String,
    size_bytes: Option<u64>,
}

struct CacheEntry {
    scanned_at: Instant,
    games: Vec<Game>,
}

pub struct GogSource {
    cache: Mutex<Option<CacheEntry>>,
}

impl GogSource {
    pub fn new() -> Self {
        Self {
            cache: Mutex::new(None),
        }
    }
}

impl Default for GogSource {
    fn default() -> Self {
        Self::new()
    }
}

impl LibrarySource for GogSource {
    fn platform(&self) -> Platform {
        Platform::Gog
    }

    fn installed(&self) -> SourceResult<Vec<Game>> {
        if let Some(cache) = self.cache.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
            if cache.scanned_at.elapsed() < CACHE_TTL {
                return Ok(cache.games.clone());
            }
        }

        let mut records = Vec::new();
        for (hive, path) in gog_roots() {
            scan_root(hive, path, &mut records);
        }

        // A game is only "installed" if its files are still on disk; registry
        // keys survive manual deletions. Keep games on drives that are merely
        // offline? No - the launcher would fail, so hide stale entries.
        let mut games: Vec<Game> = records
            .iter()
            .filter(|record| record_is_on_disk(record))
            .map(record_to_game)
            .collect();

        games.sort_by(|a, b| {
            a.title
                .to_lowercase()
                .cmp(&b.title.to_lowercase())
                .then_with(|| a.id.cmp(&b.id))
        });
        // HKLM is scanned before HKCU; keep the first (machine-wide) entry.
        games.dedup_by(|a, b| a.id == b.id);

        *self.cache.lock().unwrap_or_else(|e| e.into_inner()) = Some(CacheEntry {
            scanned_at: Instant::now(),
            games: games.clone(),
        });

        Ok(games)
    }

    fn launch(&self, game: &Game) -> SourceResult<()> {
        if game.platform != Platform::Gog {
            return Err(SourceError::invalid(format!(
                "{} is not a GOG game",
                game.id
            )));
        }

        // Re-read the registry so argument/working-dir changes made by Galaxy are
        // picked up; fall back to the values captured during the listing scan.
        let record = find_record(&game.id);
        let (exe, working_dir, launch_param) = match record {
            Some(record) => (
                record.exe,
                record.working_dir.or(Some(record.install_dir)),
                record.launch_param,
            ),
            None => {
                let exe = game
                    .exe_path
                    .as_ref()
                    .map(|path| PathBuf::from(path.as_str()));
                let working_dir = exe.as_ref().and_then(|path| path.parent()).map(Path::to_path_buf);
                (exe, working_dir, String::new())
            }
        };

        let exe = exe.ok_or_else(|| {
            SourceError::not_launchable(format!("{} has no executable path", game.title))
        })?;

        if !exe.is_file() {
            return Err(SourceError::not_launchable(format!(
                "{} executable is missing: {}",
                game.title,
                exe.display()
            )));
        }

        let mut command = Command::new(&exe);
        let args = split_launch_args(&launch_param);
        if !args.is_empty() {
            command.args(args);
        }
        if let Some(dir) = working_dir.filter(|dir| dir.is_dir()) {
            command.current_dir(dir);
        }

        command.spawn().map(|_| ()).map_err(|err| {
            SourceError::io(format!("failed to launch GOG game '{}': {err}", game.title))
        })
    }
}

/// Registry locations to scan, in priority order.
fn gog_roots() -> [(&'static RegKey, &'static str); 3] {
    [
        (HKLM, r"SOFTWARE\WOW6432Node\GOG.com\Games"),
        (HKLM, r"SOFTWARE\GOG.com\Games"),
        (HKCU, r"Software\GOG.com\Games"),
    ]
}

/// Read every game subkey under one registry root, ignoring missing keys.
fn scan_root(hive: &RegKey, path: &str, out: &mut Vec<GogRecord>) {
    let root = match hive.open_subkey_with_flags(path, KEY_READ) {
        Ok(root) => root,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return,
        Err(err) => {
            // Access denied is expected on locked-down machines; never panic.
            eprintln!("[kudos] gog: cannot open HKCU/HKLM key {path}: {err}");
            return;
        }
    };

    for name in root.enum_keys() {
        let name = match name {
            Ok(name) => name,
            Err(err) => {
                eprintln!("[kudos] gog: cannot enumerate a game key under {path}: {err}");
                continue;
            }
        };

        match root.open_subkey_with_flags(&name, KEY_READ) {
            Ok(key) => {
                if let Some(record) = read_record(&key, &name) {
                    out.push(record);
                }
            }
            Err(err) => {
                eprintln!("[kudos] gog: cannot read game key {path}\\{name}: {err}");
            }
        }
    }
}

/// Build a record from one game subkey. `fallback_id` is the subkey name, which
/// is normally the numeric product id itself.
fn read_record(key: &RegKey, fallback_id: &str) -> Option<GogRecord> {
    let id = read_string(key, "gameId")
        .or_else(|| read_string(key, "productId"))
        .unwrap_or_else(|| fallback_id.to_string());
    let id = id.trim().to_string();
    if id.is_empty() {
        return None;
    }

    let title = read_string(key, "gameName")
        .or_else(|| read_string(key, "name"))
        .unwrap_or_else(|| id.clone());

    let install_dir = read_string(key, "path")
        .or_else(|| read_string(key, "installDir"))
        .map(PathBuf::from);
    let raw_exe = read_string(key, "exe").or_else(|| read_string(key, "executable"));
    let working_dir = read_string(key, "workingDir").map(PathBuf::from);

    let install_dir = match install_dir {
        Some(dir) => dir,
        None => raw_exe
            .as_deref()
            .map(Path::new)
            .and_then(Path::parent)
            .map(Path::to_path_buf)?,
    };

    let exe = raw_exe.map(|exe| {
        let path = PathBuf::from(exe);
        if path.is_absolute() {
            path
        } else {
            install_dir.join(path)
        }
    });

    let launch_param = read_string(key, "launchParam")
        .or_else(|| read_string(key, "launchParameters"))
        .or_else(|| read_string(key, "launchArgs"))
        .unwrap_or_default();

    let size_bytes = read_u64(key, "size")
        .or_else(|| read_u64(key, "installSize"))
        .filter(|size| *size > 0);

    Some(GogRecord {
        id,
        title,
        install_dir,
        exe,
        working_dir,
        launch_param,
        size_bytes,
    })
}

fn record_is_on_disk(record: &GogRecord) -> bool {
    record.install_dir.is_dir()
        || record
            .exe
            .as_ref()
            .is_some_and(|exe| exe.is_file())
}

fn record_to_game(record: &GogRecord) -> Game {
    let exe_path = record
        .exe
        .as_ref()
        .map(|path| path.to_string_lossy().into_owned());
    let launch_command = exe_path.clone().unwrap_or_default();

    Game {
        id: format!("gog:{}", record.id),
        platform: Platform::Gog,
        title: record.title.clone(),
        exe_path,
        launch_command,
        is_installed: true,
        cover_image: gog_cover(&record.install_dir),
        size_bytes: record.size_bytes,
    }
}

/// GOG installers ship a `GOG.ico` in the install root. It is only an icon, but
/// it is the only artwork available offline; when absent the UI draws its own
/// styled placeholder.
fn gog_cover(install_dir: &Path) -> Option<String> {
    let icon = install_dir.join(GOG_ICO);
    match std::fs::metadata(&icon) {
        Ok(meta) if meta.is_file() && meta.len() > 0 => {
            Some(icon.to_string_lossy().into_owned())
        }
        _ => None,
    }
}

/// Find the registry record for `game_id` (`gog:<productId>`), for launching.
fn find_record(game_id: &str) -> Option<GogRecord> {
    let wanted = game_id.strip_prefix("gog:").unwrap_or(game_id);

    for (hive, path) in gog_roots() {
        let root = match hive.open_subkey_with_flags(path, KEY_READ) {
            Ok(root) => root,
            Err(_) => continue,
        };

        // Fast path: subkey name is usually the product id.
        if let Ok(key) = root.open_subkey_with_flags(wanted, KEY_READ) {
            if let Some(record) = read_record(&key, wanted) {
                if record.id == wanted {
                    return Some(record);
                }
            }
        }

        for name in root.enum_keys().flatten() {
            let key = match root.open_subkey_with_flags(&name, KEY_READ) {
                Ok(key) => key,
                Err(_) => continue,
            };
            if let Some(record) = read_record(&key, &name) {
                if record.id == wanted {
                    return Some(record);
                }
            }
        }
    }

    None
}

fn read_string(key: &RegKey, name: &str) -> Option<String> {
    if let Ok(value) = key.get_value::<String, _>(name) {
        let value = value.trim().to_string();
        if !value.is_empty() {
            return Some(value);
        }
    }

    // Some installers store numeric values as DWORDs.
    if let Ok(value) = key.get_value::<u32, _>(name) {
        return Some(value.to_string());
    }

    None
}

fn read_u64(key: &RegKey, name: &str) -> Option<u64> {
    if let Ok(value) = key.get_value::<u64, _>(name) {
        return Some(value);
    }
    read_string(key, name).and_then(|value| value.parse().ok())
}

/// Split a GOG `launchParam` string into arguments. Handles double quotes so
/// values containing spaces survive; backslashes are treated literally (Windows
/// paths, not escape sequences).
fn split_launch_args(input: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut has_token = false;

    for ch in input.chars() {
        match ch {
            '"' => {
                in_quotes = !in_quotes;
                has_token = true;
            }
            ch if ch.is_whitespace() && !in_quotes => {
                if has_token {
                    args.push(std::mem::take(&mut current));
                    has_token = false;
                }
            }
            ch => {
                current.push(ch);
                has_token = true;
            }
        }
    }

    if has_token {
        args.push(current);
    }

    args
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_record() -> GogRecord {
        GogRecord {
            id: "1207658924".to_string(),
            title: "The Witcher 3: Wild Hunt".to_string(),
            install_dir: PathBuf::from(format!(
                r"C:\Games\The Witcher 3\{}",
                std::process::id()
            )),
            exe: Some(PathBuf::from(r"C:\Games\The Witcher 3\bin\x64\witcher3.exe")),
            working_dir: Some(PathBuf::from(r"C:\Games\The Witcher 3\bin\x64")),
            launch_param: "-skipStartScreen".to_string(),
            size_bytes: Some(50_000_000_000),
        }
    }

    #[test]
    fn maps_a_registry_record_to_a_game() {
        let game = record_to_game(&sample_record());

        assert_eq!(game.id, "gog:1207658924");
        assert_eq!(game.platform, Platform::Gog);
        assert_eq!(game.title, "The Witcher 3: Wild Hunt");
        assert!(game.is_installed);
        assert_eq!(
            game.exe_path.as_deref(),
            Some(r"C:\Games\The Witcher 3\bin\x64\witcher3.exe")
        );
        assert_eq!(game.launch_command, r"C:\Games\The Witcher 3\bin\x64\witcher3.exe");
        assert_eq!(game.size_bytes, Some(50_000_000_000));
        assert!(game.cover_image.is_none());
    }

    #[test]
    fn uses_gog_ico_when_it_exists() {
        let unique = format!(
            "kudos-gog-cover-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        );
        let dir = std::env::temp_dir().join(unique);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(GOG_ICO), b"not-a-real-icon").unwrap();

        let mut record = sample_record();
        record.install_dir = dir.clone();
        let game = record_to_game(&record);
        let expected = dir.join(GOG_ICO).to_string_lossy().into_owned();

        assert_eq!(game.cover_image.as_deref(), Some(expected.as_str()));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn splits_plain_arguments() {
        assert_eq!(
            split_launch_args("--launcher-skip -skipStartScreen"),
            vec!["--launcher-skip", "-skipStartScreen"]
        );
        assert!(split_launch_args("").is_empty());
        assert!(split_launch_args("   ").is_empty());
    }

    #[test]
    fn splits_quoted_arguments_with_spaces() {
        assert_eq!(
            split_launch_args(r#"-name "Player One" -lang="en US""#),
            vec!["-name", "Player One", "-lang=en US"]
        );
        assert_eq!(
            split_launch_args(r#""C:\Program Files\Game\game.exe" --flag"#),
            vec![r"C:\Program Files\Game\game.exe", "--flag"]
        );
    }

    #[test]
    fn absent_gog_install_returns_ok() {
        // On a machine without GOG this is empty; on one with GOG it must still
        // produce well-formed, namespaced entries. Either way: never an error.
        let source = GogSource::new();
        let games = source.installed().expect("missing GOG is not an error");

        for game in games {
            assert_eq!(game.platform, Platform::Gog);
            assert!(game.id.starts_with("gog:"));
            assert!(!game.title.is_empty());
            assert!(game.is_installed);
        }
    }
}
