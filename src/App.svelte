<!--
  App shell. OWNER: Session A (structure + rail).
  Session D owns the pages in src/pages and may add components under src/lib/components.
-->
<script lang="ts">
  import { onMount } from "svelte";
  import { fade } from "svelte/transition";
  import { startController } from "./lib/controller.svelte";
  import { onGamepad } from "./lib/gamepad";
  import { router, navigate, back } from "./lib/router.svelte";
  import { openSearch, searchState } from "./lib/search.svelte";
  import {
    applyStoredAccent,
    applyStoredBackground,
    applyStoredDensity,
  } from "./lib/theme";
  import {
    enterRail,
    exitRail,
    moveRail,
    shellFocus,
  } from "./lib/shellFocus.svelte";
  import { readStartupPage } from "./lib/windowPrefs";
  import Home from "./pages/Home.svelte";
  import GameDetail from "./pages/GameDetail.svelte";
  import Settings from "./pages/Settings.svelte";
  import BootSplash from "./lib/components/BootSplash.svelte";
  import BrandMark from "./lib/components/BrandMark.svelte";
  import Icon from "./lib/components/Icon.svelte";
  import IdleHost from "./lib/components/IdleHost.svelte";
  import SearchOverlay from "./lib/components/SearchOverlay.svelte";

  const pageKey = $derived(
    router.route.name === "game" ? `game:${router.route.id}` : router.route.name,
  );

  function onKeydown(event: KeyboardEvent) {
    if (shellFocus.zone === "rail") {
      switch (event.key) {
        case "ArrowUp":
          event.preventDefault();
          moveRail(-1);
          return;
        case "ArrowDown":
          event.preventDefault();
          moveRail(1);
          return;
        case "Enter": {
          event.preventDefault();
          const run = railActions[shellFocus.index];
          exitRail();
          run?.();
          return;
        }
        case "Escape":
        case "ArrowRight":
          event.preventDefault();
          exitRail();
          return;
      }
    }

    if (event.key === "Escape" && router.route.name !== "home") {
      event.preventDefault();
      back();
    }
  }

  /** Rail actions, in visual order: Library, Search, Settings. */
  const railActions: (() => void)[] = [
    () => navigate({ name: "home" }),
    openSearch,
    () => navigate({ name: "settings" }),
  ];

  function railIndexForRoute(): number {
    return router.route.name === "settings" ? 2 : 0;
  }

  /**
   * Controller shell handling. The rail owns Up/Down/Confirm while it is
   * focused; pages own the "page" zone. Start (menu) opens the rail from
   * anywhere, Back/Left/Right closes it.
   */
  function handleShellPad(action: string): void {
    if (searchState.open) return;

    if (action === "menu") {
      if (shellFocus.zone === "rail") exitRail();
      else enterRail(railIndexForRoute());
      return;
    }

    if (shellFocus.zone !== "rail") return;

    switch (action) {
      case "up":
        moveRail(-1);
        break;
      case "down":
        moveRail(1);
        break;
      case "confirm": {
        const run = railActions[shellFocus.index];
        exitRail();
        run?.();
        break;
      }
      case "back":
        exitRail();
        break;
    }
  }

  onMount(() => {
    applyStoredAccent();
    applyStoredBackground();
    applyStoredDensity();
    const stopController = startController();
    const stopGamepad = onGamepad(handleShellPad);

    // A direct jump (no history entry): startup should not create a fake "back".
    if (router.route.name === "home" && readStartupPage() === "settings") {
      router.route = { name: "settings" };
    }

    window.addEventListener("keydown", onKeydown);
    return () => {
      stopController();
      stopGamepad();
      window.removeEventListener("keydown", onKeydown);
    };
  });
</script>

