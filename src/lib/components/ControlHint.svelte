<!--
  One control hint: keyboard keycaps when no pad (or the Keyboard override),
  SVG pad glyphs otherwise. OWNER: Session F.
-->
<script lang="ts">
  import { effectiveLayout } from "../controller.svelte";
  import type { GlyphAction } from "../controller.svelte";
  import PadGlyph from "./PadGlyph.svelte";

  interface Props {
    action: GlyphAction;
    /** Keyboard representation, e.g. "Enter" or "← →". */
    keys: string;
    label?: string;
  }

  let { action, keys, label = "" }: Props = $props();
  const layout = $derived(effectiveLayout());
</script>

<span class="control-hint">
  {#if layout === "keyboard"}
    <kbd>{keys}</kbd>
  {:else}
    <PadGlyph {layout} {action} />
  {/if}
  {#if label}<span class="label">{label}</span>{/if}
</span>

<style>
  .control-hint {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    white-space: nowrap;
  }

  kbd {
    font-family: var(--font-ui);
    font-size: 9px;
    font-weight: 650;
    letter-spacing: 0.04em;
    color: var(--fg-1);
    background: rgba(255, 255, 255, 0.07);
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.1);
    border-radius: 4px;
    padding: 2px 5px;
    line-height: 1.35;
  }

  .label {
    color: inherit;
  }
</style>
