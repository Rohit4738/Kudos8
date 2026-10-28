// Controller detection + layout classification. OWNER: Session F.
//
// The Gamepad API gives us one useful signal: `Gamepad.id`. We classify that
// string once, when a pad connects, and never look at the pad list per frame.
// `gamepad.ts` owns input polling; this module owns "which glyphs do we show".
//
// Persistence: `kudos.padLayout` stores the user's override. "auto" follows the
// connected pad; "keyboard" always shows keyboard hints; a concrete layout is
// useful when a generic pad is misidentified.
//
// `?mock=1&pad=playstation|xbox|nintendo|generic|keyboard` lets the visual smoke
// test render every glyph family without physical hardware. The preview is
// ignored outside mock mode, so the real app never invents a controller.

export type PadLayout = "xbox" | "playstation" | "nintendo" | "generic" | "keyboard";
export type PadOverride = "auto" | Exclude<PadLayout, "keyboard"> | "keyboard";
export type GlyphAction =
  | "confirm"
  | "back"
  | "secondary"
  | "quaternary"
  | "tabLeft"
  | "tabRight"
  | "menu"
  | "dpad";

export const PAD_LAYOUT_LABEL: Record<PadLayout, string> = {
  xbox: "Xbox",
  playstation: "PlayStation",
  nintendo: "Nintendo",
  generic: "Generic pad",
  keyboard: "Keyboard",
};

export const PAD_OVERRIDES: { value: PadOverride; label: string }[] = [
  { value: "auto", label: "Auto" },
  { value: "xbox", label: "Xbox" },
  { value: "playstation", label: "PlayStation" },
  { value: "nintendo", label: "Nintendo" },
  { value: "keyboard", label: "Keyboard" },
];

const OVERRIDE_KEY = "kudos.padLayout";
const VALID_OVERRIDES: ReadonlySet<string> = new Set(
  PAD_OVERRIDES.map((entry) => entry.value),
);

interface ControllerState {
  connected: boolean;
  /** Raw `Gamepad.id`; shown in Settings so a misdetection is diagnosable. */
  id: string;
  detected: PadLayout;
  override: PadOverride;
  /** Mock-only pad family from `?pad=`; `null` in the real app. */
  preview: PadLayout | null;
}

export const controller = $state<ControllerState>({
  connected: false,
  id: "",
  detected: "generic",
  override: readOverride(),
  preview: null,
});

/** The glyph family the UI should use right now. */
export function effectiveLayout(): PadLayout {
  if (controller.override === "keyboard") return "keyboard";
  if (controller.override !== "auto") return controller.override;
  if (controller.preview) return controller.preview;
  return controller.connected ? controller.detected : "keyboard";
}

/** Detect once per connect event. Pure string classification, no layout reads. */
export function layoutFromId(id: string): PadLayout {
  const value = id.toLowerCase();
  if (/xbox|xinput|microsoft|045e/.test(value)) return "xbox";
  if (/playstation|dualsense|dualshock|sony|ps[3-5]|054c/.test(value)) {
    return "playstation";
  }
  if (/nintendo|switch|joy-?con|pro controller|057e/.test(value)) {
    return "nintendo";
  }
  return "generic";
}

export function setPadOverride(value: PadOverride): void {
  controller.override = value;
  try {
    window.localStorage.setItem(OVERRIDE_KEY, value);
  } catch {
    /* storage is best-effort */
  }
}

/** Re-read the connected pads. Called on events and focus, never per frame. */
export function refreshController(): void {
  if (typeof navigator === "undefined") return;
  if (controller.preview) {
    applyPreview(controller.preview);
    return;
  }

  const pads = navigator.getGamepads?.() ?? [];
  let fallback: Gamepad | null = null;
  let standard: Gamepad | null = null;
  for (const pad of pads) {
    if (!pad?.connected) continue;
    if (pad.mapping === "standard") {
      standard = pad;
      break;
    }
    fallback ??= pad;
  }

  const pad = standard ?? fallback;
  controller.connected = Boolean(pad);
  controller.id = pad?.id ?? "";
  controller.detected = pad ? layoutFromId(pad.id) : "generic";
}

/** Attach live connection listeners. Returns a cleanup function. */
export function startController(): () => void {
  if (typeof window === "undefined") return () => {};

  const preview = previewPadFromUrl();
  if (preview) {
    controller.preview = preview;
    applyPreview(preview);
  } else {
    refreshController();
  }

  const onConnected = () => refreshController();
  const onDisconnected = () => refreshController();
  const onFocus = () => refreshController();
  const onVisibility = () => {
    if (!document.hidden) refreshController();
  };

  window.addEventListener("gamepadconnected", onConnected);
  window.addEventListener("gamepaddisconnected", onDisconnected);
  window.addEventListener("focus", onFocus);
  document.addEventListener("visibilitychange", onVisibility);

  return () => {
    window.removeEventListener("gamepadconnected", onConnected);
    window.removeEventListener("gamepaddisconnected", onDisconnected);
    window.removeEventListener("focus", onFocus);
    document.removeEventListener("visibilitychange", onVisibility);
    controller.preview = null;
  };
}

function applyPreview(layout: PadLayout): void {
  controller.connected = layout !== "keyboard";
  controller.detected = layout === "keyboard" ? "generic" : layout;
  controller.id =
    layout === "keyboard"
      ? ""
      : `Preview ${PAD_LAYOUT_LABEL[layout]} Controller (STANDARD GAMEPAD)`;
  controller.override = "auto";
}

function previewPadFromUrl(): PadLayout | null {
  if (typeof window === "undefined") return null;
  try {
    const params = new URLSearchParams(window.location.search);
    if (params.get("mock") !== "1") return null;
    const value = params.get("pad");
    if (
      value === "xbox" ||
      value === "playstation" ||
      value === "nintendo" ||
      value === "generic" ||
      value === "keyboard"
    ) {
      return value;
    }
  } catch {
    /* no URL inspection available */
  }
  return null;
}

function readOverride(): PadOverride {
  try {
    const value = window.localStorage.getItem(OVERRIDE_KEY);
    return value && VALID_OVERRIDES.has(value) ? (value as PadOverride) : "auto";
  } catch {
    return "auto";
  }
}
