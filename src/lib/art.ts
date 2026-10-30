// Cover-art helpers: local file paths, premium gradient placeholders, initials.
// OWNER: Session D.

import { convertFileSrc } from "@tauri-apps/api/core";
import type { Game, Platform } from "./types";

/** Hue families so placeholder art still hints at the store. */
const PLATFORM_HUE: Record<Platform, number> = {
  steam: 208,
  epic: 268,
  gog: 318,
};

const STOP_WORDS = new Set([
  "the",
  "a",
  "an",
  "of",
  "and",
  "in",
  "to",
  "for",
  "at",
  "on",
]);

export function isTauriRuntime(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

/**
 * Backend covers can be https URLs or absolute local paths. Local paths need
 * Tauri's asset protocol; in a plain browser they simply fall back to art.
 */
export function coverSource(path?: string | null): string | null {
  if (!path) return null;
  const trimmed = path.trim();
  if (!trimmed) return null;
  if (/^(https?:|data:|blob:|asset:)/i.test(trimmed)) return trimmed;
  if (!isTauriRuntime()) return null;
  try {
    return convertFileSrc(trimmed);
  } catch {
    return null;
  }
}

/** Two uppercase characters for the placeholder cover. */
export function initials(title: string): string {
  const words = title
    .replace(/[^\p{L}\p{N}\s]/gu, " ")
    .split(/\s+/)
    .filter(Boolean);
  const meaningful = words.filter((w) => !STOP_WORDS.has(w.toLowerCase()));
  const picked = (meaningful.length > 0 ? meaningful : words).slice(0, 2);

  if (picked.length === 0) return "K";
  if (picked.length === 1) {
    return [...picked[0]].slice(0, 2).join("").toUpperCase();
  }
  return (
    [...picked[0]][0] + [...picked[1]][0]
  ).toUpperCase();
}

function hash(seed: string): number {
  let h = 2166136261;
  for (let i = 0; i < seed.length; i += 1) {
    h ^= seed.charCodeAt(i);
    h = Math.imul(h, 16777619);
  }
  return Math.abs(h);
}

/** A layered gradient unique to the game, tinted by its platform. */
export function placeholderBackground(game: Pick<Game, "id" | "title" | "platform">): string {
  const seed = hash(`${game.id}:${game.title}`);
  const base = PLATFORM_HUE[game.platform] ?? 220;
  const hue = (base + (seed % 34) - 17 + 360) % 360;
  const hue2 = (hue + 46 + (seed % 26)) % 360;

  return [
    `radial-gradient(120% 100% at 16% 0%, hsla(${hue}, 64%, 34%, 0.95) 0%, transparent 60%)`,
    `radial-gradient(110% 90% at 100% 100%, hsla(${hue2}, 58%, 26%, 0.9) 0%, transparent 58%)`,
    `linear-gradient(155deg, hsl(${hue} 36% 15%) 0%, hsl(${hue2} 42% 8%) 100%)`,
  ].join(", ");
}

export function platformDotColor(platform: Platform): string {
  switch (platform) {
    case "steam":
      return "#66c0f4";
    case "epic":
      return "#b487ff";
    case "gog":
      return "#c084fc";
  }
}

/** Steam's landscape hero art, derived from the stable id. */
export function steamHeroUrl(id: string): string | null {
  const appid = id.startsWith("steam:") ? id.slice(6) : "";
  return /^\d+$/.test(appid)
    ? `https://cdn.cloudflare.steamstatic.com/steam/apps/${appid}/library_hero.jpg`
    : null;
}

/**
 * Hero candidate chain, best first. Steam gets the real landscape `library_hero`
 * art; the portrait cover (often a local file) and the CDN header are graceful
 * offline fallbacks. The caller walks the list on image errors.
 */
export function heroCandidates(game: Game): string[] {
  const list: string[] = [];
  const cover = coverSource(game.coverImage);

  if (game.platform === "steam") {
    const hero = steamHeroUrl(game.id);
    if (hero) list.push(hero);
    if (cover) list.push(cover);
    const appid = game.id.startsWith("steam:") ? game.id.slice(6) : "";
    if (/^\d+$/.test(appid)) {
      list.push(`https://cdn.cloudflare.steamstatic.com/steam/apps/${appid}/header.jpg`);
    }
  } else if (cover) {
    list.push(cover);
  }

  return [...new Set(list)];
}

