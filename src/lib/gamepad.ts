// Gamepad navigation via the Gamepad API. One rAF loop, shared by all pages.
// D-pad + left stick move, A confirms, B goes back, X is a secondary action,
// shoulders / triggers page. Only transform/opacity animations live in the UI;
// this file never measures layout, so the loop stays cheap.
//
// OWNER: Session D.

export type PadAction =
  | "left"
  | "right"
  | "up"
  | "down"
  | "confirm"
  | "back"
  | "secondary"
  | "tabLeft"
  | "tabRight"
  | "menu";

type Listener = (action: PadAction) => void;

const REPEATABLE: ReadonlySet<PadAction> = new Set([
  "left",
  "right",
  "up",
  "down",
]);

const REPEAT_DELAY_MS = 330;
const REPEAT_RATE_MS = 110;
const AXIS_THRESHOLD = 0.62;

const listeners = new Set<Listener>();
const held = new Map<PadAction, number>(); // action -> next repeat timestamp
let frame = 0;

const BUTTON_MAP: Record<number, PadAction> = {
  0: "confirm", // A / cross
  1: "back", // B / circle
  2: "secondary", // X / square
  4: "tabLeft", // L1
  5: "tabRight", // R1
  9: "menu", // start
  12: "up",
  13: "down",
  14: "left",
  15: "right",
};

const ALL_ACTIONS: PadAction[] = [
  "left",
  "right",
  "up",
  "down",
  "confirm",
  "back",
  "secondary",
  "tabLeft",
  "tabRight",
  "menu",
];

export function onGamepad(listener: Listener): () => void {
  listeners.add(listener);
  if (listeners.size === 1) start();
  return () => {
    listeners.delete(listener);
    if (listeners.size === 0) stop();
  };
}

function start(): void {
  if (frame) return;
  frame = requestAnimationFrame(poll);
}

function stop(): void {
  if (frame) cancelAnimationFrame(frame);
  frame = 0;
  held.clear();
}

function poll(now: number): void {
  frame = requestAnimationFrame(poll);
  const active = readActiveActions();
  if (active.size === 0 && held.size === 0) return;

  for (const action of ALL_ACTIONS) {
    const down = active.has(action);
    const nextRepeat = held.get(action);

    if (!down) {
      held.delete(action);
      continue;
    }

    if (nextRepeat === undefined) {
      held.set(action, now + REPEAT_DELAY_MS);
      emit(action);
      continue;
    }

    if (REPEATABLE.has(action) && now >= nextRepeat) {
      held.set(action, now + REPEAT_RATE_MS);
      emit(action);
    }
  }
}

function readActiveActions(): Set<PadAction> {
  const active = new Set<PadAction>();
  const pad = firstConnected();
  if (!pad) return active;

  for (let index = 0; index < pad.buttons.length; index += 1) {
    const button = pad.buttons[index];
    const action = BUTTON_MAP[index];
    if (action && button?.pressed) active.add(action);
  }

  const [x = 0, y = 0] = pad.axes;
  if (x <= -AXIS_THRESHOLD) active.add("left");
  if (x >= AXIS_THRESHOLD) active.add("right");
  if (y <= -AXIS_THRESHOLD) active.add("up");
  if (y >= AXIS_THRESHOLD) active.add("down");

  return active;
}

function firstConnected(): Gamepad | null {
  if (typeof navigator === "undefined" || !navigator.getGamepads) return null;
  const pads = navigator.getGamepads();
  for (const pad of pads) {
    if (pad?.connected) return pad;
  }
  return null;
}

function emit(action: PadAction): void {
  for (const listener of listeners) {
    try {
      listener(action);
    } catch {
      /* one bad listener must not stop the loop */
    }
  }
}
