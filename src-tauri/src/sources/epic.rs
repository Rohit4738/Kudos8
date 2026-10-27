//! Epic Games source. OWNER: Session C.
//!
//! Installed games are described by the Epic Games Launcher's per-app manifest
//! files (`*.item` under `C:\ProgramData\Epic\EpicGamesLauncher\Data\Manifests`).
//! Each manifest is small JSON; we read the handful of fields we need and skip
//! entries that are not launchable games (DLC, Unreal Engine plugins/assets,
//! partially downloaded apps, stale manifests).
//!
//! Launching prefers the official launcher protocol so Epic's authentication and
//! DRM flow still runs. When the launcher is not present we fall back to starting
//! the game executable directly.

use std::collections::BTreeMap;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

use serde_json::Value;

use crate::error::{SourceError, SourceResult};
use crate::launch;
use crate::models::{Game, Platform};
use crate::sources::LibrarySource;

/// Default manifest directory. Overridable with `KUDOS_EPIC_MANIFEST_DIR` so the
/// source can be exercised against a fixture directory without a real install.
const DEFAULT_MANIFEST_DIR: &str = r"C:\ProgramData\Epic\EpicGamesLauncher\Data\Manifests";

/// Known locations of the Epic Games Launcher executable (32- and 64-bit views).
const LAUNCHER_CANDIDATES: &[&str] = &[
    r"C:\Program Files (x86)\Epic Games\Launcher\Portal\Binaries\Win64\EpicGamesLauncher.exe",
    r"C:\Program Files\Epic Games\Launcher\Portal\Binaries\Win64\EpicGamesLauncher.exe",
    r"C:\Program Files (x86)\Epic Games\Launcher\Portal\Binaries\Win32\EpicGamesLauncher.exe",
];

/// Manifest files are tiny; a cache hit keyed on the directory signature keeps a
/// full refresh well under the 300 ms budget.
#[derive(Default)]
struct CacheEntry {
    signature: u64,
    games: Vec<Game>,
}

pub struct EpicSource {
    cache: Mutex<Option<CacheEntry>>,
}

impl EpicSource {
    pub fn new() -> Self {
        Self {
            cache: Mutex::new(None),
        }
    }

    fn manifest_dir() -> PathBuf {
        std::env::var_os("KUDOS_EPIC_MANIFEST_DIR")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(DEFAULT_MANIFEST_DIR))
    }
}

impl Default for EpicSource {
    fn default() -> Self {
        Self::new()
    }
}

impl EpicSource {
    /// Scan one manifest directory. Shared by `installed` and the fixture tests.
    fn scan(&self, dir: &Path) -> SourceResult<Vec<Game>> {
        if !dir.is_dir() {
            // Epic is simply not installed on this machine.
            return Ok(Vec::new());
        }

        let entries = fs::read_dir(&dir).map_err(|err| {
            SourceError::unavailable(format!(
                "cannot read Epic manifests at {}: {err}",
                dir.display()
            ))
        })?;

        // Collect manifests and build a cheap signature of the directory so we can
        // avoid reparsing every JSON file on each refresh.
        let mut manifests: Vec<(PathBuf, u64, u64)> = Vec::new(); // path, size, mtime(ns)
        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(err) => {
                    eprintln!("[kudos] epic: skipping unreadable directory entry: {err}");
                    continue;
                }
            };

