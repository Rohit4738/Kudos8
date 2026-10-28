<!--
  RECENTLY PLAYED — a compact row above the main library rail.
  OWNER: Session G.
  Self-contained: pass the library in, it resolves ids against it and hides
  itself when there is nothing recent. Reuses GameArt so art/fallbacks match.
-->
<script lang="ts">
  import { onMount } from "svelte";
  import { loadRecent, recent } from "../history.svelte";
  import { relativePlayed, resolveRecent, type RecentGame } from "../recent";
  import type { Game } from "../types";
  import GameArt from "./GameArt.svelte";

  interface Props {
    games: Game[];
    onOpen: (game: Game) => void;
    /** Optional override for previews/tests; defaults to the shared store. */
    entries?: RecentGame[];
    limit?: number;
    label?: string;
  }

  let { games, onOpen, entries, limit = 10, label = "Recently played" }: Props = $props();

  const feed = $derived(entries ?? recent.entries);
  const items = $derived(resolveRecent(feed, games).slice(0, limit));

  onMount(() => {
    if (entries === undefined) {
      // Force a refresh: the launch journal may have changed since the last visit.
      void loadRecent(true);
    }
  });
</script>

{#if items.length > 0}
  <section class="recent" aria-label={label}>
    <header class="head">
      <span class="eyebrow">{label}</span>
      <span class="rule" aria-hidden="true"></span>
    </header>

    <div class="row">
      {#each items as item, index (item.entry.id)}
        <button
          type="button"
          class="tile"
          onclick={() => onOpen(item.game)}
          aria-label={`Open ${item.game.title}`}
        >
          <span class="frame">
            <GameArt game={item.game} eager={index < 4} />
          </span>
          <span class="caption">
            <span class="name">{item.game.title}</span>
            <span class="when">{relativePlayed(item.entry.lastPlayedUnix)}</span>
          </span>
        </button>
      {/each}
    </div>
  </section>
{/if}

<style>
  .recent {
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-width: 0;
  }

  .head {
    display: flex;
    align-items: center;
    gap: 12px;
    min-height: 14px;
  }

  .eyebrow {
    flex: 0 0 auto;
    font-size: 9.5px;
    font-weight: 650;
    letter-spacing: 0.2em;
    text-transform: uppercase;
    color: var(--fg-2);
  }

  .rule {
    flex: 1;
    height: 1px;
    background: linear-gradient(
      90deg,
      rgba(255, 255, 255, 0.09),
      rgba(255, 255, 255, 0.015) 70%,
      transparent
    );
  }

  .row {
    display: flex;
    align-items: flex-start;
    gap: clamp(10px, 1.2vw, 16px);
    overflow-x: auto;
    overflow-y: hidden;
    padding: 2px 2px 4px;
    scrollbar-width: none;
  }

  .row::-webkit-scrollbar {
    display: none;
  }

  .tile {
    --recent-w: clamp(72px, 9.5vh, 96px);
    flex: 0 0 auto;
    width: var(--recent-w);
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 0;
    text-align: left;
    border-radius: 10px;
    opacity: 0.78;
    transform: translateZ(0);
    transition:
      transform 220ms var(--ease-out),
      opacity 220ms var(--ease-out);
    will-change: transform, opacity;
  }

  .tile:hover,
  .tile:focus-visible {
    opacity: 1;
    transform: translateY(-2px) scale(1.03);
  }

  .frame {
    position: relative;
    display: block;
    width: 100%;
    aspect-ratio: 2 / 3;
    border-radius: clamp(7px, 0.8vw, 10px);
    overflow: hidden;
    background: var(--bg-2);
    box-shadow:
      0 6px 18px rgba(0, 0, 0, 0.34),
      inset 0 0 0 1px rgba(255, 255, 255, 0.06);
  }

  .caption {
    display: flex;
    flex-direction: column;
    gap: 1px;
    padding: 0 1px;
    min-width: 0;
  }

  .name {
    font-size: 10px;
    font-weight: 550;
    line-height: 1.3;
    color: var(--fg-1);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .when {
    font-size: 8.5px;
    letter-spacing: 0.02em;
    color: var(--fg-2);
  }

  @media (prefers-reduced-motion: reduce) {
    .tile {
      transition: opacity 160ms linear;
    }
    .tile:hover,
    .tile:focus-visible {
      transform: none;
    }
  }
</style>
