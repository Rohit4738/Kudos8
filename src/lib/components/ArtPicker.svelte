<!--
  ART PICKER (Session M) - choose SteamGridDB artwork for one game.

  The picker only ever shows thumbnails; `applyArt` downloads the full image once
  and caches it locally. For non-Steam games it also copies the file into Steam's
  grid folder when the shortcut exists. Every failure stays in the dialog or a
  toast: the game's existing local cover is never lost.
-->
<script lang="ts">
  import { onMount, tick } from "svelte";
  import { fade, scale } from "svelte/transition";
  import { isTauriRuntime } from "../art";
  import {
    ART_KINDS,
    applyArt,
    getSecretStatus,
    searchArt,
    type ArtKind,
    type ArtResult,
  } from "../artProvider";
  import { onGamepad, type PadAction } from "../gamepad";
  import { navigate } from "../router.svelte";
  import { pushToast } from "../toast.svelte";
  import type { Game } from "../types";
  import ApiKeyGuide from "./ApiKeyGuide.svelte";
  import Icon from "./Icon.svelte";
  import Spinner from "./Spinner.svelte";

  interface Props {
    game: Game;
    onclose: () => void;
  }

  let { game, onclose }: Props = $props();

  type LoadState = "loading" | "ready" | "empty" | "error" | "nokey";

  const params =
    typeof window === "undefined"
      ? new URLSearchParams()
      : new URLSearchParams(window.location.search);
  const mock = params.get("mock") === "1";
  const mockArt = params.get("art") ?? "results";

  const MOCK_LABELS = [
    "Aurora",
    "Midnight",
    "Ember",
    "Voyager",
    "Static",
    "Neon",
    "Frost",
    "Echo",
  ];
  const COLUMNS: Record<ArtKind, number> = {
    grid: 5,
    hero: 2,
    logo: 4,
    icon: 4,
  };

  let kind = $state<ArtKind>("grid");
  let loadState = $state<LoadState>("loading");
  let results = $state<ArtResult[]>([]);
  let error = $state<string | null>(null);
  let selected = $state(0);
  let applying = $state<string | null>(null);
  let applied = $state<string | null>(null);
  let request = 0;
  let panel = $state<HTMLElement | null>(null);
  let list = $state<HTMLElement | null>(null);

  const kindInfo = $derived(ART_KINDS.find((option) => option.kind === kind));

  onMount(() => {
    void load(kind);
    void tick().then(() => panel?.focus());

    const stopGamepad = onGamepad(onPad);
    window.addEventListener("keydown", onKeydown, true);
    return () => {
      stopGamepad();
      window.removeEventListener("keydown", onKeydown, true);
    };
  });

  async function load(next: ArtKind): Promise<void> {
    const token = ++request;
    error = null;
    applied = null;
    selected = 0;
    loadState = "loading";
    results = [];

    if (mock) {
      await wait(420);
      if (token !== request) return;
      if (mockArt === "nokey") {
        loadState = "nokey";
        return;
      }
      if (mockArt === "error") {
        error =
          "SteamGridDB rate limit reached (HTTP 429). Try again in a minute.";
        loadState = "error";
        return;
      }
      if (mockArt === "empty") {
        loadState = "empty";
        return;
      }
      results = mockResults(next);
      loadState = "ready";
      return;
    }

    if (!isTauriRuntime()) {
      loadState = "nokey";
      return;
    }

    const secret = await getSecretStatus("steamgriddb");
    if (token !== request) return;
    if (!secret.hasKey) {
      loadState = "nokey";
      return;
    }

    try {
      const found = await searchArt(game.id, next);
      if (token !== request) return;
      results = found;
      loadState = found.length === 0 ? "empty" : "ready";
    } catch (err) {
      if (token !== request) return;
      error = typeof err === "string" ? err : "Could not reach SteamGridDB.";
      loadState = "error";
    }
  }

  function chooseKind(next: ArtKind): void {
    if (next === kind) return;
    kind = next;
    void load(next);
  }

  function cycleKind(delta: number): void {
    const index = ART_KINDS.findIndex((option) => option.kind === kind);
    const next = ART_KINDS[(index + delta + ART_KINDS.length) % ART_KINDS.length];
    if (next) chooseKind(next.kind);
  }

  /** Real column count from the rendered grid (responsive-safe). */
  function columns(): number {
    if (list) {
      const value = getComputedStyle(list).gridTemplateColumns;
      const count = value.split(" ").filter(Boolean).length;
      if (count > 0) return count;
    }
    return COLUMNS[kind];
  }

  function move(delta: number): void {
    if (results.length === 0) return;
    selected = (selected + delta + results.length) % results.length;
    void tick().then(() => {
      list
        ?.querySelector<HTMLElement>(`[data-index="${selected}"]`)
        ?.scrollIntoView({ block: "nearest" });
    });
  }

  async function apply(candidate: ArtResult): Promise<void> {
    if (applying) return;
    applying = candidate.id;
    try {
      if (mock) {
        await wait(700);
        applied = candidate.id;
        pushToast(
          "success",
          "Artwork applied",
          `The ${kind} art is now used for “${game.title}”.`,
        );
        return;
      }

      const result = await applyArt(game.id, candidate.url, kind, true);
      applied = candidate.id;

      if (result.wroteToSteam) {
        pushToast(
          "success",
          "Artwork applied",
          "Saved locally and copied into Steam's grid folder.",
        );
      } else if (result.steamError) {
        pushToast(
          "info",
          "Artwork saved locally",
          `Steam was not updated: ${result.steamError}`,
        );
      } else {
        pushToast(
          "success",
          "Artwork applied",
          `The ${kind} art is now used for “${game.title}”.`,
        );
      }
    } catch (err) {
      pushToast(
        "error",
        "Couldn't apply artwork",
        typeof err === "string" ? err : "The image could not be saved.",
      );
    } finally {
      applying = null;
    }
  }

  function onKeydown(event: KeyboardEvent): void {
    if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      onclose();
      return;
    }

    if (event.key === "ArrowLeft") {
      event.preventDefault();
      event.stopPropagation();
      move(-1);
    } else if (event.key === "ArrowRight") {
      event.preventDefault();
      event.stopPropagation();
      move(1);
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      event.stopPropagation();
      move(-columns());
    } else if (event.key === "ArrowDown") {
      event.preventDefault();
      event.stopPropagation();
      move(columns());
    } else if (event.key === "Enter") {
      const hit = results[selected];
      if (hit) {
        event.preventDefault();
        event.stopPropagation();
        void apply(hit);
      }
    } else if (/^[1-4]$/.test(event.key)) {
      const option = ART_KINDS[Number(event.key) - 1];
      if (option) {
        event.preventDefault();
        event.stopPropagation();
        chooseKind(option.kind);
      }
    } else if (event.key === "[" || event.key === "]") {
      event.preventDefault();
      event.stopPropagation();
      cycleKind(event.key === "[" ? -1 : 1);
    }
  }

  function onPad(action: PadAction): void {
    if (action === "back") {
      onclose();
      return;
    }
    if (action === "tabLeft") {
      cycleKind(-1);
      return;
    }
    if (action === "tabRight") {
      cycleKind(1);
      return;
    }
    if (action === "left") move(-1);
    else if (action === "right") move(1);
    else if (action === "up") move(-columns());
    else if (action === "down") move(columns());
    else if (action === "confirm") {
      const hit = results[selected];
      if (hit) void apply(hit);
    }
  }

  function openSettings(): void {
    onclose();
    navigate({ name: "settings" });
  }

  function thumbError(event: Event, candidate: ArtResult): void {
    const image = event.currentTarget as HTMLImageElement;
    if (image.dataset.fallback !== "1" && candidate.url !== candidate.thumbUrl) {
      image.dataset.fallback = "1";
      image.src = candidate.url;
      return;
    }
    image.style.opacity = "0";
  }

  function wait(ms: number): Promise<void> {
    return new Promise((resolve) => setTimeout(resolve, ms));
  }

  function mockResults(next: ArtKind): ArtResult[] {
    return MOCK_LABELS.map((label, index) => {
      const url = mockSvg(next, index, label);
      return {
        id: `${next}-${index}`,
        kind: next,
        url,
        thumbUrl: url,
        width: next === "hero" ? 1920 : next === "icon" ? 512 : 600,
        height: next === "hero" ? 620 : next === "grid" ? 900 : next === "logo" ? 360 : 512,
        author: index % 3 === 0 ? "Community" : undefined,
      };
    });
  }

  function mockSvg(next: ArtKind, index: number, label: string): string {
    const width =
      next === "hero" ? 1920 : next === "icon" ? 512 : next === "logo" ? 640 : 600;
    const height =
      next === "hero" ? 620 : next === "icon" ? 512 : next === "logo" ? 360 : 900;
    const hue = (205 + index * 29) % 360;
    const hue2 = (hue + 48) % 360;
    const titleSize = Math.round(Math.min(width, height) * 0.12);
    const subSize = Math.round(Math.min(width, height) * 0.038);
    const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}" viewBox="0 0 ${width} ${height}"><defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="hsl(${hue} 68% 42%)"/><stop offset="1" stop-color="hsl(${hue2} 72% 16%)"/></linearGradient></defs><rect width="100%" height="100%" fill="url(#g)"/><circle cx="${width * 0.74}" cy="${height * 0.26}" r="${Math.min(width, height) * 0.2}" fill="rgba(255,255,255,0.12)"/><circle cx="${width * 0.2}" cy="${height * 0.82}" r="${Math.min(width, height) * 0.14}" fill="rgba(255,255,255,0.07)"/><text x="${width / 2}" y="${height / 2}" fill="rgba(255,255,255,0.94)" font-family="Segoe UI, sans-serif" font-size="${titleSize}" font-weight="700" text-anchor="middle" dominant-baseline="middle">${label}</text><text x="${width / 2}" y="${height / 2 + titleSize}" fill="rgba(255,255,255,0.6)" font-family="Segoe UI, sans-serif" font-size="${subSize}" letter-spacing="${subSize * 0.18}" text-anchor="middle">${next.toUpperCase()} · MOCK</text></svg>`;
    return `data:image/svg+xml,${encodeURIComponent(svg)}`;
  }
