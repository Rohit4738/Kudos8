// Recently played store. OWNER: Session G.
//
// Two layers, on purpose:
//  1. The backend (`get_recent_games`) is authoritative across sessions: it merges
//     Steam's own LastPlayed with Kudos' launch journal for every platform.
//  2. A small localStorage mirror (`kudos.recentLaunches`) keeps the row useful in
//     a plain browser preview and updates instantly after a launch, before the
//     next backend round-trip. The backend result always wins where both exist.
//
// Steam wins over the local mirror for the same id; see `mergeRecent`.

import { getRecentGames } from "./api";
import { isTauriRuntime } from "./art";
import { DEV_GAMES, isDevMockEnabled } from "./devMock";
import { humanError } from "./format";
import { mergeRecent, type RecentGame } from "./recent";

export type RecentStatus = "idle" | "loading" | "ready" | "error";

interface RecentState {
  entries: RecentGame[];
  status: RecentStatus;
  error: string | null;
  usingMock: boolean;
}

const LOCAL_KEY = "kudos.recentLaunches";
const LOCAL_LIMIT = 40;
const ROW_LIMIT = 12;

export const recent = $state<RecentState>({
  entries: [],
  status: "idle",
  error: null,
  usingMock: false,
});

let inFlight: Promise<void> | null = null;

function readLocalLaunches(): RecentGame[] {
  if (typeof window === "undefined") return [];
  try {
    const raw = window.localStorage.getItem(LOCAL_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw) as Record<string, unknown>;
    if (!parsed || typeof parsed !== "object") return [];

    return Object.entries(parsed).flatMap(([id, value]) => {
      if (typeof value !== "number" || !Number.isFinite(value) || value <= 0) return [];
      return [{ id, lastPlayedUnix: Math.floor(value), source: "kudos" as const }];
    });
  } catch {
    return [];
  }
}

function writeLocalLaunches(entries: RecentGame[]): void {
  if (typeof window === "undefined") return;
  try {
    const payload: Record<string, number> = {};
    for (const entry of mergeRecent(entries).slice(0, LOCAL_LIMIT)) {
      if (entry.source === "kudos") payload[entry.id] = entry.lastPlayedUnix;
    }
    window.localStorage.setItem(LOCAL_KEY, JSON.stringify(payload));
  } catch {
    /* storage is best-effort */
  }
}

/** A launch from this UI: mirror it locally so the row moves immediately. */
export function recordLaunch(gameId: string, whenMs = Date.now()): void {
  const timestamp = Math.floor(whenMs / 1000);
  if (!gameId || timestamp <= 0) return;

  const local = mergeRecent(readLocalLaunches(), [
    { id: gameId, lastPlayedUnix: timestamp, source: "kudos" },
  ]);
  writeLocalLaunches(local);

  recent.entries = mergeRecent(recent.entries, [
    { id: gameId, lastPlayedUnix: timestamp, source: "kudos" },
  ]).slice(0, ROW_LIMIT);
}

/** Demo-only history so `?mock=1` has something to show, mirroring devMock's philosophy. */
function demoEntries(): RecentGame[] {
  const now = Math.floor(Date.now() / 1000);
  return DEV_GAMES.filter((game) => game.isInstalled)
    .slice(0, 6)
    .map((game, index) => ({
      id: game.id,
      lastPlayedUnix: now - 900 - index * 5_400,
      source: game.platform === "steam" ? ("steam" as const) : ("kudos" as const),
    }));
}

export function loadRecent(force = false): Promise<void> {
  if (inFlight) return inFlight;
  if (recent.status === "ready" && !force) return Promise.resolve();

  recent.status = "loading";
  recent.error = null;

  inFlight = (async () => {
    if (isDevMockEnabled()) {
      recent.entries = demoEntries();
      recent.usingMock = true;
      recent.status = "ready";
      return;
    }

    let backend: RecentGame[] = [];
    let backendError: string | null = null;
    if (isTauriRuntime()) {
      try {
        backend = await getRecentGames(ROW_LIMIT);
      } catch (error) {
        backendError = humanError(error);
      }
    }

    recent.entries = mergeRecent(backend, readLocalLaunches()).slice(0, ROW_LIMIT);
    recent.usingMock = false;
    recent.error = backendError;
    recent.status = backendError ? "error" : "ready";
  })().finally(() => {
    inFlight = null;
  });

  return inFlight;
}
