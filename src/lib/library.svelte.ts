// Cached library store shared by Home and GameDetail.
// OWNER: Session D. Talks to Tauri only through src/lib/api.ts.

import { listGames } from "./api";
import { DEV_GAMES, isDevMockEnabled } from "./devMock";
import { humanError } from "./format";
import type { Game } from "./types";

export type LibraryStatus = "idle" | "loading" | "ready" | "error";
export type SortMode = "title" | "platform" | "installed";

const SORT_KEY = "kudos.sort";
const HIDDEN_KEY = "kudos.hiddenGames";

export const SORT_OPTIONS: { value: SortMode; label: string }[] = [
  { value: "title", label: "Title" },
  { value: "platform", label: "Platform" },
  { value: "installed", label: "Installed first" },
];

interface LibraryState {
  games: Game[];
  status: LibraryStatus;
  error: string | null;
  usingMock: boolean;
}

export const library = $state<LibraryState>({
  games: [],
  status: "idle",
  error: null,
  usingMock: false,
});

let inFlight: Promise<void> | null = null;

interface RawGame extends Omit<Game, "coverImage" | "sizeBytes" | "exePath"> {
  coverImage?: string | null;
  sizeBytes?: number | null;
  exePath?: string | null;
}

function normalize(raw: Game | RawGame): Game {
  return {
    ...raw,
    coverImage: raw.coverImage ?? undefined,
    sizeBytes: raw.sizeBytes ?? undefined,
    exePath: raw.exePath ?? undefined,
  };
}

export function loadLibrary(force = false): Promise<void> {
  if (inFlight) return inFlight;
  if (library.status === "ready" && !force) return Promise.resolve();

  library.status = "loading";
  library.error = null;

  inFlight = (async () => {
    if (isDevMockEnabled()) {
      await wait(260);
      library.games = DEV_GAMES.map(normalize);
      library.usingMock = true;
      library.status = "ready";
      return;
    }

    try {
      const games = await listGames();
      library.games = games.map(normalize);
      library.usingMock = false;
      library.status = "ready";
    } catch (error) {
      library.games = [];
      library.usingMock = false;
      library.status = "error";
      library.error = humanError(error);
    }
  })().finally(() => {
    inFlight = null;
  });

  return inFlight;
}

export function findGame(id: string): Game | null {
  return library.games.find((game) => game.id === id) ?? null;
}

// ---------------------------------------------------------------------------
// Persisted library preferences: sort order + hidden games. Hidden games stay
// in `library.games` (so detail pages and Settings can still name them) and are
// filtered by Home only.
// ---------------------------------------------------------------------------

export const libraryPrefs = $state<{ sort: SortMode; hidden: string[] }>({
  sort: readSort(),
  hidden: readHidden(),
});

export function setSortMode(mode: SortMode): void {
  libraryPrefs.sort = mode;
  try {
    window.localStorage.setItem(SORT_KEY, mode);
  } catch {
    /* storage is best-effort */
  }
}

export function isHidden(id: string): boolean {
  return libraryPrefs.hidden.includes(id);
}

export function hideGame(id: string): void {
  if (isHidden(id)) return;
  libraryPrefs.hidden = [...libraryPrefs.hidden, id];
  writeHidden();
}

export function unhideGame(id: string): void {
  if (!isHidden(id)) return;
  libraryPrefs.hidden = libraryPrefs.hidden.filter((entry) => entry !== id);
  writeHidden();
}

export function unhideAll(): void {
  libraryPrefs.hidden = [];
  writeHidden();
}

/** Sort a copy; `title` order is the backend default, kept stable. */
export function sortGames(games: Game[], mode: SortMode): Game[] {
  const sorted = [...games];
  if (mode === "installed") {
    sorted.sort(
      (a, b) =>
        Number(b.isInstalled) - Number(a.isInstalled) || titleOrder(a, b),
    );
  } else if (mode === "platform") {
    sorted.sort(
      (a, b) => PLATFORM_ORDER[a.platform] - PLATFORM_ORDER[b.platform] || titleOrder(a, b),
    );
  } else {
    sorted.sort(titleOrder);
  }
  return sorted;
}

const PLATFORM_ORDER: Record<Game["platform"], number> = {
  steam: 0,
  epic: 1,
  gog: 2,
};

function titleOrder(a: Game, b: Game): number {
  return a.title.localeCompare(b.title, undefined, { sensitivity: "base" });
}

function readSort(): SortMode {
  try {
    const value = window.localStorage.getItem(SORT_KEY);
    if (value === "platform" || value === "installed" || value === "title") {
      return value;
    }
  } catch {
    /* storage is best-effort */
  }
  return "title";
}

function readHidden(): string[] {
  try {
    const raw = window.localStorage.getItem(HIDDEN_KEY);
    if (!raw) return [];
    const parsed: unknown = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];
    return parsed.filter((entry): entry is string => typeof entry === "string");
  } catch {
    return [];
  }
}

function writeHidden(): void {
  try {
    window.localStorage.setItem(HIDDEN_KEY, JSON.stringify(libraryPrefs.hidden));
  } catch {
    /* storage is best-effort */
  }
}

function wait(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
