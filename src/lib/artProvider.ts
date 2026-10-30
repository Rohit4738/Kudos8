// SteamGridDB artwork + secure API-key storage (Session M).
//
// Typed, feature-detected wrappers around the additive Tauri commands. Nothing
// here throws into the UI: a missing command (older build), a browser preview or
// a backend error all resolve to a safe value, and the existing local art
// fallback keeps working.

import { invoke } from "@tauri-apps/api/core";
import { isTauriRuntime } from "./art";

export type ArtKind = "grid" | "hero" | "logo" | "icon";

export const ART_KINDS: { kind: ArtKind; label: string; ratio: string }[] = [
  { kind: "grid", label: "Grid", ratio: "600 x 900" },
  { kind: "hero", label: "Hero", ratio: "1920 x 620" },
  { kind: "logo", label: "Logo", ratio: "transparent" },
  { kind: "icon", label: "Icon", ratio: "square" },
];

export interface ArtResult {
  id: string;
  kind: ArtKind;
  url: string;
  thumbUrl: string;
  width: number;
  height: number;
  author?: string;
}

export interface ArtStatus {
  grid?: string;
  hero?: string;
  logo?: string;
  icon?: string;
  source: "steamgriddb" | "local" | "none";
  /** Quiet auto-art failure recorded by the backend, if any. */
  error?: string;
}

export interface ApplyArtResult {
  localPath: string;
  wroteToSteam: boolean;
  steamError?: string;
}

export interface SecretStatus {
  hasKey: boolean;
  hint?: string;
}

export type SecretProvider = "steam" | "steamgriddb";

const EMPTY_STATUS: ArtStatus = { source: "none" };

function unavailable(error: unknown): boolean {
  const message = typeof error === "string" ? error : String(error ?? "");
  return (
    message.includes("not found") ||
    message.includes("unknown command") ||
    message.includes("__TAURI") ||
    message.includes("Command") && message.includes("not allowed")
  );
}

/** Frontend-safe status for one provider. Never returns key material. */
export async function getSecretStatus(
  provider: SecretProvider,
): Promise<SecretStatus> {
  if (!isTauriRuntime()) return { hasKey: false };
  try {
    return await invoke<SecretStatus>("get_secret_status", { provider });
  } catch {
    return { hasKey: false };
  }
}

export async function setSecret(
  provider: SecretProvider,
  value: string,
): Promise<SecretStatus> {
  return invoke<SecretStatus>("set_secret", { provider, value });
}

export async function clearSecret(
  provider: SecretProvider,
): Promise<SecretStatus> {
  return invoke<SecretStatus>("clear_secret", { provider });
}

/** Candidate artwork for one game. Rejects with an actionable message. */
export async function searchArt(
  gameId: string,
  kind: ArtKind,
): Promise<ArtResult[]> {
  if (!isTauriRuntime()) return [];
  try {
    return await invoke<ArtResult[]>("search_art", { gameId, kind });
  } catch (error) {
    if (unavailable(error)) return [];
    throw error;
  }
}

/** Download one candidate into the local cache (+ Steam's grid folder). */
export async function applyArt(
  gameId: string,
  url: string,
  kind: ArtKind,
  replace = false,
): Promise<ApplyArtResult> {
  return invoke<ApplyArtResult>("apply_art", { gameId, url, kind, replace });
}

export async function getArtStatus(gameId: string): Promise<ArtStatus> {
  if (!isTauriRuntime()) return EMPTY_STATUS;
  try {
    return await invoke<ArtStatus>("get_art_status", { gameId });
  } catch {
    return EMPTY_STATUS;
  }
}

/** True when a local path can be rendered by the WebView. */
export function isLocalArtPath(value: string | null | undefined): boolean {
  if (!value) return false;
  return !value.includes("://");
}
