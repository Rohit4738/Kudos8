// Filter vocabulary used by Home and FilterBar. OWNER: Session D.

import type { Platform } from "./types";

export type Filter = "all" | Platform;

export interface FilterDef {
  id: Filter;
  label: string;
}

export const FILTERS: FilterDef[] = [
  { id: "all", label: "All" },
  { id: "steam", label: "Steam" },
  { id: "epic", label: "Epic" },
  { id: "gog", label: "GOG" },
];

export type FilterCounts = Record<Filter, number>;

export function matchesFilter(
  platform: Platform,
  filter: Filter,
): boolean {
  return filter === "all" || platform === filter;
}
