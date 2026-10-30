//! Typed wrappers around the Tauri commands. OWNER: Session A.
//! Session D: import from here, never call `invoke` directly with string literals.

import { invoke } from "@tauri-apps/api/core";
import type { AchievementSummary, Game, SteamStatus } from "./types";
import type { RecentGame } from "./recent";
import type { SteamPlaytime, SteamSyncStatus } from "./steam";

/** All games from Steam + Epic + GOG, merged and sorted by title. */
export function listGames(): Promise<Game[]> {
  return invoke<Game[]>("list_games");
}

/** Launch a game by its namespaced id. Rejects with a message on failure. */
export function launchGame(gameId: string): Promise<void> {
  return invoke<void>("launch_game", { gameId });
}

/** Add a game to the Steam library as a non-Steam shortcut. */
export function addToSteam(gameId: string): Promise<void> {
  return invoke<void>("add_to_steam", { gameId });
}

/** Steam client detection, for the Settings page. */
export function getSteamStatus(): Promise<SteamStatus> {
  return invoke<SteamStatus>("get_steam_status");
}

/**
 * Local Steam achievement count for one game. Never rejects for missing data:
 * unsupported platforms and unreadable caches come back as `supported: false`.
 */
export function getAchievements(gameId: string): Promise<AchievementSummary> {
  return invoke<AchievementSummary>("get_achievements", { gameId });
}

/**
 * Recently played ids, newest first. Additive Session G command:
 * Steam `LastPlayed` merged with Kudos' own launch journal.
 * Never rejects; an empty list simply hides the row.
 */
export function getRecentGames(limit = 12): Promise<RecentGame[]> {
  return invoke<RecentGame[]>("get_recent_games", { limit });
}

// ---------------------------------------------------------------------------
// Session H additive contract. These commands may not exist yet while H is
// still building the backend, so every caller must treat a rejection as
// "feature unavailable" and keep the installed-only library working.
// ---------------------------------------------------------------------------

/** Steam apps known from localconfig that are NOT installed on this PC. */
export function getSteamOwnedGames(): Promise<Game[]> {
  return invoke<Game[]>("get_steam_owned_games");
}

/** Per-app playtime keyed by the same "steam:<appid>" id as `Game`. */
export function getSteamPlaytime(): Promise<SteamPlaytime[]> {
  return invoke<SteamPlaytime[]>("get_steam_playtime");
}

/** Cache status for the Steam library card in Settings. */
export function getSteamSyncStatus(): Promise<SteamSyncStatus> {
  return invoke<SteamSyncStatus>("get_steam_sync_status");
}

/**
 * The app's only network call. User-triggered from Settings; never at startup.
 * Resolves with the fresh cache status or rejects with a user-readable message.
 */
export function syncSteamMetadata(): Promise<SteamSyncStatus> {
  return invoke<SteamSyncStatus>("sync_steam_metadata");
}

/**
 * True when Tauri rejected because the command is not registered. Used to
 * distinguish "H has not landed yet" from a real backend failure without
 * showing a raw IPC error to the user.
 */
export function isMissingCommand(error: unknown): boolean {
  const text =
    typeof error === "string"
      ? error
      : error instanceof Error
        ? error.message
        : "";
  return /command\b.*\b(not found|unknown|does not exist)|unknown command|not implemented/i.test(
    text,
  );
}

