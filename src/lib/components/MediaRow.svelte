<!--
  One independently focusable, collapsible Home row.
  OWNER: Session I.
-->
<script lang="ts">
  import { tick } from "svelte";
  import type { Game } from "../types";
  import Icon from "./Icon.svelte";
  import RowTile from "./RowTile.svelte";

  type RowSize = "lg" | "md" | "sm";

  interface Props {
    title: string;
    items: Game[];
    size?: RowSize;
    active: boolean;
    activeIndex: number;
    metaFor?: (game: Game) => string;
    favorites?: string[];
    subtitle?: string;
    onFocus: (index: number) => void;
    onOpen: (game: Game) => void;
    onToggleFavorite: (game: Game) => void;
  }

  let {
    title,
    items,
    size = "md",
    active,
    activeIndex,
    metaFor = () => "",
    favorites = [],
    subtitle = "",
    onFocus,
    onOpen,
    onToggleFavorite,
  }: Props = $props();

  let collapsed = $state(false);
  let viewport = $state<HTMLDivElement | null>(null);
  let scrollFrame = 0;
  const favoriteSet = $derived(new Set(favorites));

  $effect(() => {
    // Only the focused row follows its selection; other rows keep their offset.
    if (!active || collapsed) return;
    const index = activeIndex;
    const count = items.length;
    if (count === 0) return;

    void tick().then(() => {
      if (!viewport) return;
      const tile = viewport.querySelectorAll<HTMLElement>(".tile")[index];
      if (!tile) return;

      const left = tile.offsetLeft - viewport.clientWidth / 2 + tile.offsetWidth / 2;
      cancelAnimationFrame(scrollFrame);
      scrollFrame = requestAnimationFrame(() => {
        const reduced =
          typeof window !== "undefined" &&
          window.matchMedia("(prefers-reduced-motion: reduce)").matches;
        viewport?.scrollTo({
          left: Math.max(0, left),
          behavior: reduced ? "auto" : "smooth",
        });
      });
    });

    return () => cancelAnimationFrame(scrollFrame);
  });
</script>

<section
  class="row"
  class:active
  class:collapsed
  class:lg={size === "lg"}
  class:sm={size === "sm"}
  aria-label={title}
