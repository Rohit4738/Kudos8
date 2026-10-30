// Filter vocabulary used by Home and FilterBar. OWNER: Session D / I.
// Session I adds installed-state filters and persists the last choice.

import type { Game, Platform } from "./types";

export type Filter = "all" | "installed" | "not-installed" | Platform;

export interface FilterDef {
  id: Filter;
  label: string;
}

export type FilterCounts = Record<Filter, number>;

const FILTER_KEY = "kudos.libraryFilter";

/** Always-visible filters, including stores with no games (count 0). */
export const FILTERS: FilterDef[] = [
  { id: "all", label: "All" },
  { id: "installed", label: "Installed" },
  { id: "not-installed", label: "Not installed" },
];

export const PLATFORM_FILTERS: FilterDef[] = [
  { id: "steam", label: "Steam" },
  { id: "epic", label: "Epic" },
  { id: "gog", label: "GOG" },
];

export function matchesFilter(game: Pick<Game, "platform" | "isInstalled">, filter: Filter): boolean {
  if (filter === "all") return true;
  if (filter === "installed") return game.isInstalled;
  if (filter === "not-installed") return !game.isInstalled;
  return game.platform === filter;
}

export function countFilters(games: Pick<Game, "platform" | "isInstalled">[]): FilterCounts {
  const counts: FilterCounts = {
    all: games.length,
    installed: 0,
    "not-installed": 0,
    steam: 0,
    epic: 0,
    gog: 0,
  };

  for (const game of games) {
    if (game.isInstalled) counts.installed += 1;
    else counts["not-installed"] += 1;
    if (game.platform === "steam") counts.steam += 1;
    else if (game.platform === "epic") counts.epic += 1;
    else if (game.platform === "gog") counts.gog += 1;
  }

  return counts;
}

/** All six filters. Empty stores keep a 0 count and show a friendly empty state. */
export function visibleFilters(_counts?: FilterCounts): FilterDef[] {
  return [...FILTERS, ...PLATFORM_FILTERS];
}

/** The chosen filter stays selected even when its store currently has no games. */
export function resolveFilter(filter: Filter, _counts?: FilterCounts): Filter {
  return filter;
}

export function readStoredFilter(): Filter {
  try {
    const value = window.localStorage.getItem(FILTER_KEY);
    if (
      value === "all" ||
      value === "installed" ||
      value === "not-installed" ||
      value === "steam" ||
      value === "epic" ||
      value === "gog"
    ) {
      return value;
    }
  } catch {
    /* storage is best-effort */
  }
  return "all";
}

export function storeFilter(filter: Filter): void {
  try {
    window.localStorage.setItem(FILTER_KEY, filter);
  } catch {
    /* storage is best-effort */
  }
}