            let path = entry.path();
            let is_manifest = path
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| ext.eq_ignore_ascii_case("item"));
            if !is_manifest {
                continue;
            }

            match entry.metadata() {
                Ok(meta) if meta.is_file() => {
                    let mtime = meta
                        .modified()
                        .ok()
                        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|since| since.as_nanos() as u64)
                        .unwrap_or(0);
                    manifests.push((path, meta.len(), mtime));
                }
                Ok(_) => {}
                Err(err) => {
                    eprintln!(
                        "[kudos] epic: cannot stat manifest {}: {err}",
                        path.display()
                    );
                }
            }
        }

        let signature = signature_of(&manifests);
        if let Some(cache) = self.cache.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
            if cache.signature == signature {
                return Ok(cache.games.clone());
            }
        }

        let mut games: BTreeMap<String, (Game, String)> = BTreeMap::new();
        for (path, _, _) in &manifests {
            let bytes = match fs::read(path) {
                Ok(bytes) => bytes,
                Err(err) => {
                    eprintln!(
                        "[kudos] epic: cannot read manifest {}: {err}",
                        path.display()
                    );
                    continue;
                }
            };

            match parse_manifest(&bytes) {
                Some((game, install_dir, version)) => {
                    // Guard against leftover manifests for apps that were removed
                    // or moved since they were installed.
                    if !install_dir.is_dir() {
                        continue;
                    }

                    // Several manifests can exist for one app after an update or
                    // a moved install; keep the one with the newest build label so
                    // the result is deterministic.
                    match games.get(&game.id) {
                        Some((_, existing_version)) if existing_version >= &version => {}
                        _ => {
                            games.insert(game.id.clone(), (game, version));
                        }
                    }
                }
                None => {
                    eprintln!(
                        "[kudos] epic: skipping non-game or malformed manifest {}",
                        path.display()
                    );
                }
            }
        }

        let games: Vec<Game> = games.into_values().map(|(game, _)| game).collect();
        *self.cache.lock().unwrap_or_else(|e| e.into_inner()) = Some(CacheEntry {
            signature,
            games: games.clone(),
        });

        Ok(games)
    }
}

impl LibrarySource for EpicSource {
    fn platform(&self) -> Platform {
        Platform::Epic
    }

    fn installed(&self) -> SourceResult<Vec<Game>> {
        self.scan(&Self::manifest_dir())
    }

    fn launch(&self, game: &Game) -> SourceResult<()> {
        if game.platform != Platform::Epic {
            return Err(SourceError::invalid(format!(
                "{} is not an Epic game",
                game.id
            )));
        }

        // Preferred path: let the launcher authenticate the user, apply updates
        // and satisfy DRM before starting the game.
        if epic_launcher_installed() {
            return launch::open(&game.launch_command);
        }

        // Fallback: start the executable directly, from its install directory.
        if let Some(exe) = game.exe_path.as_deref() {
            let exe = Path::new(exe);
            if exe.is_file() {
                let mut command = Command::new(exe);
                if let Some(dir) = exe.parent() {
                    command.current_dir(dir);
                }
                return command
                    .spawn()
                    .map(|_| ())
                    .map_err(|err| {
                        SourceError::io(format!(
                            "failed to launch Epic game '{}': {err}",
                            game.title
                        ))
                    });
            }
        }

        // Last resort: hand the URI to the shell; the protocol may still be
        // registered even if our launcher probes failed.
        launch::open(&game.launch_command)
    }
}

