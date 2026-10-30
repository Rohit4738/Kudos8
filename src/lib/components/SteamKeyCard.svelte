<!--
  SETTINGS CARD - optional Steam Web API key (Session L).

  Adds the account's COMPLETE owned Steam library on top of the local
  localconfig.vdf catalogue. The key is stored by Rust in Session M's DPAPI
  vault; this component only ever sees `hasKey` / `lastSync` / `ownedCount`.

  Network rules: nothing here calls Steam on mount except the one exception the
  contract allows - a stored key whose cache is older than 6 hours. That check
  runs 1.5 s after the card is interactive, never during app boot. Save and Test
  are explicit user actions.
-->
<script lang="ts">
  import { onMount } from "svelte";
  import { loadLibrary } from "../library.svelte";
  import { isDevMockEnabled } from "../devMock";
  import { pushToast } from "../toast.svelte";
  import {
    clearSteamApiKey,
    getSteamApiStatus,
    isStale,
    lastSyncLabel,
    maskedKeyHint,
    setSteamApiKey,
    syncSteamOwned,
    validateSteamKey,
    type SteamApiStatus,
  } from "../steamKey";
  import Icon from "./Icon.svelte";
  import ApiKeyGuide from "./ApiKeyGuide.svelte";
  import Spinner from "./Spinner.svelte";

  type MockMode = "no" | "invalid" | "valid" | null;

  const mockMode: MockMode = (() => {
    if (!isDevMockEnabled() || typeof window === "undefined") return null;
    const value = new URLSearchParams(window.location.search).get("steamkey");
    return value === "no" || value === "invalid" || value === "valid" ? value : null;
  })();

  const desktop = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

  let status = $state<SteamApiStatus | null>(null);
  let loading = $state(true);
  let busy = $state<"save" | "test" | "forget" | null>(null);
  let draft = $state("");
  let formError = $state<string | null>(null);
  let revealed = $state(false);

  const hasKey = $derived(Boolean(status?.hasKey));
  const error = $derived(formError ?? status?.error ?? null);
  const hint = $derived(maskedKeyHint(status));

  onMount(() => {
    if (mockMode) {
      loading = false;
      status = mockStatus(mockMode);
      return;
    }
    if (!desktop) {
      loading = false;
      status = null;
      return;
    }

    let cancelled = false;
    void (async () => {
      const next = await getSteamApiStatus();
      if (cancelled) return;
      status = next;
      loading = false;
      // Contract exception: refresh an existing key's stale cache, after the
      // UI is interactive. Never runs during app boot or without a key.
      if (next?.hasKey && isStale(next)) {
        window.setTimeout(() => {
          if (!cancelled) void runSync(true);
        }, 1500);
      }
    })();

    return () => {
      cancelled = true;
    };
  });

  function mockStatus(mode: Exclude<MockMode, null>): SteamApiStatus {
    if (mode === "valid") {
      return {
        hasKey: true,
        lastSync: Math.floor(Date.now() / 1000) - 2 * 3600,
        ownedCount: 412,
      };
    }
    if (mode === "invalid") {
      return {
        hasKey: false,
        lastSync: null,
        ownedCount: 0,
        error:
          "A Steam Web API key is exactly 32 hexadecimal characters (0-9, A-F). Copy the whole key from steamcommunity.com/dev/apikey and paste it again.",
      };
    }
    return { hasKey: false, lastSync: null, ownedCount: 0 };
  }

  async function save(): Promise<void> {
    if (busy) return;
    formError = null;
    const formatError = validateSteamKey(draft);
    if (formatError) {
      formError = formatError;
      return;
    }

    busy = "save";
    if (mockMode) {
      await wait(650);
      status = { hasKey: true, lastSync: Math.floor(Date.now() / 1000), ownedCount: 412 };
      draft = "";
      busy = null;
      pushToast("success", "Steam key saved", "Preview data: 412 owned games.");
      return;
    }

    const next = await setSteamApiKey(draft);
    status = next;
    // Keep the pasted value only when the vault write itself failed; a saved
    // key with a sync error (for example a private profile) can be cleared.
    if (next.hasKey || !next.error) draft = "";
    busy = null;
    if (next.error) {
      pushToast("error", "Couldn't sync the Steam library", next.error);
    } else {
      pushToast(
        "success",
        "Steam key saved",
        `${next.ownedCount} owned games loaded from the Steam Web API.`,
      );
      void loadLibrary(true);
    }
  }

  async function test(): Promise<void> {
    if (busy || !hasKey) return;
    formError = null;
    busy = "test";
    if (mockMode) {
      await wait(700);
      status = {
        hasKey: true,
        lastSync: Math.floor(Date.now() / 1000),
        ownedCount: status?.ownedCount || 412,
      };
      busy = null;
      pushToast("success", "Steam answered", "Preview data: 412 owned games.");
      return;
    }
    const next = await syncSteamOwned();
    status = next;
    busy = null;
    if (next.error) {
      pushToast("error", "Steam sync failed", next.error);
    } else {
      pushToast("success", "Steam library refreshed", `${next.ownedCount} owned games.`);
      void loadLibrary(true);
    }
  }

  async function forget(): Promise<void> {
    if (busy) return;
    busy = "forget";
    formError = null;
    if (mockMode) {
      await wait(350);
      status = { hasKey: false, lastSync: null, ownedCount: 0 };
      busy = null;
      pushToast("info", "Steam key removed", "Preview data cleared.");
      return;
    }
    const next = await clearSteamApiKey();
    busy = null;
    if (next.error) {
      // Keep the previous stats visible: a failed vault delete must not pretend
      // the key is gone. The error tells the user what to do next.
      status = { ...(status ?? next), error: next.error };
      pushToast("error", "Couldn't remove the Steam key", next.error);
      return;
    }
    status = next;
    draft = "";
    pushToast("info", "Steam key removed", "The encrypted key and cached library are gone.");
    void loadLibrary(true);
  }

  /** The only automatic call: stale cache + stored key, after the UI is up. */
  async function runSync(silent: boolean): Promise<void> {
    const next = await syncSteamOwned();
    status = next;
    if (!silent) return;
    if (!next.error) void loadLibrary(true);
  }

  function wait(ms: number): Promise<void> {
    return new Promise((resolve) => setTimeout(resolve, ms));
  }
