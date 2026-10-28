//! Typed wrappers around the Tauri commands. OWNER: Session A.
//! Session D: import from here, never call `invoke` directly with string literals.

import { invoke } from "@tauri-apps/api/core";
import type { Game, SteamStatus } from "./types";
import type { RecentGame } from "./recent";

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
 * Recently played ids, newest first. Additive Session G command:
 * Steam `LastPlayed` merged with Kudos' own launch journal.
 * Never rejects; an empty list simply hides the row.
 */
export function getRecentGames(limit = 12): Promise<RecentGame[]> {
  return invoke<RecentGame[]>("get_recent_games", { limit });
}
