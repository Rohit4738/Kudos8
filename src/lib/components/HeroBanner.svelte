<!--
  PS5-style hero banner: fixed-height landscape art for the focused game.
  Art changes crossfade; each new layer settles with a transform-only parallax.
  OWNER: Session I.
-->
<script lang="ts">
  import { fade } from "svelte/transition";
  import { heroCandidates, placeholderBackground } from "../art";
  import { platformShort } from "../format";
  import {
    isSteamOwnedGame,
    playtimeLabel,
    primaryActionLabel,
    type SteamPlaytime,
  } from "../steam";
  import type { Game } from "../types";
  import Icon from "./Icon.svelte";
  import PlatformBadge from "./PlatformBadge.svelte";

  interface Props {
    game: Game | null;
    meta?: string;
    /** Row-derived kicker, e.g. "Favorite" or "Your library". */
    kicker?: string;
    playtime?: SteamPlaytime | null;
    favorite?: boolean;
    busy?: boolean;
    onPrimary?: (game: Game) => void;
    onDetails?: (game: Game) => void;
    onFavorite?: (game: Game) => void;
  }

  let {
    game,
    meta = "",
    kicker = "",
    playtime = null,
    favorite = false,
    busy = false,
    onPrimary,
    onDetails,
    onFavorite,
  }: Props = $props();

  let attempt = $state(0);
  let ready = $state(false);
  let failed = $state(false);
  let lastId = $state("");

  const candidates = $derived(game ? heroCandidates(game) : []);
  const src = $derived(candidates[attempt] ?? null);
  const showImage = $derived(Boolean(src) && !failed);
  const action = $derived(game ? primaryActionLabel(game) : "Play");
  const installable = $derived(
    game ? game.isInstalled || isSteamOwnedGame(game) : false,
  );
  const hours = $derived(playtimeLabel(playtime?.minutes));
  const defaultKicker = $derived(
    game ? (game.isInstalled ? "Continue playing" : "In your Steam account") : "Kudos",
  );

  $effect(() => {
    const id = game?.id ?? "";
    if (id === lastId) return;
    lastId = id;
    attempt = 0;
    ready = false;
    failed = false;
  });

  function onError(): void {
    if (attempt < candidates.length - 1) {
      attempt += 1;
      ready = false;
    } else {
      failed = true;
      ready = false;
    }
  }
</script>