/// Parse one manifest. Returns the game plus its install directory so the caller
/// can verify the app is still present on disk.
fn parse_manifest(bytes: &[u8]) -> Option<(Game, PathBuf, String)> {
    // Some manifests start with a UTF-8 BOM; serde_json rejects it.
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    let manifest: Value = serde_json::from_slice(bytes).ok()?;

    let app_name = string_field(&manifest, "AppName");
    if app_name.is_empty() {
        return None;
    }

    // Incomplete downloads are not playable yet.
    if bool_field(&manifest, "bIsIncomplete").unwrap_or(false)
        || bool_field(&manifest, "bIsIncompleteInstall").unwrap_or(false)
    {
        return None;
    }

    // DLC / plugins / engine assets are explicitly not applications.
    if bool_field(&manifest, "bIsApplication") == Some(false) {
        return None;
    }
    if is_non_game_technical_type(&manifest) {
        return None;
    }

    let launch_executable = string_field(&manifest, "LaunchExecutable");
    let launch_command = string_field(&manifest, "LaunchCommand");
    if launch_executable.is_empty() && launch_command.is_empty() {
        return None;
    }

    let install_dir = PathBuf::from(string_field(&manifest, "InstallLocation"));
    if install_dir.as_os_str().is_empty() {
        return None;
    }

    let title = {
        let display_name = string_field(&manifest, "DisplayName");
        if display_name.is_empty() {
            app_name.clone()
        } else {
            display_name
        }
    };

    let exe_path = resolve_executable(&install_dir, &launch_executable);
    let size_bytes = u64_field(&manifest, "InstallSize").filter(|size| *size > 0);

    // The launcher URI needs only the AppName. Keep it as the primary launch
    // command; `launch()` falls back to `exe_path` when the launcher is absent.
    let launch_command = format!(
        "com.epicgames.launcher://apps/{app_name}?action=launch&silent=true"
    );

    let game = Game {
        id: format!("epic:{app_name}"),
        platform: Platform::Epic,
        title,
        exe_path,
        launch_command,
        is_installed: true,
        cover_image: None,
        size_bytes,
    };

    // Not part of the `Game` wire format, but useful for resolving duplicate
    // manifests for the same app after an update.
    let version = {
        let build = string_field(&manifest, "BuildVersion");
        if build.is_empty() {
            string_field(&manifest, "AppVersionString")
        } else {
            build
        }
    };

    Some((game, install_dir, version))
}

fn is_non_game_technical_type(manifest: &Value) -> bool {
    const NON_GAMES: &[&str] = &[
        "dlc",
        "plugin",
        "codeplugin",
        "asset",
        "engine",
        "enginecontent",
        "content",
        "mod",
        "mods",
    ];

    if let Some(value) = manifest.get("TechnicalType").and_then(Value::as_str) {
        if NON_GAMES.contains(&value.trim().to_ascii_lowercase().as_str()) {
            return true;
        }
    }

    if let Some(categories) = manifest.get("AppCategories").and_then(Value::as_array) {
        for category in categories.iter().filter_map(Value::as_str) {
            let category = category.trim().to_ascii_lowercase();
            if category == "dlc" || category == "plugins" || category == "assets" {
                return true;
            }
        }
    }

    false
}

/// Join `LaunchExecutable` onto the install directory unless it is already
/// absolute. Epic stores the latter as a plain relative file name in practice.
fn resolve_executable(install_dir: &Path, executable: &str) -> Option<String> {
    let executable = executable.trim();
    if executable.is_empty() {
        return None;
    }

    let path = Path::new(executable);
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        install_dir.join(path)
    };

    Some(path.to_string_lossy().into_owned())
}

fn string_field(manifest: &Value, key: &str) -> String {
    manifest
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or_default()
        .to_string()
}

fn bool_field(manifest: &Value, key: &str) -> Option<bool> {
    manifest.get(key).and_then(Value::as_bool)
}

fn u64_field(manifest: &Value, key: &str) -> Option<u64> {
    let value = manifest.get(key)?;
    value
        .as_u64()
        .or_else(|| value.as_str().and_then(|text| text.trim().parse().ok()))
}

fn signature_of(manifests: &[(PathBuf, u64, u64)]) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    manifests.len().hash(&mut hasher);
    for (path, size, mtime) in manifests {
        path.hash(&mut hasher);
        size.hash(&mut hasher);
        mtime.hash(&mut hasher);
    }
    hasher.finish()
}

