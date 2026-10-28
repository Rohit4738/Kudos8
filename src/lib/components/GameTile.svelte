<!--
  A single library tile. Only transform and opacity animate; the glow ring is a
  static shadow that fades in/out, never a per-frame box-shadow animation.
  OWNER: Session D.
-->
<script lang="ts">
  import { platformDotColor } from "../art";
  import { platformShort } from "../format";
  import type { Game } from "../types";
  import GameArt from "./GameArt.svelte";

  interface Props {
    game: Game;
    active: boolean;
    index: number;
    onSelect: (index: number) => void;
    onOpen: (game: Game) => void;
    onHide: (game: Game) => void;
  }

  let { game, active, index, onSelect, onOpen, onHide }: Props = $props();

  const HOLD_MS = 650;
  let pressTimer: ReturnType<typeof setTimeout> | undefined;
  let suppressOpen = false;

  function startPress(event: PointerEvent): void {
    if (event.pointerType === "mouse" && event.button !== 0) return;
    cancelPress();
    pressTimer = setTimeout(() => {
      pressTimer = undefined;
      suppressOpen = true;
      // A held touch may never produce a click; clear the guard on its own.
      setTimeout(() => {
        suppressOpen = false;
      }, 900);
      onHide(game);
    }, HOLD_MS);
  }

  function cancelPress(): void {
    if (pressTimer) clearTimeout(pressTimer);
    pressTimer = undefined;
  }

  function handleClick(): void {
    if (suppressOpen) {
      suppressOpen = false;
      return;
    }
    onOpen(game);
  }

  function handleContextMenu(event: MouseEvent): void {
    event.preventDefault();
    cancelPress();
    if (suppressOpen) {
      suppressOpen = false;
      return;
    }
    onHide(game);
  }
</script>

<button
  type="button"
  class="tile"
  class:active
  aria-current={active ? "true" : undefined}
  aria-label={`${game.title}, ${platformShort(game.platform)}, ${
    game.isInstalled ? "installed" : "not downloaded"
  }. Hold or right-click to hide.`}
  title="Open · hold or right-click to hide"
  onmouseenter={() => onSelect(index)}
  onfocus={() => onSelect(index)}
  onpointerdown={startPress}
  onpointerup={cancelPress}
  onpointerleave={cancelPress}
  onpointercancel={cancelPress}
  oncontextmenu={handleContextMenu}
  onclick={handleClick}
>
  <span class="frame">
    <GameArt {game} />
    {#if !game.isInstalled}
      <span class="library-tag">In library</span>
    {/if}
  </span>

  <span class="caption">
    <span
      class="dot"
      style:background={platformDotColor(game.platform)}
      aria-hidden="true"
    ></span>
    <span class="name">{game.title}</span>
  </span>
</button>

<style>
  .tile {
    position: relative;
    flex: 0 0 auto;
    width: var(--tile-w, 220px);
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 0;
    text-align: left;
    border-radius: var(--radius-md);
    opacity: 0.46;
    transform: scale(0.94) translateY(6px);
    transform-origin: center 72%;
    transition:
      transform 380ms var(--ease-out),
      opacity 300ms var(--ease-out);
    will-change: transform, opacity;
    z-index: 1;
  }

  .tile.active {
    opacity: 1;
    transform: scale(1.075) translateY(-6px);
    z-index: 3;
  }

  .tile:active .frame {
    transform: scale(0.985);
  }

  .frame {
    position: relative;
    display: block;
    width: 100%;
    aspect-ratio: 2 / 3;
    border-radius: clamp(10px, 1vw, 16px);
    overflow: hidden;
    background: var(--bg-2);
    box-shadow: 0 16px 42px rgba(0, 0, 0, 0.42);
    transition: transform var(--dur-fast) var(--ease-out);
  }

  .frame::after {
    content: "";
    position: absolute;
    inset: 0;
    border-radius: inherit;
    box-shadow:
      0 0 0 1.5px rgba(255, 255, 255, 0.68),
      0 0 26px 2px var(--accent-glow),
      0 34px 78px rgba(0, 0, 0, 0.62);
    opacity: 0;
    transition: opacity 320ms var(--ease-out);
    pointer-events: none;
    z-index: 2;
  }

  .tile.active .frame::after {
    opacity: 1;
  }

  .library-tag {
    position: absolute;
    top: 10px;
    left: 10px;
    z-index: 3;
    padding: 4px 8px;
    border-radius: 999px;
    font-size: 9px;
    font-weight: 600;
    letter-spacing: 0.11em;
    text-transform: uppercase;
    color: rgba(255, 255, 255, 0.8);
    background: rgba(5, 8, 16, 0.62);
    backdrop-filter: blur(10px);
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.1);
  }

  .caption {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 3px;
    min-width: 0;
    opacity: 0.8;
    transition: opacity 300ms var(--ease-out);
  }

  .tile.active .caption {
    opacity: 1;
  }

  .dot {
    width: 5px;
    height: 5px;
    border-radius: 50%;
    flex: 0 0 auto;
    opacity: 0.85;
  }

  .name {
    font-size: 12.5px;
    font-weight: 550;
    letter-spacing: 0.005em;
    color: var(--fg-1);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .tile.active .name {
    color: var(--fg-0);
  }
</style>
