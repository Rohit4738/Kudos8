//! Frontend-side copy of the Rust contract in `src-tauri/src/models.rs`.
//! OWNER: Session A. Change both files in the same commit.

export type Platform = "steam" | "epic" | "gog";

export interface Game {
  /** Namespaced unique id, e.g. "steam:730". */
  id: string;
  platform: Platform;
  title: string;
  exePath?: string;
  launchCommand: string;
  isInstalled: boolean;
  /** https URL or local file path. */
  coverImage?: string;
  sizeBytes?: number;
}

export interface SteamStatus {
  installed: boolean;
  running: boolean;
  steamPath?: string;
}

export const PLATFORM_LABEL: Record<Platform, string> = {
  steam: "Steam",
  epic: "Epic Games",
  gog: "GOG",
};
