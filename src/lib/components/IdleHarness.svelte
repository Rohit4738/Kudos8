<!--
  SESSION G DEV HARNESS — mounted by `?idle=1` from main.ts.
  OWNER: Session G. Not part of the normal app shell.
  `?idle=1&mock=1`          → opens ambient mode with demo games
  `?idle=1&mock=1&shell=1`  → stays on this page (row + settings preview)
-->
<script lang="ts">
  import { onMount } from "svelte";
  import { loadRecent, recent } from "../history.svelte";
  import {
    enterIdle,
    exitIdle,
    idlePrefs,
    idleState,
    setIdleGate,
    updateIdlePrefs,
  } from "../idle.svelte";
  import { loadLibrary, library } from "../library.svelte";
  import { applyStoredAccent } from "../theme";
  import IdleAmbient from "./IdleAmbient.svelte";
  import IdleSettings from "./IdleSettings.svelte";
  import RecentlyPlayedRow from "./RecentlyPlayedRow.svelte";

  const params = typeof window !== "undefined" ? new URLSearchParams(window.location.search) : null;
  const shellOnly = params?.get("shell") === "1";

  let ready = $state(false);
  let autoTimer: ReturnType<typeof setTimeout> | undefined;

  onMount(() => {
    applyStoredAccent();
    setIdleGate(() => library.games.length > 0);

    void (async () => {
      await loadLibrary();
      await loadRecent(true);
      ready = true;
      if (!shellOnly) {
        autoTimer = setTimeout(() => enterIdle(true), 400);
      }
    })();

    return () => {
      setIdleGate(null);
      if (autoTimer) clearTimeout(autoTimer);
    };
  });
</script>

<svelte:head>
  <title>Kudos — idle mode preview</title>
</svelte:head>

<div class="harness">
  <header>
    <p class="eyebrow">Session G preview</p>
    <h1>Idle &amp; recent</h1>
    <p class="sub">
      {#if !ready}
        Loading the preview data…
      {:else}
        {library.games.length} games · {recent.entries.length} recent entries ·
        {shellOnly ? "shell mode" : "ambient mode opens in a moment"}
      {/if}
    </p>
  </header>

  <RecentlyPlayedRow games={library.games} onOpen={() => {}} />

  <div class="rail-note">
    <span>Home rail sits here in the real app</span>
  </div>

  <IdleSettings
    value={idlePrefs}
    onChange={(next) => updateIdlePrefs(next)}
    onPreview={() => enterIdle(true)}
  />
</div>

{#if idleState.active}
  <IdleAmbient
    games={library.games}
    recent={recent.entries}
    settings={idlePrefs}
    onExit={exitIdle}
  />
{/if}

<style>
  .harness {
    height: 100%;
    overflow-y: auto;
    max-width: 1080px;
    margin: 0 auto;
    padding: 44px 48px 60px;
    display: flex;
    flex-direction: column;
    gap: 22px;
  }

  .eyebrow {
    margin: 0 0 7px;
    font-size: 10px;
    font-weight: 650;
    letter-spacing: 0.24em;
    text-transform: uppercase;
    color: var(--accent);
  }

  h1 {
    margin: 0;
    font-family: var(--font-display);
    font-size: clamp(32px, 4.8vh, 48px);
    font-weight: 600;
    line-height: 1.02;
    letter-spacing: -0.038em;
  }

  .sub {
    margin: 10px 0 0;
    font-size: 12.5px;
    color: var(--fg-2);
    font-variant-numeric: tabular-nums;
  }

  .rail-note {
    display: grid;
    place-items: center;
    height: 220px;
    border-radius: var(--radius-lg);
    font-size: 11px;
    letter-spacing: 0.16em;
    text-transform: uppercase;
    color: var(--fg-2);
    background:
      repeating-linear-gradient(
        135deg,
        rgba(255, 255, 255, 0.018) 0 10px,
        transparent 10px 20px
      ),
      rgba(255, 255, 255, 0.02);
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.05);
  }
</style>
