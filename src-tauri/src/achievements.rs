//! Local Steam achievement counts. OWNER: Session F.
//!
//! No Steam Web API, no API key, no network request. Steam writes two binary
//! VDF files under `<Steam>/appcache/stats/`:
//!
//! - `UserGameStats_<accountid>_<appid>.bin` — the active profile's stat cache
//!   (root magic is the case-sensitive key `cache`). Achievement groups are
//!   integer bitfields; `AchievementTimes` maps unlocked bit indexes to times.
//! - `UserGameStatsSchema_<appid>.bin` — the game's stat/achievement schema.
//!   Its root key is the appid; `stats[*].type == "ACHIEVEMENTS"` entries carry
//!   a `bits` table with one entry per achievement.
//!
//! The command never fails: every missing file, locked file, half-written file
//! or unexpected layout becomes `supported: false` with a short reason. Files
//! are read into memory first, and a full parse (including trailing END markers)
//! is required before any number is reported.
//!
//! Caching is keyed by Steam root + account + appid and stores both files'
//! `(mtime, length)` stamps, so a changed stats file is reparsed on the next
//! detail-page visit without polling.

use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::{LazyLock, Mutex};
use std::time::SystemTime;

use crate::models::AchievementSummary;

const TYPE_TABLE: u8 = 0x00;
const TYPE_STRING: u8 = 0x01;
const TYPE_INT32: u8 = 0x02;
const TYPE_FLOAT32: u8 = 0x03;
const TYPE_POINTER: u8 = 0x04;
const TYPE_WSTRING: u8 = 0x05;
const TYPE_COLOR: u8 = 0x06;
const TYPE_UINT64: u8 = 0x07;
const TYPE_END: u8 = 0x08;
const TYPE_INT64: u8 = 0x0A;
const MAX_DEPTH: usize = 64;

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Resolve the active Steam root/profile and return the local achievement
/// summary for a `steam:<appid>` id. Always returns a value, never an error.
pub fn summary_for(game_id: &str) -> AchievementSummary {
    let Some(appid) = game_id
        .strip_prefix("steam:")
        .and_then(|value| value.trim().parse::<u32>().ok())
    else {
        return AchievementSummary::unsupported(
            "Achievements are only available for Steam games.",
        );
    };

    let Some(root) = crate::sources::steam::detect_steam_root() else {
        return AchievementSummary::unsupported("Steam was not found on this PC.");
    };
    let Some(user_dir) = crate::sources::steam::active_userdata_dir(&root) else {
        return AchievementSummary::unsupported("No active Steam profile was found.");
    };
    let Some(account_id) = user_dir
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| name.parse::<u32>().ok())
    else {
        return AchievementSummary::unsupported("No active Steam profile was found.");
    };

    let stats_dir = root.join("appcache").join("stats");
    let user_path = stats_dir.join(format!("UserGameStats_{account_id}_{appid}.bin"));
    let schema_path = stats_dir.join(format!("UserGameStatsSchema_{appid}.bin"));
    let key = (
        root.to_string_lossy().into_owned(),
        account_id,
        appid,
    );

    summary_cached(&key, &user_path, &schema_path, appid)
}

// ---------------------------------------------------------------------------
// Cache
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FileStamp {
    modified: Option<SystemTime>,
    len: u64,
}

impl FileStamp {
    fn from(path: &Path) -> Option<Self> {
        let metadata = fs::metadata(path).ok()?;
        Some(Self {
            modified: metadata.modified().ok(),
            len: metadata.len(),
        })
    }
}

struct CachedSummary {
    user_stamp: Option<FileStamp>,
    schema_stamp: Option<FileStamp>,
    summary: AchievementSummary,
}

type CacheKey = (String, u32, u32);
static CACHE: LazyLock<Mutex<HashMap<CacheKey, CachedSummary>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn summary_cached(
    key: &CacheKey,
    user_path: &Path,
    schema_path: &Path,
    appid: u32,
) -> AchievementSummary {
    let user_stamp = FileStamp::from(user_path);
    let schema_stamp = FileStamp::from(schema_path);

    {
        let cache = CACHE
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(entry) = cache.get(key) {
            if entry.user_stamp == user_stamp && entry.schema_stamp == schema_stamp {
                return entry.summary.clone();
            }
        }
    }

    let summary = summary_from_files(user_path, schema_path, appid);

    let mut cache = CACHE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    // Detail pages are few; this only guards against pathological callers.
    if cache.len() > 128 {
        cache.clear();
    }
    cache.insert(
        key.clone(),
        CachedSummary {
            user_stamp,
            schema_stamp,
            summary: summary.clone(),
        },
    );
    summary
}

// ---------------------------------------------------------------------------
// File-level summary
// ---------------------------------------------------------------------------

