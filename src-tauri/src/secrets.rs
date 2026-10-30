//! Encrypted secret store for API keys (Session M).
//!
//! Design rules:
//! - Values are encrypted with Windows DPAPI (`CryptProtectData`, current user
//!   scope) before they touch disk.
//! - The store lives at `%APPDATA%\Kudos\secrets.bin` and is replaced atomically
//!   (`secrets.bin.kudos-tmp` + rename).
//! - No plaintext value is ever logged or returned to the frontend. Commands only
//!   expose `hasKey` and a short masked suffix (`hint`).
//! - A corrupt or undecryptable entry is treated as "no key" instead of taking
//!   the Settings page down.
//!
//! The file is JSON with hex-encoded DPAPI blobs, which keeps it inspectable
//! without adding a base64 dependency:
//!
//! ```json
//! { "version": 1, "entries": { "steamgriddb": "01000000d08c9ddf..." } }
//! ```

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Providers this build knows about. Adding one here is enough for the generic
/// status/set/clear commands to work; no other code changes.
pub const PROVIDERS: [&str; 2] = [PROVIDER_STEAM, PROVIDER_STEAMGRIDDB];

pub const PROVIDER_STEAM: &str = "steam";
pub const PROVIDER_STEAMGRIDDB: &str = "steamgriddb";

const STORE_FILE: &str = "secrets.bin";
const STORE_VERSION: u32 = 1;
const MAX_KEY_LEN: usize = 256;
const MIN_KEY_LEN: usize = 8;

/// Public, frontend-safe view of one provider.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretStatus {
    pub has_key: bool,
    /// Masked suffix such as `••••a1b2`. Never the key itself.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}

