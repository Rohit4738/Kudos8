// Idle / ambient mode: preferences, activity detection and the idle flag.
// OWNER: Session G.
//
// Design rules:
//  - No per-frame work. Pointer/keyboard activity only maintains a timestamp and
//    (at most twice a second) moves a single setTimeout. Gamepads are polled at
//    2 Hz, not 60 Hz.
//  - Armed only while the Home page is mounted; disarmed everywhere else.
//  - Paused while the window is hidden or unfocused, so alt-tabbing away never
//    opens the ambient screen behind another app.
//  - This is a low-activity ambient display, NOT a screensaver: it does not blank
//    the display or stop the GPU. The Settings copy says so too.

export type IdleArtSource = "recent" | "library" | "random";
export type IdleCrossfade = "slow" | "fast";

export interface IdleSettings {
  enabled: boolean;
  /** Milliseconds. 0 = never. */
  delayMs: number;
  artSource: IdleArtSource;
  crossfade: IdleCrossfade;
  showClock: boolean;
}

export const IDLE_DELAYS: ReadonlyArray<{ value: number; label: string }> = [
  { value: 30_000, label: "30s" },
  { value: 60_000, label: "1m" },
  { value: 120_000, label: "2m" },
  { value: 300_000, label: "5m" },
  { value: 0, label: "Never" },
];

export const DEFAULT_IDLE: IdleSettings = {
  enabled: true,
  delayMs: 60_000,
  artSource: "recent",
  crossfade: "slow",
  showClock: true,
};

const STORAGE_KEY = "kudos.idle";
const ART_SOURCES: IdleArtSource[] = ["recent", "library", "random"];
const CROSSFADES: IdleCrossfade[] = ["slow", "fast"];

function normalize(raw: unknown): IdleSettings {
  const input = (raw && typeof raw === "object" ? raw : {}) as Partial<IdleSettings>;
  const delay = Number(input.delayMs);
  const delayMs = IDLE_DELAYS.some((option) => option.value === delay)
    ? delay
    : DEFAULT_IDLE.delayMs;

  return {
    enabled: typeof input.enabled === "boolean" ? input.enabled : DEFAULT_IDLE.enabled,
    delayMs,
    artSource: ART_SOURCES.includes(input.artSource as IdleArtSource)
      ? (input.artSource as IdleArtSource)
      : DEFAULT_IDLE.artSource,
    crossfade: CROSSFADES.includes(input.crossfade as IdleCrossfade)
      ? (input.crossfade as IdleCrossfade)
      : DEFAULT_IDLE.crossfade,
    showClock: typeof input.showClock === "boolean" ? input.showClock : DEFAULT_IDLE.showClock,
  };
}

export function readIdlePrefs(): IdleSettings {
  if (typeof window === "undefined") return { ...DEFAULT_IDLE };
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY);
    return raw ? normalize(JSON.parse(raw)) : { ...DEFAULT_IDLE };
  } catch {
    return { ...DEFAULT_IDLE };
  }
}

export function writeIdlePrefs(settings: IdleSettings): IdleSettings {
  const next = normalize(settings);
  if (typeof window !== "undefined") {
    try {
      window.localStorage.setItem(STORAGE_KEY, JSON.stringify(next));
    } catch {
      /* storage is best-effort */
    }
  }
  return next;
}

export const idlePrefs = $state<IdleSettings>(readIdlePrefs());

/** Live-updates the store + storage. Reschedules an armed watcher immediately. */
export function updateIdlePrefs(patch: Partial<IdleSettings>): IdleSettings {
  const next = writeIdlePrefs({ ...idlePrefs, ...patch });
  Object.assign(idlePrefs, next);
  if (armed > 0) schedule();
  return next;
}

// ---------------------------------------------------------------------------
// Idle state
// ---------------------------------------------------------------------------

export const idleState = $state<{ active: boolean }>({ active: false });

let gate: (() => boolean) | null = null;

/**
 * Lets the app-level idle host decide whether there is anything to show.
 * `enterIdle()` refuses (and the timer retries later) when the gate is false.
 */
export function setIdleGate(can: (() => boolean) | null): void {
  gate = can;
}

export function canEnterIdle(): boolean {
  return gate ? gate() : true;
}

/**
 * Opens the ambient screen. `force` is for the Settings preview, which may run
 * with idle auto-start disabled but still needs something on screen.
 */
export function enterIdle(force = false): boolean {
  if (idleState.active) return true;
  if (!force && (!idlePrefs.enabled || idlePrefs.delayMs <= 0)) return false;
  if (!canEnterIdle()) return false;

  clearTimer();
  idleState.active = true;
  return true;
}

/** Closes the ambient screen and restarts the idle clock from zero. */
export function exitIdle(): void {
  const wasActive = idleState.active;
  idleState.active = false;
  lastActivity = nowMs();
  if (wasActive && armed > 0) schedule();
}

// ---------------------------------------------------------------------------
// Activity detection
// ---------------------------------------------------------------------------

let armed = 0;
let timer: ReturnType<typeof setTimeout> | undefined;
let deadline = 0;
let lastActivity = nowMs();
let listenersAttached = false;
let padTimer: ReturnType<typeof setInterval> | undefined;
let padSignature = "";