{#if game}
  <section class="hero" aria-label={`Featured game: ${game.title}`}>
    <div class="glow" style:background={placeholderBackground(game)} aria-hidden="true"></div>

    <div class="art" aria-hidden="true">
      {#key game.id}
        <div class="layer" style:background={placeholderBackground(game)} in:fade={{ duration: 260 }} out:fade={{ duration: 220 }}>
          {#if showImage}
            <img
              {src}
              class:ready
              alt=""
              draggable="false"
              decoding="async"
              loading="eager"
              onload={() => (ready = true)}
              onerror={onError}
            />
          {/if}
        </div>
      {/key}
    </div>

    <div class="shade" aria-hidden="true"></div>

    <div class="content">
      <div class="lede">
        <p class="eyebrow">{kicker || defaultKicker}</p>
        <h1 title={game.title}>{game.title}</h1>

        <div class="tags">
          <PlatformBadge platform={game.platform} />
          <span class="state" class:ready={game.isInstalled}>
            <span class="dot" aria-hidden="true"></span>
            {game.isInstalled ? "Installed" : "Not installed"}
          </span>
          {#if hours}
            <span class="playtime">{hours}</span>
          {/if}
        </div>

        {#if meta}
          <p class="meta">{meta}</p>
        {/if}

        <div class="actions">
          <button
            type="button"
            class="primary"
            disabled={!installable || busy}
            onclick={() => (game ? onPrimary?.(game) : undefined)}
          >
            {#if busy}
              <span class="pip busy" aria-hidden="true"></span>
              {game.isInstalled ? "Launching…" : "Opening Steam…"}
            {:else}
              <span class="play-icon" aria-hidden="true">
                <Icon name={game.isInstalled ? "play" : "download"} size={17} />
              </span>
              {action}
            {/if}
          </button>

          <button type="button" class="secondary" onclick={() => (game ? onDetails?.(game) : undefined)}>
            Details
            <Icon name="arrow-right" size={15} />
          </button>

          {#if onFavorite}
            <button
              type="button"
              class="favorite"
              class:on={favorite}
              aria-pressed={favorite}
              aria-label={favorite ? `Remove ${game.title} from favorites` : `Add ${game.title} to favorites`}
              title={favorite ? "Remove from favorites" : "Add to favorites"}
              onclick={() => (game ? onFavorite?.(game) : undefined)}
            >
              <Icon name={favorite ? "star-fill" : "star"} size={18} />
            </button>
          {/if}
        </div>
      </div>

      <div class="side" aria-hidden="true">
        <span class="store">{platformShort(game.platform)}</span>
        {#if hours}<span class="hours">{hours}</span>{/if}
      </div>
    </div>
  </section>
{/if}

<style>
  .hero {
    position: relative;
    height: clamp(310px, 39vh, 430px);
    border-radius: clamp(20px, 2vw, 30px);
    overflow: hidden;
    isolation: isolate;
    box-shadow:
      0 30px 80px rgba(0, 0, 0, 0.5),
      inset 0 0 0 1px rgba(255, 255, 255, 0.08);
    background: var(--bg-2);
  }

  .glow {
    position: absolute;
    inset: -20%;
    z-index: 0;
    filter: blur(60px);
    opacity: 0.5;
    transform: scale(1.05);
    pointer-events: none;
  }

  .art {
    position: absolute;
    inset: 0;
    z-index: 1;
  }

  .layer {
    position: absolute;
    inset: -3%;
    background-size: cover;
    background-position: center;
  }

  .layer img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    filter: saturate(1.08) brightness(0.86);
    animation: hero-settle 680ms var(--ease-out) both;
    will-change: transform, opacity;
    opacity: 0;
    transition: opacity 380ms var(--ease-out);
  }

  .layer img.ready {
    opacity: 1;
  }

  @keyframes hero-settle {
    from {
      transform: translate3d(1.6%, 1.2%, 0) scale(1.07);
    }
    to {
      transform: translate3d(0, 0, 0) scale(1.015);
    }
  }

  .shade {
    position: absolute;
    inset: 0;
    z-index: 2;
    pointer-events: none;
    background:
      linear-gradient(
        96deg,
        rgba(4, 5, 9, 0.94) 0%,
        rgba(4, 5, 9, 0.72) 34%,
        rgba(4, 5, 9, 0.16) 68%,
        rgba(4, 5, 9, 0.4) 100%
      ),
      linear-gradient(180deg, rgba(4, 5, 9, 0.14) 0%, transparent 42%, rgba(4, 5, 9, 0.84) 100%);
  }

  .content {
    position: absolute;
    inset: auto 0 0;
    z-index: 3;
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: 32px;
    padding: clamp(24px, 3.4vw, 44px);
  }

  .lede {
    min-width: 0;
    max-width: min(760px, 68%);
  }

  .eyebrow {
    margin: 0 0 10px;
    font-size: 10px;
    font-weight: 650;
    letter-spacing: 0.22em;
    text-transform: uppercase;
    color: var(--accent);
    text-shadow: 0 2px 16px rgba(0, 0, 0, 0.5);
  }

  h1 {
    margin: 0;
    font-family: var(--font-display);
    font-size: clamp(34px, 5.2vh, 64px);
    line-height: 0.96;
    font-weight: 620;
    letter-spacing: -0.045em;
    color: #fff;
    text-shadow: 0 4px 34px rgba(0, 0, 0, 0.48);
    display: -webkit-box;
    line-clamp: 2;
    -webkit-line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    text-wrap: balance;
  }

  .tags {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
    margin-top: 14px;
  }

  .state,
  .playtime {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    padding: 5px 10px;
    border-radius: 999px;
    font-size: 10.5px;
    font-weight: 550;
    letter-spacing: 0.045em;
    color: var(--fg-1);
    background: rgba(7, 10, 17, 0.42);
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.12);
    backdrop-filter: blur(12px);
    white-space: nowrap;
  }

  .playtime {
    color: rgba(255, 255, 255, 0.9);
    font-variant-numeric: tabular-nums;
  }

  .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--fg-2);
  }

  .state.ready .dot {
    background: var(--ok);
    box-shadow: 0 0 10px rgba(61, 220, 151, 0.55);
  }

  .meta {
    margin: 10px 0 0;
    font-size: 12.5px;
    line-height: 1.5;
    color: rgba(231, 238, 255, 0.74);
    text-shadow: 0 1px 12px rgba(0, 0, 0, 0.5);
    overflow-wrap: anywhere;
  }

  .actions {
    display: flex;
    align-items: center;
    gap: 11px;
    flex-wrap: wrap;
    margin-top: 22px;
  }

  .primary {
    display: inline-flex;
    align-items: center;
    gap: 11px;
    height: 52px;
    padding: 0 26px;
    border-radius: 999px;
    font-size: 14.5px;
    font-weight: 650;
    letter-spacing: -0.005em;
    color: #05070c;
    background: #f7f9ff;
    box-shadow:
      0 16px 40px rgba(0, 0, 0, 0.42),
      inset 0 0 0 1px rgba(255, 255, 255, 0.55);
    transition:
      transform 240ms var(--ease-out),
      opacity 200ms var(--ease-out);
    will-change: transform;
  }

  .primary:hover:not(:disabled) {
    transform: translateY(-2px) scale(1.025);
  }

  .primary:active:not(:disabled) {
    transform: scale(0.985);
  }

  .primary:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .play-icon {
    display: grid;
    place-items: center;
    width: 27px;
    height: 27px;
    border-radius: 50%;
    color: #fff;
    background: #05070c;
  }

  .secondary {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    height: 46px;
    padding: 0 18px;
    border-radius: 999px;
    font-size: 12.5px;
    font-weight: 600;
    color: var(--fg-0);
    background: rgba(8, 12, 20, 0.4);
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.16);
    backdrop-filter: blur(16px);
    transition: transform 220ms var(--ease-out);
    will-change: transform;
  }

  .secondary:hover {
    transform: translateY(-2px);
  }

  .favorite {
    display: grid;
    place-items: center;
    width: 46px;
    height: 46px;
    border-radius: 50%;
    color: var(--fg-1);
    background: rgba(8, 12, 20, 0.4);
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.16);
    backdrop-filter: blur(16px);
    transition:
      transform 220ms var(--ease-out),
      color 200ms var(--ease-out);
  }

  .favorite:hover {
    transform: translateY(-2px);
    color: #fff;
  }

  .favorite.on {
    color: #ffd166;
    box-shadow:
      inset 0 0 0 1px rgba(255, 209, 102, 0.42),
      0 0 24px rgba(255, 209, 102, 0.16);
  }

  .pip.busy {
    width: 16px;
    height: 16px;
    border-radius: 50%;
    border: 2px solid rgba(5, 7, 12, 0.25);
    border-top-color: #05070c;
    animation: pip-spin 720ms linear infinite;
  }

  @keyframes pip-spin {
    to {
      transform: rotate(1turn);
    }
  }

  .side {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 6px;
    flex: 0 0 auto;
    padding-bottom: 4px;
    opacity: 0.72;
  }

  .store {
    font-size: 9.5px;
    font-weight: 650;
    letter-spacing: 0.24em;
    text-transform: uppercase;
    color: rgba(255, 255, 255, 0.72);
  }

  .hours {
    font-family: var(--font-display);
    font-size: clamp(26px, 3.4vh, 40px);
    font-weight: 600;
    letter-spacing: -0.04em;
    color: rgba(255, 255, 255, 0.92);
    font-variant-numeric: tabular-nums;
  }

  @media (max-width: 980px) {
    .hero {
      height: clamp(320px, 44vh, 430px);
    }
    .lede {
      max-width: 100%;
    }
    .side {
      display: none;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .layer img {
      animation: none;
      transform: none;
    }
  }
</style>