<div class="shell">
  <div class="backdrop" aria-hidden="true"></div>
  <nav
    class="rail"
    class:rail-zone={shellFocus.zone === "rail"}
    aria-label="Primary"
  >
    <button
      class="brand"
      title="Kudos"
      aria-label="Kudos home"
      onclick={() => navigate({ name: "home" })}
    >
      <BrandMark size={46} glow />
    </button>

    <div class="rail-items">
      <button
        class="rail-item"
        class:focused={shellFocus.zone === "rail" && shellFocus.index === 0}
        class:active={router.route.name === "home" || router.route.name === "game"}
        title="Library"
        onclick={() => navigate({ name: "home" })}
      >
        <span class="glyph">▦</span>
        <span class="rail-label">Library</span>
      </button>

      <button
        class="rail-item"
        class:focused={shellFocus.zone === "rail" && shellFocus.index === 1}
        title="Search (Ctrl+K or /)"
        onclick={openSearch}
      >
        <span class="glyph"><Icon name="search" size={18} /></span>
        <span class="rail-label">Search</span>
      </button>

      <button
        class="rail-item"
        class:focused={shellFocus.zone === "rail" && shellFocus.index === 2}
        class:active={router.route.name === "settings"}
        title="Settings"
        onclick={() => navigate({ name: "settings" })}
      >
        <span class="glyph">⚙</span>
        <span class="rail-label">Settings</span>
      </button>
    </div>

    <div class="rail-foot">
      {#if shellFocus.zone === "rail"}
        <span class="rail-pad">A select · B close</span>
      {:else}
        <span>v0.2</span>
      {/if}
    </div>
  </nav>

  <main class="stage">
    {#key pageKey}
      <div class="page" transition:fade={{ duration: 180 }}>
        {#if router.route.name === "home"}
          <Home />
        {:else if router.route.name === "game"}
          <GameDetail id={router.route.id} />
        {:else}
          <Settings />
        {/if}
      </div>
    {/key}
  </main>
</div>

<IdleHost />
<SearchOverlay />
<BootSplash />

<style>
  .shell {
    position: relative;
    isolation: isolate;
    overflow: hidden;
    display: grid;
    grid-template-columns: var(--rail-w) 1fr;
    height: 100%;
  }

  .backdrop {
    position: absolute;
    inset: 0;
    z-index: 0;
    pointer-events: none;
    background:
      radial-gradient(1200px 700px at 78% -12%, rgba(var(--accent-rgb), 0.18), transparent 62%),
      radial-gradient(900px 600px at 4% 110%, rgba(var(--accent-2-rgb), 0.12), transparent 60%),
      linear-gradient(160deg, var(--bg-1) 0%, var(--bg-0) 100%);
  }

  :global(html[data-background="solid"]) .backdrop {
    background: var(--bg-0);
  }

  :global(html[data-background="gradient"]) .backdrop {
    inset: -18%;
    background:
      radial-gradient(42% 54% at 28% 30%, color-mix(in srgb, var(--accent) 30%, transparent), transparent 70%),
      radial-gradient(40% 48% at 76% 72%, rgba(var(--accent-2-rgb), 0.28), transparent 70%),
      linear-gradient(135deg, var(--bg-0) 0%, var(--bg-2) 52%, var(--bg-0) 100%);
    animation: backdrop-drift 26s ease-in-out infinite alternate;
    will-change: transform;
  }

  @keyframes backdrop-drift {
    from {
      transform: translate3d(-2.5%, -2%, 0) scale(1.04);
    }
    to {
      transform: translate3d(2.5%, 2%, 0) scale(1.12);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    :global(html[data-background="gradient"]) .backdrop {
      animation: none;
    }
  }

  .rail,
  .stage {
    position: relative;
    z-index: 1;
  }

  .rail {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 22px;
    padding: 22px 10px 16px;
    border-right: 1px solid rgba(255, 255, 255, 0.05);
    background: rgba(5, 7, 12, 0.55);
    backdrop-filter: blur(18px);
  }

  .brand {
    width: 46px;
    height: 46px;
    padding: 0;
    border-radius: 14px;
    display: grid;
    place-items: center;
    transition: transform var(--dur-fast) var(--ease-out);
  }
  .brand:hover {
    transform: translateY(-1px) scale(1.04);
  }

  .rail-items {
    display: flex;
    flex-direction: column;
    gap: 8px;
    width: 100%;
    margin-top: 8px;
  }

  .rail-item {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 5px;
    padding: 12px 4px;
    border-radius: var(--radius-md);
    color: var(--fg-2);
    transition:
      background var(--dur-fast) var(--ease-out),
      color var(--dur-fast) var(--ease-out);
  }
  .rail-item:hover {
    color: var(--fg-0);
    background: rgba(255, 255, 255, 0.05);
  }
  .rail-item.active {
    color: var(--fg-0);
    background: rgba(var(--accent-rgb), 0.16);
    box-shadow: inset 0 0 0 1px rgba(var(--accent-rgb), 0.28);
  }

  .rail-item.focused {
    color: var(--fg-0);
    background: rgba(var(--accent-rgb), 0.26);
    box-shadow:
      inset 0 0 0 2px var(--accent),
      0 0 22px rgba(var(--accent-rgb), 0.28);
    transform: translateX(2px);
  }

  .rail-item.focused .rail-label {
    opacity: 1;
  }

  .rail.rail-zone {
    border-right-color: var(--line-accent);
    background: color-mix(in srgb, var(--bg-0) 72%, transparent);
  }

  .rail-pad {
    font-size: 9px;
    letter-spacing: 0.02em;
    color: var(--accent);
    text-align: center;
    line-height: 1.5;
  }

  .glyph {
    display: grid;
    place-items: center;
    height: 19px;
    font-size: 19px;
    line-height: 1;
  }

  .rail-label {
    font-size: 10px;
    letter-spacing: 0.02em;
    color: inherit;
    opacity: 0.8;
  }

  .rail-foot {
    margin-top: auto;
    font-size: 10px;
    color: var(--fg-2);
  }

  .stage {
    min-width: 0;
    overflow: hidden;
    position: relative;
  }

  .page {
    height: 100%;
    overflow-y: auto;
    padding: 40px 48px 56px;
  }

  @media (max-width: 1100px) {
    .page {
      padding: 28px 32px 44px;
    }
  }
</style>
