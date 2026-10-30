<!--
  HOME / LIBRARY
  PS5-style hero + independently focusable rows. Vertical focus model:
  Up/Down between rows, Left/Right inside a row, Enter opens, Esc backs out.
  OWNER: Session D / Session I (PS5 redesign).
-->
<script lang="ts">
  import { onMount } from "svelte";
  import { fade } from "svelte/transition";
  import { coverSource, placeholderBackground } from "../lib/art";
  import {
    countFilters,
    matchesFilter,
    readStoredFilter,
    resolveFilter,
    storeFilter,
    visibleFilters,
    type Filter,
  } from "../lib/filter";
  import { favorites, toggleFavorite } from "../lib/favorites.svelte";
  import { loadRecent, recent } from "../lib/history.svelte";
  import { armIdleWatcher } from "../lib/idle.svelte";
  import {
    hideGame as hideLibraryGame,
    isHidden,
    library,
    libraryPrefs,
    loadLibrary,
    playtimeFor,
    sortGames,
    syncSteamLibrary,
  } from "../lib/library.svelte";
  import { relativePlayed, resolveRecent } from "../lib/recent";
  import { navigate } from "../lib/router.svelte";
  import { openSearch, searchState } from "../lib/search.svelte";
  import { playtimeLabel, playtimeSummary, relativeLong } from "../lib/steam";
  import { applyStoredAccent, storedBackground } from "../lib/theme";
  import { pushToast } from "../lib/toast.svelte";
  import { onGamepad } from "../lib/gamepad";
  import { humanError } from "../lib/format";
  import { recordLaunch } from "../lib/history.svelte";
  import { launchGame } from "../lib/api";
  import { restoreFullscreenPref } from "../lib/windowPrefs";
  import { PLATFORM_LABEL, type Game } from "../lib/types";
  import ControlHint from "../lib/components/ControlHint.svelte";
  import FilterBar from "../lib/components/FilterBar.svelte";
  import HeroBanner from "../lib/components/HeroBanner.svelte";
  import Icon from "../lib/components/Icon.svelte";
  import LaunchOverlay from "../lib/components/LaunchOverlay.svelte";
  import MediaRow from "../lib/components/MediaRow.svelte";
  import Onboarding from "../lib/components/Onboarding.svelte";
  import Toaster from "../lib/components/Toaster.svelte";

  interface Ambient {
    key: string;
    image: string | null;
    background: string;
  }

  interface HomeRow {
    key: string;
    title: string;
    subtitle: string;
    items: Game[];
    size: "lg" | "md" | "sm";
    meta: (game: Game) => string;
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

  const greeting = greetingFor(new Date().getHours());

  let filter = $state<Filter>(readStoredFilter());
  let rowIndex = $state(0);
  let colIndex = $state(0);
  let background = $state(storedBackground());
  let launching = $state<Game | null>(null);
  let launchMode = $state<"play" | "install">("play");

  let ambientSnapshot = ambientFor(null);
  let ambient = $state<Ambient>(ambientSnapshot);
  let ambientPrev = $state<Ambient | null>(null);
  let ambientTimer: ReturnType<typeof setTimeout> | undefined;

  const shown = $derived(library.games.filter((game) => !isHidden(game.id)));
  const hiddenCount = $derived(library.games.length - shown.length);
  const counts = $derived(countFilters(shown));
  const activeFilter = $derived(resolveFilter(filter, counts));
  const visible = $derived(
    sortGames(
      shown.filter((game) => matchesFilter(game, activeFilter)),
      libraryPrefs.sort,
    ),
  );

  const favoriteGames = $derived.by(() => {
    const byId = new Map(shown.map((game) => [game.id, game]));
    const picked: Game[] = [];
    for (const id of favorites.ids) {
      const game = byId.get(id);
      if (game && matchesFilter(game, activeFilter)) picked.push(game);
    }
    return picked;
  });

  const recentItems = $derived(resolveRecent(recent.entries, visible));

  const continueItems = $derived.by(() => {
    const fromRecent = recentItems.map((item) => item.game).filter((game) => game.isInstalled);
    if (fromRecent.length > 0) return fromRecent.slice(0, 8);

    return visible
      .filter((game) => game.isInstalled && (playtimeFor(game.id)?.minutes ?? 0) > 0)
      .sort((a, b) => (playtimeFor(b.id)?.minutes ?? 0) - (playtimeFor(a.id)?.minutes ?? 0))
      .slice(0, 8);
  });

  function metaPlaytime(game: Game): string {
    return playtimeLabel(playtimeFor(game.id)?.minutes);
  }

  function metaContinue(game: Game): string {
    const entry = recent.entries.find((item) => item.id === game.id);
    const played = playtimeLabel(playtimeFor(game.id)?.minutes);
    if (played && entry) return `${played} · ${relativePlayed(entry.lastPlayedUnix)}`;
    return played || (entry ? relativePlayed(entry.lastPlayedUnix) : "");
  }

  function metaRecent(game: Game): string {
    const entry = recent.entries.find((item) => item.id === game.id);
    if (entry) return relativePlayed(entry.lastPlayedUnix);
    return playtimeLabel(playtimeFor(game.id)?.minutes);
  }

  function metaOwned(game: Game): string {
    return playtimeLabel(playtimeFor(game.id)?.minutes);
  }

  const rows = $derived.by(() => {
    const list: HomeRow[] = [];

    if (favoriteGames.length > 0) {
      list.push({
        key: "favorites",
        title: "Favorites",
        subtitle: "Pinned by you",
        items: favoriteGames,
        size: "md",
        meta: metaPlaytime,
      });
    }

    if (continueItems.length > 0) {
      list.push({
        key: "continue",
        title: "Continue playing",
        subtitle: "Recent sessions and hours played",
        items: continueItems,
        size: "lg",
        meta: metaContinue,
      });
    }

    if (recentItems.length > 0) {
      list.push({
        key: "recent",
        title: "Recently played",
        subtitle: "Newest first",
        items: recentItems.map((item) => item.game),
        size: "sm",
        meta: metaRecent,
      });
    }

    if (visible.length > 0) {
      list.push({
        key: "library",
        title: "Your library",
        subtitle: `${counts.installed} installed · ${counts["not-installed"]} not installed`,
        items: visible,
        size: "md",
        meta: metaPlaytime,
      });
    }

    const owned = visible.filter(
      (game) => !game.isInstalled && game.platform === "steam",
    );
    if (owned.length > 0) {
      list.push({
        key: "steam-account",
        title: "In your Steam account",
        subtitle: "Owned · installs through Steam",
        items: owned,
        size: "md",
        meta: metaOwned,
      });
    }

    return list;
  });

  const rowCursor = $derived(rows.length === 0 ? 0 : Math.min(rowIndex, rows.length - 1));
  const activeRow = $derived(rows[rowCursor] ?? null);
  const colCursor = $derived(
    activeRow && activeRow.items.length > 0
      ? Math.min(colIndex, activeRow.items.length - 1)
      : 0,
  );
  const focused = $derived(activeRow?.items[colCursor] ?? null);
  const favoriteSet = $derived(new Set(favorites.ids));
  const heroKicker = $derived.by(() => {
    switch (activeRow?.key) {
      case "favorites":
        return "Favorite";
      case "recent":
        return "Recently played";
      case "library":
        return "Your library";
      case "steam-account":
        return "In your Steam account";
      default:
        return "Continue playing";
    }
  });

  const focusedMeta = $derived.by(() => {
    if (!focused) return "";
    const playtime = playtimeFor(focused.id);
    const entry = recent.entries.find((item) => item.id === focused.id);
    const last = entry?.lastPlayedUnix ?? playtime?.lastPlayed ?? null;
    const summary = playtimeSummary(playtime?.minutes, last);
    if (summary) return summary;
    if (!focused.isInstalled) return "Owned in your Steam library — not downloaded on this PC.";
    return "";
  });

  const trulyEmpty = $derived(
    library.status === "ready" && library.games.length === 0,
  );
  const allHidden = $derived(library.games.length > 0 && shown.length === 0);
  const filterEmpty = $derived(
    !trulyEmpty &&
      !allHidden &&
      library.games.length > 0 &&
      rows.length === 0,
  );

  $effect(() => {
    const next = ambientFor(focused);
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

    const stopGamepad = onGamepad(handlePad);
    const stopIdle = armIdleWatcher();

    return () => {
      stopGamepad();
      stopIdle();
      if (ambientTimer) clearTimeout(ambientTimer);
    };
  });

  async function refreshLibrary(force = false): Promise<void> {
    rowIndex = 0;
    colIndex = 0;
    await Promise.allSettled([loadLibrary(force), loadRecent(true)]);
  }

  function setFilter(next: Filter): void {
    filter = next;
    storeFilter(next);
    rowIndex = 0;
    colIndex = 0;
  }

  function cycleFilter(direction: 1 | -1): void {
    const defs = visibleFilters(counts);
    const index = defs.findIndex((entry) => entry.id === activeFilter);
    const next = defs[(index + direction + defs.length) % defs.length];
    if (next) setFilter(next.id);
  }

  function focusTile(nextRow: number, nextCol: number): void {
    rowIndex = nextRow;
    colIndex = nextCol;
  }

  function moveRow(delta: 1 | -1): void {
    if (rows.length === 0) return;
    const next = (rowCursor + delta + rows.length) % rows.length;
    rowIndex = next;
    colIndex = Math.min(colCursor, Math.max(0, rows[next].items.length - 1));
  }

  function moveCol(delta: 1 | -1): void {
    if (!activeRow || activeRow.items.length === 0) return;
    colIndex = (colCursor + delta + activeRow.items.length) % activeRow.items.length;
  }

  function openGame(game: Game): void {
    navigate({ name: "game", id: game.id });
  }

  function toggleFav(game: Game): void {
    const added = toggleFavorite(game.id);
    pushToast(
      "info",
      added ? "Added to favorites" : "Removed from favorites",
      added ? `“${game.title}” now sits at the top of your library.` : `“${game.title}” left your favorites.`,
    );
  }

  function hideFocused(): void {
    if (!focused) return;
    hideLibraryGame(focused.id);
    pushToast(
      "info",
      "Hidden from library",
      `“${focused.title}” is hidden. Unhide it from Settings at any time.`,
    );
  }

  async function runPrimary(game: Game): Promise<void> {
    if (launching) return;
    const installing = !game.isInstalled;
    if (installing && game.platform !== "steam") {
      openGame(game);
      return;
    }

    launching = game;
    launchMode = installing ? "install" : "play";
    const started = performance.now();
    let failure: string | null = null;

    try {
      if (library.usingMock) {
        await wait(1150);
      } else {
        await launchGame(game.id);
        if (!installing) recordLaunch(game.id);
      }
    } catch (error) {
      failure = humanError(error);
    }

    await waitAtLeast(started, failure ? 600 : 1000);
    launching = null;

    if (failure) {
      pushToast("error", installing ? "Couldn't open Steam" : "Couldn't launch", failure);
    } else if (installing) {
      pushToast(
        "info",
        "Steam install dialog opened",
        `Approve “${game.title}” in Steam. Kudos will pick it up after it finishes installing.`,
      );
    } else if (library.usingMock) {
      pushToast(
        "info",
        "Preview mode",
        "Launching is simulated with mock data. The real app hands off to the game window.",
      );
    }
  }

  async function onboardingSync(): Promise<void> {
    const ok = await syncSteamLibrary();
    if (!ok) await loadLibrary(true);
  }

  function handlePad(action: string): void {
    switch (action) {
      case "left":
        moveCol(-1);
        break;
      case "right":
        moveCol(1);
        break;
      case "up":
        moveRow(-1);
        break;
      case "down":
        moveRow(1);
        break;
      case "tabLeft":
        cycleFilter(-1);
        break;
      case "tabRight":
        cycleFilter(1);
        break;
      case "confirm":
        if (focused) openGame(focused);
        break;
      case "secondary":
        if (focused) toggleFav(focused);
        break;
      case "back":
        if (activeFilter !== "all") setFilter("all");
        break;
      case "menu":
        navigate({ name: "settings" });
        break;
    }
  }

  function onKeydown(event: KeyboardEvent): void {
    if (
      searchState.open ||
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
        moveCol(-1);
        break;
      case "arrowright":
      case "d":
        event.preventDefault();
        moveCol(1);
        break;
      case "arrowup":
      case "w":
        event.preventDefault();
        moveRow(-1);
        break;
      case "arrowdown":
      case "s":
        event.preventDefault();
        moveRow(1);
        break;
      case "home":
        if (activeRow) {
          event.preventDefault();
          colIndex = 0;
        }
        break;
      case "end":
        if (activeRow) {
          event.preventDefault();
          colIndex = Math.max(0, activeRow.items.length - 1);
        }
        break;
      case "enter":
      case " ":
        if (focused) {
          event.preventDefault();
          openGame(focused);
        }
        break;
      case "p":
        if (focused) {
          event.preventDefault();
          void runPrimary(focused);
        }
        break;
      case "f":
        if (focused) {
          event.preventDefault();
          toggleFav(focused);
        }
        break;
      case "h":
        if (focused) {
          event.preventDefault();
          hideFocused();
        }
        break;
      case "[":
        event.preventDefault();
        cycleFilter(-1);
        break;
      case "]":
        event.preventDefault();
        cycleFilter(1);
        break;
      case "escape":
        if (activeFilter !== "all") {
          event.preventDefault();
          setFilter("all");
        }
        break;
    }
  }

  function wait(ms: number): Promise<void> {
    return new Promise((resolve) => setTimeout(resolve, ms));
  }

  async function waitAtLeast(started: number, ms: number): Promise<void> {
    const remaining = ms - (performance.now() - started);
    if (remaining > 0) await wait(remaining);
  }

  const emptyFilterCopy = $derived(
    activeFilter === "installed"
      ? "Nothing installed matches this filter yet. Switch to All to see the rest of your Steam account."
      : activeFilter === "not-installed"
        ? "Everything in your library is installed. Nice."
        : `No ${PLATFORM_LABEL[activeFilter as keyof typeof PLATFORM_LABEL] ?? ""} games found. That store may not be installed on this PC, or it has no games.`,
  );
</script>

<svelte:window onkeydown={onKeydown} />

<div class="home">
  {#if background === "ambient"}
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
  {/if}

  <header class="head">
    <div class="head-copy">
      <p class="eyebrow">Kudos · {greeting}</p>
      <h1>Your library</h1>
    </div>

    <div class="head-side">
      {#if library.usingMock}
        <span
          class="demo"
          title="Preview data. Add ?mock=1 by hand to simulate a library; omit it to use the real commands."
        >
          Preview data
        </span>
      {/if}

      {#if library.syncStatus?.appCount}
        <span class="owned-chip" title="Apps known from your Steam account">
          {library.syncStatus.appCount} in Steam
        </span>
      {/if}

      <span class="libcount">
        {#if library.status === "loading" && library.games.length === 0}
          Scanning…
        {:else}
          <strong>{shown.length}</strong>
          {shown.length === 1 ? "game" : "games"}
        {/if}
      </span>

      {#if hiddenCount > 0}
        <button
          type="button"
          class="hidden-chip"
          title="Review hidden games in Settings"
          onclick={() => navigate({ name: "settings" })}
        >
          {hiddenCount} hidden
        </button>
      {/if}

      <button type="button" class="search-btn" onclick={openSearch} title="Search (Ctrl+K or /)">
        <Icon name="search" size={15} />
        <span>Search</span>
        <kbd>/</kbd>
      </button>
    </div>
  </header>

  {#if library.status === "loading" && library.games.length === 0}
    <div class="skeleton-hero" aria-hidden="true"></div>
    <div class="skeleton-row" aria-hidden="true">
      {#each Array(5) as _, index (index)}
        <span class="skeleton-tile" style:animation-delay={`${index * 80}ms`}></span>
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
  {:else if trulyEmpty}
    <Onboarding
      syncing={library.syncState === "syncing"}
      scanning={library.status === "loading"}
      note={library.syncError ?? ""}
      onSync={() => void onboardingSync()}
      onRefresh={() => void refreshLibrary(true)}
    />
  {:else if allHidden}
    <div class="state-card" in:fade={{ duration: 220 }}>
      <span class="state-icon"><Icon name="hide" size={22} /></span>
      <div class="state-copy">
        <h2>Every game is hidden</h2>
        <p>
          Hidden games stay installed and launchable. Review the {hiddenCount} hidden
          {hiddenCount === 1 ? "game" : "games"} in Settings, or unhide everything.
        </p>
      </div>
      <button type="button" class="state-action" onclick={() => navigate({ name: "settings" })}>
        <Icon name="sliders" size={15} />
        Manage hidden games
      </button>
    </div>
  {:else}
    {#if focused}
      <HeroBanner
        game={focused}
        meta={focusedMeta}
        kicker={heroKicker}
        playtime={playtimeFor(focused.id)}
        favorite={favoriteSet.has(focused.id)}
        busy={launching?.id === focused.id}
        onPrimary={(game) => void runPrimary(game)}
        onDetails={openGame}
        onFavorite={toggleFav}
      />
    {/if}

    <FilterBar value={activeFilter} {counts} onChange={setFilter} />

    {#if filterEmpty}
      <div class="state-card" in:fade={{ duration: 220 }}>
        <span class="state-icon"><Icon name="library" size={22} /></span>
        <div class="state-copy">
          <h2>No games in this filter</h2>
          <p>{emptyFilterCopy}</p>
        </div>
        <button type="button" class="state-action" onclick={() => setFilter("all")}>
          <Icon name="library" size={15} />
          Show all games
        </button>
      </div>
    {:else}
      <div class="rows">
        {#each rows as row, index (row.key)}
          <MediaRow
            title={row.title}
            subtitle={row.subtitle}
            items={row.items}
            size={row.size}
            active={index === rowCursor}
            activeIndex={index === rowCursor ? colCursor : 0}
            metaFor={row.meta}
            favorites={favorites.ids}
            onFocus={(next) => focusTile(index, next)}
            onOpen={openGame}
            onToggleFavorite={toggleFav}
          />
        {/each}
      </div>
    {/if}

    <footer class="hints">
      <ControlHint action="dpad" keys="↑ ↓" label="Rows" />
      <ControlHint action="dpad" keys="← →" label="Browse" />
      <ControlHint action="confirm" keys="Enter" label="Open" />
      <ControlHint action="back" keys="Esc" label="Back" />
      <span class="hint subtle"><kbd>P</kbd> Play / install</span>
      <span class="hint subtle"><kbd>F</kbd> Favorite</span>
      <span class="hint subtle hint-end">Hold or right-click a tile to favorite</span>
    </footer>
  {/if}
</div>

<LaunchOverlay
  active={launching !== null}
  title={launching?.title}
  label={launchMode === "install" ? "Opening Steam" : "Launching"}
  hint={launchMode === "install" ? "Approve the install in Steam" : "Handing over to the game window"}
/>
<Toaster />

<style>
  .home {
    position: relative;
    min-height: 100%;
    display: flex;
    flex-direction: column;
    gap: clamp(12px, 1.6vh, 20px);
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
    opacity: 0.52;
    animation: ambient-in 900ms var(--ease-out) both;
    will-change: opacity;
  }

  .ambient-layer img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    filter: blur(46px) saturate(1.32) brightness(0.6);
    transform: scale(1.14);
    opacity: 0.95;
  }

  .ambient-wash {
    position: absolute;
    inset: 0;
    background:
      linear-gradient(
        180deg,
        rgba(5, 6, 10, 0.74) 0%,
        rgba(5, 6, 10, 0.3) 32%,
        rgba(5, 6, 10, 0.64) 76%,
        rgba(5, 6, 10, 0.9) 100%
      ),
      radial-gradient(90% 70% at 50% 40%, transparent 18%, rgba(5, 6, 10, 0.46) 100%);
  }

  @keyframes ambient-in {
    from {
      opacity: 0;
    }
    to {
      opacity: 0.52;
    }
  }

  /* ---------- header ---------- */
  .head {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: 24px;
  }

  .head-copy {
    min-width: 0;
  }

  .eyebrow {
    margin: 0 0 5px;
    font-size: 9.5px;
    font-weight: 650;
    letter-spacing: 0.2em;
    text-transform: uppercase;
    color: var(--accent);
  }

  h1 {
    margin: 0;
    font-family: var(--font-display);
    font-size: clamp(24px, 3.2vh, 34px);
    font-weight: 600;
    line-height: 1.02;
    letter-spacing: -0.038em;
    color: var(--fg-0);
  }

  .head-side {
    display: flex;
    align-items: center;
    gap: 9px;
    padding-bottom: 2px;
    flex-wrap: wrap;
    justify-content: flex-end;
  }

  .demo {
    padding: 4px 9px;
    border-radius: 999px;
    font-size: 8.5px;
    font-weight: 700;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--accent);
    background: color-mix(in srgb, var(--accent) 13%, transparent);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--accent) 30%, transparent);
    cursor: help;
  }

  .owned-chip,
  .hidden-chip {
    padding: 4px 9px;
    border-radius: 999px;
    font-size: 9px;
    font-weight: 600;
    letter-spacing: 0.05em;
    color: var(--fg-2);
    background: rgba(255, 255, 255, 0.045);
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.07);
    font-variant-numeric: tabular-nums;
  }

  .hidden-chip {
    transition:
      color 160ms var(--ease-out),
      transform 160ms var(--ease-out);
  }

  .hidden-chip:hover {
    color: var(--fg-0);
    transform: translateY(-1px);
  }

  .libcount {
    font-size: 11px;
    color: var(--fg-2);
    font-variant-numeric: tabular-nums;
    padding: 4px 2px;
  }

  .libcount strong {
    color: var(--fg-0);
    font-weight: 600;
  }

  .search-btn {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    height: 32px;
    padding: 0 9px 0 11px;
    border-radius: 999px;
    font-size: 11px;
    font-weight: 550;
    color: var(--fg-2);
    background: rgba(255, 255, 255, 0.05);
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.08);
    transition:
      color 160ms var(--ease-out),
      transform 160ms var(--ease-out);
  }

  .search-btn:hover {
    color: var(--fg-0);
    transform: translateY(-1px);
  }

  kbd {
    font-family: var(--font-ui);
    font-size: 8.5px;
    font-weight: 650;
    color: var(--fg-2);
    background: rgba(255, 255, 255, 0.07);
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.1);
    border-radius: 4px;
    padding: 1px 4px;
    line-height: 1.4;
  }

  /* ---------- rows / skeleton ---------- */
  .rows {
    display: flex;
    flex-direction: column;
    gap: clamp(14px, 2.1vh, 24px);
  }

  .skeleton-hero {
    height: clamp(310px, 39vh, 430px);
    border-radius: clamp(20px, 2vw, 30px);
    background:
      linear-gradient(160deg, rgba(255, 255, 255, 0.07), rgba(255, 255, 255, 0.02)),
      var(--bg-2);
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.05);
    animation: skeleton-pulse 1.7s ease-in-out infinite;
  }

  .skeleton-row {
    display: flex;
    gap: 16px;
    overflow: hidden;
  }

  .skeleton-tile {
    flex: 0 0 auto;
    width: clamp(126px, 14.5vh, 164px);
    aspect-ratio: 2 / 3;
    border-radius: 14px;
    background:
      linear-gradient(160deg, rgba(255, 255, 255, 0.07), rgba(255, 255, 255, 0.02)),
      var(--bg-2);
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.05);
    animation: skeleton-pulse 1.7s ease-in-out infinite;
  }

  @keyframes skeleton-pulse {
    0%,
    100% {
      opacity: 0.4;
    }
    50% {
      opacity: 0.78;
    }
  }

  /* ---------- states ---------- */
  .state-card {
    width: 100%;
    min-height: 170px;
    display: flex;
    align-items: center;
    gap: 18px;
    padding: 26px 28px;
    border-radius: var(--radius-lg);
    background:
      linear-gradient(160deg, rgba(255, 255, 255, 0.045), rgba(255, 255, 255, 0.015));
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.07);
    backdrop-filter: blur(18px);
    flex-wrap: wrap;
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
    transition: transform 160ms var(--ease-out);
    will-change: transform;
  }

  .state-action:hover {
    transform: translateY(-1px) scale(1.02);
  }

  /* ---------- footer ---------- */
  .hints {
    display: flex;
    align-items: center;
    gap: 18px;
    padding-top: 4px;
    font-size: 10px;
    color: var(--fg-2);
    flex-wrap: wrap;
  }

  .hint {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    white-space: nowrap;
  }

  .hint.subtle {
    opacity: 0.78;
  }

  .hint-end {
    margin-left: auto;
    opacity: 0.6;
  }

  @media (max-width: 900px) {
    .head {
      align-items: flex-start;
      flex-direction: column;
      gap: 10px;
    }
    .head-side {
      justify-content: flex-start;
    }
    .hint:nth-child(2) {
      display: none;
    }
    .hint-end {
      display: none;
    }
  }
</style>
