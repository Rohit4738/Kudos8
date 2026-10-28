<!--
  HOME / LIBRARY
  PS5-style horizontal rail, browse-before-commit info panel, keyboard + gamepad.
  OWNER: Session D.
-->
<script lang="ts">
  import { onMount, tick } from "svelte";
  import { fade } from "svelte/transition";
  import { coverSource, placeholderBackground } from "../lib/art";
  import { FILTERS, matchesFilter, type Filter, type FilterCounts } from "../lib/filter";
  import { loadLibrary, library } from "../lib/library.svelte";
  import { navigate } from "../lib/router.svelte";
  import { applyStoredAccent } from "../lib/theme";
  import { onGamepad } from "../lib/gamepad";
  import { restoreFullscreenPref } from "../lib/windowPrefs";
  import { PLATFORM_LABEL, type Game, type Platform } from "../lib/types";
  import FilterBar from "../lib/components/FilterBar.svelte";
  import GameTile from "../lib/components/GameTile.svelte";
  import Icon from "../lib/components/Icon.svelte";
  import InfoPanel from "../lib/components/InfoPanel.svelte";

  interface Ambient {
    key: string;
    image: string | null;
    background: string;
  }

  function ambientFor(game: Game | null): Ambient {
    if (!game) {
      return {
        key: "__empty",
        image: null,
        background: [
          "radial-gradient(1200px 720px at 72% -12%, rgba(48, 82, 160, 0.34), transparent 62%)",
          "radial-gradient(900px 640px at 6% 108%, rgba(104, 68, 180, 0.26), transparent 58%)",
        ].join(", "),
      };
    }
    return {
      key: game.id,
      image: coverSource(game.coverImage),
      background: placeholderBackground(game),
    };
  }

  const greeting = greetingFor(new Date().getHours());

  function greetingFor(hour: number): string {
    if (hour < 5) return "Still up?";
    if (hour < 12) return "Good morning";
    if (hour < 18) return "Good afternoon";
    return "Good evening";
  }

  function isTypingTarget(target: EventTarget | null): boolean {
    const element = target as HTMLElement | null;
    if (!element) return false;
    const tag = element.tagName;
    return (
      tag === "INPUT" ||
      tag === "TEXTAREA" ||
      tag === "SELECT" ||
      element.isContentEditable
    );
  }

  function isInteractiveTarget(target: EventTarget | null): boolean {
    const element = target as HTMLElement | null;
    if (!element) return false;
    return (
      element.tagName === "BUTTON" ||
      element.tagName === "A" ||
      element.tagName === "INPUT" ||
      element.tagName === "TEXTAREA" ||
      element.tagName === "SELECT" ||
      element.isContentEditable
    );
  }

  let filter = $state<Filter>("all");
  let activeIndex = $state(0);
  let offset = $state(0);

  let viewport = $state<HTMLDivElement | null>(null);
  let track = $state<HTMLDivElement | null>(null);
  let resizeFrame = 0;
  let ambientTimer: ReturnType<typeof setTimeout> | undefined;

  let ambientSnapshot = ambientFor(null);
  let ambient = $state<Ambient>(ambientSnapshot);
  let ambientPrev = $state<Ambient | null>(null);

  const visible = $derived(
    library.games.filter((game) => matchesFilter(game.platform, filter)),
  );
  const active = $derived(
    visible.length === 0 ? 0 : Math.min(activeIndex, visible.length - 1),
  );
  const selected = $derived(visible[active] ?? null);
  const counts = $derived<FilterCounts>({
    all: library.games.length,
    steam: library.games.filter((game) => game.platform === "steam").length,
    epic: library.games.filter((game) => game.platform === "epic").length,
    gog: library.games.filter((game) => game.platform === "gog").length,
  });
  const emptyCopy = $derived(
    filter === "all"
      ? "Your Steam, Epic and GOG libraries are empty. Install a game, then refresh."
      : `No ${PLATFORM_LABEL[filter as Platform]} games in your library yet.`,
  );

  $effect(() => {
    const next = ambientFor(selected);
    if (next.key === ambientSnapshot.key) return;

    ambientPrev = ambientSnapshot;
    ambientSnapshot = next;
    ambient = next;

    if (ambientTimer) clearTimeout(ambientTimer);
    ambientTimer = setTimeout(() => {
      ambientPrev = null;
    }, 900);
  });

  onMount(() => {
    applyStoredAccent();
    void restoreFullscreenPref();
    void refreshLibrary();

    const observer = new ResizeObserver(onResize);
    if (viewport) observer.observe(viewport);

    const stopGamepad = onGamepad(handlePad);

    return () => {
      observer.disconnect();
      stopGamepad();
      cancelAnimationFrame(resizeFrame);
      if (ambientTimer) clearTimeout(ambientTimer);
    };
  });

  async function refreshLibrary(force = false): Promise<void> {
    activeIndex = 0;
    offset = 0;
    await loadLibrary(force);
    await tick();
    centerOn(0);
  }

  function centerOn(index: number): void {
    if (!viewport || !track) return;
    const tile = track.children[index] as HTMLElement | undefined;
    if (!tile) {
      offset = 0;
      return;
    }
    const next = tile.offsetLeft + tile.offsetWidth / 2 - viewport.clientWidth / 2;
    offset = Number.isFinite(next) ? Math.max(0, next) : 0;
  }

  function onResize(): void {
    cancelAnimationFrame(resizeFrame);
    resizeFrame = requestAnimationFrame(() => centerOn(active));
  }

  function setFilter(next: Filter): void {
    if (next === filter) return;
    filter = next;
    activeIndex = 0;
    offset = 0;
  }

  function cycleFilter(direction: 1 | -1): void {
    const index = FILTERS.findIndex((entry) => entry.id === filter);
    const next = FILTERS[(index + direction + FILTERS.length) % FILTERS.length];
    if (next) setFilter(next.id);
  }

  function step(delta: 1 | -1): void {
    if (visible.length === 0) return;
    const next = (active + delta + visible.length) % visible.length;
    activeIndex = next;
    centerOn(next);
  }

  function select(index: number): void {
    // Mouse hover browses in place: the rail only glides for controller input.
    activeIndex = index;
  }

  function openGame(game: Game): void {
    navigate({ name: "game", id: game.id });
  }

  function handlePad(action: string): void {
    switch (action) {
      case "left":
        step(-1);
        break;
      case "right":
        step(1);
        break;
      case "up":
      case "tabLeft":
        cycleFilter(-1);
        break;
      case "down":
      case "tabRight":
        cycleFilter(1);
        break;
      case "confirm": {
        const game = visible[active];
        if (game) openGame(game);
        break;
      }
      case "menu":
        navigate({ name: "settings" });
        break;
    }
  }

  function onKeydown(event: KeyboardEvent): void {
    if (
      event.defaultPrevented ||
      event.ctrlKey ||
      event.metaKey ||
      event.altKey ||
      isTypingTarget(event.target)
    ) {
      return;
    }

    switch (event.key.toLowerCase()) {
      case "arrowleft":
      case "a":
        event.preventDefault();
        step(-1);
        break;
      case "arrowright":
      case "d":
        event.preventDefault();
        step(1);
        break;
      case "arrowup":
      case "w":
        event.preventDefault();
        cycleFilter(-1);
        break;
      case "arrowdown":
      case "s":
        event.preventDefault();
        cycleFilter(1);
        break;
      case "home":
        if (visible.length > 0) {
          event.preventDefault();
          activeIndex = 0;
          centerOn(0);
        }
        break;
      case "end":
        if (visible.length > 0) {
          event.preventDefault();
          const last = visible.length - 1;
          activeIndex = last;
          centerOn(last);
        }
        break;
      case "enter":
      case " ":
        if (isInteractiveTarget(event.target)) return;
        if (selected) {
          event.preventDefault();
          openGame(selected);
        }
        break;
    }
  }
