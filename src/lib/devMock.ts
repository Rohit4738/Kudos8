// Dev-only fake library. Explicit opt-in only: the real `list_games` command is
// always the default, including in a plain browser / `vite dev`.
//
// Enable:  open the app with `?mock=1`
// Disable: `?mock=0` (or just omit the flag)
//
// OWNER: Session D. Session E removed the automatic browser/dev default so a
// production build can never show fake games by accident.

import type { Game } from "./types";
import type { SteamPlaytime, SteamSyncStatus } from "./steam";

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

// ---------------------------------------------------------------------------
// Session I mock: owned-but-not-installed Steam games + playtime + sync status.
// These mirror the frozen H contract so the whole UI is reviewable before the
// Rust commands land. Nothing here is used outside `?mock=1`.
// ---------------------------------------------------------------------------

const OWNED_SOURCE: DevGame[] = [
  ["620", "Portal 2", 13.0],
  ["570", "Dota 2", 42.0],
  ["440", "Team Fortress 2", 30.0],
  ["271590", "Grand Theft Auto V", 105.0],
  ["489830", "The Elder Scrolls V: Skyrim Special Edition", 55.0],
  ["105600", "Terraria", 4.0],
  ["1174180", "Red Dead Redemption 2", 115.0],
  ["1086940", "Baldur's Gate 3", 150.0],
  ["2358720", "Black Myth: Wukong", 130.0],
  ["413150", "Stardew Valley", 8.0],
  ["367520", "Hollow Knight", 9.0],
  ["391540", "Undertale", 2.0],
].map(([appid, title, size]) => ({
  id: `steam:${appid}`,
  platform: "steam" as const,
  title: title as string,
  launchCommand: `steam://install/${appid}`,
  isInstalled: false,
  coverImage: cover(Number(appid)),
  sizeBytes: (size as number) * GB,
}));

export const DEV_OWNED_GAMES: Game[] = OWNED_SOURCE.map(normalize);

const NOW = Math.floor(Date.now() / 1000);
const DAY = 86_400;

export const DEV_PLAYTIME: SteamPlaytime[] = [
  { appId: "steam:1091500", minutes: 2520, twoWeeksMinutes: 168, lastPlayed: NOW - 3 * DAY },
  { appId: "steam:1245620", minutes: 7200, twoWeeksMinutes: 240, lastPlayed: NOW - 26 * 3600 },
  { appId: "steam:1145360", minutes: 930, twoWeeksMinutes: 0, lastPlayed: NOW - 5 * DAY },
  { appId: "steam:757310", minutes: 75, twoWeeksMinutes: 0, lastPlayed: NOW - 20 * DAY },
  { appId: "epic:ControlUltimate", minutes: 1320, twoWeeksMinutes: 90, lastPlayed: NOW - 8 * DAY },
  { appId: "epic:DeathStranding", minutes: 360, twoWeeksMinutes: 0, lastPlayed: NOW - 12 * DAY },
  { appId: "gog:1207658924", minutes: 5400, twoWeeksMinutes: 0, lastPlayed: NOW - 40 * DAY },
  { appId: "gog:1423049311", minutes: 780, twoWeeksMinutes: 60, lastPlayed: NOW - 6 * DAY },
  { appId: "steam:620", minutes: 300, twoWeeksMinutes: 0, lastPlayed: NOW - 60 * DAY },
  { appId: "steam:271590", minutes: 1800, twoWeeksMinutes: 0, lastPlayed: NOW - 90 * DAY },
  { appId: "steam:1086940", minutes: 4500, twoWeeksMinutes: 300, lastPlayed: NOW - 2 * DAY },
  { appId: "steam:413150", minutes: 1200, twoWeeksMinutes: 0, lastPlayed: NOW - 14 * DAY },
];

export const DEV_SYNC_STATUS: SteamSyncStatus = {
  cached: true,
  appCount: 72,
  fetchedAt: NOW - 7200,
  ownedAppCount: 13,
};

/** Mock "Sync now": pretend the network round-trip succeeded just now. */
export function mockSyncStatus(): SteamSyncStatus {
  return {
    cached: false,
    appCount: DEV_SYNC_STATUS.appCount,
    fetchedAt: Math.floor(Date.now() / 1000),
    ownedAppCount: DEV_SYNC_STATUS.ownedAppCount,
  };
}

/** `?mock=1&empty=1` exercises the first-run onboarding state. */
export function isDevEmpty(): boolean {
  if (!isDevMockEnabled()) return false;
  try {
    return new URLSearchParams(window.location.search).get("empty") === "1";
  } catch {
    return false;
  }
}

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
