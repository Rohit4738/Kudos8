<!--
  SETTINGS
  Accent colour + borderless fullscreen (Tauri window API), persisted locally.
  Also shows the live Steam client status. OWNER: Session D.
-->
<script lang="ts">
  import { onMount } from "svelte";
  import { getSteamStatus } from "../lib/api";
  import { humanError } from "../lib/format";
  import { onGamepad } from "../lib/gamepad";
  import { back } from "../lib/router.svelte";
  import {
    DEFAULT_ACCENT,
    ACCENT_PRESETS,
    applyAccent,
    applyStoredAccent,
    storedAccent,
  } from "../lib/theme";
  import { pushToast } from "../lib/toast.svelte";
  import {
    currentFullscreen,
    isTauriRuntime,
    readFullscreenPref,
    setBorderlessFullscreen,
  } from "../lib/windowPrefs";
  import type { SteamStatus } from "../lib/types";
  import Icon from "../lib/components/Icon.svelte";
  import Spinner from "../lib/components/Spinner.svelte";
  import Toaster from "../lib/components/Toaster.svelte";

  const desktop = isTauriRuntime();

  let accent = $state(storedAccent());
  let fullscreen = $state(readFullscreenPref());
  let fullscreenBusy = $state(false);

  let steam = $state<SteamStatus | null>(null);
  let steamLoading = $state(true);
  let steamError = $state<string | null>(null);

  onMount(() => {
    applyStoredAccent();
    void syncWindowState();
    void loadSteam();

    const stopGamepad = onGamepad((action) => {
      if (action === "back") back();
    });
    return stopGamepad;
  });

  async function syncWindowState(): Promise<void> {
    if (!desktop) return;
    try {
      fullscreen = await currentFullscreen();
    } catch {
      /* keep the persisted preference */
    }
  }

  async function toggleFullscreen(): Promise<void> {
    if (!desktop || fullscreenBusy) return;

    const next = !fullscreen;
    fullscreenBusy = true;
    try {
      await setBorderlessFullscreen(next);
      fullscreen = next;
      pushToast(
        "success",
        next ? "Borderless fullscreen on" : "Windowed",
        next
          ? "The window now fills the screen without a title bar."
          : "The title bar and window frame are back.",
      );
    } catch (error) {
      pushToast("error", "Couldn't change window mode", humanError(error));
    } finally {
      fullscreenBusy = false;
    }
  }

  function chooseAccent(value: string): void {
    accent = applyAccent(value);
  }

  function onCustomInput(event: Event): void {
    const input = event.currentTarget as HTMLInputElement;
    chooseAccent(input.value);
  }

  async function loadSteam(): Promise<void> {
    steamLoading = true;
    steamError = null;

    if (!desktop) {
      steamLoading = false;
      steamError = "Steam detection is available in the desktop app.";
      return;
    }

    try {
      steam = await getSteamStatus();
    } catch (error) {
      steamError = humanError(error);
    } finally {
      steamLoading = false;
    }
  }
</script>