</script>

<svelte:window onkeydown={onKeydown} />

<div class="home">
  <div class="ambient" aria-hidden="true">
    {#each ambientPrev ? [ambientPrev, ambient] : [ambient] as layer (layer.key)}
      <div class="ambient-layer" style:background={layer.background}>
        {#if layer.image}
          <img src={layer.image} alt="" draggable="false" />
        {/if}
      </div>
    {/each}
    <div class="ambient-wash"></div>
  </div>

  <header class="head">
    <div class="head-copy">
      <p class="eyebrow">Project Kudos</p>
      <h1>{greeting}</h1>
    </div>

    <div class="head-side">
      {#if library.usingMock}
        <span
          class="demo"
          title="Dev mock data. Add ?mock=0 (or clear kudos.devMock) to use the real list_games command."
        >
          Preview data
        </span>
      {/if}
      <span class="libcount">
        {#if library.status === "loading" && library.games.length === 0}
          Scanning…
        {:else}
          <strong>{library.games.length}</strong>
          {library.games.length === 1 ? "game" : "games"}
        {/if}
      </span>
    </div>
  </header>

  <FilterBar value={filter} {counts} onChange={setFilter} />

  <div class="rail-viewport" bind:this={viewport}>
    {#if library.status === "loading" && library.games.length === 0}
      <div class="skeleton-track" aria-hidden="true">
        {#each Array(6) as _, index (index)}
          <div class="skeleton-tile" style:animation-delay={`${index * 80}ms`}>
            <span class="skeleton-cover"></span>
            <span class="skeleton-line"></span>
          </div>
        {/each}
      </div>
    {:else if library.status === "error" && library.games.length === 0}
      <div class="state-card" in:fade={{ duration: 220 }}>
        <span class="state-icon danger"><Icon name="alert" size={22} /></span>
        <div class="state-copy">
          <h2>Couldn't read your library</h2>
          <p>{library.error}</p>
        </div>
        <button type="button" class="state-action" onclick={() => void refreshLibrary(true)}>
          <Icon name="refresh" size={15} />
          Retry
        </button>
      </div>
    {:else if visible.length === 0}
      <div class="state-card" in:fade={{ duration: 220 }}>
        <span class="state-icon"><Icon name="library" size={22} /></span>
        <div class="state-copy">
          <h2>No games found</h2>
          <p>{emptyCopy}</p>
        </div>
        {#if filter === "all"}
          <button type="button" class="state-action" onclick={() => void refreshLibrary(true)}>
            <Icon name="refresh" size={15} />
            Refresh
          </button>
        {:else}
          <button type="button" class="state-action" onclick={() => setFilter("all")}>
            <Icon name="library" size={15} />
            Show all games
          </button>
        {/if}
      </div>
    {:else}
      <div
        class="rail-track"
        bind:this={track}
        style:transform={`translate3d(${-offset}px, 0, 0)`}
      >
        {#each visible as game, index (game.id)}
          <GameTile
            {game}
            active={index === active}
            index={index}
            onSelect={select}
            onOpen={openGame}
          />
        {/each}
      </div>
    {/if}
  </div>

  <div class="lower">
    {#if visible.length > 0}
      <InfoPanel game={selected} index={active} total={visible.length} onOpen={openGame} />
    {/if}
  </div>

  <footer class="hints">
    <span class="hint">
      <Icon name="keyboard" size={13} />
      Arrows / WASD browse
      <span class="dot-sep">·</span>
      ↑↓ filter
      <span class="dot-sep">·</span>
      Enter open
    </span>
    <span class="hint">
      <Icon name="gamepad" size={14} />
      D-pad browse
      <span class="dot-sep">·</span>
      A open
      <span class="dot-sep">·</span>
      B back
    </span>
    <span class="hint subtle">Esc back</span>
  </footer>
</div>

<style>
  .home {
    --tile-w: clamp(150px, 23vh, 280px);
    --tile-gap: clamp(16px, 2vh, 26px);
    position: relative;
    min-height: 100%;
    display: flex;
    flex-direction: column;
    gap: clamp(14px, 2.2vh, 24px);
    isolation: isolate;
  }

  /* ---------- ambient art ---------- */
  .ambient {
    position: absolute;
    inset: -12px;
    z-index: -1;
    overflow: hidden;
    pointer-events: none;
  }

  .ambient-layer {
    position: absolute;
    inset: -8%;
    background-size: cover;
    background-position: center;
    opacity: 0.6;
    animation: ambient-in 900ms var(--ease-out) both;
    will-change: opacity;
  }

  .ambient-layer img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    filter: blur(40px) saturate(1.3) brightness(0.66);
    transform: scale(1.12);
    opacity: 0.95;
  }

  .ambient-wash {
    position: absolute;
    inset: 0;
    background:
      linear-gradient(
        180deg,
        rgba(5, 6, 10, 0.72) 0%,
        rgba(5, 6, 10, 0.26) 34%,
        rgba(5, 6, 10, 0.58) 74%,
        rgba(5, 6, 10, 0.88) 100%
      ),
      radial-gradient(90% 70% at 50% 40%, transparent 20%, rgba(5, 6, 10, 0.42) 100%);
  }

  @keyframes ambient-in {
    from {
      opacity: 0;
    }
    to {
      opacity: 0.6;
    }
  }

  /* ---------- header ---------- */
  .head {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: 24px;
  }

  .eyebrow {
    margin: 0 0 7px;
    font-size: 10px;
    font-weight: 650;
    letter-spacing: 0.24em;
    text-transform: uppercase;
    color: var(--accent);
  }

  h1 {
    margin: 0;
    font-family: var(--font-display);
    font-size: clamp(30px, 4.6vh, 46px);
    font-weight: 600;
    line-height: 1.02;
    letter-spacing: -0.035em;
    color: var(--fg-0);
  }

  .head-side {
    display: flex;
    align-items: center;
    gap: 12px;
    padding-bottom: 5px;
  }

  .demo {
    padding: 5px 10px;
    border-radius: 999px;
    font-size: 9.5px;
    font-weight: 650;
    letter-spacing: 0.11em;
    text-transform: uppercase;
    color: var(--accent);
    background: color-mix(in srgb, var(--accent) 13%, transparent);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--accent) 30%, transparent);
    cursor: help;
  }

  .libcount {
    font-size: 12px;
    color: var(--fg-2);
    font-variant-numeric: tabular-nums;
  }

  .libcount strong {
    color: var(--fg-0);
    font-weight: 600;
  }

  /* ---------- rail ---------- */
  .rail-viewport {
    position: relative;
    display: flex;
    align-items: center;
    min-height: calc(var(--tile-w) * 1.5 + 64px);
    padding-block: 30px;
    overflow: hidden;
  }

  .rail-viewport::before {
    content: "";
    position: absolute;
    left: 50%;
    top: 50%;
    width: min(64vw, 760px);
    height: 78%;
    transform: translate(-50%, -50%);
    border-radius: 50%;
    background: radial-gradient(
      ellipse at center,
      color-mix(in srgb, var(--accent) 17%, transparent) 0%,
      transparent 68%
    );
    filter: blur(18px);
    pointer-events: none;
    opacity: 0.9;
  }

  .rail-track {
    position: relative;
    z-index: 1;
    display: flex;
    align-items: center;
    gap: var(--tile-gap);
    width: 100%;
    padding-inline: calc(50% - var(--tile-w) / 2);
    transform: translate3d(0, 0, 0);
    transition: transform 540ms cubic-bezier(0.16, 1, 0.3, 1);
    will-change: transform;
  }

  /* ---------- skeleton ---------- */
  .skeleton-track {
    position: relative;
    z-index: 1;
    display: flex;
    align-items: center;
    gap: var(--tile-gap);
    width: 100%;
    padding-inline: calc(50% - var(--tile-w) / 2);
  }

  .skeleton-tile {
    flex: 0 0 auto;
    width: var(--tile-w);
    display: flex;
    flex-direction: column;
    gap: 12px;
    animation: skeleton-pulse 1.7s ease-in-out infinite;
  }

  .skeleton-cover {
    display: block;
    width: 100%;
    aspect-ratio: 2 / 3;
    border-radius: clamp(10px, 1vw, 16px);
    background:
      linear-gradient(160deg, rgba(255, 255, 255, 0.085), rgba(255, 255, 255, 0.025)),
      var(--bg-2);
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.04);
  }

  .skeleton-line {
    display: block;
    width: 62%;
    height: 9px;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.08);
  }

  @keyframes skeleton-pulse {
    0%,
    100% {
      opacity: 0.38;
    }
    50% {
      opacity: 0.8;
    }
  }

  /* ---------- states ---------- */
  .state-card {
    width: 100%;
    min-height: calc(var(--tile-w) * 1.12);
    display: flex;
    align-items: center;
    gap: 18px;
    padding: 26px 28px;
    border-radius: var(--radius-lg);
    background:
      linear-gradient(160deg, rgba(255, 255, 255, 0.045), rgba(255, 255, 255, 0.015));
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.07);
    backdrop-filter: blur(18px);
  }

  .state-icon {
    display: grid;
    place-items: center;
    width: 46px;
    height: 46px;
    border-radius: 14px;
    flex: 0 0 auto;
    color: var(--accent);
    background: color-mix(in srgb, var(--accent) 13%, transparent);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--accent) 26%, transparent);
  }

  .state-icon.danger {
    color: var(--danger);
    background: rgba(255, 95, 109, 0.12);
    box-shadow: inset 0 0 0 1px rgba(255, 95, 109, 0.24);
  }

  .state-copy {
    min-width: 0;
    flex: 1;
  }

  .state-copy h2 {
    margin: 0 0 5px;
    font-size: 20px;
    font-weight: 600;
    letter-spacing: -0.02em;
  }

  .state-copy p {
    margin: 0;
    max-width: 62ch;
    font-size: 13px;
    line-height: 1.55;
    color: var(--fg-2);
    overflow-wrap: anywhere;
  }

  .state-action {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    flex: 0 0 auto;
    height: 40px;
    padding: 0 16px;
    border-radius: 999px;
    font-size: 12.5px;
    font-weight: 600;
    color: var(--fg-0);
    background: rgba(255, 255, 255, 0.08);
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.12);
    transition: transform var(--dur-fast) var(--ease-out);
    will-change: transform;
  }

  .state-action:hover {
    transform: translateY(-1px) scale(1.02);
  }

  .state-action:active {
    transform: scale(0.98);
  }

  /* ---------- lower ---------- */
  .lower {
    margin-top: auto;
  }

  .hints {
    display: flex;
    align-items: center;
    gap: 20px;
    padding-top: 2px;
    font-size: 10.5px;
    color: var(--fg-2);
    flex-wrap: wrap;
  }

  .hint {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    white-space: nowrap;
  }

  .hint.subtle {
    margin-left: auto;
    opacity: 0.7;
  }

  .dot-sep {
    opacity: 0.38;
  }

  @media (max-width: 860px) {
    .home {
      --tile-w: clamp(136px, 20vh, 200px);
    }
    .hint:nth-child(2) {
      display: none;
    }
    .state-card {
      flex-wrap: wrap;
    }
  }
</style>