</script>

<div
  class="scrim"
  role="presentation"
  transition:fade={{ duration: 160 }}
  onclick={() => onclose()}
>
  <dialog
    open
    bind:this={panel}
    class="panel"
    aria-modal="true"
    aria-labelledby="art-picker-title"
    aria-busy={loadState === "loading"}
    tabindex="-1"
    transition:scale={{ duration: 220, start: 0.985, opacity: 0 }}
    onclick={(event) => event.stopPropagation()}
  >
    <header class="head">
      <div class="head-copy">
        <p class="eyebrow">Change artwork</p>
        <h2 id="art-picker-title">{game.title}</h2>
        <p class="sub">
          {kindInfo?.label ?? "Grid"} · {kindInfo?.ratio ?? ""} — provided by
          SteamGridDB
        </p>
      </div>
      <button
        type="button"
        class="close"
        aria-label="Close artwork picker"
        onclick={() => onclose()}
      >
        <Icon name="close" size={18} />
      </button>
    </header>

    <div class="tabs" role="tablist" aria-label="Artwork type">
      {#each ART_KINDS as option, index (option.kind)}
        <button
          type="button"
          role="tab"
          class="tab"
          class:active={kind === option.kind}
          aria-selected={kind === option.kind}
          onclick={() => chooseKind(option.kind)}
        >
          <span class="tab-index">{index + 1}</span>
          <span class="tab-copy">
            <span class="tab-label">{option.label}</span>
            <span class="tab-ratio">{option.ratio}</span>
          </span>
        </button>
      {/each}
    </div>

    <div class="body">
      {#if loadState === "loading"}
        <div class="status" role="status">
          <Spinner size={22} />
          <p>Looking for {kind} art…</p>
          <span>One quick request to SteamGridDB.</span>
        </div>
      {:else if loadState === "nokey"}
        <div class="status no-key">
          <span class="status-icon"><Icon name="spark" size={20} /></span>
          <p>Connect SteamGridDB to change artwork.</p>
          <span>
            Until then Kudos keeps the store cover, the local GOG.ico, or the
            gradient initials.
          </span>
          <ApiKeyGuide provider="steamgriddb" open />
          <button type="button" class="primary" onclick={openSettings}>
            Open Settings → Artwork
          </button>
        </div>
      {:else if loadState === "error"}
        <div class="status error" role="alert">
          <span class="status-icon"><Icon name="alert" size={20} /></span>
          <p>Couldn't load artwork</p>
          <span>{error}</span>
          <button type="button" class="primary" onclick={() => void load(kind)}>
            <Icon name="refresh" size={14} />
            Try again
          </button>
        </div>
      {:else if loadState === "empty"}
        <div class="status">
          <span class="status-icon"><Icon name="search" size={20} /></span>
          <p>No {kind} art found</p>
          <span>
            SteamGridDB has no approved {kind} art for “{game.title}”. Try another
            type; your current cover stays as it is.
          </span>
        </div>
      {:else}
        <div
          class="grid"
          class:hero={kind === "hero"}
          class:logo={kind === "logo" || kind === "icon"}
          role="listbox"
          aria-label={`${kindInfo?.label ?? "Artwork"} candidates`}
          bind:this={list}
        >
          {#each results as candidate, index (candidate.id)}
            <button
              type="button"
              class="tile"
              class:active={selected === index}
              class:applied={applied === candidate.id}
              data-index={index}
              role="option"
              aria-selected={selected === index}
              aria-label={`${kindInfo?.label ?? "Artwork"} ${index + 1}${candidate.author ? ` by ${candidate.author}` : ""}`}
              disabled={applying !== null}
              onmouseenter={() => (selected = index)}
              onclick={() => {
                selected = index;
                void apply(candidate);
              }}
            >
              <img
                src={candidate.thumbUrl || candidate.url}
                alt=""
                loading={index < 8 ? "eager" : "lazy"}
                decoding="async"
                draggable="false"
                onerror={(event) => thumbError(event, candidate)}
              />
              <span class="tile-shade" aria-hidden="true"></span>
              {#if candidate.author}
                <span class="author">{candidate.author}</span>
              {/if}
              {#if applying === candidate.id}
                <span class="tile-state"><Spinner size={18} /></span>
              {:else if applied === candidate.id}
                <span class="tile-state applied">
                  <Icon name="check" size={13} />
                  Applied
                </span>
              {/if}
            </button>
          {/each}
        </div>
      {/if}
    </div>

    <footer class="foot">
      <span class="hint"><kbd>1</kbd>–<kbd>4</kbd> Type</span>
      <span class="hint"><kbd>↑</kbd><kbd>↓</kbd><kbd>←</kbd><kbd>→</kbd> Browse</span>
      <span class="hint"><kbd>Enter</kbd> Apply</span>
      <span class="hint"><kbd>Esc</kbd> Close</span>
      <span class="attribution">
        Artwork provided by
        <a href="https://www.steamgriddb.com" target="_blank" rel="noreferrer"
          >SteamGridDB</a
        >
      </span>
    </footer>
  </dialog>
</div>

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 160;
    display: flex;
    justify-content: center;
    align-items: center;
    padding: clamp(18px, 4vh, 44px);
    background:
      radial-gradient(70% 60% at 50% 0%, rgba(var(--accent-rgb), 0.12), transparent 70%),
      rgba(3, 4, 8, 0.78);
    backdrop-filter: blur(16px) saturate(1.1);
  }

  .panel {
    position: relative;
    width: min(1080px, 100%);
    max-height: min(88vh, 880px);
    margin: 0;
    padding: 0;
    border: 0;
    color: inherit;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    border-radius: 24px;
    background: rgba(11, 14, 22, 0.96);
    box-shadow:
      inset 0 0 0 1px rgba(255, 255, 255, 0.1),
      0 40px 120px rgba(0, 0, 0, 0.72);
    outline: none;
    will-change: transform, opacity;
  }

  .head {
    display: flex;
    align-items: flex-start;
    gap: 18px;
    padding: 20px 22px 14px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.07);
  }

  .head-copy {
    flex: 1;
    min-width: 0;
  }

  .eyebrow {
    margin: 0 0 5px;
    font-size: 9.5px;
    font-weight: 650;
    letter-spacing: 0.22em;
    text-transform: uppercase;
    color: var(--accent);
  }

  h2 {
    margin: 0;
    font-family: var(--font-display);
    font-size: clamp(20px, 2.4vw, 30px);
    font-weight: 600;
    letter-spacing: -0.03em;
    line-height: 1.05;
    overflow-wrap: anywhere;
  }

  .sub {
    margin: 7px 0 0;
    font-size: 11.5px;
    color: var(--fg-2);
  }

  .close {
    flex: 0 0 auto;
    display: grid;
    place-items: center;
    width: 34px;
    height: 34px;
    border-radius: 50%;
    color: var(--fg-1);
    background: rgba(255, 255, 255, 0.06);
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.08);
    transition:
      transform var(--dur-fast, 140ms) var(--ease-out, ease),
      color var(--dur-fast, 140ms);
  }

  .close:hover {
    color: var(--fg-0);
    transform: rotate(90deg);
  }

  .tabs {
    display: flex;
    gap: 6px;
    padding: 12px 18px;
    overflow-x: auto;
    scrollbar-width: none;
    border-bottom: 1px solid rgba(255, 255, 255, 0.06);
  }

  .tab {
    flex: 0 0 auto;
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 8px 13px 8px 9px;
    border-radius: 12px;
    color: var(--fg-2);
    background: rgba(255, 255, 255, 0.035);
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.055);
    transition:
      background 160ms var(--ease-out, ease),
      color 160ms var(--ease-out, ease),
      transform 160ms var(--ease-out, ease);
  }

  .tab:hover {
    color: var(--fg-0);
    transform: translateY(-1px);
  }

  .tab.active {
    color: var(--fg-0);
    background: color-mix(in srgb, var(--accent) 22%, rgba(255, 255, 255, 0.05));
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--accent) 40%, transparent);
  }

  .tab-index {
    display: grid;
    place-items: center;
    width: 19px;
    height: 19px;
    border-radius: 6px;
    font-size: 9px;
    font-weight: 700;
    color: var(--fg-2);
    background: rgba(255, 255, 255, 0.07);
  }

  .tab.active .tab-index {
    color: #05070c;
    background: var(--accent);
  }

  .tab-copy {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 1px;
  }

  .tab-label {
    font-size: 12px;
    font-weight: 600;
  }

  .tab-ratio {
    font-size: 9px;
    color: var(--fg-2);
    letter-spacing: 0.02em;
  }

  .body {
    flex: 1;
    min-height: 260px;
    overflow-y: auto;
    scrollbar-width: thin;
    padding: 16px 18px 18px;
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(5, minmax(0, 1fr));
    gap: 12px;
  }

  .grid.hero {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

  .grid.logo {
    grid-template-columns: repeat(4, minmax(0, 1fr));
  }

  .tile {
    position: relative;
    display: block;
    width: 100%;
    padding: 0;
    border: 0;
    aspect-ratio: 2 / 3;
    border-radius: 12px;
    overflow: hidden;
    background: rgba(255, 255, 255, 0.04);
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.08);
    cursor: pointer;
    transition:
      transform 180ms var(--ease-out, ease),
      box-shadow 180ms var(--ease-out, ease),
      opacity 180ms var(--ease-out, ease);
    will-change: transform;
  }

  .grid.hero .tile {
    aspect-ratio: 1920 / 620;
  }

  .grid.logo .tile {
    aspect-ratio: 16 / 9;
  }

  .tile img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    transition: transform 260ms var(--ease-out, ease), opacity 200ms;
    will-change: transform;
  }

  .tile:hover:not(:disabled) {
    transform: translateY(-2px);
    box-shadow:
      inset 0 0 0 1px rgba(255, 255, 255, 0.16),
      0 12px 30px rgba(0, 0, 0, 0.35);
  }

  .tile:hover:not(:disabled) img {
    transform: scale(1.035);
  }

  .tile.active {
    box-shadow:
      inset 0 0 0 2px var(--accent),
      0 0 0 1px color-mix(in srgb, var(--accent) 50%, transparent),
      0 16px 38px rgba(0, 0, 0, 0.4);
  }

  .tile:disabled {
    cursor: default;
  }

  .tile-shade {
    position: absolute;
    inset: 0;
    background: linear-gradient(180deg, transparent 62%, rgba(3, 5, 10, 0.72) 100%);
    pointer-events: none;
  }

  .author {
    position: absolute;
    left: 8px;
    bottom: 7px;
    max-width: calc(100% - 16px);
    font-size: 9px;
    font-weight: 550;
    color: rgba(255, 255, 255, 0.66);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    pointer-events: none;
  }

  .tile-state {
    position: absolute;
    top: 8px;
    right: 8px;
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 4px 7px;
    border-radius: 999px;
    font-size: 9px;
    font-weight: 650;
    letter-spacing: 0.03em;
    color: #05070c;
    background: rgba(255, 255, 255, 0.9);
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
    animation: pop 180ms var(--ease-out, ease);
  }

  .tile-state.applied {
    background: var(--ok);
    color: #04120b;
  }

  @keyframes pop {
    from {
      opacity: 0;
      transform: scale(0.86);
    }
    to {
      opacity: 1;
      transform: none;
    }
  }

  .status {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    max-width: 560px;
    margin: 0 auto;
    padding: 44px 22px 52px;
    text-align: center;
  }

  .status p {
    margin: 0;
    font-size: 14px;
    font-weight: 600;
    color: var(--fg-0);
  }

  .status > span:not(.status-icon) {
    font-size: 11.5px;
    line-height: 1.55;
    color: var(--fg-2);
  }

  .status-icon {
    display: grid;
    place-items: center;
    width: 44px;
    height: 44px;
    margin-bottom: 4px;
    border-radius: 13px;
    color: var(--accent);
    background: color-mix(in srgb, var(--accent) 13%, transparent);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--accent) 24%, transparent);
  }

  .status.error .status-icon {
    color: var(--danger);
    background: rgba(255, 95, 109, 0.12);
    box-shadow: inset 0 0 0 1px rgba(255, 95, 109, 0.24);
  }

  .status.no-key :global(.guide) {
    width: 100%;
    margin-top: 14px;
    text-align: left;
  }

  .primary {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    margin-top: 14px;
    padding: 9px 15px;
    border-radius: 999px;
    font-size: 12px;
    font-weight: 650;
    color: #05070c;
    background: var(--accent);
    transition: transform var(--dur-fast, 140ms) var(--ease-out, ease);
  }

  .primary:hover {
    transform: translateY(-1px);
  }

  .foot {
    display: flex;
    align-items: center;
    gap: 13px;
    flex-wrap: wrap;
    padding: 10px 20px 12px;
    border-top: 1px solid rgba(255, 255, 255, 0.06);
    font-size: 10px;
    color: var(--fg-2);
  }

  .hint {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    white-space: nowrap;
  }

  kbd {
    font-family: var(--font-ui);
    font-size: 9px;
    font-weight: 650;
    color: var(--fg-1);
    background: rgba(255, 255, 255, 0.07);
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.1);
    border-radius: 4px;
    padding: 1px 4px;
    line-height: 1.4;
  }

  .attribution {
    margin-left: auto;
    font-size: 10px;
    color: var(--fg-2);
  }

  .attribution a {
    color: var(--accent);
    text-decoration: none;
  }

  @media (max-width: 720px) {
    .scrim {
      padding: 12px;
    }
    .panel {
      max-height: 94vh;
      border-radius: 18px;
    }
    .grid,
    .grid.logo {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
    .grid.hero {
      grid-template-columns: 1fr;
    }
    .foot .hint:nth-child(2),
    .foot .hint:nth-child(3) {
      display: none;
    }
    .attribution {
      margin-left: 0;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .panel,
    .tile,
    .tile img,
    .tab,
    .close,
    .primary,
    .tile-state {
      transition: none;
      animation: none;
    }
    .close:hover,
    .tab:hover,
    .tile:hover:not(:disabled),
    .tile:hover:not(:disabled) img,
    .primary:hover {
      transform: none;
    }
  }
</style>
