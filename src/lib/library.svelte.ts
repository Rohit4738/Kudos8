// Cached library store shared by Home and GameDetail.
// OWNER: Session D. Talks to Tauri only through src/lib/api.ts.

import { listGames } from "./api";
import { DEV_GAMES, isDevMockEnabled } from "./devMock";
import { humanError } from "./format";
import type { Game } from "./types";

export type LibraryStatus = "idle" | "loading" | "ready" | "error";

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

function wait(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
