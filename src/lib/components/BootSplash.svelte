<!--
  Short startup vignette. It never blocks the app (pages render behind it),
  removes itself at 600 ms and any input skips it. OWNER: Session I.
-->
<script lang="ts">
  import { onMount } from "svelte";

  let visible = $state(true);
  let leaving = $state(false);

  onMount(() => {
    const reduced =
      typeof window !== "undefined" &&
      window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    const fadeAt = reduced ? 0 : 330;
    const removeAt = reduced ? 80 : 600;
    let removeTimer: ReturnType<typeof setTimeout> | undefined;

    const fadeTimer = setTimeout(() => (leaving = true), fadeAt);
    removeTimer = setTimeout(() => (visible = false), removeAt);

    function skip(): void {
      visible = false;
    }

    window.addEventListener("pointerdown", skip, { capture: true, once: true });
    window.addEventListener("keydown", skip, { capture: true, once: true });
    window.addEventListener("wheel", skip, { capture: true, once: true });

    return () => {
      clearTimeout(fadeTimer);
      if (removeTimer) clearTimeout(removeTimer);
      window.removeEventListener("pointerdown", skip, true);
      window.removeEventListener("keydown", skip, true);
      window.removeEventListener("wheel", skip, true);
    };
  });
</script>

{#if visible}
  <div class="boot" class:leaving aria-hidden="true">
    <div class="core">
      <span class="mark">K</span>
      <span class="word">Kudos</span>
      <span class="bar"><span class="fill"></span></span>
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
    background: #04050a;
    opacity: 1;
    transition: opacity 260ms var(--ease-out);
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
    gap: 14px;
    animation: boot-in 420ms var(--ease-out) both;
    will-change: transform, opacity;
  }

  @keyframes boot-in {
    from {
      opacity: 0;
      transform: translateY(10px) scale(0.97);
    }
    to {
      opacity: 1;
      transform: none;
    }
  }

  .mark {
    display: grid;
    place-items: center;
    width: 64px;
    height: 64px;
    border-radius: 20px;
    font-size: 27px;
    font-weight: 800;
    letter-spacing: -0.06em;
    color: #fff;
    background: linear-gradient(145deg, var(--accent) 0%, #7b4cff 100%);
    box-shadow:
      0 18px 50px var(--accent-glow),
      inset 0 0 0 1px rgba(255, 255, 255, 0.18);
  }

  .word {
    font-family: var(--font-display);
    font-size: 15px;
    font-weight: 600;
    letter-spacing: 0.32em;
    text-transform: uppercase;
    color: rgba(244, 247, 255, 0.86);
    padding-left: 0.32em;
  }

  .bar {
    display: block;
    width: 92px;
    height: 2px;
    border-radius: 999px;
    overflow: hidden;
    background: rgba(255, 255, 255, 0.1);
  }

  .fill {
    display: block;
    width: 100%;
    height: 100%;
    border-radius: inherit;
    background: linear-gradient(90deg, var(--accent), #b58cff);
    transform-origin: left center;
    animation: boot-fill 480ms var(--ease-out) both;
    will-change: transform;
  }

  @keyframes boot-fill {
    from {
      transform: scaleX(0);
    }
    to {
      transform: scaleX(1);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .core,
    .fill {
      animation: none;
    }
    .boot {
      transition: none;
    }
  }
</style>
