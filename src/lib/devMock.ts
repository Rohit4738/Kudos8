// Dev-only fake library. Explicit opt-in only: the real `list_games` command is
// always the default, including in a plain browser / `vite dev`.
//
// Enable:  open the app with `?mock=1`
// Disable: `?mock=0` (or just omit the flag)
//
// OWNER: Session D. Session E removed the automatic browser/dev default so a
// production build can never show fake games by accident.

import type { Game } from "./types";

interface DevGame
  extends Omit<Game, "coverImage" | "sizeBytes" | "exePath"> {
  coverImage?: string | null;
  sizeBytes?: number | null;
  exePath?: string | null;
}

const STEAM_ART = "https://cdn.akamai.steamstatic.com/steam/apps";
const cover = (appId: number): string => `${STEAM_ART}/${appId}/library_600x900.jpg`;

const GB = 1_000_000_000;

const SOURCE: DevGame[] = [
  {
    id: "steam:1091500",
    platform: "steam",
    title: "Cyberpunk 2077",
    launchCommand: "steam://rungameid/1091500",
    isInstalled: true,
    coverImage: cover(1091500),
    sizeBytes: 72.4 * GB,
    exePath: "C:\\Games\\Cyberpunk 2077\\bin\\x64\\Cyberpunk2077.exe",
  },
  {
    id: "epic:ControlUltimate",
    platform: "epic",
    title: "Control Ultimate Edition",
    launchCommand: "com.epicgames.launcher://apps/ControlUltimate?action=launch",
    isInstalled: true,
    coverImage: cover(870780),
    sizeBytes: 42.6 * GB,
  },
  {
    id: "steam:1245620",
    platform: "steam",
    title: "Elden Ring",
    launchCommand: "steam://rungameid/1245620",
    isInstalled: true,
    coverImage: cover(1245620),
    sizeBytes: 60.1 * GB,
  },
  {
    id: "gog:1207658924",
    platform: "gog",
    title: "The Witcher 3: Wild Hunt",
    launchCommand: "goggalaxy://openGameView/1207658924",
    isInstalled: true,
    coverImage: cover(292030),
    sizeBytes: 50.3 * GB,
  },
  {
    id: "epic:DeathStranding",
    platform: "epic",
    title: "Death Stranding Director's Cut",
    launchCommand: "com.epicgames.launcher://apps/DeathStranding?action=launch",
    isInstalled: true,
    coverImage: cover(1190460),
    sizeBytes: 79.8 * GB,
  },
  {
    id: "steam:1145360",
    platform: "steam",
    title: "Hades",
    launchCommand: "steam://rungameid/1145360",
    isInstalled: true,
    coverImage: cover(1145360),
    sizeBytes: 15.2 * GB,
  },
  {
    id: "gog:1423049311",
    platform: "gog",
    title: "Disco Elysium — The Final Cut",
    launchCommand: "goggalaxy://openGameView/1423049311",
    isInstalled: true,
    coverImage: cover(632470),
    sizeBytes: 22.1 * GB,
  },
  {
    id: "epic:AlanWake2",
    platform: "epic",
    title: "Alan Wake 2",
    launchCommand: "com.epicgames.launcher://apps/AlanWake2?action=launch",
    isInstalled: true,
    // Deliberately null: exercises the premium placeholder artwork.
    coverImage: null,
    sizeBytes: 85.4 * GB,
  },
  {
    id: "gog:1440077168",
    platform: "gog",
    title: "Heroes of Might & Magic III",
    launchCommand: "goggalaxy://openGameView/1440077168",
    isInstalled: false,
    coverImage: null,
  },
  {
    id: "steam:757310",
    platform: "steam",
    title: "Sable",
    launchCommand: "steam://rungameid/757310",
    isInstalled: false,
    coverImage: cover(757310),
    sizeBytes: 9.8 * GB,
  },
];

function normalize(game: DevGame): Game {
  return {
    ...game,
    coverImage: game.coverImage ?? undefined,
    sizeBytes: game.sizeBytes ?? undefined,
    exePath: game.exePath ?? undefined,
  };
}

export const DEV_GAMES: Game[] = SOURCE.map(normalize);

export function isDevMockEnabled(): boolean {
  if (typeof window === "undefined") return false;
  try {
    const flag = new URLSearchParams(window.location.search).get("mock");
    return flag === "1";
  } catch {
    /* no URL inspection available */
    return false;
  }
}