<section class="settings">
  <header class="head">
    <div>
      <p class="eyebrow">Settings</p>
      <h1>Make it yours</h1>
      <p class="sub">A few small touches that change the whole room.</p>
    </div>
    <button type="button" class="back" onclick={back}>
      <Icon name="arrow-left" size={16} />
      Library
    </button>
  </header>

  <div class="grid">
    <section class="card accent-card">
      <div class="card-head">
        <span class="card-icon accent">
          <Icon name="spark" size={18} />
        </span>
        <div>
          <h2>Accent colour</h2>
          <p>Focus rings, highlights and the active tile glow.</p>
        </div>
      </div>

      <div class="swatches">
        {#each ACCENT_PRESETS as preset (preset.value)}
          <button
            type="button"
            class="swatch"
            class:active={accent === preset.value}
            style:--swatch={preset.value}
            aria-label={preset.name}
            aria-pressed={accent === preset.value}
            title={preset.name}
            onclick={() => chooseAccent(preset.value)}
          >
            <span class="fill"></span>
            {#if accent === preset.value}
              <span class="check"><Icon name="check" size={15} stroke={2.6} /></span>
            {/if}
          </button>
        {/each}

        <label class="custom" title="Custom colour">
          <input
            type="color"
            value={accent}
            aria-label="Custom accent colour"
            oninput={onCustomInput}
          />
          <span class="custom-plus"><Icon name="plus" size={16} /></span>
        </label>
      </div>

      <div class="accent-foot">
        <span class="token">{accent}</span>
        <button type="button" class="text-btn" onclick={() => chooseAccent(DEFAULT_ACCENT)}>
          Reset
        </button>
      </div>
    </section>

    <section class="card">
      <div class="card-head">
        <span class="card-icon">
          <Icon name="sliders" size={18} />
        </span>
        <div>
          <h2>Window</h2>
          <p>How Kudos sits on your desktop.</p>
        </div>
      </div>

      <div class="row">
        <div class="row-copy">
          <p>Borderless fullscreen</p>
          <span>Edge-to-edge play with no title bar.</span>
        </div>
        <button
          type="button"
          class="toggle"
          class:on={fullscreen}
          role="switch"
          aria-checked={fullscreen}
          aria-label="Borderless fullscreen"
          disabled={!desktop || fullscreenBusy}
          onclick={() => void toggleFullscreen()}
        >
          <span class="knob">
            {#if fullscreenBusy}<Spinner size={12} />{/if}
          </span>
        </button>
      </div>

      {#if !desktop}
        <p class="note">
          Window controls are available in the desktop app. This browser preview
          remembers the preference but cannot resize the window.
        </p>
      {:else}
        <p class="note">
          Saved automatically. Kudos restores this mode the next time it starts.
        </p>
      {/if}
    </section>

    <section class="card steam-card">
      <div class="card-head">
        <span class="card-icon">
          <Icon name="gamepad" size={18} />
        </span>
        <div>
          <h2>Steam</h2>
          <p>Used by the GOG “Add to Steam” shortcut flow.</p>
        </div>
      </div>

      {#if steamLoading}
        <div class="steam-loading">
          <Spinner size={16} />
          Checking the Steam client…
        </div>
      {:else if steamError}
        <p class="steam-error">{steamError}</p>
      {:else if steam}
        <ul class="rows">
          <li>
            <span>Client</span>
            <span class="value" class:ok={steam.installed}>
              {steam.installed ? "Detected" : "Not found"}
            </span>
          </li>
          <li>
            <span>Running</span>
            <span class="value" class:ok={steam.running}>
              {steam.running ? "Yes — close it to add shortcuts" : "No"}
            </span>
          </li>
          {#if steam.steamPath}
            <li class="path-row">
              <span>Path</span>
              <span class="path">{steam.steamPath}</span>
            </li>
          {/if}
        </ul>
      {/if}

      <button type="button" class="text-btn refresh" onclick={() => void loadSteam()}>
        <Icon name="refresh" size={14} />
        Refresh
      </button>
    </section>

    <section class="card about-card">
      <div class="brand">K</div>
      <div class="about-copy">
        <h2>Project Kudos</h2>
        <p>
          A lightweight, controller-first launcher for Steam, Epic and GOG.
          Version 0.1.0.
        </p>
        <p class="legal">Fonts: Inter (SIL Open Font License).</p>
      </div>
    </section>
  </div>
</section>

<Toaster />

<style>
  .settings {
    max-width: 1080px;
    margin: 0 auto;
    padding-bottom: 20px;
  }

  .head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 24px;
    margin-bottom: clamp(24px, 3.4vh, 38px);
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
    font-size: 13.5px;
    color: var(--fg-2);
  }

  .back {
    flex: 0 0 auto;
    display: inline-flex;
    align-items: center;
    gap: 8px;
    height: 38px;
    padding: 0 15px;
    border-radius: 999px;
    font-size: 12px;
    font-weight: 550;
    color: var(--fg-1);
    background: rgba(255, 255, 255, 0.04);
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.07);
    transition: transform var(--dur-fast) var(--ease-out);
    will-change: transform;
  }

  .back:hover {
    transform: translateX(-2px);
  }

  /* ---------- cards ---------- */
  .grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 16px;
  }

  .card {
    position: relative;
    padding: 24px;
    border-radius: var(--radius-lg);
    background:
      linear-gradient(160deg, rgba(255, 255, 255, 0.045), rgba(255, 255, 255, 0.014));
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.065);
    backdrop-filter: blur(18px);
    min-width: 0;
  }

  .accent-card {
    grid-column: span 2;
  }

  .card-head {
    display: flex;
    align-items: flex-start;
    gap: 13px;
    margin-bottom: 20px;
  }

  .card-icon {
    display: grid;
    place-items: center;
    width: 38px;
    height: 38px;
    border-radius: 12px;
    flex: 0 0 auto;
    color: var(--fg-1);
    background: rgba(255, 255, 255, 0.055);
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.07);
  }

  .card-icon.accent {
    color: var(--accent);
    background: color-mix(in srgb, var(--accent) 14%, transparent);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--accent) 26%, transparent);
  }

  h2 {
    margin: 0;
    font-size: 15.5px;
    font-weight: 600;
    letter-spacing: -0.01em;
  }

  .card-head p {
    margin: 4px 0 0;
    font-size: 12px;
    line-height: 1.45;
    color: var(--fg-2);
  }

  /* ---------- accent ---------- */
  .swatches {
    display: flex;
    align-items: center;
    gap: 14px;
    flex-wrap: wrap;
  }

  .swatch {
    position: relative;
    width: 42px;
    height: 42px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.14);
    transition: transform var(--dur-fast) var(--ease-out);
    will-change: transform;
  }

  .swatch:hover {
    transform: translateY(-2px);
  }

  .swatch.active {
    box-shadow:
      inset 0 0 0 1px rgba(255, 255, 255, 0.3),
      0 0 0 2px var(--swatch);
  }

  .fill {
    position: absolute;
    inset: 3px;
    border-radius: 50%;
    background: var(--swatch);
    opacity: 0.9;
    transition: transform var(--dur-fast) var(--ease-out);
  }

  .swatch.active .fill {
    transform: scale(0.86);
  }

  .check {
    position: relative;
    z-index: 1;
    display: grid;
    place-items: center;
    color: #fff;
    filter: drop-shadow(0 1px 2px rgba(0, 0, 0, 0.45));
  }

  .custom {
    position: relative;
    width: 42px;
    height: 42px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    cursor: pointer;
    color: var(--fg-1);
    background:
      conic-gradient(
        from 90deg,
        #ff5f6d,
        #ffb547,
        #3ddc97,
        #4c8dff,
        #8b5cf6,
        #ff5f6d
      );
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.18);
    overflow: hidden;
  }

  .custom::after {
    content: "";
    position: absolute;
    inset: 3px;
    border-radius: 50%;
    background: rgba(8, 11, 18, 0.82);
  }

  .custom input {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    opacity: 0;
    cursor: pointer;
    z-index: 3;
  }

  .custom-plus {
    position: relative;
    z-index: 2;
    display: grid;
    place-items: center;
    pointer-events: none;
  }

  .accent-foot {
    display: flex;
    align-items: center;
    gap: 12px;
    margin-top: 20px;
  }

  .token {
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--fg-2);
    font-variant-numeric: tabular-nums;
    padding: 4px 8px;
    border-radius: 6px;
    background: rgba(255, 255, 255, 0.045);
  }

  .text-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    align-self: flex-start;
    font-size: 11.5px;
    font-weight: 550;
    color: var(--fg-2);
    transition: transform var(--dur-fast) var(--ease-out);
  }

  .text-btn:hover {
    transform: translateX(1px);
    color: var(--fg-0);
  }

  /* ---------- rows / toggle ---------- */
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 22px;
    padding: 14px 0 2px;
  }

  .row-copy p {
    margin: 0 0 4px;
    font-size: 13.5px;
    font-weight: 550;
    color: var(--fg-0);
  }

  .row-copy span {
    font-size: 11.5px;
    color: var(--fg-2);
    line-height: 1.5;
  }

  .toggle {
    position: relative;
    flex: 0 0 auto;
    width: 50px;
    height: 28px;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.12);
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.09);
  }

  .toggle::before {
    content: "";
    position: absolute;
    inset: 0;
    border-radius: inherit;
    background: var(--accent);
    opacity: 0;
    transition: opacity 220ms var(--ease-out);
  }

  .toggle.on::before {
    opacity: 1;
  }

  .toggle:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .knob {
    position: absolute;
    top: 3px;
    left: 3px;
    z-index: 1;
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    border-radius: 50%;
    color: #05070c;
    background: #fff;
    box-shadow: 0 2px 6px rgba(0, 0, 0, 0.28);
    transform: translate3d(0, 0, 0);
    transition: transform 220ms var(--ease-out);
    will-change: transform;
  }

  .toggle.on .knob {
    transform: translate3d(22px, 0, 0);
  }

  .note {
    margin: 14px 0 0;
    font-size: 10.5px;
    line-height: 1.5;
    color: var(--fg-2);
  }

  /* ---------- steam ---------- */
  .steam-loading {
    display: flex;
    align-items: center;
    gap: 9px;
    font-size: 12px;
    color: var(--fg-2);
    padding: 8px 0;
  }

  .steam-error {
    margin: 8px 0 0;
    font-size: 11.5px;
    line-height: 1.5;
    color: var(--fg-2);
  }

  .rows {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
  }

  .rows li {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 18px;
    padding: 9px 0;
    border-bottom: 1px solid rgba(255, 255, 255, 0.055);
    font-size: 12px;
    color: var(--fg-2);
  }

  .rows li:last-child {
    border-bottom: none;
  }

  .value {
    color: var(--fg-1);
    text-align: right;
  }

  .value.ok {
    color: var(--ok);
  }

  .path-row {
    align-items: flex-start;
  }

  .path {
    color: var(--fg-2);
    font-size: 10.5px;
    line-height: 1.45;
    text-align: right;
    overflow-wrap: anywhere;
  }

  .refresh {
    margin-top: 14px;
  }

  /* ---------- about ---------- */
  .about-card {
    grid-column: span 2;
    display: flex;
    align-items: center;
    gap: 16px;
  }

  .brand {
    display: grid;
    place-items: center;
    width: 48px;
    height: 48px;
    border-radius: 14px;
    flex: 0 0 auto;
    font-size: 20px;
    font-weight: 700;
    letter-spacing: -0.05em;
    color: #fff;
    background: linear-gradient(145deg, var(--accent) 0%, #8b5cf6 100%);
    box-shadow: 0 10px 30px var(--accent-glow);
  }

  .about-copy h2 {
    margin-bottom: 4px;
  }

  .about-copy p {
    margin: 0;
    font-size: 11.5px;
    line-height: 1.55;
    color: var(--fg-2);
    max-width: 62ch;
  }

  .legal {
    margin-top: 4px !important;
    opacity: 0.75;
  }

  @media (max-width: 980px) {
    .grid {
      grid-template-columns: 1fr;
    }
    .accent-card,
    .about-card {
      grid-column: auto;
    }
    .head {
      flex-direction: column;
    }
  }
</style>
