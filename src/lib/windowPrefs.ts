// Borderless-fullscreen preference, persisted in localStorage and applied
// through the Tauri window API. Safe to import outside Tauri.
// OWNER: Session D.

import { getCurrentWindow } from "@tauri-apps/api/window";
import { isTauriRuntime } from "./art";

const FULLSCREEN_KEY = "kudos.fullscreen";
const STARTUP_KEY = "kudos.startupPage";

export type StartupPage = "home" | "settings";

export { isTauriRuntime };

export function readFullscreenPref(): boolean {
  try {
    return window.localStorage.getItem(FULLSCREEN_KEY) === "1";
  } catch {
    return false;
  }
}

function writeFullscreenPref(on: boolean): void {
  try {
    window.localStorage.setItem(FULLSCREEN_KEY, on ? "1" : "0");
  } catch {
    /* storage is best-effort */
  }
}

/** Borderless fullscreen is Tauri's undecorated fullscreen window mode. */
export async function setBorderlessFullscreen(on: boolean): Promise<void> {
  if (!isTauriRuntime()) {
    throw new Error("Window controls are only available in the desktop app.");
  }
  await getCurrentWindow().setFullscreen(on);
  writeFullscreenPref(on);
}

/** Reads the live window state (settings page). */
export async function currentFullscreen(): Promise<boolean> {
  if (!isTauriRuntime()) return readFullscreenPref();
  return getCurrentWindow().isFullscreen();
}

/** Re-applies the saved preference once, at startup. */
export async function restoreFullscreenPref(): Promise<void> {
  if (!isTauriRuntime() || !readFullscreenPref()) return;
  try {
    const win = getCurrentWindow();
    if (!(await win.isFullscreen())) {
      await win.setFullscreen(true);
    }
  } catch {
    /* A failed restore must never block the library. */
  }
}

/** Which page Kudos opens on. */
export function readStartupPage(): StartupPage {
  try {
    return window.localStorage.getItem(STARTUP_KEY) === "settings"
      ? "settings"
      : "home";
  } catch {
    return "home";
  }
}

export function saveStartupPage(page: StartupPage): void {
  try {
    window.localStorage.setItem(STARTUP_KEY, page);
  } catch {
    /* storage is best-effort */
  }
}