fn epic_launcher_installed() -> bool {
    EpicSource::manifest_dir().is_dir()
        || LAUNCHER_CANDIDATES
            .iter()
            .any(|candidate| Path::new(candidate).is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Realistic Epic manifest fixture (ProgramData `*.item` format). Task board
    /// C6 requires shared fixtures under `src-tauri/tests/fixtures/`.
    const FORTNITE: &str = include_str!("../../tests/fixtures/epic-fortnite.item");

    #[test]
    fn parses_a_typical_game_manifest() {
        let (game, install, version) =
            parse_manifest(FORTNITE.as_bytes()).expect("Fortnite should parse");

        assert_eq!(game.id, "epic:Fortnite");
        assert_eq!(game.platform, Platform::Epic);
        assert_eq!(game.title, "Fortnite");
        assert!(game.is_installed);
        assert_eq!(
            game.exe_path.as_deref(),
            Some(r"C:\Program Files\Epic Games\Fortnite\FortniteLauncher.exe")
        );
        assert_eq!(
            game.launch_command,
            "com.epicgames.launcher://apps/Fortnite?action=launch&silent=true"
        );
        assert_eq!(game.size_bytes, Some(123_456_789));
        assert_eq!(install, PathBuf::from(r"C:\Program Files\Epic Games\Fortnite"));
        assert_eq!(version, "31.00");
    }

    #[test]
    fn accepts_a_utf8_bom() {
        let mut bytes = vec![0xEF, 0xBB, 0xBF];
        bytes.extend_from_slice(FORTNITE.as_bytes());
        assert!(parse_manifest(&bytes).is_some());
    }

    #[test]
    fn skips_incomplete_installs() {
        for field in ["bIsIncomplete", "bIsIncompleteInstall"] {
            let json = FORTNITE.replace(
                "\"bIsIncompleteInstall\": false",
                &format!("\"{field}\": true"),
            );
            assert!(
                parse_manifest(json.as_bytes()).is_none(),
                "{field} should hide the game"
            );
        }
    }

    #[test]
    fn skips_non_application_entries() {
        let json = FORTNITE.replace("\"bIsApplication\": true", "\"bIsApplication\": false");
        assert!(parse_manifest(json.as_bytes()).is_none());
    }

    #[test]
    fn skips_dlc_and_engine_assets() {
        let dlc = FORTNITE.replace(
            "\"AppCategories\": [\"public\", \"games\", \"applications\"]",
            "\"TechnicalType\": \"DLC\", \"AppCategories\": [\"public\", \"dlc\"]",
        );
        assert!(parse_manifest(dlc.as_bytes()).is_none());

        let asset = FORTNITE.replace(
            "\"AppCategories\": [\"public\", \"games\", \"applications\"]",
            "\"AppCategories\": [\"assets\"]",
        );
        assert!(parse_manifest(asset.as_bytes()).is_none());
    }

    #[test]
    fn skips_entries_without_a_launch_target() {
        let json = FORTNITE.replace(
            "\"LaunchExecutable\": \"FortniteLauncher.exe\",",
            "",
        );
        assert!(parse_manifest(json.as_bytes()).is_none());
    }

    #[test]
    fn skips_malformed_json() {
        assert!(parse_manifest(b"{ not json").is_none());
        assert!(parse_manifest(b"[]").is_none());
    }

    #[test]
    fn falls_back_to_app_name_for_the_title() {
        let json = FORTNITE.replace("\"DisplayName\": \"Fortnite\",", "");
        let (game, _, _) = parse_manifest(json.as_bytes()).unwrap();
        assert_eq!(game.title, "Fortnite");
    }

    #[test]
    fn keeps_absolute_executables_and_joins_relative_ones() {
        let absolute = FORTNITE.replace(
            "\"LaunchExecutable\": \"FortniteLauncher.exe\",",
            "\"LaunchExecutable\": \"C:\\\\Other\\\\Game.exe\",",
        );
        let (game, _, _) = parse_manifest(absolute.as_bytes()).unwrap();
        assert_eq!(game.exe_path.as_deref(), Some(r"C:\Other\Game.exe"));

        let nested = FORTNITE.replace(
            "\"LaunchExecutable\": \"FortniteLauncher.exe\",",
            "\"LaunchExecutable\": \"bin\\\\game.exe\",",
        );
        let (game, _, _) = parse_manifest(nested.as_bytes()).unwrap();
        assert_eq!(
            game.exe_path.as_deref(),
            Some(r"C:\Program Files\Epic Games\Fortnite\bin\game.exe")
        );
    }

    #[test]
    fn handles_numeric_quirks_in_manifest_fields() {
        let json = FORTNITE.replace("\"InstallSize\": 123456789", "\"InstallSize\": \"987\"");
        let (game, _, _) = parse_manifest(json.as_bytes()).unwrap();
        assert_eq!(game.size_bytes, Some(987));

        let missing = FORTNITE.replace("\"InstallSize\": 123456789,", "");
        let (game, _, _) = parse_manifest(missing.as_bytes()).unwrap();
        assert_eq!(game.size_bytes, None);
    }

    #[test]
    fn scans_a_full_directory_quickly_and_invalidates_the_cache() {
        use std::time::{Duration, Instant};

        let unique = format!(
            "kudos-epic-scan-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        );
        let base = std::env::temp_dir().join(unique);
        let manifests = base.join("manifests");
        std::fs::create_dir_all(&manifests).unwrap();

        let write_manifest = |index: usize| {
            let install = base.join(format!("game-{index}"));
            std::fs::create_dir_all(&install).unwrap();
            let manifest = serde_json::json!({
                "AppName": format!("Game{index}"),
                "DisplayName": format!("Game {index}"),
                "InstallLocation": install.to_string_lossy(),
                "LaunchExecutable": "game.exe",
                "bIsApplication": true,
                "InstallSize": 1024,
                "AppCategories": ["public", "games", "applications"],
            });
            std::fs::write(
                manifests.join(format!("game-{index}.item")),
                manifest.to_string(),
            )
            .unwrap();
        };

        for index in 0..60 {
            write_manifest(index);
        }

        let source = EpicSource::new();

        let started = Instant::now();
        let first = source.scan(&manifests).expect("scan should succeed");
        let cold = started.elapsed();
        assert_eq!(first.len(), 60);
        assert!(
            cold < Duration::from_millis(300),
            "cold scan took {cold:?}, expected < 300 ms"
        );
        println!("[test] Epic cold scan of 60 manifests: {cold:?}");

        let started = Instant::now();
        let second = source.scan(&manifests).expect("cached scan should succeed");
        let warm = started.elapsed();
        assert_eq!(second.len(), 60);
        assert!(
            warm < Duration::from_millis(300),
            "cached scan took {warm:?}, expected < 300 ms"
        );
        println!("[test] Epic cached scan of 60 manifests: {warm:?}");
        assert_eq!(
            first.iter().map(|g| &g.id).collect::<Vec<_>>(),
            second.iter().map(|g| &g.id).collect::<Vec<_>>()
        );

        // A newly installed app changes the directory signature, so the next
        // scan must not serve a stale cache entry.
        write_manifest(60);
        let third = source.scan(&manifests).expect("rescan should succeed");
        assert_eq!(third.len(), 61);

        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn prefers_the_newest_build_when_manifests_duplicate() {
        let unique = format!(
            "kudos-epic-dupe-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        );
        let base = std::env::temp_dir().join(unique);
        let manifests = base.join("manifests");
        std::fs::create_dir_all(&manifests).unwrap();

        for (build, title, folder) in [
            ("1.0", "Old Build", "old"),
            ("2.0", "New Build", "new"),
        ] {
            let install = base.join(folder);
            std::fs::create_dir_all(&install).unwrap();
            let manifest = serde_json::json!({
                "AppName": "Duplicate",
                "BuildVersion": build,
                "DisplayName": title,
                "InstallLocation": install.to_string_lossy(),
                "LaunchExecutable": format!("{folder}.exe"),
                "bIsApplication": true,
            });
            std::fs::write(manifests.join(format!("{folder}.item")), manifest.to_string()).unwrap();
        }

        let source = EpicSource::new();
        let games = source.scan(&manifests).expect("scan should succeed");

        assert_eq!(games.len(), 1);
        assert_eq!(games[0].id, "epic:Duplicate");
        assert_eq!(games[0].title, "New Build");
        assert!(games[0]
            .exe_path
            .as_deref()
            .is_some_and(|path| path.ends_with(r"new\new.exe")));

        std::fs::remove_dir_all(&base).ok();
    }
}
