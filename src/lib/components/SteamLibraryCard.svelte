<!--
  Settings card for Session H's Steam catalogue: counts, cache status and the
  only network action in the app ("Sync now"). OWNER: Session I.
-->
<script lang="ts">
  import { library, loadLibrary, syncSteamLibrary } from "../library.svelte";
  import { syncFetchedLabel } from "../steam";
  import { pushToast } from "../toast.svelte";
  import Icon from "./Icon.svelte";
  import Spinner from "./Spinner.svelte";

  const installedCount = $derived(library.games.filter((game) => game.isInstalled).length);
  const ownedCount = $derived(
    library.games.filter((game) => game.platform === "steam" && !game.isInstalled).length,
  );
  const appCount = $derived(library.syncStatus?.appCount ?? null);
  const syncing = $derived(library.syncState === "syncing");
  const unavailable = $derived(
    library.steamState === "unavailable" || library.syncState === "unavailable",
  );

  async function syncNow(): Promise<void> {
    const ok = await syncSteamLibrary();
    if (ok) {
      pushToast(
        "success",
        "Steam library synced",
        `${library.syncStatus?.appCount ?? 0} apps known, ${ownedCount} ready to install from Kudos.`,
      );
    } else if (library.syncState === "unavailable") {
      pushToast(
        "info",
        "Sync not available yet",
        library.syncError ?? "This build lists installed games only.",
      );
    } else {
      pushToast(
        "error",
        "Couldn't sync Steam",
        library.syncError ?? "Steam did not answer. Try again in a moment.",
      );
    }
  }

  async function refresh(): Promise<void> {
    await loadLibrary(true);
    pushToast("info", "Library refreshed", "Installed games were scanned again.");
  }
</script>

