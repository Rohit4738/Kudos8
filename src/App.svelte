<!--
  App shell. OWNER: Session A (structure + rail).
  Session D owns the pages in src/pages and may add components under src/lib/components.
-->
<script lang="ts">
  import { onMount } from "svelte";
  import { fade } from "svelte/transition";
  import { router, navigate, back } from "./lib/router.svelte";
  import Home from "./pages/Home.svelte";
  import GameDetail from "./pages/GameDetail.svelte";
  import Settings from "./pages/Settings.svelte";

  const pageKey = $derived(
    router.route.name === "game" ? `game:${router.route.id}` : router.route.name,
  );

  function onKeydown(event: KeyboardEvent) {
    if (event.key === "Escape" && router.route.name !== "home") {
      event.preventDefault();
      back();
    }
  }

  onMount(() => {
    window.addEventListener("keydown", onKeydown);
    return () => window.removeEventListener("keydown", onKeydown);
  });
</script>

<div class="shell">
  <nav class="rail" aria-label="Primary">
    <button
      class="brand"
      title="Project Kudos"
      onclick={() => navigate({ name: "home" })}
    >
      K
    </button>

    <div class="rail-items">
      <button
        class="rail-item"
        class:active={router.route.name === "home" || router.route.name === "game"}
        title="Library"
        onclick={() => navigate({ name: "home" })}
      >
        <span class="glyph">▦</span>
        <span class="rail-label">Library</span>
      </button>

      <button
        class="rail-item"
        class:active={router.route.name === "settings"}
        title="Settings"
        onclick={() => navigate({ name: "settings" })}
      >
        <span class="glyph">⚙</span>
        <span class="rail-label">Settings</span>
      </button>
    </div>

    <div class="rail-foot">v0.1</div>
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

<style>
  .shell {
    display: grid;
    grid-template-columns: var(--rail-w) 1fr;
    height: 100%;
    background:
      radial-gradient(1200px 700px at 78% -12%, rgba(76, 141, 255, 0.18), transparent 62%),
      radial-gradient(900px 600px at 4% 110%, rgba(123, 76, 255, 0.12), transparent 60%),
      linear-gradient(160deg, var(--bg-1) 0%, var(--bg-0) 100%);
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
    border-radius: 14px;
    display: grid;
    place-items: center;
    font-weight: 800;
    font-size: 20px;
    letter-spacing: -0.04em;
    color: #fff;
    background: linear-gradient(145deg, var(--accent) 0%, #7b4cff 100%);
    box-shadow: 0 6px 22px var(--accent-glow);
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
    background: rgba(76, 141, 255, 0.16);
    box-shadow: inset 0 0 0 1px rgba(76, 141, 255, 0.28);
  }

  .glyph {
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
