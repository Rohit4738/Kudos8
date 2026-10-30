// Search overlay state + dependency-free fuzzy matching. OWNER: Session I.
//
// The matcher is deliberately small: normalized prefix/substring scores first,
// then an ordered-subsequence score with word-boundary and consecutive-run
// bonuses. No Fuse.js, no new dependency, no network.

import type { Game } from "./types";

export const searchState = $state<{ open: boolean }>({ open: false });

export function openSearch(): void {
  searchState.open = true;
}

export function closeSearch(): void {
  searchState.open = false;
}

function normalize(value: string): string {
  return value
    .normalize("NFD")
    .replace(/\p{Diacritic}/gu, "")
    .toLowerCase()
    .replace(/[^\p{L}\p{N}\s]/gu, " ")
    .replace(/\s+/g, " ")
    .trim();
}

function isBoundary(value: string, index: number): boolean {
  return index === 0 || value[index - 1] === " ";
}

/** Higher is better; null means "no match". Exported for tests/preview. */
export function fuzzyScore(query: string, title: string): number | null {
  const needle = normalize(query);
  if (!needle) return 0;
  const haystack = normalize(title);
  if (!haystack) return null;

  if (haystack.startsWith(needle)) {
    return 2000 - haystack.length;
  }

  const direct = haystack.indexOf(needle);
  if (direct >= 0) {
    const boundaryBonus = isBoundary(haystack, direct) ? 260 : 0;
    return 1200 + boundaryBonus - direct * 3 - haystack.length * 0.4;
  }

  // Ordered subsequence: every query character must appear in order.
  let score = 0;
  let cursor = 0;
  let previous = -2;
  let consecutive = 0;
  for (const char of needle) {
    if (char === " ") {
      consecutive = 0;
      continue;
    }
    const found = haystack.indexOf(char, cursor);
    if (found < 0) return null;
    score += 12;
    if (found === previous + 1) {
      consecutive += 1;
      score += 14 + consecutive * 3;
    } else {
      consecutive = 0;
    }
    if (isBoundary(haystack, found)) score += 24;
    previous = found;
    cursor = found + 1;
  }

  // Prefer compact matches in short titles.
  return score - haystack.length * 0.6 - previous * 0.4;
}

export interface SearchHit {
  game: Game;
  score: number;
}

export function searchGames(
  query: string,
  games: Game[],
  favorites: string[] = [],
  limit = 18,
): SearchHit[] {
  const favoriteIds = new Set(favorites);
  const hits: SearchHit[] = [];

  for (const game of games) {
    const score = fuzzyScore(query, game.title);
    if (score === null) continue;
    hits.push({ game, score: score + (favoriteIds.has(game.id) ? 30 : 0) });
  }

  hits.sort((a, b) => b.score - a.score || a.game.title.localeCompare(b.game.title));
  return hits.slice(0, limit);
}
