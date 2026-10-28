<!--
  Controller glyphs drawn as inline SVG. OWNER: Session F.
  PlayStation uses hand-written paths for ✕ ○ □ △ - never an icon font, so the
  shapes stay identical on every Windows install. Xbox/Nintendo/generic letters
  are SVG text inside a drawn button, not a font icon.
-->
<script lang="ts">
  import type { GlyphAction, PadLayout } from "../controller.svelte";

  interface Props {
    layout: Exclude<PadLayout, "keyboard">;
    action: GlyphAction;
    size?: number;
  }

  let { layout, action, size = 18 }: Props = $props();

  const family = $derived(
    layout === "xbox" ? "xbox" : layout === "nintendo" ? "nintendo" : "generic",
  );
  const face = $derived(
    action === "confirm" ||
      action === "back" ||
      action === "secondary" ||
      action === "quaternary",
  );
  const faceColor = $derived(
    family === "xbox"
      ? action === "confirm"
        ? "#3ddc97"
        : action === "back"
          ? "#ff5f6d"
          : action === "secondary"
            ? "#4c8dff"
            : "#f5c542"
      : family === "nintendo"
        ? action === "confirm"
          ? "#ff6b6b"
          : action === "back"
            ? "#ffd166"
            : action === "secondary"
              ? "#5bc0eb"
              : "#7bd88f"
        : "#c7d0e6",
  );
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="1.7"
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
  focusable="false"
  class="glyph"
>
  {#if action === "dpad"}
    <path d="M12 5.4 14.7 8.4H9.3L12 5.4Z" fill="currentColor" stroke="none" />
    <path d="M12 18.6 9.3 15.6h5.4L12 18.6Z" fill="currentColor" stroke="none" />
    <path d="M5.4 12 8.4 9.3v5.4L5.4 12Z" fill="currentColor" stroke="none" />
    <path d="M18.6 12 15.6 14.7V9.3L18.6 12Z" fill="currentColor" stroke="none" />
  {:else if action === "tabLeft" || action === "tabRight"}
    <rect x="2.5" y="7" width="19" height="10" rx="4.4" opacity="0.5" />
    <text
      x="12"
      y="12.4"
      text-anchor="middle"
      dominant-baseline="middle"
      fill="currentColor"
      stroke="none"
      font-family="Inter Variable, Segoe UI, sans-serif"
      font-size="8.4"
      font-weight="700"
      letter-spacing="0.3">{action === "tabLeft" ? "L1" : "R1"}</text
    >
  {:else if action === "menu"}
    <circle cx="12" cy="12" r="8.6" opacity="0.42" />
    <path d="M8 9.4h8M8 12h8M8 14.6h8" />
  {:else if layout === "playstation"}
    <circle cx="12" cy="12" r="9" opacity="0.18" fill="currentColor" stroke="none" />
    {#if action === "confirm"}
      <path d="M8.6 8.6 15.4 15.4M15.4 8.6 8.6 15.4" stroke-width="2.1" />
    {:else if action === "back"}
      <circle cx="12" cy="12" r="5.1" stroke-width="1.9" />
    {:else if action === "secondary"}
      <rect x="7.3" y="7.3" width="9.4" height="9.4" rx="1.2" stroke-width="1.9" />
    {:else}
      <path d="M12 6.1 17.8 16.4H6.2L12 6.1Z" stroke-width="1.9" />
    {/if}
  {:else if face}
    <circle cx="12" cy="12" r="9.2" fill={faceColor} stroke="none" />
    <text
      x="12"
      y="12.5"
      text-anchor="middle"
      dominant-baseline="middle"
      fill="#05070c"
      stroke="none"
      font-family="Inter Variable, Segoe UI, sans-serif"
      font-size="10"
      font-weight="800"
    >{action === "confirm"
      ? "A"
      : action === "back"
        ? "B"
        : action === "secondary"
          ? "X"
          : "Y"}</text
    >
  {/if}
</svg>

<style>
  .glyph {
    display: block;
    flex: 0 0 auto;
  }
</style>
