// Accent colour theming. Persisted in localStorage, applied through CSS vars.
// OWNER: Session D. The CSS tokens themselves live in src/app.css.

export interface AccentPreset {
  name: string;
  value: string;
}

export const DEFAULT_ACCENT = "#4c8dff";

export const ACCENT_PRESETS: AccentPreset[] = [
  { name: "Horizon", value: "#4c8dff" },
  { name: "Nebula", value: "#8b5cf6" },
  { name: "Aurora", value: "#2ee6c5" },
  { name: "Solar", value: "#ffb547" },
  { name: "Sakura", value: "#ff6b9d" },
  { name: "Crimson", value: "#ff5f6d" },
];

const ACCENT_KEY = "kudos.accent";

export function normalizeHex(input: string | null | undefined): string | null {
  if (!input) return null;
  let value = input.trim();
  if (!value.startsWith("#")) value = `#${value}`;
  if (/^#[0-9a-f]{3}$/i.test(value)) {
    const [, r, g, b] = value;
    value = `#${r}${r}${g}${g}${b}${b}`;
  }
  return /^#[0-9a-f]{6}$/i.test(value) ? value.toLowerCase() : null;
}

export function storedAccent(): string {
  try {
    return normalizeHex(window.localStorage.getItem(ACCENT_KEY)) ?? DEFAULT_ACCENT;
  } catch {
    return DEFAULT_ACCENT;
  }
}

/** Applies an accent to the document (and persists it by default). */
export function applyAccent(input: string, persist = true): string {
  const value = normalizeHex(input) ?? DEFAULT_ACCENT;
  const [r, g, b] = hexToRgb(value);
  const root = document.documentElement;

  root.style.setProperty("--accent", value);
  root.style.setProperty("--accent-soft", mix(value, "#05060a", 0.45));
  root.style.setProperty("--accent-glow", `rgba(${r}, ${g}, ${b}, 0.42)`);

  if (persist) {
    try {
      window.localStorage.setItem(ACCENT_KEY, value);
    } catch {
      /* storage is best-effort */
    }
  }
  return value;
}

export function applyStoredAccent(): string {
  return applyAccent(storedAccent(), false);
}

function hexToRgb(hex: string): [number, number, number] {
  return [
    Number.parseInt(hex.slice(1, 3), 16),
    Number.parseInt(hex.slice(3, 5), 16),
    Number.parseInt(hex.slice(5, 7), 16),
  ];
}

function toHex(value: number): string {
  return Math.round(Math.max(0, Math.min(255, value)))
    .toString(16)
    .padStart(2, "0");
}

/** Mixes two hex colours. `weight` is how much of `b` to use (0..1). */
function mix(a: string, b: string, weight: number): string {
  const [ar, ag, ab] = hexToRgb(a);
  const [br, bg, bb] = hexToRgb(b);
  const t = Math.max(0, Math.min(1, weight));
  return `#${toHex(ar + (br - ar) * t)}${toHex(ag + (bg - ag) * t)}${toHex(
    ab + (bb - ab) * t,
  )}`;
}
