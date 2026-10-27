//! Cross-source helpers. Owned by Session A; use freely from any source.

use std::path::Path;
use std::process::Command;

use crate::error::{SourceError, SourceResult};

/// Start a command produced by a source.
///
/// - An existing file (`.exe` path) is spawned directly.
/// - Anything else is handed to the Windows shell, which handles `steam://`,
///   `com.epicgames.launcher://`, and `goggalaxy://` style URIs.
pub fn open(command: &str) -> SourceResult<()> {
    let command = command.trim();
    if command.is_empty() {
        return Err(SourceError::not_launchable("empty launch command"));
    }

    let path = Path::new(command);
    let mut process = if path.is_file() {
        Command::new(path)
    } else {
        let mut cmd = Command::new("cmd");
        cmd.args(["/C", "start", "", command]);
        cmd
    };

    process
        .spawn()
        .map(|_| ())
        .map_err(|e| SourceError::io(format!("failed to launch '{command}': {e}")))
}
