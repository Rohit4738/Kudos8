<!--
  IDLE / AMBIENT MODE — a PS5-style attract screen.
  OWNER: Session G.

  Performance notes:
  - Two image layers max. Each layer is one compositing layer (`will-change:
    transform, opacity`); the Ken Burns pan/zoom is transform-only.
  - The next 1–2 images are preloaded into a cache capped at 3 detached images.
  - All timers stop while the window is hidden/unfocused, and the drift pauses
    when `prefers-reduced-motion` is set (opacity-only crossfade remains).
  - This is a low-activity ambient display, NOT a screensaver. It does not blank
    the display or suspend the GPU; Windows power settings do that.
-->
<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { coverSource, placeholderBackground } from "../art";
  import { DEFAULT_IDLE, type IdleSettings } from "../idle.svelte";
  import { relativePlayed, resolveRecent, type RecentGame } from "../recent";
  import type { Game } from "../types";
  import PlatformBadge from "./PlatformBadge.svelte";

  interface Props {
    games: Game[];
    recent?: RecentGame[];
    settings?: IdleSettings;
    onExit: () => void;
  }

  let { games, recent = [], settings = DEFAULT_IDLE, onExit }: Props = $props();

  interface AmbientArt {
    key: string;
    game: Game;
    primary: string | null;
    fallback: string | null;
    background: string;
    direction: "a" | "b";
  }

  let layers = $state<AmbientArt[]>([]);
  let currentId = $state<string | null>(null);
  let failedUrls = $state<string[]>([]);
  let paused = $state(false);
  let leaving = $state(false);
  let reducedMotion = $state(false);
  let mounted = $state(false);
  let started = $state(false);

  let rootEl = $state<HTMLDivElement | null>(null);
  let exiting = false;
  let ordinal = 1;

  let slideTimer: ReturnType<typeof setTimeout> | undefined;
  let fadeTimer: ReturnType<typeof setTimeout> | undefined;
  let exitTimer: ReturnType<typeof setTimeout> | undefined;
  let graceTimer: ReturnType<typeof setTimeout> | undefined;
  let clockTimer: ReturnType<typeof setInterval> | undefined;
  let padTimer: ReturnType<typeof setInterval> | undefined;
  let media: MediaQueryList | null = null;

  let inputArmed = false;
  let padSignature = "";
  let savedFocus: HTMLElement | null = null;
  let savedPageScroll: number[] = [];
  let savedWindowScroll = { x: 0, y: 0 };
  let previousBodyOverflow = "";

  const slideMs = $derived(settings.crossfade === "fast" ? 8_000 : 15_000);
  const fadeMs = $derived(settings.crossfade === "fast" ? 1_000 : 2_000);

  const pool = $derived.by(() => {
    const installed = games.filter((game) => game.isInstalled);
    if (settings.artSource !== "recent") return installed;
    const picked = resolveRecent(recent, installed).map((item) => item.game);
    return picked.length > 0 ? picked : installed;
  });

  const current = $derived(layers.length > 0 ? layers[layers.length - 1] : null);
  const currentEntry = $derived(
    current ? recent.find((entry) => entry.id === current.game.id) ?? null : null,
  );

  let now = $state(new Date());
  let clock = $derived(
    now.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }),
  );
  let date = $derived(
    now.toLocaleDateString([], { weekday: "long", month: "long", day: "numeric" }),
  );

  // ---- art -----------------------------------------------------------------

  function steamHero(game: Game): string | null {
    if (game.platform !== "steam") return null;
    const appid = game.id.startsWith("steam:") ? game.id.slice(6) : "";
    return /^\d+$/.test(appid)
      ? `https://cdn.cloudflare.steamstatic.com/steam/apps/${appid}/library_hero.jpg`
      : null;
  }

  function coverFor(game: Game): string | null {
    // GOG's only local art is a tiny .ico; it looks bad full-screen, so use the
    // generated gradient instead (Steam has the local 600x900 cache).
    if (game.coverImage && /\.ico$/i.test(game.coverImage)) return null;
    return coverSource(game.coverImage);
  }

  function makeArt(game: Game): AmbientArt {
    const cover = coverFor(game);
    const hero = steamHero(game);
    const primary = hero ?? cover;
    return {
      key: game.id,
      game,
      primary,
      fallback: primary && cover && cover !== primary ? cover : null,
      background: placeholderBackground(game),
      direction: ordinal % 2 === 1 ? "a" : "b",
    };
  }

  function imageFor(art: AmbientArt): string | null {
    if (art.primary && !failedUrls.includes(art.primary)) return art.primary;
    if (art.fallback && !failedUrls.includes(art.fallback)) return art.fallback;
    return null;
  }

  function onImageError(url: string): void {
    if (!failedUrls.includes(url)) failedUrls = [...failedUrls, url];
  }

  // Detached-image cache, capped: the WebView keeps decoded art bounded.
  const preloaded: HTMLImageElement[] = [];
  function preload(url: string | null): void {
    if (!url) return;
    if (preloaded.some((image) => image.dataset.idleSrc === url)) return;
    const image = new Image();
    image.decoding = "async";
    image.dataset.idleSrc = url;
    image.src = url;
    preloaded.push(image);
    while (preloaded.length > 3) preloaded.shift();
  }

  function preloadAroundCurrent(): void {
    const count = pool.length;
    if (count < 2) return;
    const at = currentId ? pool.findIndex((game) => game.id === currentId) : 0;
    const base = at < 0 ? 0 : at;
    for (let step = 1; step <= Math.min(2, count - 1); step += 1) {
      const game = pool[(base + step) % count];
      preload(steamHero(game) ?? coverFor(game));
    }
  }

  // ---- sequence ------------------------------------------------------------

  function nextIndex(): number {
    const count = pool.length;
    if (count === 0) return -1;
    if (count === 1) return 0;
    const at = currentId ? pool.findIndex((game) => game.id === currentId) : -1;
    if (at < 0) return 0;
    if (settings.artSource === "random") {
      let next = at;
      while (next === at) next = Math.floor(Math.random() * count);
      return next;
    }
    return (at + 1) % count;
  }

  function start(): void {
    if (started || pool.length === 0) return;
    const first = makeArt(pool[0]);
    ordinal += 1;
    currentId = first.key;
    layers = [first];
    started = true;
    preloadAroundCurrent();
    scheduleSlide();
  }

  function advance(): void {
    if (exiting || paused || pool.length === 0) return;
    const next = nextIndex();
    if (next < 0) return;
    const art = makeArt(pool[next]);
    ordinal += 1;

    if (art.key === currentId && pool.length > 1) {
      // Defensive: never show the same game twice in a row.
      scheduleSlide();
      return;
    }

    const previous = layers[layers.length - 1];
    layers = previous ? [previous, art] : [art];
    currentId = art.key;
    preloadAroundCurrent();

    if (fadeTimer) clearTimeout(fadeTimer);
    fadeTimer = setTimeout(() => {
      // The new layer is fully opaque; drop the old compositing layer.
      if (layers.length > 1) layers = [layers[layers.length - 1]];
    }, fadeMs + 120);

    scheduleSlide();
  }

  function scheduleSlide(): void {
    if (slideTimer) clearTimeout(slideTimer);
    slideTimer = undefined;
    if (!mounted || exiting || paused || pool.length === 0) return;
    slideTimer = setTimeout(advance, slideMs);
  }

  // ---- exit ----------------------------------------------------------------

  function captureSavedState(): void {
    savedFocus =
      document.activeElement instanceof HTMLElement ? document.activeElement : null;
    savedPageScroll = [...document.querySelectorAll<HTMLElement>(".page")].map(
      (page) => page.scrollTop,
    );
    savedWindowScroll = { x: window.scrollX, y: window.scrollY };
    previousBodyOverflow = document.body.style.overflow;
    document.body.style.overflow = "hidden";
  }

  function restoreSavedState(): void {
    document.body.style.overflow = previousBodyOverflow;
    [...document.querySelectorAll<HTMLElement>(".page")].forEach((page, index) => {
      const top = savedPageScroll[index];
      if (top !== undefined) page.scrollTop = top;
    });
    window.scrollTo(savedWindowScroll.x, savedWindowScroll.y);
  }

  function requestExit(): void {
    if (exiting) return;
    exiting = true;
    leaving = true;
    paused = true;
    clearTimers();
    restoreSavedState();

    exitTimer = setTimeout(() => {
      onExit();
      requestAnimationFrame(() => {
        savedFocus?.focus({ preventScroll: true });
      });
    }, 270);
  }

  function clearTimers(): void {
    if (slideTimer) clearTimeout(slideTimer);
    if (fadeTimer) clearTimeout(fadeTimer);
    if (exitTimer) clearTimeout(exitTimer);
    if (graceTimer) clearTimeout(graceTimer);
    if (clockTimer) clearInterval(clockTimer);
    if (padTimer) clearInterval(padTimer);
    slideTimer = undefined;
    fadeTimer = undefined;
    exitTimer = undefined;
    graceTimer = undefined;
    clockTimer = undefined;
    padTimer = undefined;
  }

  // ---- pausing -------------------------------------------------------------

  function setPaused(next: boolean): void {
    if (paused === next) return;
    paused = next;
    if (next) {
      if (slideTimer) clearTimeout(slideTimer);
      slideTimer = undefined;
    } else {
      scheduleSlide();
    }
  }

  function onVisibility(): void {
    if (document.hidden) setPaused(true);
    else setPaused(false);
  }

  // ---- gamepad wake --------------------------------------------------------

  function readPadSignature(): string {
    if (typeof navigator.getGamepads !== "function") return "";
    let signature = "";
    try {
      for (const pad of navigator.getGamepads()) {
        if (!pad) continue;
        signature += `${pad.index}:`;
        for (const button of pad.buttons) signature += button.pressed ? "1" : "0";
        for (const axis of pad.axes) {
          signature += axis > 0.7 ? "+" : axis < -0.7 ? "-" : "0";
        }
        signature += ";";
      }
    } catch {
      return "";
    }
    return signature;
  }

  function pollPad(): void {
    if (exiting) return;
    const signature = readPadSignature();
    if (signature !== padSignature) {
      padSignature = signature;
      if (inputArmed) requestExit();
    }
  }

  // ---- lifecycle -----------------------------------------------------------

  onMount(() => {
    mounted = true;
    captureSavedState();
    rootEl?.focus({ preventScroll: true });

    media = window.matchMedia("(prefers-reduced-motion: reduce)");
    reducedMotion = media.matches;
    const onMediaChange = (event: MediaQueryListEvent) => (reducedMotion = event.matches);
    media.addEventListener("change", onMediaChange);

    // Any input exits, after a short grace period so the click that opened the
    // preview cannot immediately dismiss it.
    graceTimer = setTimeout(() => {
      inputArmed = true;
      graceTimer = undefined;
    }, 800);

    const inputOptions: AddEventListenerOptions = { capture: true, passive: true };
    const onInput = (event: Event) => {
      if (!inputArmed || exiting) return;
      if (event.cancelable) event.preventDefault();
      requestExit();
    };
    window.addEventListener("pointerdown", onInput, inputOptions);
    window.addEventListener("pointermove", onInput, inputOptions);
    window.addEventListener("mousedown", onInput, inputOptions);
    window.addEventListener("touchstart", onInput, inputOptions);
    window.addEventListener("keydown", onInput, inputOptions);
    window.addEventListener("wheel", onInput, { capture: true, passive: false });

    const onWindowBlur = () => setPaused(true);
    const onWindowFocus = () => setPaused(false);
    window.addEventListener("blur", onWindowBlur);
    window.addEventListener("focus", onWindowFocus);
    document.addEventListener("visibilitychange", onVisibility);

    padSignature = readPadSignature();
    padTimer = setInterval(pollPad, 400);

    const tickClock = () => {
      const next = new Date();
      if (next.getMinutes() !== now.getMinutes()) now = next;
    };
    clockTimer = setInterval(tickClock, 15_000);

    return () => {
      mounted = false;
      clearTimers();
      media?.removeEventListener("change", onMediaChange);
      window.removeEventListener("pointerdown", onInput, inputOptions);
      window.removeEventListener("pointermove", onInput, inputOptions);
      window.removeEventListener("mousedown", onInput, inputOptions);
      window.removeEventListener("touchstart", onInput, inputOptions);
      window.removeEventListener("keydown", onInput, inputOptions);
      window.removeEventListener("wheel", onInput, true);
      window.removeEventListener("blur", onWindowBlur);
      window.removeEventListener("focus", onWindowFocus);
      document.removeEventListener("visibilitychange", onVisibility);
      restoreSavedState();
    };
  });

  $effect(() => {
    if (!mounted) return;
    if (!started && pool.length > 0) start();
    else if (started && pool.length === 0) requestExit();
  });

  onDestroy(() => {
    clearTimers();
  });
