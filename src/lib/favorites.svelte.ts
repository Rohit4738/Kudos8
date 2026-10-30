// Favorites: persisted ids shared by the library rows, tiles, detail page and
// the Settings list. OWNER: Session I.

const FAVORITES_KEY = "kudos.favorites";
const MAX_FAVORITES = 200;

interface FavoritesState {
  ids: string[];
}

function readFavorites(): string[] {
  try {
    const raw = window.localStorage.getItem(FAVORITES_KEY);
    if (!raw) return [];
    const parsed: unknown = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];
    return [
      ...new Set(
        parsed.filter((entry): entry is string => typeof entry === "string" && entry.length > 0),
      ),
    ].slice(0, MAX_FAVORITES);
  } catch {
    return [];
  }
}

export const favorites = $state<FavoritesState>({ ids: readFavorites() });

export function isFavorite(id: string): boolean {
  return favorites.ids.includes(id);
}

/** Newest first so the Favorites row opens with what you just starred. */
export function toggleFavorite(id: string): boolean {
  if (!id) return false;
  const exists = favorites.ids.includes(id);
  favorites.ids = exists
    ? favorites.ids.filter((entry) => entry !== id)
    : [id, ...favorites.ids].slice(0, MAX_FAVORITES);
  writeFavorites();
  return !exists;
}

export function removeFavorite(id: string): void {
  if (!isFavorite(id)) return;
  favorites.ids = favorites.ids.filter((entry) => entry !== id);
  writeFavorites();
}

export function clearFavorites(): void {
  favorites.ids = [];
  writeFavorites();
}

function writeFavorites(): void {
  try {
    window.localStorage.setItem(FAVORITES_KEY, JSON.stringify(favorites.ids));
  } catch {
    /* storage is best-effort */
  }
}
