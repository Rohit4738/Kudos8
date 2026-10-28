// Recently played: the additive wire type returned by `get_recent_games`.
// OWNER: Session G. Mirrors `src-tauri/src/steam_recent.rs`.
//
// This is deliberately not folded into `types.ts` / `models.rs`: `Game`,
// `Platform`, `LibrarySource` and every existing command stay untouched.

import type { Game } from "./types";

export type RecentSource = "steam" | "kudos";

export interface RecentGame {
  /** Stable namespaced id, e.g. "steam:730". Resolves against the current library. */
  id: string;
  /** Unix seconds. Steam's LastPlayed or the time Kudos launched the game. */
  lastPlayedUnix: number;
  /** Where the timestamp came from. Steam wins when both exist. */
  source: RecentSource;
}

/**
 * Merges any number of recent lists by id. Contract: Steam's `LastPlayed`
 * wins when present; within the same source the newest timestamp wins.
 */
export function mergeRecent(...groups: RecentGame[][]): RecentGame[] {
  const byId = new Map<string, RecentGame>();

  for (const group of groups) {
    for (const entry of group) {
      if (!entry.id || !Number.isFinite(entry.lastPlayedUnix) || entry.lastPlayedUnix <= 0) {
        continue;
      }
      const current = byId.get(entry.id);
      if (!current) {
        byId.set(entry.id, { ...entry });
        continue;
      }
      const steamBeatsKudos = current.source !== "steam" && entry.source === "steam";
      const sameSourceIsNewer =
        current.source === entry.source && entry.lastPlayedUnix > current.lastPlayedUnix;
      if (steamBeatsKudos || sameSourceIsNewer) {
        byId.set(entry.id, { ...entry });
      }
    }
  }

  return [...byId.values()].sort((a, b) => b.lastPlayedUnix - a.lastPlayedUnix);
}

/** "Just now", "2h ago", "3d ago" — short enough for a reduced-prominence caption. */
export function relativePlayed(unixSeconds: number, nowMs = Date.now()): string {
  if (!Number.isFinite(unixSeconds) || unixSeconds <= 0) return "";
  const then = unixSeconds * 1000;
  const minutes = Math.max(0, Math.round((nowMs - then) / 60_000));

  if (minutes < 2) return "Just now";
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.round(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.round(hours / 24);
  if (days < 7) return `${days}d ago`;
  const weeks = Math.round(days / 7);
  if (weeks < 5) return `${weeks}w ago`;

  try {
    return new Date(then).toLocaleDateString(undefined, {
      month: "short",
      day: "numeric",
    });
  } catch {
    return "";
  }
}

export interface ResolvedRecent {
  game: Game;
  entry: RecentGame;
}

/**
 * Resolves recent ids against the live library, skipping games that are no
 * longer present or no longer installed. Order is preserved (newest first).
 */
export function resolveRecent(entries: RecentGame[], games: Game[]): ResolvedRecent[] {
  const byId = new Map(games.map((game) => [game.id, game]));
  const seen = new Set<string>();
  const resolved: ResolvedRecent[] = [];

  for (const entry of entries) {
    if (seen.has(entry.id)) continue;
    const game = byId.get(entry.id);
    if (!game || !game.isInstalled) continue;
    seen.add(entry.id);
    resolved.push({ game, entry });
  }
  return resolved;
}
