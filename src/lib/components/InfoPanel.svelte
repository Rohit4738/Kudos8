<!--
  PS5-style lower info panel. The selected tile crossfades into this panel
  without a click - controller highlight or mouse hover both update it.
  OWNER: Session D.
-->
<script lang="ts">
  import { fly } from "svelte/transition";
  import { formatBytes, playState, shortPlayState } from "../format";
  import type { Game } from "../types";
  import Icon from "./Icon.svelte";
  import PlatformBadge from "./PlatformBadge.svelte";

  interface Props {
    game: Game | null;
    index: number;
    total: number;
    onOpen: (game: Game) => void;
  }

  let { game, index, total, onOpen }: Props = $props();
</script>

<section class="info" aria-live="polite">
  {#if game}
    {#key game.id}
      <div
        class="inner"
        in:fly={{ y: 14, duration: 320, opacity: 0 }}
      >
        <div class="meta">
          <div class="badges">
            <PlatformBadge platform={game.platform} />
            <span class="state" class:ready={game.isInstalled}>
              <span class="state-dot" aria-hidden="true"></span>
              {playState(game)}
            </span>
          </div>

          <h2>{game.title}</h2>

          <div class="sub">
            <span>{formatBytes(game.sizeBytes)}</span>
            <span class="sep" aria-hidden="true">/</span>
            <span>{game.isInstalled ? "Ready to launch" : "Available in your store"}</span>
          </div>
        </div>

        <div class="side">
          <span class="counter" aria-hidden="true">
            {String(index + 1).padStart(2, "0")}
            <span>/ {String(total).padStart(2, "0")}</span>
          </span>

          <button type="button" class="cta" onclick={() => onOpen(game)}>
            <span>Open game</span>
            <kbd>Enter</kbd>
            <Icon name="arrow-right" size={16} />
          </button>
        </div>
      </div>
    {/key}
  {:else}
    <div class="idle">
      <p>Select a game to see its details</p>
      <span class="hintline">
        <Icon name="gamepad" size={15} />
        D-pad to browse
      </span>
    </div>
  {/if}
</section>

<style>
  .info {
    min-height: 132px;
    display: flex;
    align-items: flex-end;
    padding-top: 18px;
    border-top: 1px solid rgba(255, 255, 255, 0.07);
    position: relative;
    z-index: 4;
  }

  .inner {
    width: 100%;
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: 28px;
  }

  .meta {
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .badges {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
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
    color: var(--fg-2);
    background: rgba(255, 255, 255, 0.035);
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.05);
    white-space: nowrap;
  }

  .state.ready {
    color: rgba(210, 255, 235, 0.9);
  }

  .state-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--fg-2);
    box-shadow: 0 0 10px transparent;
  }

  .state.ready .state-dot {
    background: var(--ok);
    box-shadow: 0 0 10px rgba(61, 220, 151, 0.5);
  }

  h2 {
    margin: 0;
    font-family: var(--font-display);
    font-size: clamp(26px, 3.5vh, 42px);
    line-height: 1.04;
    font-weight: 600;
    letter-spacing: -0.032em;
    color: var(--fg-0);
    max-width: 22ch;
    overflow-wrap: anywhere;
  }

  .sub {
    display: flex;
    align-items: center;
    gap: 9px;
    color: var(--fg-2);
    font-size: 12.5px;
    font-weight: 450;
  }

  .sep {
    opacity: 0.4;
  }

  .side {
    display: flex;
    align-items: center;
    gap: 22px;
    flex: 0 0 auto;
  }

  .counter {
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.08em;
    color: var(--fg-1);
    font-variant-numeric: tabular-nums;
  }

  .counter span {
    color: var(--fg-2);
    font-weight: 450;
  }

  .cta {
    display: inline-flex;
    align-items: center;
    gap: 10px;
    height: 44px;
    padding: 0 18px;
    border-radius: 999px;
    font-size: 13px;
    font-weight: 600;
    color: #06080e;
    background: #f4f7ff;
    box-shadow:
      0 10px 30px rgba(0, 0, 0, 0.35),
      inset 0 0 0 1px rgba(255, 255, 255, 0.5);
    transition: transform var(--dur-fast) var(--ease-out);
    will-change: transform;
  }

  .cta:hover {
    transform: translateY(-2px) scale(1.02);
  }

  .cta:active {
    transform: scale(0.98);
  }

  .cta kbd {
    font-family: var(--font-ui);
    font-size: 9.5px;
    font-weight: 650;
    letter-spacing: 0.04em;
    color: rgba(6, 8, 14, 0.62);
    background: rgba(6, 8, 14, 0.08);
    border-radius: 5px;
    padding: 2px 5px;
  }

  .idle {
    width: 100%;
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 18px;
    color: var(--fg-2);
  }

  .idle p {
    margin: 0;
    font-size: clamp(19px, 2.2vh, 24px);
    font-weight: 500;
    letter-spacing: -0.01em;
    color: var(--fg-1);
  }

  .hintline {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    font-size: 11px;
    color: var(--fg-2);
  }

  @media (max-width: 720px) {
    .inner {
      align-items: flex-start;
      flex-direction: column;
      gap: 16px;
    }
    .side {
      width: 100%;
      justify-content: space-between;
    }
    .info {
      min-height: 180px;
    }
  }
</style>