<section class="steam-library" aria-label="Steam library sync">
  <header class="head">
    <span class="icon"><Icon name="library" size={18} /></span>
    <div class="copy">
      <h2>Steam library</h2>
      <p>The rest of your account, not just what is downloaded on this PC.</p>
    </div>
    <span class="pill" class:cached={library.syncStatus?.cached}>
      {#if syncing}
        Syncing
      {:else if unavailable}
        Local only
      {:else if library.syncStatus?.cached}
        Cached
      {:else if library.syncStatus}
        Fresh
      {:else}
        Waiting
      {/if}
    </span>
  </header>

  <div class="stats">
    <div class="stat">
      <span class="value">{appCount ?? "—"}</span>
      <span class="label">Apps in Steam</span>
    </div>
    <div class="stat">
      <span class="value">{ownedCount}</span>
      <span class="label">Owned, not installed</span>
    </div>
    <div class="stat">
      <span class="value">{installedCount}</span>
      <span class="label">Installed</span>
    </div>
    <div class="stat wide">
      <span class="value sync">{syncFetchedLabel(library.syncStatus?.fetchedAt)}</span>
      <span class="label">Metadata cache</span>
    </div>
  </div>

  <p class="note">
    {#if unavailable}
      {library.syncError ??
        "Owned-game sync is not available in this build yet. Installed Steam, Epic and GOG games still work."}
    {:else if library.syncState === "error" && library.syncError}
      {library.syncError}
    {:else}
      Sync reads your Steam account's app list and playtime. It is the only network call
      Kudos makes, and only when you press the button.
    {/if}
  </p>

  <div class="actions">
    <button type="button" class="primary" disabled={syncing} onclick={() => void syncNow()}>
      {#if syncing}
        <Spinner size={15} />
        Syncing…
      {:else}
        <Icon name="refresh" size={15} />
        Sync now
      {/if}
    </button>

    <button type="button" class="ghost" disabled={library.status === "loading"} onclick={() => void refresh()}>
      {#if library.status === "loading"}
        <Spinner size={14} />
      {:else}
        <Icon name="library" size={15} />
      {/if}
      Rescan this PC
    </button>

    {#if library.usingMock}
      <span class="preview">Preview data</span>
    {/if}
  </div>
</section>

<style>
  .steam-library {
    grid-column: span 2;
    position: relative;
    padding: 24px;
    border-radius: var(--radius-lg);
    background:
      linear-gradient(160deg, rgba(255, 255, 255, 0.045), rgba(255, 255, 255, 0.014));
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.065);
    backdrop-filter: blur(18px);
    min-width: 0;
  }

  .head {
    display: flex;
    align-items: flex-start;
    gap: 13px;
    margin-bottom: 18px;
  }

  .icon {
    display: grid;
    place-items: center;
    width: 38px;
    height: 38px;
    border-radius: 12px;
    flex: 0 0 auto;
    color: var(--accent);
    background: color-mix(in srgb, var(--accent) 14%, transparent);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--accent) 26%, transparent);
  }

  .copy {
    flex: 1;
    min-width: 0;
  }

  h2 {
    margin: 0;
    font-size: 15.5px;
    font-weight: 600;
    letter-spacing: -0.01em;
  }

  .copy p {
    margin: 4px 0 0;
    font-size: 12px;
    line-height: 1.45;
    color: var(--fg-2);
  }

  .pill {
    flex: 0 0 auto;
    padding: 5px 10px;
    border-radius: 999px;
    font-size: 9px;
    font-weight: 700;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--fg-2);
    background: rgba(255, 255, 255, 0.05);
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.08);
    font-variant-numeric: tabular-nums;
  }

  .pill.cached {
    color: rgba(195, 255, 227, 0.9);
    background: rgba(61, 220, 151, 0.1);
    box-shadow: inset 0 0 0 1px rgba(61, 220, 151, 0.24);
  }

  .stats {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 9px;
  }

  .stat {
    padding: 13px 14px;
    border-radius: 13px;
    background: rgba(255, 255, 255, 0.03);
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.05);
    min-width: 0;
  }

  .value {
    display: block;
    font-family: var(--font-display);
    font-size: 22px;
    font-weight: 600;
    letter-spacing: -0.03em;
    color: var(--fg-0);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .value.sync {
    font-size: 13px;
    line-height: 1.5;
    font-weight: 550;
    color: var(--fg-1);
  }

  .label {
    display: block;
    margin-top: 3px;
    font-size: 9px;
    font-weight: 650;
    letter-spacing: 0.13em;
    text-transform: uppercase;
    color: var(--fg-2);
  }

  .note {
    margin: 14px 0 0;
    font-size: 10.5px;
    line-height: 1.55;
    color: var(--fg-2);
    max-width: 76ch;
  }

  .actions {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
    margin-top: 16px;
  }

  .primary {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    height: 38px;
    padding: 0 16px;
    border-radius: 999px;
    font-size: 12px;
    font-weight: 600;
    color: #05070c;
    background: #f4f7ff;
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.4);
    transition: transform 200ms var(--ease-out);
  }

  .primary:hover:not(:disabled) {
    transform: translateY(-1px) scale(1.02);
  }

  .primary:disabled {
    opacity: 0.65;
    cursor: default;
  }

  .ghost {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    height: 38px;
    padding: 0 14px;
    border-radius: 999px;
    font-size: 11.5px;
    font-weight: 550;
    color: var(--fg-1);
    background: rgba(255, 255, 255, 0.05);
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.08);
    transition: transform 200ms var(--ease-out);
  }

  .ghost:hover:not(:disabled) {
    transform: translateY(-1px);
    color: var(--fg-0);
  }

  .ghost:disabled {
    opacity: 0.55;
    cursor: default;
  }

  .preview {
    font-size: 8.5px;
    font-weight: 700;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--accent);
    padding: 4px 8px;
    border-radius: 999px;
    background: color-mix(in srgb, var(--accent) 12%, transparent);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--accent) 28%, transparent);
  }

  @media (max-width: 880px) {
    .stats {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
    .steam-library {
      grid-column: auto;
    }
  }
</style>
