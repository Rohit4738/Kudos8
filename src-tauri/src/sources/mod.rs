//! Platform sources. Each store implements [`LibrarySource`].
//!
//! Ownership:
//! - `steam.rs` -> Session B
//! - `epic.rs`  -> Session C
//! - `gog.rs`   -> Session C
//!
//! Session A owns `mod.rs` (the trait + registry). Do not change the trait signature
//! without updating TARGET.md and notifying the team.

pub mod epic;
pub mod gog;
pub mod steam;

use crate::error::SourceResult;
use crate::models::{Game, Platform};

/// One store integration.
///
/// Implementations must be cheap to construct and safe to share across threads.
pub trait LibrarySource: Send + Sync {
    /// Which store this source speaks for.
    fn platform(&self) -> Platform;

    /// Every game this store knows about, installed or not.
    ///
    /// Return `Ok(Vec::new())` when the store client is absent - that is not an error.
    /// Return `Err(SourceError::Unavailable)` when the store exists but cannot be read.
    fn installed(&self) -> SourceResult<Vec<Game>>;

    /// Start `game`. Must verify the game belongs to this platform.
    fn launch(&self, game: &Game) -> SourceResult<()>;
}
