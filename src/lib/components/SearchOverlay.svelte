<!--
  Keyboard-first fuzzy search overlay (`/` or Ctrl/Cmd+K). No dependency:
  matching lives in `search.svelte.ts`. OWNER: Session I.
-->
<script lang="ts">
  import { onMount, tick } from "svelte";
  import { fade, scale } from "svelte/transition";
  import { platformShort } from "../format";
  import { favorites } from "../favorites.svelte";
  import { library, playtimeFor } from "../library.svelte";
  import { navigate } from "../router.svelte";
  import { closeSearch, searchGames, searchState } from "../search.svelte";
  import { playtimeLabel } from "../steam";
  import type { Game } from "../types";
  import GameArt from "./GameArt.svelte";
  import Icon from "./Icon.svelte";

  let query = $state("");
  let index = $state(0);
  let input = $state<HTMLInputElement | null>(null);

  const results = $derived(searchGames(query, library.games, favorites.ids, 18));
  const active = $derived(results.length === 0 ? 0 : Math.min(index, results.length - 1));
  const favoriteSet = $derived(new Set(favorites.ids));
  const total = $derived(library.games.length);

  $effect(() => {
    if (!searchState.open) return;
    query = "";
    index = 0;
    void tick().then(() => input?.focus());
  });

  function isTypingTarget(target: EventTarget | null): boolean {
    const element = target as HTMLElement | null;
    if (!element) return false;
    const tag = element.tagName;
    return tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT" || element.isContentEditable;
  }

  function openResult(game: Game): void {
    closeSearch();
    navigate({ name: "game", id: game.id });
  }

  function onKeydown(event: KeyboardEvent): void {
    if (searchState.open) {
      if (event.key === "Escape") {
        event.preventDefault();
        event.stopPropagation();
        closeSearch();
        return;
      }
      if (event.key === "ArrowDown" || event.key === "ArrowUp") {
        event.preventDefault();
        event.stopPropagation();
        if (results.length === 0) return;
        const delta = event.key === "ArrowDown" ? 1 : -1;
        index = (active + delta + results.length) % results.length;
        return;
      }
      if (event.key === "Enter") {
        const hit = results[active];
        if (hit) {
          event.preventDefault();
          event.stopPropagation();
          openResult(hit.game);
        }
      }
      return;
    }

    if (event.defaultPrevented || isTypingTarget(event.target)) return;
    const meta = event.ctrlKey || event.metaKey;
    if (meta && event.key.toLowerCase() === "k") {
      event.preventDefault();
      searchState.open = true;
      return;
    }
    if (!meta && !event.altKey && event.key === "/") {
      event.preventDefault();
      searchState.open = true;
    }
  }

  onMount(() => {
    window.addEventListener("keydown", onKeydown, true);
    return () => window.removeEventListener("keydown", onKeydown, true);
  });
</script>

