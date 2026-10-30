// Steam library helpers for the Session H additive contract.
// OWNER: Session I. Mirrors the frozen wire shapes from the H brief:
//   get_steam_owned_games -> Game[] (owned but not installed, platform "steam")
//   get_steam_playtime    -> SteamPlaytime[]
//   get_steam_sync_status -> SteamSyncStatus
//
// Nothing here talks to Tauri; `api.ts` owns the command wrappers and the
// library store owns the graceful fallback when H's commands are absent.

import { relativePlayed } from "./recent";
import type { Game } from "./types";

export interface SteamPlaytime {
  /** "steam:<appid>", identical to the Game id. */
  appId: string;
  minutes: number;
  twoWeeksMinutes: number;
  /** Unix seconds, or null when Steam has never recorded a session. */
  lastPlayed: number | null;
}

export interface SteamSyncStatus {
  cached: boolean;
  appCount: number;
  /** Unix seconds (or ms, tolerated) when the metadata was fetched. */
  fetchedAt: number | null;
  ownedAppCount: number;
}

export type SteamSupport = "unknown" | "ready" | "fallback" | "unavailable";

/**
 * Merge installed games with Steam-owned games. Installed wins for the same id
 * (the installed copy has the executable and the real size). Order is stable:
 * installed first, then owned additions, both title-sorted by the caller.
 */
export function mergeOwnedGames(installed: Game[], owned: Game[]): Game[] {
  const byId = new Map<string, Game>();
  for (const game of installed) byId.set(game.id, game);

  for (const game of owned) {
    if (byId.has(game.id)) continue;
    byId.set(game.id, {
      ...game,
      platform: game.platform ?? "steam",
      isInstalled: false,
      launchCommand: game.launchCommand || `steam://install/${game.id.slice(6)}`,
    });
  }

  return [...byId.values()];
}

/** A Steam title the account owns but this PC has not downloaded. */
export function isSteamOwnedGame(game: Pick<Game, "platform" | "isInstalled">): boolean {
  return game.platform === "steam" && !game.isInstalled;
}

/** The primary action label for a game (Play / Install / Details). */
export function primaryActionLabel(game: Pick<Game, "platform" | "isInstalled">): string {
  if (game.isInstalled) return "Play";
  if (isSteamOwnedGame(game)) return "Install";
  return "Not installed";
}

/** "42 h", "45 min", "" for unknown/zero. Never returns "0 h". */
export function playtimeLabel(minutes?: number | null): string {
  if (minutes === undefined || minutes === null || !Number.isFinite(minutes) || minutes <= 0) {
    return "";
  }
  if (minutes < 60) return `${Math.max(1, Math.round(minutes))} min`;
  const hours = minutes / 60;
  const rounded = hours >= 10 ? Math.round(hours) : Math.round(hours * 10) / 10;
  return `${rounded} h`;
}

/** "42 h played · last played 3 days ago" (either half may be absent). */
export function playtimeSummary(
  minutes?: number | null,
  lastPlayed?: number | null,
): string {
  const parts: string[] = [];
  const played = playtimeLabel(minutes);
  if (played) parts.push(`${played} played`);
  if (lastPlayed && Number.isFinite(lastPlayed) && lastPlayed > 0) {
    parts.push(`last played ${relativeLong(lastPlayed)}`);
  }
  return parts.join(" · ");
}

/** Longer, human wording for the detail page and hero. */
export function relativeLong(unixSeconds: number, nowMs = Date.now()): string {
  if (!Number.isFinite(unixSeconds) || unixSeconds <= 0) return "";
  const minutes = Math.max(0, Math.round((nowMs - unixSeconds * 1000) / 60_000));
  if (minutes < 2) return "just now";
  if (minutes < 60) return `${minutes} minutes ago`;
  const hours = Math.round(minutes / 60);
  if (hours < 24) return `${hours} ${hours === 1 ? "hour" : "hours"} ago`;
  const days = Math.round(hours / 24);
  if (days === 1) return "yesterday";
  if (days < 30) return `${days} days ago`;
  const months = Math.round(days / 30);
  if (months < 12) return `${months} ${months === 1 ? "month" : "months"} ago`;
  try {
    return new Date(unixSeconds * 1000).toLocaleDateString(undefined, {
      year: "numeric",
      month: "short",
      day: "numeric",
    });
  } catch {
    return "";
  }
}

/** Sync card wording. Tolerates seconds or milliseconds from the backend. */
export function syncFetchedLabel(fetchedAt: number | null | undefined): string {
  if (!fetchedAt || !Number.isFinite(fetchedAt) || fetchedAt <= 0) {
    return "Never synced";
  }
  const seconds = fetchedAt > 1_000_000_000_000 ? Math.floor(fetchedAt / 1000) : fetchedAt;
  return relativePlayed(seconds);
}

export function toPlaytimeMap(entries: SteamPlaytime[]): Record<string, SteamPlaytime> {
  const map: Record<string, SteamPlaytime> = {};
  for (const entry of entries) {
    if (!entry?.appId || !Number.isFinite(entry.minutes)) continue;
    map[entry.appId] = entry;
  }
  return map;
}
