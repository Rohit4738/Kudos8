// Optional Steam Web API support (Session L). Typed, feature-detected wrappers
// for the five additive commands. Nothing here throws into the UI: a missing
// command (older build), a browser preview or a backend error all resolve to a
// safe value, and the installed-only library keeps working.
//
// The key itself never comes back from Rust. `SteamApiStatus` only says whether
// one is saved, when the cache was refreshed and how many games it holds.

import { invoke } from "@tauri-apps/api/core";
import { isMissingCommand } from "./api";
import { isTauriRuntime } from "./art";
import type { Game } from "./types";

/** Mirrors `steam_api::SteamApiStatus`. No key material, by design. */
export interface SteamApiStatus {
  hasKey: boolean;
  /** Unix seconds of the last successful API sync, or null. */
  lastSync: number | null;
  /** Games in the cached API response (excluded non-games already removed). */
  ownedCount: number;
  /** Actionable message; omitted when healthy. */
  error?: string;
}

/** Six hours, matching the Rust cache TTL. */
export const STEAM_API_TTL_SECONDS = 6 * 60 * 60;

export const EMPTY_STEAM_API_STATUS: SteamApiStatus = {
  hasKey: false,
  lastSync: null,
  ownedCount: 0,
};

/** Steam Web API keys are exactly 32 hexadecimal characters. */
const KEY_PATTERN = /^[0-9a-fA-F]{32}$/;

/**
 * Client-side format check. Returns an actionable message, or null when the
 * value looks like a key. The backend validates again; this is only for instant
 * feedback before a round-trip.
 */
export function validateSteamKey(value: string): string | null {
  const trimmed = value.trim();
  if (!trimmed) return "Paste your Steam Web API key first.";
  if (trimmed.length !== 32) {
    return `A Steam key is 32 characters; this one is ${trimmed.length}. Copy the whole key from steamcommunity.com/dev/apikey.`;
  }
  if (!KEY_PATTERN.test(trimmed)) {
    return "A Steam key contains only 0-9 and A-F. Check for spaces or a missing character.";
  }
  return null;
}

/** "••••••••" when saved, "Not saved" otherwise. Never a real suffix. */
export function maskedKeyHint(status: SteamApiStatus | null): string {
  return status?.hasKey ? "••••••••••••••••••••••••••••••••" : "Not saved";
}

function offlineStatus(error?: string): SteamApiStatus {
  return { ...EMPTY_STEAM_API_STATUS, error };
}

function normalizeStatus(value: SteamApiStatus): SteamApiStatus {
  return {
    hasKey: Boolean(value?.hasKey),
    lastSync:
      typeof value?.lastSync === "number" && Number.isFinite(value.lastSync) && value.lastSync > 0
        ? value.lastSync
        : null,
    ownedCount:
      typeof value?.ownedCount === "number" && Number.isFinite(value.ownedCount)
        ? Math.max(0, Math.round(value.ownedCount))
        : 0,
    ...(value?.error ? { error: String(value.error) } : {}),
  };
}

/**
 * Local status: DPAPI read + cache read, no network. `null` means the command
 * is not registered in this build (older Rust side).
 */
export async function getSteamApiStatus(): Promise<SteamApiStatus | null> {
  if (!isTauriRuntime()) return null;
  try {
    return normalizeStatus(await invoke<SteamApiStatus>("get_steam_api_status"));
  } catch (error) {
    if (isMissingCommand(error)) return null;
    return offlineStatus(
      "Couldn't read the saved Steam key. The Windows vault may be unavailable.",
    );
  }
}

/** Save a key and immediately try one sync. Never throws. */
export async function setSteamApiKey(value: string): Promise<SteamApiStatus> {
  const formatError = validateSteamKey(value);
  if (formatError) return offlineStatus(formatError);
  if (!isTauriRuntime()) {
    return offlineStatus("Saving a Steam key is available in the desktop app.");
  }
  try {
    return normalizeStatus(
      await invoke<SteamApiStatus>("set_steam_api_key", { value: value.trim() }),
    );
  } catch (error) {
    if (isMissingCommand(error)) {
      return offlineStatus("Steam API support isn't available in this build yet.");
    }
    return offlineStatus(
      "Couldn't save the key. Check that Kudos can write to %APPDATA%\\Kudos.",
    );
  }
}

/** Forget the key and the cached API library. Never throws. */
export async function clearSteamApiKey(): Promise<SteamApiStatus> {
  if (!isTauriRuntime()) {
    return offlineStatus("Removing a Steam key is available in the desktop app.");
  }
  try {
    return normalizeStatus(await invoke<SteamApiStatus>("clear_steam_api_key"));
  } catch (error) {
    if (isMissingCommand(error)) {
      return offlineStatus("Steam API support isn't available in this build yet.");
    }
    return offlineStatus("Couldn't remove the saved key. Close Kudos and try again.");
  }
}

/**
 * Explicit refresh, bypassing the 6 h TTL. User-triggered only (Save or Test).
 * Never called on app startup.
 */
export async function syncSteamOwned(): Promise<SteamApiStatus> {
  if (!isTauriRuntime()) {
    return offlineStatus("Steam API sync is available in the desktop app.");
  }
  try {
    return normalizeStatus(await invoke<SteamApiStatus>("sync_steam_owned"));
  } catch (error) {
    if (isMissingCommand(error)) {
      return offlineStatus("Steam API support isn't available in this build yet.");
    }
    return offlineStatus(
      "Steam didn't answer. Check your connection and try again in a moment.",
    );
  }
}

/**
 * The cached complete owned list. Reads the cache only, never the network.
 * `[]` when no key/cache/command exists, so callers can merge unconditionally.
 */
export async function getSteamOwnedApiGames(): Promise<Game[]> {
  if (!isTauriRuntime()) return [];
  try {
    const games = await invoke<Game[]>("get_steam_owned_api_games");
    return Array.isArray(games) ? games : [];
  } catch {
    return [];
  }
}

/** True when a status is older than the 6 h TTL (or has never synced). */
export function isStale(status: SteamApiStatus | null, nowMs = Date.now()): boolean {
  if (!status?.hasKey) return false;
  if (!status.lastSync) return true;
  const seconds = Math.floor(nowMs / 1000);
  return seconds - status.lastSync >= STEAM_API_TTL_SECONDS;
}

/** "2 hours ago", "Never", tolerating seconds or milliseconds. */
export function lastSyncLabel(lastSync: number | null | undefined): string {
  if (!lastSync || !Number.isFinite(lastSync) || lastSync <= 0) return "Never synced";
  const seconds = lastSync > 1_000_000_000_000 ? Math.floor(lastSync / 1000) : lastSync;
  const minutes = Math.max(0, Math.round((Date.now() / 1000 - seconds) / 60));
  if (minutes < 1) return "Just now";
  if (minutes < 60) return `${minutes} min ago`;
  const hours = Math.round(minutes / 60);
  if (hours < 24) return `${hours} ${hours === 1 ? "hour" : "hours"} ago`;
  const days = Math.round(hours / 24);
  return `${days} ${days === 1 ? "day" : "days"} ago`;
}
