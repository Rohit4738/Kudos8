<!--
  GAME DETAIL
  Big cover, Play, and the GOG-only "Add to Steam" flow with loading / success /
  "Close Steam first" states. OWNER: Session D.
-->
<script lang="ts">
  import { onMount } from "svelte";
  import { addToSteam, getAchievements, launchGame } from "../lib/api";
  import { recordLaunch } from "../lib/history.svelte";
  import {
    coverSource,
    heroCandidates,
    isTauriRuntime,
    placeholderBackground,
  } from "../lib/art";
  import { getArtStatus, type ArtStatus } from "../lib/artProvider";
  import { isFavorite, toggleFavorite } from "../lib/favorites.svelte";
  import {
    formatBytes,
    humanError,
    isSteamRunningError,
    platformShort,
    playState,
  } from "../lib/format";
  import { onGamepad } from "../lib/gamepad";
  import {
    findGame,
    hideGame,
    isHidden,
    library,
    loadLibrary,
    playtimeFor,
    unhideGame,
  } from "../lib/library.svelte";
  import { focusNext } from "../lib/padNav";
  import { back } from "../lib/router.svelte";
  import { shellFocus } from "../lib/shellFocus.svelte";
  import {
    isSteamOwnedGame,
    playtimeSummary,
    primaryActionLabel,
  } from "../lib/steam";
  import { applyStoredAccent } from "../lib/theme";
  import { pushToast } from "../lib/toast.svelte";
  import type { AchievementSummary } from "../lib/types";
  import ArtPicker from "../lib/components/ArtPicker.svelte";
  import ControlHint from "../lib/components/ControlHint.svelte";
  import GameArt from "../lib/components/GameArt.svelte";
  import Icon from "../lib/components/Icon.svelte";
  import LaunchOverlay from "../lib/components/LaunchOverlay.svelte";
  import PlatformBadge from "../lib/components/PlatformBadge.svelte";
  import Spinner from "../lib/components/Spinner.svelte";
  import Toaster from "../lib/components/Toaster.svelte";

  let { id }: { id: string } = $props();

  type SteamState = "idle" | "loading" | "success" | "error";

  let launching = $state(false);
  let launchMode = $state<"play" | "install">("play");
  let steamState = $state<SteamState>("idle");
  let steamError = $state<string | null>(null);

  let artOpen = $state(false);
  let artStatus = $state<ArtStatus | null>(null);
  let localCover = $state<string | null>(null);
  let localHero = $state<string | null>(null);
  let artFor = $state("");

  const game = $derived(findGame(id));
  const displayGame = $derived(
    game && localCover ? { ...game, coverImage: localCover } : game,
  );
  const hidden = $derived(game ? isHidden(game.id) : false);
  const favorite = $derived(game ? isFavorite(game.id) : false);
  const playtime = $derived(game ? playtimeFor(game.id) : null);
  const playedSummary = $derived(
    playtime ? playtimeSummary(playtime.minutes, playtime.lastPlayed) : "",
  );
  const primaryLabel = $derived(game ? primaryActionLabel(game) : "Play");
  const canLaunch = $derived(
    game ? game.isInstalled || isSteamOwnedGame(game) : false,
  );

  const heroSources = $derived.by(() => {
    if (!displayGame) return [];
    const list = localHero
      ? [localHero, ...heroCandidates(displayGame)]
      : heroCandidates(displayGame);
    return [...new Set(list)];
  });
  let heroAttempt = $state(0);
  let heroFailed = $state(false);
  let heroFor = $state("");
  const heroImage = $derived(heroFailed ? null : (heroSources[heroAttempt] ?? null));

  $effect(() => {
    const currentId = game?.id ?? "";
    if (currentId === heroFor) return;
    heroFor = currentId;
    heroAttempt = 0;
    heroFailed = false;
  });

  // Session M: reflect art the user already applied (and auto-art written after
  // "Add to Steam") without waiting for a library rescan.
  $effect(() => {
    const current = game;
    if (!current || current.id === artFor) return;
    artFor = current.id;
    localCover = null;
    localHero = null;
    void refreshArt(current.id);
  });

  async function refreshArt(gameId: string): Promise<void> {
    if (!isTauriRuntime()) return;
    const status = await getArtStatus(gameId);
    artStatus = status;
    if (status.grid) localCover = coverSource(status.grid);
    if (status.hero) localHero = coverSource(status.hero);
  }

  function closeArt(): void {
    artOpen = false;
    if (game) void refreshArt(game.id);
  }

  const loading = $derived(library.status === "loading" && !game);
  const libraryError = $derived(library.status === "error" ? library.error : null);

  // Preview counts keep the visual smoke test meaningful without a backend.
  const MOCK_ACHIEVEMENTS: Record<string, AchievementSummary> = {
    "steam:1091500": { supported: true, unlocked: 18, total: 45 },
    "steam:1245620": { supported: true, unlocked: 11, total: 42 },
    "steam:1145360": { supported: true, unlocked: 43, total: 49 },
    "steam:757310": { supported: true, unlocked: 0, total: 26 },
  };

  let achievements = $state<AchievementSummary | null>(null);
  let achievementRequest = 0;

  // Local-only, after the page is painted. A missing or unreadable cache never
  // blocks the tile grid and never shows an error.
  $effect(() => {
    const current = game;
    achievementRequest += 1;
    const token = achievementRequest;
    achievements = null;

    if (!current || current.platform !== "steam") return;
    if (library.usingMock) {
      achievements = MOCK_ACHIEVEMENTS[current.id] ?? {
        supported: false,
        unlocked: 0,
        total: 0,
      };
      return;
    }

    void getAchievements(current.id)
      .then((summary) => {
        if (token === achievementRequest) achievements = summary;
      })
      .catch(() => {
        /* unsupported/read failures already come back as supported:false */
      });
  });

  function toggleHidden(): void {
    if (!game) return;
    if (hidden) {
      unhideGame(game.id);
      pushToast("success", "Back in your library", `“${game.title}” is visible again.`);
      return;
    }
    hideGame(game.id);
    pushToast(
      "info",
      "Hidden from library",
      `“${game.title}” is hidden. Unhide it from Settings at any time.`,
    );
    back();
  }

  onMount(() => {
    applyStoredAccent();
    if (library.status === "idle") void loadLibrary();

    const stopGamepad = onGamepad((action) => {
      if (artOpen) return;
      if (shellFocus.zone === "rail") return;
      if (action === "up") focusNext(-1);
      else if (action === "down") focusNext(1);
      else if (action === "back") back();
      else if (action === "confirm") void play();
      else if (action === "secondary" && game?.platform === "gog") void addSteam();
    });

    function onKeydown(event: KeyboardEvent): void {
      if (
        artOpen ||
        event.defaultPrevented ||
        event.ctrlKey ||
        event.metaKey ||
        event.altKey ||
        isInteractiveTarget(event.target)
      ) {
        return;
      }
      if (event.key === "Enter" && game) {
        event.preventDefault();
        void play();
      } else if (event.key.toLowerCase() === "f" && game) {
        event.preventDefault();
        toggleFav();
      }
    }

    window.addEventListener("keydown", onKeydown);
    return () => {
      stopGamepad();
      window.removeEventListener("keydown", onKeydown);
    };
  });

  function toggleFav(): void {
    if (!game) return;
    const added = toggleFavorite(game.id);
    pushToast(
      "info",
      added ? "Added to favorites" : "Removed from favorites",
      added
        ? `“${game.title}” now sits at the top of your library.`
        : `“${game.title}” left your favorites.`,
    );
  }

  function onHeroError(): void {
    if (heroAttempt < heroSources.length - 1) heroAttempt += 1;
    else heroFailed = true;
  }

  async function play(): Promise<void> {
    if (!game || launching || !canLaunch) return;

    const installing = !game.isInstalled;
    launchMode = installing ? "install" : "play";
    launching = true;
    const started = performance.now();
    let failure: string | null = null;

    try {
      if (library.usingMock) {
        // Preview mode: show the launching state without a backend.
        await wait(1500);
      } else {
        await launchGame(game.id);
        // Mirror the backend journal locally so the recent row moves immediately.
        if (!installing) recordLaunch(game.id);
      }
    } catch (error) {
      failure = humanError(error);
    }

    await waitAtLeast(started, failure ? 650 : 1350);
    launching = false;

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
        "Launching is simulated with dev mock data. The real app hands off to the game window.",
      );
    }
  }

  async function addSteam(): Promise<void> {
    if (!game || steamState === "loading" || steamState === "success") return;

    steamState = "loading";
    steamError = null;

    try {
      if (library.usingMock) {
        await wait(900);
        steamState = "success";
        pushToast(
          "success",
          "Preview mode",
          "No shortcut was written. Restart Steam to see the new shortcut in your Library in the real app.",
        );
        return;
      }

      const addedId = game.id;
      await addToSteam(game.id);
      steamState = "success";
      pushToast(
        "success",
        "Shortcut added to Steam",
        "Restart Steam to see the new shortcut in your Library.",
      );
      // Auto-art runs after this command resolves. Poll twice, quietly, so the
      // local cover appears without blocking the toast.
      void wait(3500).then(() => refreshArt(addedId));
      void wait(9000).then(() => refreshArt(addedId));
    } catch (error) {
      const message = humanError(error);
      steamState = "error";
      steamError = message;

      if (isSteamRunningError(message)) {
        pushToast(
          "error",
          "Close Steam first",
          "Steam is running. Quit Steam completely, then add the game again.",
        );
      } else {
        pushToast("error", "Couldn't add to Steam", message);
      }
    }
  }

  function wait(ms: number): Promise<void> {
    return new Promise((resolve) => setTimeout(resolve, ms));
  }

  async function waitAtLeast(started: number, ms: number): Promise<void> {
    const remaining = ms - (performance.now() - started);
    if (remaining > 0) await wait(remaining);
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
</script>

<section class="detail">
  <button type="button" class="back" onclick={back}>
    <Icon name="arrow-left" size={16} />
    <span>Library</span>
    <ControlHint action="back" keys="Esc" />
  </button>

  {#if loading}
    <div class="loading">
      <Spinner size={26} />
      <span>Reading your library…</span>
    </div>
  {:else if !game}
    <div class="missing">
      <span class="missing-icon"><Icon name="alert" size={24} /></span>
      <h1>{libraryError ? "Couldn't read your library" : "Game not found"}</h1>
      <p>
        {libraryError ??
          `No game with id “${id}” is in the current library. It may have been removed or renamed.`}
      </p>
      <button type="button" class="ghost" onclick={back}>
        <Icon name="arrow-left" size={15} />
        Back to library
      </button>
    </div>
  {:else}
    <article class="hero">
      <div class="hero-bg" style:background={placeholderBackground(game)}>
        {#if heroImage}
          <img src={heroImage} alt="" draggable="false" onerror={onHeroError} />
        {/if}
        <div class="hero-shade"></div>
      </div>

      <div class="hero-grid">
        <div class="cover">
          <GameArt game={displayGame ?? game} eager />
        </div>

        <div class="lede">
          <div class="tags">
            <PlatformBadge platform={game.platform} />
            <span class="state" class:ready={game.isInstalled}>
              <span class="state-dot" aria-hidden="true"></span>
              {playState(game)}
            </span>
          </div>

          <h1>{game.title}</h1>

          <p class="summary">
            {game.isInstalled
              ? "Ready when you are."
              : isSteamOwnedGame(game)
                ? "Owned in your Steam library. Install it through Steam and it appears here, ready to play."
                : "In your library, not downloaded. Install it from your store to play."}
          </p>

          {#if playedSummary}
            <p class="played-line">{playedSummary}</p>
          {/if}

          <div class="actions">
            <button
              type="button"
              class="play"
              disabled={!canLaunch || launching}
              onclick={() => void play()}
            >
              <span class="play-icon">
                <Icon name={game.isInstalled ? "play" : "download"} size={17} />
              </span>
              {launching
                ? launchMode === "install"
                  ? "Opening Steam…"
                  : "Launching…"
                : primaryLabel}
            </button>

            <button
              type="button"
              class="favorite-action"
              class:on={favorite}
              aria-pressed={favorite}
              aria-label={favorite ? "Remove from favorites" : "Add to favorites"}
              onclick={toggleFav}
            >
              <Icon name={favorite ? "star-fill" : "star"} size={16} />
              <span>{favorite ? "Favorited" : "Favorite"}</span>
            </button>

            {#if game.platform === "gog"}
              <button
                type="button"
                class="steam"
                disabled={steamState === "loading" || steamState === "success"}
                onclick={() => void addSteam()}
              >
                {#if steamState === "loading"}
                  <Spinner size={16} />
                  <span>Adding…</span>
                {:else if steamState === "success"}
                  <Icon name="check" size={16} />
                  <span>Added to Steam</span>
                {:else}
                  <Icon name="plus" size={16} />
                  <span>Add to Steam</span>
                {/if}
              </button>
            {/if}

            {#if game.platform !== "steam"}
              <button
                type="button"
                class="art-action"
                onclick={() => (artOpen = true)}
              >
                <Icon name="spark" size={16} />
                <span>Change artwork</span>
              </button>
            {/if}

            <button type="button" class="hide-action" onclick={toggleHidden}>
              <Icon name={hidden ? "eye" : "hide"} size={16} />
              <span>{hidden ? "Unhide" : "Hide"}</span>
            </button>
          </div>

          {#if artStatus?.error}
            <p class="art-note" title={artStatus.error}>
              Automatic artwork was incomplete. Choose Change artwork to retry.
            </p>
          {/if}

          {#if game.platform === "gog"}
            <p class="steam-note" class:error={steamState === "error"}>
              {#if steamState === "error" && steamError}
                {steamError}
              {:else if steamState === "success"}
                Restart Steam to see the new shortcut in your Library.
              {:else}
                Adds a non-Steam shortcut. Steam must be closed while Kudos writes it.
              {/if}
            </p>
          {/if}

          <p class="key-hint">
            <ControlHint action="dpad" keys="↑ ↓" label="Move" />
            <ControlHint action="confirm" keys="Enter" label={primaryLabel} />
            <ControlHint action="secondary" keys="F" label="Favorite" />
            {#if game.platform === "gog"}
              <ControlHint action="secondary" keys="X" label="Add to Steam" />
            {/if}
            <ControlHint action="back" keys="Esc" label="Back" />
          </p>
        </div>
      </div>
    </article>

    <dl class="facts">
      <div>
        <dt>Status</dt>
        <dd>{playState(game)}</dd>
      </div>
      <div>
        <dt>Store</dt>
        <dd>{platformShort(game.platform)}</dd>
      </div>
      {#if playedSummary}
        <div>
          <dt>Playtime</dt>
          <dd>{playedSummary}</dd>
        </div>
      {/if}
      <div>
        <dt>Size on disk</dt>
        <dd>{formatBytes(game.sizeBytes)}</dd>
      </div>
      <div class="wide">
        <dt>Executable</dt>
        <dd class="path">
          {game.exePath ??
            (game.isInstalled
              ? `Managed by ${platformShort(game.platform)}`
              : `Install through ${platformShort(game.platform)}`)}
        </dd>
      </div>
    </dl>

    {#if game.platform !== "steam"}
      <p class="ach-unavailable">Achievements not available for this platform</p>
    {:else if achievements?.supported}
      <section class="achievements" aria-label="Achievements">
        <div class="ach-head">
          <span class="ach-icon"><Icon name="trophy" size={17} /></span>
          <h2>Achievements</h2>
          <span class="ach-count">
            — {achievements.unlocked} / {achievements.total} unlocked
          </span>
        </div>
        <div
          class="ach-bar"
          role="progressbar"
          aria-valuemin="0"
          aria-valuemax={achievements.total}
          aria-valuenow={achievements.unlocked}
          aria-label="Achievement progress"
        >
          <span
            style:transform={`scaleX(${
              achievements.total > 0 ? achievements.unlocked / achievements.total : 0
            })`}
          ></span>
        </div>
      </section>
    {/if}
  {/if}
</section>

<LaunchOverlay
  active={launching}
  title={game?.title}
  label={launchMode === "install" ? "Opening Steam" : "Launching"}
  hint={launchMode === "install" ? "Approve the install in Steam" : "Handing over to the game window"}
/>

{#if artOpen && game}
  <ArtPicker {game} onclose={closeArt} />
{/if}

<Toaster />

<style>
  .detail {
    min-height: 100%;
    display: flex;
    flex-direction: column;
  }

  /* ---------- back ---------- */
  .back {
    align-self: flex-start;
    display: inline-flex;
    align-items: center;
    gap: 8px;
    height: 34px;
    padding: 0 12px;
    margin-bottom: clamp(16px, 2.4vh, 24px);
    border-radius: 999px;
    font-size: 12px;
    font-weight: 500;
    color: var(--fg-1);
    background: rgba(255, 255, 255, 0.045);
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.07);
    transition:
      transform var(--dur-fast) var(--ease-out),
      opacity var(--dur-fast) var(--ease-out);
    will-change: transform;
  }

  .back:hover {
    transform: translateX(-2px);
  }

  /* ---------- hero ---------- */
  .hero {
    position: relative;
    overflow: hidden;
    border-radius: clamp(18px, 2vw, 30px);
    min-height: clamp(360px, 58vh, 580px);
    display: flex;
    align-items: flex-end;
    box-shadow:
      0 34px 90px rgba(0, 0, 0, 0.48),
      inset 0 0 0 1px rgba(255, 255, 255, 0.07);
    isolation: isolate;
  }

  .hero-bg {
    position: absolute;
    inset: 0;
    z-index: -1;
    overflow: hidden;
  }

  .hero-bg img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    filter: saturate(1.15) brightness(0.78);
    animation: hero-zoom 900ms var(--ease-out) both;
    will-change: transform, opacity;
  }

  .hero-shade {
    position: absolute;
    inset: 0;
    background:
      linear-gradient(
        100deg,
        rgba(5, 7, 12, 0.9) 0%,
        rgba(5, 7, 12, 0.62) 38%,
        rgba(5, 7, 12, 0.18) 72%,
        rgba(5, 7, 12, 0.42) 100%
      ),
      linear-gradient(180deg, transparent 45%, rgba(5, 7, 12, 0.75) 100%);
  }

  @keyframes hero-zoom {
    from {
      opacity: 0.35;
      transform: scale(1.08);
    }
    to {
      opacity: 1;
      transform: scale(1);
    }
  }

  .hero-grid {
    position: relative;
    z-index: 1;
    width: 100%;
    display: flex;
    align-items: flex-end;
    gap: clamp(24px, 3.4vw, 48px);
    padding: clamp(24px, 3.4vw, 48px);
  }

  .cover {
    flex: 0 0 auto;
    width: clamp(176px, 26vh, 290px);
    aspect-ratio: 2 / 3;
    border-radius: clamp(12px, 1.2vw, 18px);
    overflow: hidden;
    box-shadow:
      0 0 0 1px rgba(255, 255, 255, 0.16),
      0 26px 70px rgba(0, 0, 0, 0.6),
      0 0 40px 2px var(--accent-glow);
    animation: cover-in 700ms var(--ease-out) both;
    will-change: transform, opacity;
    transform-origin: center 70%;
  }

  @keyframes cover-in {
    from {
      opacity: 0;
      transform: translateY(22px) scale(0.9);
    }
    to {
      opacity: 1;
      transform: translateY(0) scale(1);
    }
  }

  .lede {
    flex: 1;
    min-width: 0;
    padding-bottom: 4px;
    animation: lede-in 620ms 60ms var(--ease-out) both;
    will-change: transform, opacity;
  }

  @keyframes lede-in {
    from {
      opacity: 0;
      transform: translateY(16px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }

  .tags {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
    margin-bottom: 14px;
  }

  .state {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    padding: 4px 10px 4px 8px;
    border-radius: 999px;
    font-size: 10.5px;
    font-weight: 550;
    letter-spacing: 0.045em;
    color: var(--fg-1);
    background: rgba(8, 12, 20, 0.35);
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.1);
    backdrop-filter: blur(10px);
    white-space: nowrap;
  }

  .state-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--fg-2);
  }

  .state.ready .state-dot {
    background: var(--ok);
    box-shadow: 0 0 10px rgba(61, 220, 151, 0.55);
  }

  h1 {
    margin: 0;
    font-family: var(--font-display);
    font-size: clamp(34px, 6vh, 62px);
    line-height: 0.98;
    font-weight: 600;
    letter-spacing: -0.042em;
    color: var(--fg-0);
    max-width: 18ch;
    text-wrap: balance;
  }

  .summary {
    margin: 12px 0 0;
    font-size: 13.5px;
    line-height: 1.55;
    color: var(--fg-1);
    max-width: 46ch;
  }

  .played-line {
    margin: 8px 0 0;
    font-size: 11.5px;
    color: var(--fg-2);
    font-variant-numeric: tabular-nums;
  }

  .actions {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 12px;
    margin-top: 24px;
  }

  .play {
    display: inline-flex;
    align-items: center;
    gap: 12px;
    height: 58px;
    padding: 0 34px;
    border-radius: 999px;
    font-size: 16px;
    font-weight: 650;
    letter-spacing: -0.01em;
    color: #05070c;
    background: #f7f9ff;
    box-shadow:
      0 16px 40px rgba(0, 0, 0, 0.42),
      inset 0 0 0 1px rgba(255, 255, 255, 0.55);
    transition:
      transform var(--dur-med) var(--ease-out),
      opacity var(--dur-fast) var(--ease-out);
    will-change: transform;
  }

  .play:hover:not(:disabled) {
    transform: translateY(-2px) scale(1.03);
  }

  .play:active:not(:disabled) {
    transform: scale(0.985);
  }

  .play:disabled {
    opacity: 0.46;
    cursor: default;
  }

  .play-icon {
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    border-radius: 50%;
    color: #fff;
    background: #05070c;
  }

  .steam {
    display: inline-flex;
    align-items: center;
    gap: 9px;
    height: 48px;
    padding: 0 20px;
    border-radius: 999px;
    font-size: 13px;
    font-weight: 600;
    color: var(--fg-0);
    background: rgba(9, 13, 22, 0.42);
    box-shadow:
      inset 0 0 0 1px rgba(255, 255, 255, 0.16),
      0 10px 26px rgba(0, 0, 0, 0.24);
    backdrop-filter: blur(16px);
    transition:
      transform var(--dur-med) var(--ease-out),
      opacity var(--dur-fast) var(--ease-out);
    will-change: transform;
  }

  .steam:hover:not(:disabled) {
    transform: translateY(-2px);
    background: rgba(255, 255, 255, 0.09);
  }

  .steam:active:not(:disabled) {
    transform: scale(0.98);
  }

  .steam:disabled {
    opacity: 0.68;
    cursor: default;
  }

  .hide-action {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    height: 40px;
    padding: 0 15px;
    border-radius: 999px;
    font-size: 12px;
    font-weight: 550;
    color: var(--fg-2);
    background: rgba(9, 13, 22, 0.3);
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.1);
    backdrop-filter: blur(12px);
    transition:
      color var(--dur-fast) var(--ease-out),
      transform var(--dur-fast) var(--ease-out);
    will-change: transform;
  }

  .hide-action:hover {
    color: var(--fg-0);
    transform: translateY(-1px);
  }

  .favorite-action,
  .art-action {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    height: 40px;
    padding: 0 15px;
    border-radius: 999px;
    font-size: 12px;
    font-weight: 550;
    color: var(--fg-2);
    background: rgba(9, 13, 22, 0.3);
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.1);
    backdrop-filter: blur(12px);
    transition:
      color var(--dur-fast) var(--ease-out),
      transform var(--dur-fast) var(--ease-out);
    will-change: transform;
  }

  .favorite-action:hover,
  .art-action:hover {
    color: var(--fg-0);
    transform: translateY(-1px);
  }

  .favorite-action.on {
    color: #ffd166;
    box-shadow:
      inset 0 0 0 1px rgba(255, 209, 102, 0.36),
      0 0 22px rgba(255, 209, 102, 0.12);
  }

  .art-note {
    margin: 10px 0 0;
    font-size: 10.5px;
    line-height: 1.5;
    color: var(--fg-2);
    opacity: 0.86;
  }

  .steam-note {
    margin: 10px 0 0;
    min-height: 17px;
    font-size: 11px;
    line-height: 1.5;
    color: var(--fg-2);
    max-width: 50ch;
  }

  .steam-note.error {
    color: var(--danger);
  }

  .key-hint {
    display: flex;
    align-items: center;
    gap: 14px;
    flex-wrap: wrap;
    margin: 16px 0 0;
    font-size: 10.5px;
    color: var(--fg-2);
  }

  /* ---------- achievements ---------- */
  .achievements {
    margin-top: clamp(16px, 2.4vh, 24px);
    padding: 18px 20px;
    border-radius: 14px;
    background:
      linear-gradient(150deg, rgba(255, 255, 255, 0.045), rgba(255, 255, 255, 0.015));
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.06);
  }

  .ach-head {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
  }

  .ach-icon {
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    border-radius: 9px;
    flex: 0 0 auto;
    color: var(--accent);
    background: color-mix(in srgb, var(--accent) 13%, transparent);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--accent) 26%, transparent);
  }

  .ach-head h2 {
    margin: 0;
    font-size: 13.5px;
    font-weight: 600;
    letter-spacing: -0.01em;
  }

  .ach-count {
    font-size: 11.5px;
    color: var(--fg-2);
    font-variant-numeric: tabular-nums;
  }

  .ach-bar {
    margin-top: 13px;
    height: 8px;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.08);
    overflow: hidden;
  }

  .ach-bar span {
    display: block;
    width: 100%;
    height: 100%;
    border-radius: inherit;
    transform-origin: left center;
    background: linear-gradient(
      90deg,
      var(--accent),
      color-mix(in srgb, var(--accent) 58%, #ffffff)
    );
    transition: transform 620ms var(--ease-out);
    will-change: transform;
  }

  .ach-unavailable {
    margin: clamp(16px, 2.4vh, 24px) 0 0;
    font-size: 11px;
    color: var(--fg-2);
    opacity: 0.82;
  }

  /* ---------- facts ---------- */
  .facts {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(170px, 1fr));
    gap: 12px;
    margin: clamp(16px, 2.4vh, 24px) 0 0;
  }

  .facts > div {
    padding: 15px 17px;
    border-radius: 14px;
    background: rgba(255, 255, 255, 0.028);
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.055);
    min-width: 0;
  }

  .facts .wide {
    grid-column: span 2;
  }

  dt {
    margin-bottom: 6px;
    font-size: 9.5px;
    font-weight: 600;
    letter-spacing: 0.13em;
    text-transform: uppercase;
    color: var(--fg-2);
  }

  dd {
    margin: 0;
    font-size: 12.5px;
    color: var(--fg-1);
  }

  dd.path {
    font-size: 11px;
    color: var(--fg-2);
    overflow-wrap: anywhere;
    line-height: 1.45;
  }

  /* ---------- loading / missing ---------- */
  .loading {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 12px;
    color: var(--fg-2);
    font-size: 13px;
    padding: 40px 0;
  }

  .missing {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    justify-content: center;
    max-width: 560px;
    padding: 30px 0 60px;
  }

  .missing-icon {
    display: grid;
    place-items: center;
    width: 50px;
    height: 50px;
    border-radius: 15px;
    margin-bottom: 18px;
    color: var(--danger);
    background: rgba(255, 95, 109, 0.12);
    box-shadow: inset 0 0 0 1px rgba(255, 95, 109, 0.24);
  }

  .missing h1 {
    margin: 0 0 8px;
    font-size: clamp(30px, 4vh, 40px);
    letter-spacing: -0.035em;
  }

  .missing p {
    margin: 0 0 22px;
    font-size: 13px;
    line-height: 1.6;
    color: var(--fg-2);
    overflow-wrap: anywhere;
  }

  .ghost {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    height: 40px;
    padding: 0 16px;
    border-radius: 999px;
    font-size: 12.5px;
    font-weight: 600;
    color: var(--fg-0);
    background: rgba(255, 255, 255, 0.07);
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.1);
    transition: transform var(--dur-fast) var(--ease-out);
  }

  .ghost:hover {
    transform: translateY(-1px);
  }

  @media (max-width: 900px) {
    .hero-grid {
      flex-direction: column;
      align-items: flex-start;
    }
    .cover {
      width: clamp(120px, 18vh, 180px);
    }
    .hero {
      min-height: auto;
    }
    .hero-shade {
      background:
        linear-gradient(180deg, rgba(5, 7, 12, 0.4) 0%, rgba(5, 7, 12, 0.92) 78%),
        linear-gradient(100deg, rgba(5, 7, 12, 0.8), transparent);
    }
    .facts .wide {
      grid-column: auto;
    }
  }
</style>
