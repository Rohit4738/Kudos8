//! Error type shared by every `LibrarySource`.
//!
//! It implements `Serialize`, so it can be returned directly from a Tauri command:
//! the frontend receives the `Display` string.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceError {
    /// A source could not be read (missing client, corrupt manifest, ...).
    Unavailable(String),
    /// The game id is unknown to this source.
    GameNotFound(String),
    /// The game exists but no launch command is usable.
    NotLaunchable(String),
    /// OS-level failure.
    Io(String),
    /// Still a stub, or the caller made an invalid request.
    Invalid(String),
    /// Deliberately left for another session.
    NotImplemented(String),
}

impl SourceError {
    pub fn unavailable(msg: impl Into<String>) -> Self {
        SourceError::Unavailable(msg.into())
    }
    pub fn game_not_found(id: impl Into<String>) -> Self {
        SourceError::GameNotFound(id.into())
    }
    pub fn not_launchable(msg: impl Into<String>) -> Self {
        SourceError::NotLaunchable(msg.into())
    }
    pub fn io(msg: impl Into<String>) -> Self {
        SourceError::Io(msg.into())
    }
    pub fn invalid(msg: impl Into<String>) -> Self {
        SourceError::Invalid(msg.into())
    }
    pub fn not_implemented(msg: impl Into<String>) -> Self {
        SourceError::NotImplemented(msg.into())
    }
}

impl fmt::Display for SourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SourceError::Unavailable(m) => write!(f, "source unavailable: {m}"),
            SourceError::GameNotFound(m) => write!(f, "game not found: {m}"),
            SourceError::NotLaunchable(m) => write!(f, "game not launchable: {m}"),
            SourceError::Io(m) => write!(f, "io error: {m}"),
            SourceError::Invalid(m) => write!(f, "invalid request: {m}"),
            SourceError::NotImplemented(m) => write!(f, "not implemented yet: {m}"),
        }
    }
}

impl std::error::Error for SourceError {}

impl serde::Serialize for SourceError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

pub type SourceResult<T> = Result<T, SourceError>;