fn summary_from_files(user_path: &Path, schema_path: &Path, appid: u32) -> AchievementSummary {
    // `fs::read` copies the whole file into a private buffer before parsing, so
    // Steam writing/locking the original cannot tear a partially read tree.
    let user_bytes = match fs::read(user_path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return AchievementSummary::unsupported(
                "Steam has no local achievement data for this game.",
            );
        }
        Err(_) => {
            return AchievementSummary::unsupported(
                "Steam achievement data could not be read.",
            );
        }
    };

    let schema_bytes = match fs::read(schema_path) {
        Ok(bytes) => bytes,
        Err(_) => {
            return AchievementSummary::unsupported(
                "Steam has no achievement list for this game.",
            );
        }
    };

    let groups = match parse_schema(&schema_bytes, appid) {
        Ok(groups) => groups,
        Err(_) => {
            return AchievementSummary::unsupported(
                "Steam achievement data could not be read.",
            );
        }
    };
    let total: u32 = groups.iter().map(|group| group.bits).sum();
    if total == 0 {
        return AchievementSummary::unsupported("This game has no Steam achievements.");
    }

    let stats = match parse_user_stats(&user_bytes) {
        Ok(stats) => stats,
        Err(_) => {
            return AchievementSummary::unsupported(
                "Steam achievement data could not be read.",
            );
        }
    };

    let unlocked = count_unlocked(&groups, &stats);
    AchievementSummary {
        supported: true,
        unlocked: unlocked.min(total),
        total,
        reason: None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AchievementGroup {
    id: String,
    bits: u32,
}

#[derive(Debug, Default, Clone)]
struct UserAchievementStat {
    /// Steam stores achievement groups as integer bitfields (`data`).
    data: Option<u32>,
    /// Bit indexes listed under `AchievementTimes`; a second, belt-and-braces
    /// unlock signal that Steam also writes.
    times: Vec<u32>,
}

fn parse_schema(data: &[u8], appid: u32) -> Result<Vec<AchievementGroup>, String> {
    let root = parse_document(data)?;
    let root_key = appid.to_string();
    let root_table = table_of(&root, &root_key)
        .ok_or_else(|| format!("schema root is not {root_key}"))?;
    let stats = table_of(root_table, "stats").ok_or("schema has no stats table")?;

    let mut groups = Vec::new();
    for (id, value) in stats {
        let Some(stat) = value.as_table() else {
            continue;
        };
        let kind = string_of(stat, "type").ok_or("stat has no type")?;
        if kind != "ACHIEVEMENTS" {
            continue;
        }
        let bits = table_of(stat, "bits").ok_or("achievement stat has no bits table")?;
        groups.push(AchievementGroup {
            id: id.clone(),
            bits: u32::try_from(bits.len()).map_err(|_| "too many achievement bits")?,
        });
    }
    Ok(groups)
}

fn parse_user_stats(data: &[u8]) -> Result<HashMap<String, UserAchievementStat>, String> {
    let root = parse_document(data)?;
    // The magic is case-sensitive. `Cache` must not be accepted.
    let cache = table_of(&root, "cache").ok_or("user stats magic is not `cache`")?;

    let mut stats = HashMap::new();
    for (id, value) in cache {
        let Some(table) = value.as_table() else {
            continue;
        };
        let data = int_of(table, "data").map(|value| value as u32);
        let times = table_of(table, "AchievementTimes")
            .map(|times| {
                times
                    .iter()
                    .filter_map(|(key, _)| key.parse::<u32>().ok())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        if data.is_some() || !times.is_empty() {
            stats.insert(
                id.clone(),
                UserAchievementStat {
                    data,
                    times,
                },
            );
        }
    }
    Ok(stats)
}

fn count_unlocked(
    groups: &[AchievementGroup],
    stats: &HashMap<String, UserAchievementStat>,
) -> u32 {
    let mut unlocked = 0u32;
    for group in groups {
        let Some(stat) = stats.get(&group.id) else {
            continue;
        };

        let mask = if group.bits >= 32 {
            u32::MAX
        } else {
            (1u32 << group.bits).wrapping_sub(1)
        };
        let bit_count = stat
            .data
            .map(|data| (data & mask).count_ones())
            .unwrap_or(0);
        let time_count = stat
            .times
            .iter()
            .filter(|index| **index < group.bits)
            .count() as u32;
        unlocked = unlocked.saturating_add(bit_count.max(time_count).min(group.bits));
    }
    unlocked
}

// ---------------------------------------------------------------------------
// Binary VDF ("Bust") parser
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum BValue {
    Table(Vec<(String, BValue)>),
    String(String),
    Int32(i32),
    Int64(i64),
    UInt64(u64),
    Float32(f32),
    Bytes(Vec<u8>),
}

impl BValue {
    fn as_table(&self) -> Option<&[(String, BValue)]> {
        match self {
            BValue::Table(entries) => Some(entries),
            _ => None,
        }
    }
}

fn parse_document(data: &[u8]) -> Result<Vec<(String, BValue)>, String> {
    let mut parser = Parser {
        data,
        pos: 0,
        depth: 0,
    };
    let mut root = Vec::new();

    while !parser.eof() {
        let kind = parser.byte()?;
        if kind == TYPE_END {
            // Some Steam builds append more than one END marker after the root
            // entry. Anything else after a root END is a parse error.
            if parser.rest_is_end() {
                return Ok(root);
            }
            return Err("trailing data after root END".into());
        }
        let key = parser.cstring()?;
        let value = parser.value(kind)?;
        root.push((key, value));
    }

    Ok(root)
}

struct Parser<'a> {
    data: &'a [u8],
    pos: usize,
    depth: usize,
}

impl<'a> Parser<'a> {
    fn eof(&self) -> bool {
        self.pos >= self.data.len()
    }

    fn rest_is_end(&self) -> bool {
        self.data[self.pos..].iter().all(|byte| *byte == TYPE_END)
    }

    fn byte(&mut self) -> Result<u8, String> {
        let Some(byte) = self.data.get(self.pos).copied() else {
            return Err("unexpected end of file".into());
        };
        self.pos += 1;
        Ok(byte)
    }

    fn bytes(&mut self, len: usize) -> Result<&'a [u8], String> {
        let end = self
            .pos
            .checked_add(len)
            .ok_or_else(|| "length overflow".to_string())?;
        let slice = self
            .data
            .get(self.pos..end)
            .ok_or_else(|| "unexpected end of file".to_string())?;
        self.pos = end;
        Ok(slice)
    }

    fn cstring(&mut self) -> Result<String, String> {
        let start = self.pos;
        while self.pos < self.data.len() && self.data[self.pos] != 0 {
            self.pos += 1;
        }
        if self.pos >= self.data.len() {
            return Err("unterminated string".into());
        }
        let value = String::from_utf8_lossy(&self.data[start..self.pos]).into_owned();
        self.pos += 1;
        Ok(value)
    }

    fn wide_string(&mut self) -> Result<String, String> {
        let start = self.pos;
        loop {
            if self.pos + 1 >= self.data.len() {
                return Err("unterminated wide string".into());
            }
            if self.data[self.pos] == 0 && self.data[self.pos + 1] == 0 {
                break;
            }
            self.pos += 2;
        }
        let mut units = Vec::with_capacity((self.pos - start) / 2);
        for pair in self.data[start..self.pos].chunks_exact(2) {
            units.push(u16::from_le_bytes([pair[0], pair[1]]));
        }
        self.pos += 2;
        Ok(String::from_utf16_lossy(&units))
    }

    fn table(&mut self) -> Result<Vec<(String, BValue)>, String> {
        if self.depth >= MAX_DEPTH {
            return Err("binary VDF is nested too deeply".into());
        }
        self.depth += 1;
        let mut entries = Vec::new();

        loop {
            let kind = self.byte()?;
            if kind == TYPE_END {
                self.depth -= 1;
                return Ok(entries);
            }
            let key = self.cstring()?;
            let value = self.value(kind)?;
            entries.push((key, value));
        }
    }

    fn value(&mut self, kind: u8) -> Result<BValue, String> {
        match kind {
            TYPE_TABLE => Ok(BValue::Table(self.table()?)),
            TYPE_STRING => Ok(BValue::String(self.cstring()?)),
            TYPE_INT32 => {
                let bytes = self.bytes(4)?;
                Ok(BValue::Int32(i32::from_le_bytes(
                    bytes.try_into().map_err(|_| "short int32")?,
                )))
            }
            TYPE_FLOAT32 => {
                let bytes = self.bytes(4)?;
                Ok(BValue::Float32(f32::from_le_bytes(
                    bytes.try_into().map_err(|_| "short float32")?,
                )))
            }
            TYPE_POINTER | TYPE_COLOR => Ok(BValue::Bytes(self.bytes(4)?.to_vec())),
            TYPE_WSTRING => Ok(BValue::String(self.wide_string()?)),
            TYPE_UINT64 => {
                let bytes = self.bytes(8)?;
                Ok(BValue::UInt64(u64::from_le_bytes(
                    bytes.try_into().map_err(|_| "short uint64")?,
                )))
            }
            TYPE_INT64 => {
                let bytes = self.bytes(8)?;
                Ok(BValue::Int64(i64::from_le_bytes(
                    bytes.try_into().map_err(|_| "short int64")?,
                )))
            }
            other => Err(format!("unknown binary VDF type 0x{other:02x}")),
        }
    }
}

fn table_of<'a>(entries: &'a [(String, BValue)], key: &str) -> Option<&'a [(String, BValue)]> {
    entries
        .iter()
        .find(|(name, _)| name == key)
        .and_then(|(_, value)| value.as_table())
}

