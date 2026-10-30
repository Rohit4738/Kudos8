// Shell focus zones. OWNER: Session O (controller polish).
//
// The shell has two controller zones: the page (Home rows, Settings cards, ...)
// and the left rail. Pages own the "page" zone; App owns the "rail" zone. Both
// read this store so a d-pad press never drives two things at once.

export type ShellZone = "page" | "rail";

export const RAIL_ITEM_COUNT = 3;

interface ShellFocusState {
  zone: ShellZone;
  /** Rail index: 0 Library, 1 Search, 2 Settings. */
  index: number;
}

export const shellFocus = $state<ShellFocusState>({ zone: "page", index: 0 });

export function enterRail(index = 0): void {
  shellFocus.index = Math.max(0, Math.min(RAIL_ITEM_COUNT - 1, index));
  shellFocus.zone = "rail";
}

export function exitRail(): void {
  shellFocus.zone = "page";
}

export function moveRail(delta: 1 | -1): void {
  shellFocus.index =
    (shellFocus.index + delta + RAIL_ITEM_COUNT) % RAIL_ITEM_COUNT;
}
