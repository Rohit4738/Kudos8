<!--
  SETTINGS PAGE
  OWNER: Session D - replace this placeholder UI freely.
  Contract: `getSteamStatus()` from src/lib/api.ts.
-->
<script lang="ts">
  import { onMount } from "svelte";
  import { getSteamStatus } from "../lib/api";
  import type { SteamStatus } from "../lib/types";

  let status = $state<SteamStatus | null>(null);
  let error = $state<string | null>(null);

  onMount(async () => {
    try {
      status = await getSteamStatus();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  });
</script>

<section class="settings">
  <header>
    <p class="eyebrow">Settings</p>
    <h1>Preferences</h1>
  </header>

  <div class="panel">
    <h2>Steam</h2>
    {#if error}
      <p class="bad">Could not read Steam status: {error}</p>
    {:else if !status}
      <p class="muted">Checking Steam…</p>
    {:else}
      <ul class="rows">
        <li>
          <span>Installed</span>
          <span class:ok={status.installed} class:dim={!status.installed}>
            {status.installed ? "Detected" : "Not found"}
          </span>
        </li>
        <li>
          <span>Running</span>
          <span class:ok={status.running} class:dim={!status.running}>
            {status.running ? "Yes" : "No"}
          </span>
        </li>
        <li>
          <span>Install path</span>
          <span class="dim path">{status.steamPath ?? "—"}</span>
        </li>
      </ul>
    {/if}
  </div>

  <div class="panel">
    <h2>About</h2>
    <p class="muted">
      Project Kudos v0.1.0 — a lightweight PS5-style launcher for Steam, Epic and GOG.
    </p>
  </div>
</section>

<style>
  .settings {
    max-width: 820px;
  }
  .eyebrow {
    margin: 0 0 6px;
    font-size: 11px;
    letter-spacing: 0.18em;
    text-transform: uppercase;
    color: var(--accent);
  }
  h1 {
    margin: 0 0 28px;
    font-size: 40px;
    letter-spacing: -0.02em;
  }
  .panel {
    padding: 22px 24px;
    border-radius: var(--radius-lg);
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid rgba(255, 255, 255, 0.06);
    margin-bottom: 18px;
  }
  h2 {
    margin: 0 0 14px;
    font-size: 16px;
    font-weight: 600;
  }
  .rows {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .rows li {
    display: flex;
    justify-content: space-between;
    gap: 18px;
    font-size: 13px;
    color: var(--fg-1);
    padding-bottom: 10px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.05);
  }
  .rows li:last-child {
    border-bottom: none;
    padding-bottom: 0;
  }
  .ok {
    color: var(--ok);
  }
  .dim {
    color: var(--fg-2);
  }
  .path {
    font-size: 12px;
    word-break: break-all;
    text-align: right;
  }
  .muted {
    color: var(--fg-2);
    font-size: 13px;
    line-height: 1.6;
  }
  .bad {
    color: var(--danger);
    font-size: 13px;
  }
</style>