function nowMs(): number {
  return typeof performance !== "undefined" ? performance.now() : Date.now();
}

function clearTimer(): void {
  if (timer !== undefined) {
    clearTimeout(timer);
    timer = undefined;
  }
  deadline = 0;
}

function schedule(): void {
  clearTimer();
  if (!armed || idleState.active) return;
  if (!idlePrefs.enabled || idlePrefs.delayMs <= 0) return;

  const wait = Math.max(0, idlePrefs.delayMs - (nowMs() - lastActivity));
  deadline = nowMs() + wait;
  timer = setTimeout(fire, wait);
}

function fire(): void {
  timer = undefined;
  deadline = 0;
  if (!armed || idleState.active) return;
  if (!idlePrefs.enabled || idlePrefs.delayMs <= 0) return;

  // Hidden / unfocused pauses the clock instead of firing behind another app.
  if (isPaused()) {
    lastActivity = nowMs();
    schedule();
    return;
  }

  if (!canEnterIdle()) {
    lastActivity = nowMs();
    schedule();
    return;
  }

  if (enterIdle()) {
    clearTimer();
  } else {
    lastActivity = nowMs();
    schedule();
  }
}

function isPaused(): boolean {
  return (
    typeof document !== "undefined" &&
    (document.hidden || (typeof document.hasFocus === "function" && !document.hasFocus()))
  );
}

/**
 * Any real input resets the clock. The timeout is only moved when the new
 * deadline is meaningfully later, so a 100 Hz pointermove stream costs at most
 * two timer reschedules per second.
 */
function onActivity(): void {
  if (idleState.active) return;
  const now = nowMs();
  lastActivity = now;
  if (!armed || isPaused()) return;

  if (!idlePrefs.enabled || idlePrefs.delayMs <= 0) {
    clearTimer();
    return;
  }

  const nextDeadline = now + idlePrefs.delayMs;
  if (timer === undefined || nextDeadline > deadline + 500) {
    schedule();
  }
}

function onVisibilityChange(): void {
  if (document.hidden) {
    clearTimer();
  } else {
    lastActivity = nowMs();
    schedule();
  }
}

function onWindowBlur(): void {
  clearTimer();
}

function onWindowFocus(): void {
  lastActivity = nowMs();
  schedule();
}

function readPadSignature(): string {
  if (typeof navigator === "undefined" || typeof navigator.getGamepads !== "function") {
    return "";
  }
  let signature = "";
  try {
    for (const pad of navigator.getGamepads()) {
      if (!pad) continue;
      signature += `${pad.index}:`;
      for (const button of pad.buttons) signature += button.pressed ? "1" : "0";
      for (const axis of pad.axes) {
        signature += axis > 0.55 ? "+" : axis < -0.55 ? "-" : "0";
      }
      signature += ";";
    }
  } catch {
    return "";
  }
  return signature;
}

function pollGamepads(): void {
  if (isPaused()) return;
  const signature = readPadSignature();
  if (signature !== padSignature) {
    padSignature = signature;
    onActivity();
  }
}

function attach(): void {
  if (listenersAttached) return;
  listenersAttached = true;
  padSignature = readPadSignature();

  const opts: AddEventListenerOptions = { passive: true, capture: true };
  window.addEventListener("pointermove", onActivity, opts);
  window.addEventListener("pointerdown", onActivity, opts);
  window.addEventListener("mousedown", onActivity, opts);
  window.addEventListener("wheel", onActivity, opts);
  window.addEventListener("touchstart", onActivity, opts);
  window.addEventListener("keydown", onActivity, true);
  window.addEventListener("blur", onWindowBlur);
  window.addEventListener("focus", onWindowFocus);
  document.addEventListener("visibilitychange", onVisibilityChange);
  padTimer = setInterval(pollGamepads, 500);
}

function detach(): void {
  if (!listenersAttached) return;
  listenersAttached = false;

  const opts: AddEventListenerOptions = { passive: true, capture: true };
  window.removeEventListener("pointermove", onActivity, opts);
  window.removeEventListener("pointerdown", onActivity, opts);
  window.removeEventListener("mousedown", onActivity, opts);
  window.removeEventListener("wheel", onActivity, opts);
  window.removeEventListener("touchstart", onActivity, opts);
  window.removeEventListener("keydown", onActivity, true);
  window.removeEventListener("blur", onWindowBlur);
  window.removeEventListener("focus", onWindowFocus);
  document.removeEventListener("visibilitychange", onVisibilityChange);
  if (padTimer !== undefined) {
    clearInterval(padTimer);
    padTimer = undefined;
  }
  clearTimer();
}

/**
 * Arms idle detection. Call from the Home page's `onMount` and return the
 * cleanup. Reference-counted so a dev harness and Home can coexist safely.
 */
export function armIdleWatcher(): () => void {
  armed += 1;
  if (armed === 1) attach();
  lastActivity = nowMs();
  schedule();

  let released = false;
  return () => {
    if (released) return;
    released = true;
    armed = Math.max(0, armed - 1);
    if (armed === 0) detach();
  };
}
