<!--
  GAME DETAIL
  Big cover, Play, and the GOG-only "Add to Steam" flow with loading / success /
  "Close Steam first" states. OWNER: Session D.
-->
<script lang="ts">
  import { onMount } from "svelte";
  import { addToSteam, launchGame } from "../lib/api";
  import { coverSource, placeholderBackground } from "../lib/art";
  import {
    formatBytes,
    humanError,
    isSteamRunningError,
    platformShort,
    playState,
  } from "../lib/format";
  import { onGamepad } from "../lib/gamepad";
  import { findGame, library, loadLibrary } from "../lib/library.svelte";
  import { back } from "../lib/router.svelte";
  import { applyStoredAccent } from "../lib/theme";
  import { pushToast } from "../lib/toast.svelte";
  import GameArt from "../lib/components/GameArt.svelte";
  import Icon from "../lib/components/Icon.svelte";
  import LaunchOverlay from "../lib/components/LaunchOverlay.svelte";
  import PlatformBadge from "../lib/components/PlatformBadge.svelte";
  import Spinner from "../lib/components/Spinner.svelte";
  import Toaster from "../lib/components/Toaster.svelte";

  let { id }: { id: string } = $props();

  type SteamState = "idle" | "loading" | "success" | "error";

  let launching = $state(false);
  let steamState = $state<SteamState>("idle");
  let steamError = $state<string | null>(null);

  const game = $derived(findGame(id));
  const heroImage = $derived(game ? coverSource(game.coverImage) : null);
  const loading = $derived(library.status === "loading" && !game);
  const libraryError = $derived(library.status === "error" ? library.error : null);

  onMount(() => {
    applyStoredAccent();
    if (library.status === "idle") void loadLibrary();

    const stopGamepad = onGamepad((action) => {
      if (action === "back") back();
      else if (action === "confirm") void play();
      else if (action === "secondary" && game?.platform === "gog") void addSteam();
    });

    function onKeydown(event: KeyboardEvent): void {
      if (event.defaultPrevented || event.ctrlKey || event.metaKey || event.altKey) {
        return;
      }
      if (event.key === "Enter" && !isInteractiveTarget(event.target) && game) {
        event.preventDefault();
        void play();
      }
    }

    window.addEventListener("keydown", onKeydown);
    return () => {
      stopGamepad();
      window.removeEventListener("keydown", onKeydown);
    };
  });

  async function play(): Promise<void> {
    if (!game || launching || !game.isInstalled) return;

    launching = true;
    const started = performance.now();
    let failure: string | null = null;

    try {
      if (library.usingMock) {
        // Preview mode: show the launching state without a backend.
        await wait(1500);
      } else {
        await launchGame(game.id);
      }
    } catch (error) {
      failure = humanError(error);
    }

    await waitAtLeast(started, failure ? 650 : 1350);
    launching = false;

    if (failure) {
      pushToast("error", "Couldn't launch", failure);
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

      await addToSteam(game.id);
      steamState = "success";
      pushToast(
        "success",
        "Shortcut added to Steam",
        "Restart Steam to see the new shortcut in your Library.",
      );
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
    <kbd>Esc</kbd>
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
          <img src={heroImage} alt="" draggable="false" />
        {/if}
        <div class="hero-shade"></div>
      </div>

      <div class="hero-grid">
        <div class="cover">
          <GameArt {game} eager />
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
              : "In your library, not downloaded. Install it from your store to play."}
          </p>

          <div class="actions">
            <button
              type="button"
              class="play"
              disabled={!game.isInstalled || launching}
              onclick={() => void play()}
            >
              <span class="play-icon"><Icon name="play" size={17} /></span>
              {launching ? "Launching…" : game.isInstalled ? "Play" : "Not installed"}
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
          </div>

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
            <kbd>Enter</kbd> Play
            {#if game.platform === "gog"}<span class="sep">·</span><kbd>X</kbd> Add to Steam{/if}
            <span class="sep">·</span><kbd>B</kbd> Back
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
      <div>
        <dt>Size on disk</dt>
        <dd>{formatBytes(game.sizeBytes)}</dd>
      </div>
      <div class="wide">
        <dt>Executable</dt>
        <dd class="path">
          {game.exePath ?? `Managed by ${platformShort(game.platform)}`}
        </dd>
      </div>
    </dl>
  {/if}
</section>

<LaunchOverlay active={launching} title={game?.title} />
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

  .back kbd {
    font-family: var(--font-ui);
    font-size: 9px;
    font-weight: 600;
    letter-spacing: 0.03em;
    color: var(--fg-2);
    background: rgba(255, 255, 255, 0.06);
    border-radius: 4px;
    padding: 2px 5px;
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
    gap: 7px;
    flex-wrap: wrap;
    margin: 16px 0 0;
    font-size: 10.5px;
    color: var(--fg-2);
  }

  .key-hint kbd {
    font-family: var(--font-ui);
    font-size: 9px;
    font-weight: 650;
    letter-spacing: 0.04em;
    color: var(--fg-1);
    background: rgba(255, 255, 255, 0.07);
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.1);
    border-radius: 4px;
    padding: 2px 5px;
  }

  .key-hint .sep {
    opacity: 0.35;
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
