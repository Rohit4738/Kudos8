<!--
  A 2:3 library tile used by every Home row. The active tile carries the PS5
  inner-border focus treatment. Hold or right-click toggles favorite.
  OWNER: Session I.
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
    meta?: string;
    favorite?: boolean;
    onFocus: (index: number) => void;
    onOpen: (game: Game) => void;
    onToggleFavorite: (game: Game) => void;
  }

  let {
    game,
    active,
    index,
    meta = "",
    favorite = false,
    onFocus,
    onOpen,
    onToggleFavorite,
  }: Props = $props();

  const HOLD_MS = 620;
  let pressTimer: ReturnType<typeof setTimeout> | undefined;
  let suppressOpen = false;
  let suppressReset: ReturnType<typeof setTimeout> | undefined;

  function startPress(event: PointerEvent): void {
    if (event.pointerType === "mouse" && event.button !== 0) return;
    cancelPress();
    pressTimer = setTimeout(() => {
      pressTimer = undefined;
      suppressOpen = true;
      onToggleFavorite(game);
      if (suppressReset) clearTimeout(suppressReset);
      suppressReset = setTimeout(() => {
        suppressOpen = false;
      }, 800);
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
    onToggleFavorite(game);
  }
</script>

<button
  type="button"
  class="tile"
  class:active
  class:favorite
  class:owned={!game.isInstalled}
  aria-current={active ? "true" : undefined}
  aria-label={`${game.title}, ${platformShort(game.platform)}, ${
    game.isInstalled ? "installed" : "not installed"
  }${favorite ? ", favorite" : ""}. Hold or right-click to favorite.`}
  title={`${game.title} · hold or right-click to favorite`}
  onmouseenter={() => onFocus(index)}
  onfocus={() => onFocus(index)}
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
      <span class="state-tag">Not installed</span>
    {/if}

    {#if favorite}
      <span class="fav-tag" aria-hidden="true">★</span>
    {/if}

    {#if !game.isInstalled && game.platform === "steam"}
      <span class="install-hint" aria-hidden="true">
        <span class="install-icon">↓</span>
        Install
      </span>
    {/if}
  </span>

  <span class="caption">
    <span class="line">
      <span class="dot" style:background={platformDotColor(game.platform)} aria-hidden="true"></span>
      <span class="name">{game.title}</span>
    </span>
    {#if meta}
      <span class="meta">{meta}</span>
    {/if}
  </span>
</button>

<style>
  .tile {
    position: relative;
    flex: 0 0 auto;
    width: var(--row-tile-w, 148px);
    display: flex;
    flex-direction: column;
    gap: 9px;
    padding: 0;
    text-align: left;
    border-radius: var(--radius-md);
    opacity: 0.62;
    transform: scale(0.965) translateY(3px);
    transform-origin: center 74%;
    transition:
      transform 250ms var(--ease-out),
      opacity 220ms var(--ease-out);
    will-change: transform, opacity;
    z-index: 1;
  }

  .tile.active {
    opacity: 1;
    transform: scale(1.045) translateY(-3px);
    z-index: 3;
  }

  .tile.owned {
    opacity: 0.5;
  }

  .tile.owned.active {
    opacity: 0.94;
  }

  .tile:active .frame {
    transform: scale(0.985);
  }

  .frame {
    position: relative;
    display: block;
    width: 100%;
    aspect-ratio: 2 / 3;
    border-radius: clamp(9px, 0.9vw, 15px);
    overflow: hidden;
    background: var(--bg-2);
    box-shadow:
      0 14px 34px rgba(0, 0, 0, 0.42),
      inset 0 0 0 1px rgba(255, 255, 255, 0.05);
    transition: transform 160ms var(--ease-out);
  }

  .frame::after {
    content: "";
    position: absolute;
    inset: 0;
    border-radius: inherit;
    box-shadow:
      inset 0 0 0 2px rgba(255, 255, 255, 0.92),
      inset 0 0 0 4px color-mix(in srgb, var(--accent) 55%, transparent),
      0 0 0 1px rgba(255, 255, 255, 0.24),
      0 0 34px 3px var(--accent-glow),
      0 30px 70px rgba(0, 0, 0, 0.6);
    opacity: 0;
    transition: opacity 240ms var(--ease-out);
    pointer-events: none;
    z-index: 4;
  }

  .tile.active .frame::after {
    opacity: 1;
  }

  .state-tag {
    position: absolute;
    top: 9px;
    left: 9px;
    z-index: 3;
    padding: 4px 8px;
    border-radius: 999px;
    font-size: 8.5px;
    font-weight: 700;
    letter-spacing: 0.09em;
    text-transform: uppercase;
    color: rgba(255, 255, 255, 0.9);
    background: rgba(5, 8, 16, 0.68);
    backdrop-filter: blur(10px);
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.14);
  }

  .fav-tag {
    position: absolute;
    top: 8px;
    right: 9px;
    z-index: 3;
    font-size: 14px;
    line-height: 1;
    color: #ffd166;
    filter: drop-shadow(0 1px 4px rgba(0, 0, 0, 0.6));
  }

  .install-hint {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    z-index: 3;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    padding: 20px 0 10px;
    font-size: 9.5px;
    font-weight: 650;
    letter-spacing: 0.13em;
    text-transform: uppercase;
    color: rgba(255, 255, 255, 0.9);
    background: linear-gradient(180deg, transparent, rgba(4, 6, 11, 0.82));
    opacity: 0;
    transform: translateY(4px);
    transition:
      opacity 220ms var(--ease-out),
      transform 220ms var(--ease-out);
  }

  .tile.active .install-hint {
    opacity: 1;
    transform: translateY(0);
  }

  .install-icon {
    display: grid;
    place-items: center;
    width: 14px;
    height: 14px;
    border-radius: 50%;
    font-size: 9px;
    background: rgba(255, 255, 255, 0.16);
  }

  .caption {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 0 2px;
    min-width: 0;
  }

  .line {
    display: flex;
    align-items: center;
    gap: 7px;
    min-width: 0;
  }

  .dot {
    width: 5px;
    height: 5px;
    border-radius: 50%;
    flex: 0 0 auto;
    opacity: 0.85;
  }

  .name {
    font-size: 11.5px;
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

  .meta {
    padding-left: 12px;
    font-size: 9.5px;
    letter-spacing: 0.015em;
    color: var(--fg-2);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    font-variant-numeric: tabular-nums;
  }

  @media (prefers-reduced-motion: reduce) {
    .tile {
      transition: opacity 140ms linear;
      transform: none;
    }
    .tile.active {
      transform: none;
    }
  }
</style>
