<!--
  SETTINGS CARD - SteamGridDB artwork (Session M).

  The key is validated in Rust and stored in the DPAPI vault; this component
  only ever sees `hasKey` + a masked hint. Artwork is optional: without a key the
  app keeps its local GOG.ico / gradient fallback.
-->
<script lang="ts">
  import { onMount } from "svelte";
  import { isTauriRuntime } from "../art";
  import {
    clearSecret,
    getSecretStatus,
    setSecret,
    type SecretStatus,
  } from "../artProvider";
  import { pushToast } from "../toast.svelte";
  import ApiKeyGuide from "./ApiKeyGuide.svelte";
  import Icon from "./Icon.svelte";
  import Spinner from "./Spinner.svelte";

  const desktop = isTauriRuntime();
  const params =
    typeof window === "undefined"
      ? new URLSearchParams()
      : new URLSearchParams(window.location.search);
  const mock = params.get("mock") === "1";
  const usable = desktop || mock;

  let status = $state<SecretStatus>({ hasKey: false });
  let loading = $state(true);
  let busy = $state<"save" | "forget" | null>(null);
  let draft = $state("");
  let error = $state<string | null>(null);

  onMount(async () => {
    if (mock) {
      status =
        params.get("artkey") === "saved"
          ? { hasKey: true, hint: "cdef" }
          : { hasKey: false };
      loading = false;
      return;
    }
    status = await getSecretStatus("steamgriddb");
    loading = false;
  });

  async function save() {
    if (mock) return;
    error = null;
    const value = draft.trim();
    if (!/^[A-Za-z0-9]{16,128}$/.test(value)) {
      error = "That does not look like a SteamGridDB key. Copy it again from the API page.";
      return;
    }
    busy = "save";
    try {
      status = await setSecret("steamgriddb", value);
      draft = "";
      pushToast("success", "SteamGridDB key saved");
    } catch (err) {
      error = typeof err === "string" ? err : "Could not save the key.";
    } finally {
      busy = null;
    }
  }

  async function forget() {
    if (mock) return;
    busy = "forget";
    try {
      status = await clearSecret("steamgriddb");
      pushToast("info", "SteamGridDB key removed");
    } catch (err) {
      error = typeof err === "string" ? err : "Could not remove the key.";
    } finally {
      busy = null;
    }
  }
</script>

<section class="card artwork-card">
  <div class="card-head">
    <span class="card-icon"><Icon name="spark" size={18} /></span>
    <div>
      <h2>Artwork</h2>
      <p class="sub">
        Cover art for GOG and non-Steam games, from SteamGridDB. Optional.
      </p>
    </div>
    <span class="state" class:on={status.hasKey}>
      {status.hasKey ? "Connected" : "Not connected"}
    </span>
  </div>

  {#if !usable}
    <p class="hint">Artwork keys are only available in the desktop app.</p>
  {:else if loading}
    <p class="hint"><Spinner size={14} /> Checking…</p>
  {:else}
    <div class="key-row">
      <input
        type="password"
        class="key-input"
        placeholder={status.hasKey
          ? `Saved key ••••${status.hint ?? ""}`
          : "Paste your SteamGridDB API key"}
        bind:value={draft}
        spellcheck="false"
        autocomplete="off"
        aria-label="SteamGridDB API key"
        onkeydown={(event) => {
          if (event.key === "Enter" && !mock) void save();
        }}
      />
      <button
        type="button"
        class="btn primary"
        disabled={mock || busy !== null || draft.trim().length === 0}
        onclick={save}
      >
        {#if busy === "save"}<Spinner size={13} />{:else}Save{/if}
      </button>
      {#if status.hasKey}
        <button
          type="button"
          class="btn ghost"
          disabled={mock || busy !== null}
          onclick={forget}
        >
          Forget
        </button>
      {/if}
    </div>

    {#if error}
      <p class="error">{error}</p>
    {/if}

    <ApiKeyGuide provider="steamgriddb" open={!status.hasKey} />

    <p class="attribution">
      Artwork provided by
      <a href="https://www.steamgriddb.com" target="_blank" rel="noreferrer"
        >SteamGridDB</a
      >. Images are cached locally and only downloaded when you choose them.
    </p>
  {/if}
</section>

<style>
  .card {
    padding: 18px 20px;
    border: 1px solid var(--line, rgba(255, 255, 255, 0.07));
    border-radius: var(--radius-md);
    background: var(--card, rgba(18, 23, 36, 0.72));
  }

  .card-head {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .card-icon {
    display: grid;
    place-items: center;
    width: 34px;
    height: 34px;
    border-radius: 10px;
    background: rgba(var(--accent-rgb), 0.12);
    color: var(--accent);
  }

  h2 {
    margin: 0;
    font-size: 14px;
    font-weight: 650;
    color: var(--fg-0);
  }

  .sub {
    margin: 2px 0 0;
    font-size: 11.5px;
    color: var(--fg-2);
  }

  .state {
    margin-left: auto;
    padding: 3px 9px;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.05);
    color: var(--fg-2);
    font-size: 10.5px;
    font-weight: 600;
    letter-spacing: 0.02em;
    white-space: nowrap;
  }

  .state.on {
    background: rgba(61, 220, 151, 0.12);
    color: var(--ok);
  }

  .key-row {
    display: flex;
    gap: 8px;
    margin-top: 14px;
  }

  .key-input {
    flex: 1;
    min-width: 0;
    padding: 9px 12px;
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: var(--radius-sm);
    background: rgba(5, 6, 10, 0.6);
    color: var(--fg-0);
    font: inherit;
    font-size: 12.5px;
  }

  .key-input:focus {
    outline: none;
    border-color: var(--accent);
    box-shadow: 0 0 0 3px rgba(var(--accent-rgb), 0.15);
  }

  .btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 9px 14px;
    border: 1px solid transparent;
    border-radius: var(--radius-sm);
    font: inherit;
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
    transition:
      transform var(--dur-fast, 140ms) var(--ease-out, ease),
      background var(--dur-fast, 140ms);
  }

  .btn:disabled {
    opacity: 0.45;
    cursor: default;
  }

  .btn.primary {
    background: var(--accent);
    color: #05060a;
  }

  .btn.primary:not(:disabled):hover {
    transform: translateY(-1px);
  }

  .btn.ghost {
    border-color: rgba(255, 255, 255, 0.12);
    background: transparent;
    color: var(--fg-1);
  }

  .hint {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 12px 0 0;
    font-size: 12px;
    color: var(--fg-2);
  }

  .error {
    margin: 10px 0 0;
    font-size: 12px;
    color: var(--danger);
  }

  .attribution {
    margin: 12px 0 0;
    font-size: 10.5px;
    line-height: 1.6;
    color: var(--fg-2);
  }

  .attribution a {
    color: var(--accent);
    text-decoration: none;
  }
</style>