{#if searchState.open}
  <div
    class="scrim"
    role="presentation"
    transition:fade={{ duration: 160 }}
    onclick={() => closeSearch()}
  >
    <dialog
      open
      class="panel"
      aria-modal="true"
      aria-label="Search your library"
      transition:scale={{ duration: 220, start: 0.985, opacity: 0 }}
      onclick={(event) => event.stopPropagation()}
    >
      <div class="search-row">
        <span class="search-icon" aria-hidden="true"><Icon name="search" size={20} /></span>
        <input
          bind:this={input}
          bind:value={query}
          type="text"
          class="input"
          placeholder="Search your library…"
          autocomplete="off"
          spellcheck="false"
          aria-label="Search your library"
          oninput={() => (index = 0)}
        />
        <kbd class="esc">Esc</kbd>
      </div>

      {#if results.length > 0}
        <ul class="results" role="listbox" aria-label="Search results">
          {#each results as hit, i (hit.game.id)}
            <li role="option" aria-selected={i === active}>
              <button
                type="button"
                class="hit"
                class:active={i === active}
                onmouseenter={() => (index = i)}
                onclick={() => openResult(hit.game)}
              >
                <span class="thumb"><GameArt game={hit.game} eager={i < 5} /></span>
                <span class="hit-copy">
                  <span class="hit-title">
                    {hit.game.title}
                    {#if favoriteSet.has(hit.game.id)}
                      <span class="star" aria-label="Favorite">★</span>
                    {/if}
                  </span>
                  <span class="hit-meta">
                    {platformShort(hit.game.platform)}
                    <span class="sep">·</span>
                    {hit.game.isInstalled ? "Installed" : "Not installed"}
                    {#if playtimeLabel(playtimeFor(hit.game.id)?.minutes)}
                      <span class="sep">·</span>
                      {playtimeLabel(playtimeFor(hit.game.id)?.minutes)}
                    {/if}
                  </span>
                </span>
                <span class="hit-action" aria-hidden="true">
                  {#if i === active}
                    <span class="enter">Enter</span>
                  {/if}
                  <Icon name="arrow-right" size={15} />
                </span>
              </button>
            </li>
          {/each}
        </ul>
      {:else if query.trim().length > 0}
        <div class="empty">
          <span class="empty-icon"><Icon name="search" size={18} /></span>
          <p>No games match “{query.trim()}”.</p>
          <span>Try fewer letters or a different spelling.</span>
        </div>
      {:else if total === 0}
        <div class="empty">
          <span class="empty-icon"><Icon name="library" size={18} /></span>
          <p>Your library is empty.</p>
          <span>Go back and sync your Steam library first.</span>
        </div>
      {:else}
        <div class="empty">
          <span class="empty-icon"><Icon name="spark" size={18} /></span>
          <p>Start typing to find a game.</p>
          <span>{total} games in your library.</span>
        </div>
      {/if}

      <footer class="foot">
        <span class="hint"><kbd>↑</kbd><kbd>↓</kbd> Navigate</span>
        <span class="hint"><kbd>Enter</kbd> Open</span>
        <span class="hint"><kbd>Esc</kbd> Close</span>
        <span class="count">{total} games</span>
      </footer>
    </dialog>
  </div>
{/if}

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 150;
    display: flex;
    justify-content: center;
    align-items: flex-start;
    padding: clamp(48px, 12vh, 120px) 24px 24px;
    background:
      radial-gradient(80% 60% at 50% 0%, rgba(var(--accent-rgb), 0.12), transparent 70%),
      rgba(3, 4, 8, 0.72);
    backdrop-filter: blur(16px) saturate(1.1);
  }

  .panel {
    position: relative;
    width: min(760px, 100%);
    max-width: 100%;
    max-height: min(680px, calc(100vh - 160px));
    margin: 0;
    padding: 0;
    border: 0;
    color: inherit;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    border-radius: 22px;
    background: rgba(11, 14, 22, 0.94);
    box-shadow:
      inset 0 0 0 1px rgba(255, 255, 255, 0.1),
      0 40px 120px rgba(0, 0, 0, 0.7);
    will-change: transform, opacity;
  }

  .search-row {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 16px 18px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.07);
  }

  .search-icon {
    color: var(--fg-2);
    display: grid;
    place-items: center;
    flex: 0 0 auto;
  }

  .input {
    flex: 1;
    min-width: 0;
    height: 38px;
    font: inherit;
    font-size: 18px;
    font-weight: 500;
    letter-spacing: -0.015em;
    color: var(--fg-0);
    background: none;
    border: none;
    outline: none;
    user-select: text;
  }

  .input::placeholder {
    color: var(--fg-2);
    opacity: 0.72;
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

  .esc {
    flex: 0 0 auto;
    opacity: 0.7;
  }

  .results {
    list-style: none;
    margin: 0;
    padding: 8px;
    overflow-y: auto;
    scrollbar-width: thin;
  }

  .hit {
    display: flex;
    align-items: center;
    gap: 13px;
    width: 100%;
    padding: 9px 10px;
    border-radius: 12px;
    text-align: left;
    transition:
      background 160ms var(--ease-out),
      transform 160ms var(--ease-out);
  }

  .hit.active {
    background:
      linear-gradient(
        90deg,
        color-mix(in srgb, var(--accent) 20%, transparent),
        rgba(255, 255, 255, 0.045)
      );
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--accent) 34%, transparent);
  }

  .thumb {
    flex: 0 0 auto;
    width: 38px;
    aspect-ratio: 2 / 3;
    border-radius: 7px;
    overflow: hidden;
    background: var(--bg-2);
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.08);
  }

  .hit-copy {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  .hit-title {
    display: flex;
    align-items: center;
    gap: 7px;
    font-size: 13.5px;
    font-weight: 550;
    color: var(--fg-0);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .star {
    color: #ffd166;
    font-size: 11px;
    flex: 0 0 auto;
  }

  .hit-meta {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 10.5px;
    color: var(--fg-2);
    font-variant-numeric: tabular-nums;
  }

  .sep {
    opacity: 0.4;
  }

  .hit-action {
    flex: 0 0 auto;
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--fg-2);
  }

  .hit.active .hit-action {
    color: var(--fg-0);
  }

  .enter {
    font-size: 8.5px;
    font-weight: 650;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    padding: 2px 6px;
    border-radius: 5px;
    background: rgba(255, 255, 255, 0.08);
  }

  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 5px;
    padding: 46px 24px 50px;
    text-align: center;
  }

  .empty-icon {
    display: grid;
    place-items: center;
    width: 40px;
    height: 40px;
    border-radius: 12px;
    margin-bottom: 6px;
    color: var(--accent);
    background: color-mix(in srgb, var(--accent) 13%, transparent);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--accent) 24%, transparent);
  }

  .empty p {
    margin: 0;
    font-size: 13.5px;
    font-weight: 550;
    color: var(--fg-0);
  }

  .empty span {
    font-size: 11px;
    color: var(--fg-2);
  }

  .foot {
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 10px 18px 12px;
    border-top: 1px solid rgba(255, 255, 255, 0.06);
    font-size: 10px;
    color: var(--fg-2);
  }

  .hint {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    white-space: nowrap;
  }

  .count {
    margin-left: auto;
    font-variant-numeric: tabular-nums;
  }

  @media (max-width: 720px) {
    .scrim {
      padding: 24px 14px;
    }
    .foot .hint:nth-child(2) {
      display: none;
    }
  }
</style>
