// Small presentation helpers shared by the library UI.
// OWNER: Session D.

import type { Game, Platform } from "./types";

const PLATFORM_SHORT: Record<Platform, string> = {
  steam: "Steam",
  epic: "Epic",
  gog: "GOG",
};

export function platformShort(platform: Platform): string {
  return PLATFORM_SHORT[platform];
}

/** "72.4 GB", "1.2 TB", "480 MB". Never throws on missing sizes. */
export function formatBytes(bytes?: number | null): string {
  if (bytes === undefined || bytes === null || !Number.isFinite(bytes) || bytes <= 0) {
    return "Size unknown";
  }
  if (bytes >= 1e12) return `${trimZero(bytes / 1e12)} TB`;
  if (bytes >= 1e9) return `${trimZero(bytes / 1e9)} GB`;
  if (bytes >= 1e6) return `${Math.max(1, Math.round(bytes / 1e6))} MB`;
  return `${Math.max(1, Math.round(bytes / 1e3))} KB`;
}

function trimZero(value: number): string {
  const fixed = value >= 10 ? value.toFixed(0) : value.toFixed(1);
  return fixed.endsWith(".0") ? fixed.slice(0, -2) : fixed;
}

/** The PS5-style "browse before you commit" state line. */
export function playState(game: Pick<Game, "isInstalled">): string {
  return game.isInstalled ? "Installed" : "In Library — not downloaded";
}

export function shortPlayState(game: Pick<Game, "isInstalled">): string {
  return game.isInstalled ? "Installed" : "Not downloaded";
}

/**
 * Tauri rejects commands with the `Display` string of the Rust error.
 * Strip the internal prefixes and keep the useful sentence.
 */
export function humanError(error: unknown): string {
  const raw =
    typeof error === "string"
      ? error
      : error instanceof Error
        ? error.message
        : "Something went wrong.";

  const text = raw
    .replace(/^source unavailable:\s*/i, "")
    .replace(/^game not found:\s*/i, "Game not found: ")
    .replace(/^game not launchable:\s*/i, "")
    .replace(/^invalid request:\s*/i, "")
    .replace(/^io error:\s*/i, "")
    .replace(/^not implemented yet:\s*/i, "Not available yet: ")
    .trim();

  return text.length > 0 ? text : "Something went wrong.";
}

/** Recognises the backend's "Steam is running, close it first" family of errors. */
export function isSteamRunningError(message: string): boolean {
  return /steam/i.test(message) && /(close|running|open|quit|exit)/i.test(message);
}
