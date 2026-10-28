<!--
  Cover art with a premium gradient placeholder for missing/broken covers.
  The image fades in with opacity only; no layout work on load.
  OWNER: Session D.
-->
<script lang="ts">
  import { coverSource, initials, placeholderBackground } from "../art";
  import { platformShort } from "../format";
  import type { Game } from "../types";

  interface Props {
    game: Game;
    eager?: boolean;
  }

  let { game, eager = false }: Props = $props();

  let ready = $state(false);
  let failed = $state(false);
  let attempt = $state(0);

  /**
   * Candidate chain, best first:
   *  1. the backend's cover (local Steam cache / GOG.ico / CDN URL)
   *  2. Steam's CDN 600x900 portrait art, derived from the stable id
   *  3. Steam's wider header art (cropped by object-fit)
   * Anything that fails all three falls back to the gradient-initial art.
   */
  const candidates = $derived.by(() => {
    const list: string[] = [];
    const primary = coverSource(game.coverImage);
    if (primary) list.push(primary);

    if (game.platform === "steam") {
      const appid = game.id.startsWith("steam:") ? game.id.slice(6) : "";
      if (/^\d+$/.test(appid)) {
        list.push(
          `https://cdn.cloudflare.steamstatic.com/steam/apps/${appid}/library_600x900.jpg`,
          `https://cdn.cloudflare.steamstatic.com/steam/apps/${appid}/header.jpg`,
        );
      }
    }

    return [...new Set(list)];
  });

  const src = $derived(candidates[attempt] ?? null);
  const showImage = $derived(Boolean(src) && !failed);

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

<div
  class="art"
  class:has-image={showImage}
  style:background={showImage ? undefined : placeholderBackground(game)}
>
  {#if showImage}
    <img
      {src}
      alt=""
      class:ready
      loading={eager ? "eager" : "lazy"}
      decoding="async"
      draggable="false"
      onload={() => (ready = true)}
      onerror={onError}
    />
  {/if}

  {#if !showImage || !ready}
    <div class="placeholder" aria-hidden="true">
      <span class="halo"></span>
      <span class="initials">{initials(game.title)}</span>
      <span class="store">{platformShort(game.platform)}</span>
    </div>
  {/if}

  <span class="vignette" aria-hidden="true"></span>
</div>

<style>
  .art {
    position: relative;
    width: 100%;
    height: 100%;
    overflow: hidden;
    background-color: var(--bg-2);
    isolation: isolate;
  }

  img {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
    opacity: 0;
    transition: opacity 420ms var(--ease-out);
    will-change: opacity;
  }

  img.ready {
    opacity: 1;
  }

  .placeholder {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    overflow: hidden;
  }

  .halo {
    position: absolute;
    width: 86%;
    aspect-ratio: 1;
    border-radius: 50%;
    background: radial-gradient(
      circle,
      rgba(255, 255, 255, 0.13) 0%,
      rgba(255, 255, 255, 0.02) 48%,
      transparent 68%
    );
    transform: translateY(-6%);
  }

  .initials {
    position: relative;
    font-family: var(--font-display);
    font-size: clamp(38px, 4.4vw, 86px);
    font-weight: 650;
    letter-spacing: -0.06em;
    line-height: 1;
    color: rgba(255, 255, 255, 0.9);
    text-shadow:
      0 2px 24px rgba(0, 0, 0, 0.35),
      0 0 60px rgba(255, 255, 255, 0.12);
    transform: translateY(-4%);
  }

  .store {
    position: absolute;
    bottom: 16px;
    left: 0;
    right: 0;
    text-align: center;
    font-size: 9px;
    font-weight: 650;
    letter-spacing: 0.28em;
    text-transform: uppercase;
    color: rgba(255, 255, 255, 0.44);
  }

  .vignette {
    position: absolute;
    inset: 0;
    background:
      linear-gradient(180deg, transparent 55%, rgba(4, 6, 12, 0.5) 100%),
      radial-gradient(120% 80% at 50% 0%, transparent 60%, rgba(0, 0, 0, 0.18) 100%);
    pointer-events: none;
  }
</style>