</script>

<section class="steam-key" aria-label="Steam Web API key">
  <header class="head">
    <span class="icon"><Icon name="spark" size={18} /></span>
    <div class="copy">
      <h2>Complete Steam library</h2>
      <p>
        Optional. A free Steam Web API key lists every game you own — including
        titles never installed or launched on this PC.
      </p>
    </div>
    <span class="pill" class:on={hasKey} class:bad={Boolean(error)}>
      {#if loading}
        Checking
      {:else if busy}
        Working
      {:else if hasKey}
        Key saved
      {:else}
        No key
      {/if}
    </span>
  </header>

  {#if loading}
    <div class="loading"><Spinner size={16} /> Reading the saved key…</div>
  {:else if !desktop && !mockMode}
    <p class="note">
      The Steam Web API key is available in the desktop app. In this browser
      preview you can still read the setup guide below.
    </p>
  {:else}
    <div class="stats">
      <div class="stat">
        <span class="value">{status?.ownedCount ?? 0}</span>
        <span class="label">Owned games</span>
      </div>
      <div class="stat wide">
        <span class="value sync">{lastSyncLabel(status?.lastSync)}</span>
        <span class="label">Last sync</span>
      </div>
      <div class="stat">
        <span class="value state">{hasKey ? "Saved" : "None"}</span>
        <span class="label">API key</span>
      </div>
    </div>

    <div class="field">
      <label for="steam-api-key">Steam Web API key</label>
      <div class="input-row">
        <div class="input-wrap">
          <input
            id="steam-api-key"
            type={revealed ? "text" : "password"}
            bind:value={draft}
            placeholder={hasKey ? hint : "32 hexadecimal characters"}
            autocomplete="off"
            autocapitalize="off"
            spellcheck="false"
            inputmode="text"
            aria-describedby="steam-key-help"
          />
          <button
            type="button"
            class="reveal"
            aria-label={revealed ? "Hide the key while typing" : "Show the key while typing"}
            aria-pressed={revealed}
            onclick={() => (revealed = !revealed)}
          >
            <Icon name="eye" size={15} />
          </button>
        </div>
        <button
          type="button"
          class="primary"
          disabled={busy !== null || draft.trim().length === 0}
          onclick={() => void save()}
        >
          {#if busy === "save"}<Spinner size={15} />{/if}
          Save
        </button>
      </div>
      <p id="steam-key-help" class="help">
        {#if hasKey}
          Saved in the Windows vault (DPAPI) as {hint}. Kudos never shows it again.
        {:else}
          Stored encrypted with Windows DPAPI. Never written to a config file,
          localStorage or a log.
        {/if}
      </p>
    </div>

    {#if error}
      <p class="error" role="alert">{error}</p>
    {/if}

    <div class="actions">
      <button
        type="button"
        class="ghost"
        disabled={busy !== null || !hasKey}
        onclick={() => void test()}
      >
        {#if busy === "test"}
          <Spinner size={14} />
        {:else}
          <Icon name="refresh" size={14} />
        {/if}
        Test
      </button>
      <button
        type="button"
        class="ghost danger"
        disabled={busy !== null || !hasKey}
        onclick={() => void forget()}
      >
        {#if busy === "forget"}
          <Spinner size={14} />
        {:else}
          <Icon name="close" size={14} />
        {/if}
        Forget
      </button>
      {#if mockMode}<span class="preview">Preview · {mockMode}</span>{/if}
    </div>
  {/if}

  <ApiKeyGuide provider="steam" open={!hasKey} />

  <p class="privacy">
    Privacy: the key is sent only to <code>api.steampowered.com</code> over
    HTTPS, stored encrypted with Windows DPAPI, and never logged, shown again or
    written to a config file. Kudos calls Steam only on Save, Test or a stale
    cache with a key already saved.
  </p>
</section>

<style>
  .steam-key {
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
    max-width: 70ch;
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
  }

  .pill.on {
    color: rgba(195, 255, 227, 0.9);
    background: rgba(61, 220, 151, 0.1);
    box-shadow: inset 0 0 0 1px rgba(61, 220, 151, 0.24);
  }

  .pill.bad {
    color: #ffd9d4;
    background: rgba(255, 107, 107, 0.1);
    box-shadow: inset 0 0 0 1px rgba(255, 107, 107, 0.24);
  }

  .loading {
    display: flex;
    align-items: center;
    gap: 9px;
    font-size: 12px;
    color: var(--fg-2);
    padding: 8px 0;
  }

  .stats {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
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

  .value.state {
    font-size: 15px;
    line-height: 1.6;
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

  .field {
    margin-top: 16px;
  }

  .field label {
    display: block;
    margin-bottom: 7px;
    font-size: 10px;
    font-weight: 650;
    letter-spacing: 0.13em;
    text-transform: uppercase;
    color: var(--fg-2);
  }

  .input-row {
    display: flex;
    align-items: stretch;
    gap: 9px;
  }

  .input-wrap {
    position: relative;
    flex: 1;
    min-width: 0;
  }

  input {
    width: 100%;
    height: 40px;
    padding: 0 42px 0 13px;
    border-radius: 11px;
    font-family: ui-monospace, "Cascadia Mono", Consolas, monospace;
    font-size: 12.5px;
    letter-spacing: 0.04em;
    color: var(--fg-0);
    background: rgba(0, 0, 0, 0.24);
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.09);
  }

  input::placeholder {
    color: var(--fg-2);
    letter-spacing: 0.02em;
  }

  input:focus {
    outline: none;
    box-shadow:
      inset 0 0 0 1px color-mix(in srgb, var(--accent) 55%, transparent),
      0 0 0 3px color-mix(in srgb, var(--accent) 16%, transparent);
  }

  .reveal {
    position: absolute;
    top: 0;
    right: 0;
    display: grid;
    place-items: center;
    width: 40px;
    height: 40px;
    color: var(--fg-2);
    border-radius: 0 11px 11px 0;
  }

  .reveal:hover {
    color: var(--fg-0);
  }

  .primary {
    flex: 0 0 auto;
    display: inline-flex;
    align-items: center;
    gap: 7px;
    height: 40px;
    padding: 0 17px;
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
    opacity: 0.5;
    cursor: default;
  }

  .help {
    margin: 8px 0 0;
    font-size: 10.5px;
    line-height: 1.5;
    color: var(--fg-2);
  }

  .error {
    margin: 12px 0 0;
    padding: 10px 12px;
    border-radius: 10px;
    font-size: 11.5px;
    line-height: 1.5;
    color: #ffd9d4;
    background: rgba(255, 107, 107, 0.09);
    box-shadow: inset 0 0 0 1px rgba(255, 107, 107, 0.22);
  }

  .actions {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
    margin-top: 14px;
  }

  .ghost {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    height: 36px;
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
    opacity: 0.45;
    cursor: default;
  }

  .ghost.danger:hover:not(:disabled) {
    color: #ffb4ad;
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

  .privacy {
    margin: 12px 0 0;
    font-size: 10px;
    line-height: 1.55;
    color: var(--fg-2);
    max-width: 84ch;
  }

  .privacy code {
    font-family: ui-monospace, "Cascadia Mono", Consolas, monospace;
    font-size: 9.5px;
    padding: 1px 4px;
    border-radius: 4px;
    background: rgba(255, 255, 255, 0.06);
    color: var(--fg-1);
  }

  .note {
    margin: 14px 0 0;
    font-size: 10.5px;
    line-height: 1.55;
    color: var(--fg-2);
  }

  @media (max-width: 880px) {
    .stats {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
    .steam-key {
      grid-column: auto;
    }
    .input-row {
      flex-wrap: wrap;
    }
    .primary {
      width: 100%;
      justify-content: center;
    }
  }
</style>