>
  <header class="head">
    <button
      type="button"
      class="collapse"
      aria-expanded={!collapsed}
      aria-label={`${collapsed ? "Expand" : "Collapse"} ${title}`}
      onclick={() => (collapsed = !collapsed)}
    >
      <span class="kicker">{title}</span>
      {#if subtitle}<span class="subtitle">{subtitle}</span>{/if}
      <span class="count">{items.length}</span>
      {#if active}
        <span class="browsing">Browsing</span>
      {/if}
      <span class="chevron" class:up={!collapsed} aria-hidden="true">
        <Icon name="chevron" size={14} />
      </span>
    </button>
    <span class="rule" aria-hidden="true"></span>
  </header>

  {#if !collapsed}
    <div class="viewport" bind:this={viewport}>
      <div class="track">
        {#each items as game, index (game.id)}
          <RowTile
            {game}
            active={active && index === activeIndex}
            {index}
            meta={metaFor(game)}
            favorite={favoriteSet.has(game.id)}
            onFocus={(next) => {
              onFocus(next);
            }}
            {onOpen}
            {onToggleFavorite}
          />
        {/each}
      </div>
    </div>
  {/if}
</section>

<style>
  .row {
    position: relative;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 4px 8px 4px 14px;
    border-radius: var(--radius-md);
    opacity: 0.66;
    transition:
      opacity 240ms var(--ease-out),
      background 240ms var(--ease-out);
  }

  .row.active {
    opacity: 1;
    background: linear-gradient(
      90deg,
      rgba(var(--accent-rgb), 0.11),
      rgba(var(--accent-rgb), 0.03) 46%,
      transparent 72%
    );
  }

  /* Left accent bar: unmistakable "this is the section you are browsing". */
  .row::before {
    content: "";
    position: absolute;
    left: 0;
    top: 8px;
    bottom: 8px;
    width: 3px;
    border-radius: 3px;
    background: var(--accent);
    box-shadow: 0 0 14px rgba(var(--accent-rgb), 0.55);
    opacity: 0;
    transform: scaleY(0.35);
    transform-origin: center;
    transition:
      opacity 220ms var(--ease-out),
      transform 260ms var(--ease-out);
  }

  .row.active::before {
    opacity: 1;
    transform: none;
  }

  .head {
    display: flex;
    align-items: center;
    gap: 14px;
  }

  .collapse {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
    padding: 2px 0;
    text-align: left;
    color: var(--fg-2);
    transition: color 220ms var(--ease-out);
  }

  .row.active .collapse {
    color: var(--fg-1);
  }

  .kicker {
    flex: 0 0 auto;
    font-size: 10px;
    font-weight: 650;
    letter-spacing: 0.2em;
    text-transform: uppercase;
    color: inherit;
  }

  .row.active .kicker {
    color: var(--accent);
  }

  .browsing {
    flex: 0 0 auto;
    padding: 2px 8px;
    border-radius: 999px;
    background: rgba(var(--accent-rgb), 0.18);
    box-shadow: inset 0 0 0 1px rgba(var(--accent-rgb), 0.38);
    color: var(--accent);
    font-size: 8.5px;
    font-weight: 700;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    animation: browsing-in 180ms var(--ease-out);
  }

  @keyframes browsing-in {
    from {
      opacity: 0;
      transform: translateX(-4px);
    }
    to {
      opacity: 1;
      transform: none;
    }
  }

  .subtitle {
    min-width: 0;
    font-size: 9.5px;
    letter-spacing: 0.02em;
    color: var(--fg-2);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    opacity: 0.8;
  }

  .count {
    flex: 0 0 auto;
    font-size: 9px;
    font-weight: 650;
    letter-spacing: 0.06em;
    color: var(--fg-2);
    padding: 1px 6px;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.05);
    font-variant-numeric: tabular-nums;
  }

  .chevron {
    display: grid;
    place-items: center;
    opacity: 0.55;
    transition: transform 220ms var(--ease-out);
  }

  .chevron.up {
    transform: rotate(180deg);
  }

  .rule {
    flex: 1;
    height: 1px;
    background: linear-gradient(
      90deg,
      rgba(255, 255, 255, 0.095),
      rgba(255, 255, 255, 0.018) 72%,
      transparent
    );
  }

  .row.active .rule {
    background: linear-gradient(
      90deg,
      color-mix(in srgb, var(--accent) 42%, transparent),
      rgba(255, 255, 255, 0.02) 72%,
      transparent
    );
  }

  .viewport {
    position: relative;
    overflow-x: auto;
    overflow-y: hidden;
    padding: 6px 4px 14px;
    scrollbar-width: none;
    opacity: 1;
    transition: opacity 260ms var(--ease-out);
  }

  .viewport::-webkit-scrollbar {
    display: none;
  }

  .row.active .viewport {
    opacity: 1;
  }

  .track {
    --row-tile-w: calc(clamp(126px, 14.5vh, 164px) * var(--tile-scale, 1));
    display: flex;
    align-items: flex-start;
    gap: clamp(12px, 1.2vw, 18px);
    width: max-content;
    padding-inline: 4px;
    will-change: scroll-position;
  }

  .row.lg .track {
    --row-tile-w: calc(clamp(156px, 18.5vh, 208px) * var(--tile-scale, 1));
  }

  .row.sm .track {
    --row-tile-w: calc(clamp(98px, 11.5vh, 128px) * var(--tile-scale, 1));
  }

  .collapsed .viewport {
    display: none;
  }

  @media (prefers-reduced-motion: reduce) {
    .viewport {
      transition: opacity 140ms linear;
    }
    .chevron {
      transition: none;
    }
  }
</style>