fn string_of<'a>(entries: &'a [(String, BValue)], key: &str) -> Option<&'a str> {
    entries.iter().find_map(|(name, value)| {
        if name == key {
            match value {
                BValue::String(value) => Some(value.as_str()),
                _ => None,
            }
        } else {
            None
        }
    })
}

fn int_of(entries: &[(String, BValue)], key: &str) -> Option<i64> {
    entries.iter().find_map(|(name, value)| {
        if name != key {
            return None;
        }
        match value {
            BValue::Int32(value) => Some(i64::from(*value)),
            BValue::Int64(value) => Some(*value),
            BValue::UInt64(value) => i64::try_from(*value).ok(),
            _ => None,
        }
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn fixture(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("achievements")
            .join(name)
    }

    fn fixture_summary(user: &str, schema: &str, appid: u32) -> AchievementSummary {
        summary_from_files(&fixture(user), &fixture(schema), appid)
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
            "kudos-achievements-{}-{}-{}",
            std::process::id(),
            unique,
            tag
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("temp dir");
        TempDir { path }
    }

    #[test]
    fn parses_totals_and_unlocks_from_fixture() {
        let summary = fixture_summary("stats-424242.bin", "schema-424242.bin", 424_242);
        assert!(summary.supported, "reason: {:?}", summary.reason);
        assert_eq!(summary.total, 5);
        assert_eq!(summary.unlocked, 3);
        assert_eq!(summary.reason, None);
    }

    #[test]
    fn empty_stats_file_reports_zero_unlocked() {
        let summary = fixture_summary("stats-empty.bin", "schema-424242.bin", 424_242);
        assert!(summary.supported);
        assert_eq!(summary.total, 5);
        assert_eq!(summary.unlocked, 0);
    }

    #[test]
    fn missing_user_stats_is_unsupported() {
        let summary = fixture_summary("does-not-exist.bin", "schema-424242.bin", 424_242);
        assert!(!summary.supported);
        assert_eq!(summary.total, 0);
        assert!(summary.reason.is_some());
    }

    #[test]
    fn case_sensitive_magic_is_enforced() {
        let summary = fixture_summary("stats-wrong-magic.bin", "schema-424242.bin", 424_242);
        assert!(!summary.supported);
        assert!(summary.reason.unwrap().contains("could not be read"));
    }

    #[test]
    fn corrupt_file_is_unsupported_without_panicking() {
        let summary = fixture_summary("stats-corrupt.bin", "schema-424242.bin", 424_242);
        assert!(!summary.supported);
        assert!(summary.reason.is_some());
    }

    #[test]
    fn schema_root_must_match_the_requested_appid() {
        let summary = fixture_summary("stats-424242.bin", "schema-424242.bin", 999_999);
        assert!(!summary.supported);
    }

    #[test]
    fn trailing_end_markers_are_tolerated() {
        let mut bytes = fs::read(fixture("stats-empty.bin")).unwrap();
        // Steam appends extra END markers; they must not make a valid file
        // look corrupt.
        bytes.extend_from_slice(&[TYPE_END, TYPE_END]);
        let stats = parse_user_stats(&bytes).expect("valid with extra ENDs");
        assert!(stats.is_empty());
    }

    #[test]
    fn cache_invalidates_when_the_stats_file_changes() {
        let dir = temp_dir("cache");
        let user = dir.path.join("UserGameStats_42_424242.bin");
        let schema = dir.path.join("UserGameStatsSchema_424242.bin");
        fs::copy(fixture("stats-424242.bin"), &user).unwrap();
        fs::copy(fixture("schema-424242.bin"), &schema).unwrap();

        let key = (dir.path.to_string_lossy().into_owned(), 42, 424_242);
        let first = summary_cached(&key, &user, &schema, 424_242);
        assert_eq!((first.unlocked, first.total), (3, 5));

        // Replace with the empty cache: length and/or mtime must invalidate.
        fs::copy(fixture("stats-empty.bin"), &user).unwrap();
        let second = summary_cached(&key, &user, &schema, 424_242);
        assert_eq!((second.unlocked, second.total), (0, 5));
    }

    #[test]
    fn real_steam_achievement_smoke() {
        let Some(_root) = crate::sources::steam::detect_steam_root() else {
            eprintln!("[kudos] achievement smoke test skipped: Steam not installed");
            return;
        };

        for id in ["steam:730", "steam:12150", "steam:413150"] {
            let summary = summary_for(id);
            eprintln!(
                "[kudos] achievements {id}: supported={} {}/{} reason={:?}",
                summary.supported, summary.unlocked, summary.total, summary.reason
            );
        }
    }
}