</script>

<div
  class="ambient"
  class:leaving
  class:paused
  class:reduced={reducedMotion}
  bind:this={rootEl}
  role="dialog"
  aria-modal="true"
  aria-label="Ambient mode"
  tabindex="-1"
  style={`--slide-ms:${slideMs}ms; --fade-ms:${fadeMs}ms; --drift-ms:${slideMs + fadeMs}ms;`}
>
  <div class="backdrop" aria-hidden="true"></div>

  {#each layers as layer (layer.key)}
    {@const src = imageFor(layer)}
    <div
      class="layer"
      class:drift-b={layer.direction === "b"}
      style:background={layer.background}
      aria-hidden="true"
    >
      {#if src}
        <img
          {src}
          alt=""
          draggable="false"
          decoding="async"
          loading="eager"
          onerror={() => onImageError(src)}
        />
      {/if}
    </div>
  {/each}

  <div class="scrim" aria-hidden="true"></div>

  {#if settings.showClock}
    <div class="clock" aria-label={`${clock}, ${date}`}>
      <span class="time">{clock}</span>
      <span class="date">{date}</span>
    </div>
  {/if}

  {#if current}
    <div class="meta" aria-live="off">
      {#key current.key}
        <div class="meta-inner">
          <PlatformBadge platform={current.game.platform} compact />
          <h1>{current.game.title}</h1>
          {#if currentEntry}
            <p class="when">Played {relativePlayed(currentEntry.lastPlayedUnix)}</p>
          {:else}
            <p class="when">From your library</p>
          {/if}
        </div>
      {/key}
    </div>
  {/if}

  <span class="sr-only">Press any key or move the mouse to return to the library.</span>
</div>

<style>
  .ambient {
    position: fixed;
    inset: 0;
    z-index: 400;
    overflow: hidden;
    isolation: isolate;
    background: #030407;
    opacity: 1;
    transition: opacity 260ms linear;
    cursor: none;
    outline: none;
  }

  .ambient.leaving {
    opacity: 0;
    pointer-events: none;
  }

  .backdrop {
    position: absolute;
    inset: 0;
    background:
      radial-gradient(80% 70% at 50% 35%, #10141f 0%, #05070d 68%, #030407 100%);
  }

  .layer {
    position: absolute;
    inset: -4%;
    background-size: cover;
    background-position: center;
    transform-origin: 50% 46%;
    will-change: transform, opacity;
    animation:
      layer-in var(--fade-ms) var(--ease-out) both,
      drift-a var(--drift-ms) linear both;
    backface-visibility: hidden;
  }

  .layer.drift-b {
    animation:
      layer-in var(--fade-ms) var(--ease-out) both,
      drift-b var(--drift-ms) linear both;
  }

  .layer img {
    display: block;
    width: 100%;
    height: 100%;
    object-fit: cover;
    object-position: center;
    opacity: 0.9;
    pointer-events: none;
    user-select: none;
  }

  @keyframes layer-in {
    from {
      opacity: 0;
    }
    to {
      opacity: 1;
    }
  }

  @keyframes drift-a {
    from {
      transform: scale(1.045) translate3d(0, 0, 0);
    }
    to {
      transform: scale(1.16) translate3d(-1.8%, -1.4%, 0);
    }
  }

  @keyframes drift-b {
    from {
      transform: scale(1.14) translate3d(1.5%, 1.2%, 0);
    }
    to {
      transform: scale(1.04) translate3d(0, 0, 0);
    }
  }

  /* Heavy cinematic vignette: keeps eyes on the corner copy, never a black box. */
  .scrim {
    position: absolute;
    inset: 0;
    pointer-events: none;
    background:
      linear-gradient(
        180deg,
        rgba(3, 4, 7, 0.55) 0%,
        rgba(3, 4, 7, 0.08) 26%,
        rgba(3, 4, 7, 0.16) 58%,
        rgba(3, 4, 7, 0.86) 100%
      ),
      linear-gradient(90deg, rgba(3, 4, 7, 0.72) 0%, transparent 46%),
      radial-gradient(
        105% 85% at 50% 42%,
        transparent 30%,
        rgba(3, 4, 7, 0.34) 72%,
        rgba(3, 4, 7, 0.82) 100%
      );
  }

  .clock {
    position: absolute;
    top: clamp(28px, 4.6vh, 54px);
    right: clamp(32px, 4.4vw, 68px);
    z-index: 4;
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 2px;
    text-align: right;
    color: rgba(255, 255, 255, 0.92);
    text-shadow: 0 2px 28px rgba(0, 0, 0, 0.55);
    animation: soft-in 900ms var(--ease-out) 300ms both;
  }

  .time {
    font-family: var(--font-display);
    font-size: clamp(34px, 4vw, 58px);
    font-weight: 300;
    line-height: 1;
    letter-spacing: -0.04em;
    font-variant-numeric: tabular-nums;
  }

  .date {
    font-size: clamp(10px, 1vw, 12px);
    font-weight: 550;
    letter-spacing: 0.16em;
    text-transform: uppercase;
    color: rgba(255, 255, 255, 0.55);
  }

  .meta {
    position: absolute;
    left: clamp(36px, 5.4vw, 88px);
    bottom: clamp(36px, 7vh, 82px);
    z-index: 4;
    max-width: min(58vw, 780px);
    pointer-events: none;
  }

  .meta-inner {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 12px;
    animation: meta-cycle var(--slide-ms) linear both;
  }

  .meta h1 {
    margin: 0;
    font-family: var(--font-display);
    font-size: clamp(30px, 4.2vw, 62px);
    font-weight: 600;
    line-height: 1.02;
    letter-spacing: -0.038em;
    color: rgba(255, 255, 255, 0.97);
    text-shadow: 0 4px 44px rgba(0, 0, 0, 0.58);
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  .when {
    margin: 0;
    font-size: 11.5px;
    font-weight: 550;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: rgba(255, 255, 255, 0.46);
  }

  @keyframes meta-cycle {
    0% {
      opacity: 0;
      transform: translateY(10px);
    }
    5% {
      opacity: 1;
      transform: translateY(0);
    }
    88% {
      opacity: 1;
      transform: translateY(0);
    }
    100% {
      opacity: 0;
      transform: translateY(-4px);
    }
  }

  @keyframes soft-in {
    from {
      opacity: 0;
    }
    to {
      opacity: 1;
    }
  }

  /* Blur/hidden: stop the drift, keep timers stopped by JS. The fade-in is
     allowed to finish so a paused layer is never stuck invisible. */
  .paused .layer {
    animation-play-state: running, paused;
  }
  .paused .meta-inner,
  .paused .clock {
    animation-play-state: paused;
  }

  /* Reduced motion: no pan/zoom at all, only the slow opacity crossfade. */
  .reduced .layer,
  .reduced .layer.drift-b {
    animation: layer-in var(--fade-ms) linear both;
    transform: scale(1.03);
  }
  .reduced .meta-inner {
    animation: none;
    opacity: 1;
  }

  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    padding: 0;
    margin: -1px;
    overflow: hidden;
    clip: rect(0, 0, 0, 0);
    white-space: nowrap;
    border: 0;
  }

  @media (max-width: 720px) {
    .meta {
      max-width: 78vw;
    }
    .clock .date {
      display: none;
    }
  }
</style>
