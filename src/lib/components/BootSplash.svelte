<!--
  Kudos boot vignette. The Home shell is mounted and painted underneath this
  fixed overlay, so skipping is free and never waits on the animation.
  OWNER: Session N.

  Timeline (normal motion, ~1.14s):
    0-150ms    black hold
    150-490ms  mark fades in, scale 0.92 -> 1; accent bloom blooms
    470-730ms  thin accent rule sweeps under the mark
    600-1040ms KUDOS wordmark reveals with tracking ease
    920-1140ms overlay fades to Home

  Any pointer/key/wheel/gamepad input unmounts the overlay immediately.
  prefers-reduced-motion gets a plain 200ms fade with no scale or sweep.
  `?boot=1` forces the splash even after the module has already booted once.
-->
<script module lang="ts">
  let booted = false;
</script>

<script lang="ts">
  import { onMount } from "svelte";
  import { onGamepad } from "../gamepad";
  import BrandMark from "./BrandMark.svelte";

  const params =
    typeof window !== "undefined"
      ? new URLSearchParams(window.location.search)
      : null;
  const forceBoot = params?.get("boot") === "1";
  const shouldShow = forceBoot || !booted;
  booted = true;

  const reducedMotion =
    typeof window !== "undefined" &&
    window.matchMedia("(prefers-reduced-motion: reduce)").matches;

  let visible = $state(shouldShow);
  let leaving = $state(false);

  onMount(() => {
    if (!visible) return;

    let fadeTimer: ReturnType<typeof setTimeout> | undefined;
    let removeTimer: ReturnType<typeof setTimeout> | undefined;

    function clearTimers(): void {
      if (fadeTimer) clearTimeout(fadeTimer);
      if (removeTimer) clearTimeout(removeTimer);
      fadeTimer = undefined;
      removeTimer = undefined;
    }

    function skip(event?: Event): void {
      // Swallow the first input so it only dismisses the splash; the next one
      // reaches the Home shell that is already mounted underneath.
      event?.stopPropagation();
      clearTimers();
      visible = false;
    }

    if (reducedMotion) {
      fadeTimer = setTimeout(() => (leaving = true), 30);
      removeTimer = setTimeout(() => (visible = false), 250);
    } else {
      fadeTimer = setTimeout(() => (leaving = true), 920);
      removeTimer = setTimeout(() => (visible = false), 1140);
    }

    const stopGamepad = onGamepad(() => skip());
    const passive = { capture: true, passive: true } as const;

    window.addEventListener("pointerdown", skip, passive);
    window.addEventListener("keydown", skip, { capture: true });
    window.addEventListener("wheel", skip, passive);

    return () => {
      clearTimers();
      stopGamepad();
      window.removeEventListener("pointerdown", skip, true);
      window.removeEventListener("keydown", skip, true);
      window.removeEventListener("wheel", skip, true);
    };
  });
</script>

{#if visible}
  <div
    class="boot"
    class:leaving
    class:reduced={reducedMotion}
    aria-hidden="true"
  >
    <div class="core">
      <div class="mark-wrap">
        <span class="bloom"></span>
        <div class="mark"><BrandMark size={104} glow /></div>
      </div>
      <div class="rule"><span class="rule-fill"></span></div>
      <div class="wordmark">
        <BrandMark
          variant="wordmark"
          size={190}
          reveal={!reducedMotion}
          delay={600}
        />
      </div>
    </div>
  </div>
{/if}

<style>
  .boot {
    position: fixed;
    inset: 0;
    z-index: 200;
    display: grid;
    place-items: center;
    background: var(--boot-bg, #04050a);
    opacity: 1;
    transition: opacity 220ms var(--ease-out);
    will-change: opacity;
  }

  .boot.leaving {
    opacity: 0;
    pointer-events: none;
  }

  .core {
    display: flex;
    flex-direction: column;
    align-items: center;
    transform: translateY(-6px);
  }

  .mark-wrap {
    position: relative;
    display: grid;
    place-items: center;
    width: 104px;
    height: 104px;
  }

  .bloom {
    position: absolute;
    inset: -82px;
    border-radius: 50%;
    background: radial-gradient(
      circle at 50% 50%,
      rgba(var(--accent-rgb), 0.32) 0%,
      rgba(var(--accent-rgb), 0.12) 38%,
      rgba(var(--accent-rgb), 0) 70%
    );
    opacity: 0;
    transform: scale(0.82);
    animation: bloom-in 520ms var(--ease-out) 150ms both;
    pointer-events: none;
    will-change: transform, opacity;
  }

  .mark {
    position: relative;
    z-index: 1;
    opacity: 0;
    transform: scale(0.92);
    animation: mark-in 340ms var(--ease-out) 150ms both;
    will-change: transform, opacity;
  }

  .rule {
    width: 132px;
    height: 2px;
    margin-top: 22px;
    border-radius: 999px;
    overflow: hidden;
    background: rgba(255, 255, 255, 0.08);
    opacity: 0;
    animation: rule-track-in 1ms linear 470ms both;
  }

  @keyframes rule-track-in {
    from {
      opacity: 0;
    }
    to {
      opacity: 1;
    }
  }

  .rule-fill {
    display: block;
    width: 100%;
    height: 100%;
    border-radius: inherit;
    background: linear-gradient(90deg, var(--accent), var(--brand-violet, #8b5cf6));
    box-shadow: 0 0 14px rgba(var(--accent-rgb), 0.55);
    transform: scaleX(0);
    transform-origin: left center;
    animation: rule-sweep 260ms var(--ease-out) 470ms both;
    will-change: transform;
  }

  .wordmark {
    display: flex;
    justify-content: center;
    margin-top: 18px;
  }

  @keyframes mark-in {
    from {
      opacity: 0;
      transform: scale(0.92);
    }
    to {
      opacity: 1;
      transform: scale(1);
    }
  }

  @keyframes bloom-in {
    0% {
      opacity: 0;
      transform: scale(0.82);
    }
    55% {
      opacity: 1;
      transform: scale(1.08);
    }
    100% {
      opacity: 0.55;
      transform: scale(1.14);
    }
  }

  @keyframes rule-sweep {
    from {
      transform: scaleX(0);
    }
    to {
      transform: scaleX(1);
    }
  }

  .boot.reduced {
    transition-duration: 200ms !important;
  }

  .boot.reduced .mark,
  .boot.reduced .bloom,
  .boot.reduced .rule,
  .boot.reduced .rule-fill {
    animation: none !important;
  }

  .boot.reduced .rule {
    opacity: 1;
  }

  .boot.reduced .mark {
    opacity: 1;
    transform: none;
  }

  .boot.reduced .bloom {
    opacity: 0.55;
    transform: none;
  }

  .boot.reduced .rule-fill {
    transform: scaleX(1);
  }

  @media (prefers-reduced-motion: reduce) {
    .boot {
      transition-duration: 200ms !important;
    }

    .mark,
    .bloom,
    .rule,
    .rule-fill {
      animation: none !important;
    }

    .mark {
      opacity: 1;
      transform: none;
    }

    .bloom {
      opacity: 0.55;
      transform: none;
    }

    .rule {
      opacity: 1;
    }

    .rule-fill {
      transform: scaleX(1);
    }
  }
</style>