impl SecretStatus {
    fn missing() -> Self {
        Self {
            has_key: false,
            hint: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoreFile {
    #[serde(default)]
    version: u32,
    #[serde(default)]
    entries: BTreeMap<String, String>,
}

impl Default for StoreFile {
    fn default() -> Self {
        Self {
            version: STORE_VERSION,
            entries: BTreeMap::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Paths + validation
// ---------------------------------------------------------------------------

/// `%APPDATA%\Kudos`, or `None` when the environment is unusable.
pub fn data_dir() -> Option<PathBuf> {
    std::env::var_os("APPDATA")
        .filter(|value| !value.is_empty())
        .map(|value| PathBuf::from(value).join("Kudos"))
}

pub fn store_path() -> Option<PathBuf> {
    data_dir().map(|dir| dir.join(STORE_FILE))
}

/// Providers are a fixed allow-list so a compromised frontend cannot create
/// arbitrary entries in the store.
pub fn validate_provider(provider: &str) -> Result<&'static str, String> {
    let normalized = provider.trim().to_ascii_lowercase();
    PROVIDERS
        .iter()
        .copied()
        .find(|known| *known == normalized)
        .ok_or_else(|| {
            format!(
                "Unknown secret provider \"{provider}\". Expected one of: {}",
                PROVIDERS.join(", ")
            )
        })
}

/// Trim and validate a pasted API key. The value is never included in errors.
pub fn normalize_key(value: &str) -> Result<String, String> {
    let key = value.trim().trim_matches('"').trim();
    if key.is_empty() {
        return Err("Paste an API key first.".to_string());
    }
    if key.len() < MIN_KEY_LEN {
        return Err(format!(
            "That key looks too short. API keys are at least {MIN_KEY_LEN} characters."
        ));
    }
    if key.len() > MAX_KEY_LEN {
        return Err(format!(
            "That key is too long. API keys are at most {MAX_KEY_LEN} characters."
        ));
    }
    if key.chars().any(|ch| ch.is_control()) {
        return Err("The API key contains control characters. Paste it again.".to_string());
    }
    Ok(key.to_string())
}

/// Masked suffix for the UI. Short keys show no characters at all, so the hint
/// can never reconstruct a key.
pub fn masked_hint(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    let suffix: String = trimmed
        .chars()
        .rev()
        .take(4)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    if trimmed.chars().count() < 12 {
        Some("••••".to_string())
    } else {
        Some(format!("••••{suffix}"))
    }
}

// ---------------------------------------------------------------------------
// Public API (default store path)
// ---------------------------------------------------------------------------

/// Whether a usable key exists. Never rejects: a missing/corrupt store is
/// simply "no key".
pub fn status(provider: &str) -> SecretStatus {
    match store_path() {
        Some(path) => status_at(&path, provider),
        None => SecretStatus::missing(),
    }
}

/// Encrypt and persist one key. Returns the new safe status.
pub fn set(provider: &str, value: &str) -> Result<SecretStatus, String> {
    let path = store_path().ok_or_else(|| {
        "Kudos could not find %APPDATA%. The key was not saved.".to_string()
    })?;
    set_at(&path, provider, value)
}

/// Remove one key. Returns the new safe status.
pub fn clear(provider: &str) -> Result<SecretStatus, String> {
    let path = store_path().ok_or_else(|| {
        "Kudos could not find %APPDATA%. Nothing was changed.".to_string()
    })?;
    clear_at(&path, provider)
}

/// Internal-only plaintext accessor for the HTTP client. Never expose this from
/// a Tauri command.
pub fn get(provider: &str) -> Option<String> {
    store_path().and_then(|path| get_at(&path, provider))
}

// ---------------------------------------------------------------------------
// Path-scoped implementation (used by tests with a temp file)
// ---------------------------------------------------------------------------

fn status_at(path: &Path, provider: &str) -> SecretStatus {
    let Ok(provider) = validate_provider(provider) else {
        return SecretStatus::missing();
    };
    match get_at(path, provider) {
        Some(key) => SecretStatus {
            has_key: true,
            hint: masked_hint(&key),
        },
        None => SecretStatus::missing(),
    }
}

fn set_at(path: &Path, provider: &str, value: &str) -> Result<SecretStatus, String> {
    let provider = validate_provider(provider)?;
    let key = normalize_key(value)?;
    let encrypted = protect(key.as_bytes())?;
    let mut store = load_store(path);
    store.version = STORE_VERSION;
    store.entries.insert(provider.to_string(), hex_encode(&encrypted));
    write_store(path, &store)?;
    Ok(SecretStatus {
        has_key: true,
        hint: masked_hint(&key),
    })
}

fn clear_at(path: &Path, provider: &str) -> Result<SecretStatus, String> {
    let provider = validate_provider(provider)?;
    let mut store = load_store(path);
    if store.entries.remove(provider).is_some() {
        write_store(path, &store)?;
    }
    Ok(SecretStatus::missing())
}

fn get_at(path: &Path, provider: &str) -> Option<String> {
    let store = load_store(path);
    let hex = store.entries.get(provider)?;
    let encrypted = hex_decode(hex)?;
    let plain = unprotect(&encrypted)?;
    String::from_utf8(plain).ok()
}

// ---------------------------------------------------------------------------
// Store I/O
// ---------------------------------------------------------------------------

fn load_store(path: &Path) -> StoreFile {
    let Ok(bytes) = fs::read(path) else {
        return StoreFile::default();
    };
    let mut store: StoreFile = serde_json::from_slice(&bytes).unwrap_or_default();
    if store.version == 0 {
        store.version = STORE_VERSION;
    }
    store
}

fn write_store(path: &Path, store: &StoreFile) -> Result<(), String> {
    let Some(parent) = path.parent() else {
        return Err(format!("{} has no parent directory", path.display()));
    };
    fs::create_dir_all(parent).map_err(|err| {
        format!(
            "Could not create the Kudos data folder ({}): {err}",
            parent.display()
        )
    })?;

    let json = serde_json::to_vec_pretty(store)
        .map_err(|err| format!("Could not encode the secret store: {err}"))?;
    let temp = with_suffix(path, ".kudos-tmp");
    fs::write(&temp, &json)
        .map_err(|err| format!("Could not write {}: {err}", temp.display()))?;
    if let Err(first_error) = fs::rename(&temp, path) {
        // Windows normally replaces an existing file atomically; be defensive on
        // filesystems that refuse that.
        let _ = fs::remove_file(path);
        fs::rename(&temp, path).map_err(|second_error| {
            let _ = fs::remove_file(&temp);
            format!(
                "Could not replace {} ({first_error}; retry: {second_error})",
                path.display()
            )
        })?;
    }
    Ok(())
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut os = path.as_os_str().to_os_string();
    os.push(suffix);
    PathBuf::from(os)
}

// ---------------------------------------------------------------------------
// Hex (no base64 dependency)
// ---------------------------------------------------------------------------

fn hex_encode(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(DIGITS[(byte >> 4) as usize] as char);
        out.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    out
}

fn hex_decode(text: &str) -> Option<Vec<u8>> {
    if text.len() % 2 != 0 {
        return None;
    }
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len() / 2);
    let mut index = 0;
    while index < bytes.len() {
        let high = hex_value(bytes[index])?;
        let low = hex_value(bytes[index + 1])?;
        out.push((high << 4) | low);
        index += 2;
    }
    Some(out)
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// DPAPI
// ---------------------------------------------------------------------------

#[cfg(windows)]
fn protect(plain: &[u8]) -> Result<Vec<u8>, String> {
    use std::ptr::{null, null_mut};

    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{
        CryptProtectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };

    let input = CRYPT_INTEGER_BLOB {
        cbData: plain.len() as u32,
        pbData: plain.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: null_mut(),
    };

    let ok = unsafe {
        CryptProtectData(
            &input,
            null(),
            null(),
            null(),
            null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
    };
    if ok == 0 {
        return Err(format!(
            "Windows could not encrypt the key: {}",
            std::io::Error::last_os_error()
        ));
    }
    if output.pbData.is_null() || output.cbData == 0 {
        return Err("Windows returned an empty encrypted value.".to_string());
    }

    let bytes = unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize) }.to_vec();
    unsafe {
        LocalFree(output.pbData.cast());
    }
    Ok(bytes)
}

#[cfg(windows)]
fn unprotect(encrypted: &[u8]) -> Option<Vec<u8>> {
    use std::ptr::{null, null_mut};

    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{
        CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };

    if encrypted.is_empty() {
        return None;
    }
    let input = CRYPT_INTEGER_BLOB {
        cbData: encrypted.len() as u32,
        pbData: encrypted.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: null_mut(),
    };

    let ok = unsafe {
        CryptUnprotectData(
            &input,
            null_mut(),
            null(),
            null(),
            null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
    };
    if ok == 0 || output.pbData.is_null() {
        return None;
    }
    let bytes = unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize) }.to_vec();
    unsafe {
        LocalFree(output.pbData.cast());
    }
    Some(bytes)
}

#[cfg(not(windows))]
fn protect(_plain: &[u8]) -> Result<Vec<u8>, String> {
    Err("Encrypted secret storage is only available on Windows.".to_string())
}

#[cfg(not(windows))]
fn unprotect(_encrypted: &[u8]) -> Option<Vec<u8>> {
    None
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

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
            "kudos-secrets-{}-{}-{}-{}",
            std::process::id(),
            stamp,
            unique,
            tag
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("temp dir");
        TempDir { path }
    }

    #[test]
    fn masked_hint_never_leaks_the_key() {
        let key = "0123456789abcdef0123456789abcdef";
        let hint = masked_hint(key).expect("hint");
        assert!(!hint.contains(key));
        assert_eq!(hint, "••••cdef");
        // No 5+ character run from the key survives.
        for window in key.as_bytes().windows(5) {
            let fragment = std::str::from_utf8(window).unwrap();
            assert!(!hint.contains(fragment), "hint leaked {fragment}");
        }

        let short = "abc12345";
        assert_eq!(masked_hint(short).as_deref(), Some("••••"));
        assert!(!masked_hint(short).unwrap().contains("abc"));
        assert_eq!(masked_hint("   "), None);
    }

    #[test]
    fn provider_validation_is_an_allow_list() {
        assert_eq!(validate_provider("steam").unwrap(), PROVIDER_STEAM);
        assert_eq!(
            validate_provider("  SteamGridDB ").unwrap(),
            PROVIDER_STEAMGRIDDB
        );
        assert!(validate_provider("steamgri db").is_err());
        assert!(validate_provider("").is_err());
    }

    #[test]
    fn key_validation_trims_quotes_and_rejects_junk() {
        assert_eq!(normalize_key("  abcdefgh1234  ").unwrap(), "abcdefgh1234");
        assert_eq!(
            normalize_key("\"abcdefgh1234\"").unwrap(),
            "abcdefgh1234"
        );
        assert!(normalize_key("").is_err());
        assert!(normalize_key("short").is_err());
        assert!(normalize_key(&"x".repeat(MAX_KEY_LEN + 1)).is_err());
        assert!(normalize_key("abcdefgh\u{0007}").is_err());
    }

    #[test]
    fn hex_round_trips_arbitrary_bytes() {
        let bytes: Vec<u8> = (0..=255).collect();
        let encoded = hex_encode(&bytes);
        assert_eq!(encoded.len(), 512);
        assert_eq!(hex_decode(&encoded).unwrap(), bytes);
        assert!(hex_decode("abc").is_none());
        assert!(hex_decode("zz").is_none());
    }

    #[test]
    fn store_file_never_contains_the_plaintext_key() {
        let dir = temp_dir("no-plaintext");
        let path = dir.path.join("secrets.bin");
        let key = "0123456789abcdef0123456789abcdef";
        set_at(&path, PROVIDER_STEAMGRIDDB, key).expect("set");
        let raw = fs::read(&path).expect("read store");
        let text = String::from_utf8_lossy(&raw);
        assert!(!text.contains(key));
        assert!(text.contains("steamgriddb"));
        assert!(!text.contains("hint"));
    }

    #[test]
    fn missing_store_reports_no_key_and_clear_is_idempotent() {
        let dir = temp_dir("missing");
        let path = dir.path.join("secrets.bin");
        let status = status_at(&path, PROVIDER_STEAM);
        assert!(!status.has_key);
        assert_eq!(status.hint, None);

        let status = clear_at(&path, PROVIDER_STEAM).expect("clear missing");
        assert!(!status.has_key);
        assert!(!path.exists());
    }

    #[test]
    fn corrupt_store_is_treated_as_empty() {
        let dir = temp_dir("corrupt");
        let path = dir.path.join("secrets.bin");
        fs::write(&path, b"this is not json").unwrap();
        assert!(!status_at(&path, PROVIDER_STEAMGRIDDB).has_key);
        // A later write repairs the file.
        set_at(&path, PROVIDER_STEAMGRIDDB, "abcdefgh1234").expect("set");
        assert!(status_at(&path, PROVIDER_STEAMGRIDDB).has_key);
    }

    #[cfg(windows)]
    #[test]
    fn dpapi_round_trip() {
        let plain = b"0123456789abcdef0123456789abcdef";
        let encrypted = protect(plain).expect("protect");
        assert_ne!(encrypted, plain);
        assert!(encrypted.len() > plain.len());
        let decrypted = unprotect(&encrypted).expect("unprotect");
        assert_eq!(decrypted, plain);
        // Tampering must fail closed rather than return garbage.
        let mut broken = encrypted.clone();
        let last = broken.len() - 1;
        broken[last] ^= 0xff;
        assert!(unprotect(&broken).is_none());
    }

    #[cfg(windows)]
    #[test]
    fn set_status_get_clear_round_trip() {
        let dir = temp_dir("roundtrip");
        let path = dir.path.join("secrets.bin");
        let key = "0123456789abcdef0123456789abcdef";

        let status = set_at(&path, PROVIDER_STEAMGRIDDB, key).expect("set");
        assert!(status.has_key);
        assert_eq!(status.hint.as_deref(), Some("••••cdef"));
        assert_eq!(get_at(&path, PROVIDER_STEAMGRIDDB).as_deref(), Some(key));

        // Updating replaces rather than appending.
        let second = "fedcba9876543210fedcba9876543210";
        set_at(&path, PROVIDER_STEAMGRIDDB, second).expect("update");
        assert_eq!(get_at(&path, PROVIDER_STEAMGRIDDB).as_deref(), Some(second));

        // Providers are independent.
        set_at(&path, PROVIDER_STEAM, "steam-key-123456").expect("steam");
        assert!(status_at(&path, PROVIDER_STEAM).has_key);
        assert!(status_at(&path, PROVIDER_STEAMGRIDDB).has_key);

        clear_at(&path, PROVIDER_STEAMGRIDDB).expect("clear");
        assert!(!status_at(&path, PROVIDER_STEAMGRIDDB).has_key);
        assert!(status_at(&path, PROVIDER_STEAM).has_key);
        assert!(get_at(&path, PROVIDER_STEAMGRIDDB).is_none());
    }

    #[test]
    fn atomic_write_leaves_no_temp_file() {
        let dir = temp_dir("atomic");
        let path = dir.path.join("secrets.bin");
        set_at(&path, PROVIDER_STEAM, "steam-key-123456").expect("set");
        assert!(path.is_file());
        assert!(!with_suffix(&path, ".kudos-tmp").exists());
    }
}
