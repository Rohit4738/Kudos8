//! Shared data model. THIS FILE IS THE CONTRACT with the frontend (Session D).
//! Change it only together with `src/lib/types.ts`.

use serde::{Deserialize, Serialize};

/// The store a game belongs to. Serialized as lowercase: "steam" | "epic" | "gog".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    Steam,
    Epic,
    Gog,
}

impl Platform {
    pub fn as_str(self) -> &'static str {
        match self {
            Platform::Steam => "steam",
            Platform::Epic => "epic",
            Platform::Gog => "gog",
        }
    }
}

impl std::fmt::Display for Platform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A single installed/known game. Platform-agnostic by design.
///
/// `id` is globally unique and namespaced by the source, e.g.
/// `steam:730`, `epic:Fortnite`, `gog:1207658924`. Sources MUST keep this stable.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Game {
    /// Namespaced unique id: `<platform>:<store-id>`.
    pub id: String,
    pub platform: Platform,
    pub title: String,
    /// Absolute path to the executable, when the store exposes one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exe_path: Option<String>,
    /// What to execute to start the game. May be a `steam://rungameid/...` style URI.
    pub launch_command: String,
    pub is_installed: bool,
    /// Cover art: https URL or local file path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cover_image: Option<String>,
    /// Installed size in bytes, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
}

impl Game {
    /// Convenience constructor used by all sources.
    pub fn new(
        id: impl Into<String>,
        platform: Platform,
        title: impl Into<String>,
        launch_command: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            platform,
            title: title.into(),
            exe_path: None,
            launch_command: launch_command.into(),
            is_installed: true,
            cover_image: None,
            size_bytes: None,
        }
    }
}

/// Result of `get_steam_status`. Owned by Session B.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SteamStatus {
    /// Steam client is installed on this machine.
    pub installed: bool,
    /// Steam client is currently running.
    pub running: bool,
    /// Root of the Steam installation, when found.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub steam_path: Option<String>,
}

/// Local Steam achievement summary. Owned by Session F.
///
/// Additive to the original contract: `supported == false` means the UI must
/// render nothing (or a plain "not available" line) and `total`/`unlocked` are
/// both zero. No network request and no Steam API key are involved; the numbers
/// come from Steam's own `appcache/stats` files.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AchievementSummary {
    pub supported: bool,
    pub unlocked: u32,
    pub total: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl AchievementSummary {
    pub fn unsupported(reason: impl Into<String>) -> Self {
        Self {
            supported: false,
            unlocked: 0,
            total: 0,
            reason: Some(reason.into()),
        }
    }
}
