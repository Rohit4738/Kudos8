<!--
  IDLE MODE settings card. OWNER: Session G.
  Self-contained + controlled: receives the current values and emits the next
  full object, so Settings.svelte only wires `updateIdlePrefs`.
-->
<script lang="ts">
  import { DEFAULT_IDLE, IDLE_DELAYS, type IdleSettings } from "../idle.svelte";
  import type { IdleArtSource } from "../idle.svelte";
  import Icon from "./Icon.svelte";

  interface Props {
    value: IdleSettings;
    onChange: (next: IdleSettings) => void;
    onPreview?: () => void;
  }

  let { value, onChange, onPreview }: Props = $props();

  const ART_CHOICES: ReadonlyArray<{
    id: IdleArtSource;
    title: string;
    copy: string;
  }> = [
    { id: "recent", title: "Recently played", copy: "Your last games first, then the library." },
    { id: "library", title: "Whole library", copy: "Walk through every installed game." },
    { id: "random", title: "Shuffle", copy: "Random art, never the same game twice." },
  ];

  function set(patch: Partial<IdleSettings>): void {
    onChange({ ...value, ...patch });
  }
</script>

<section class="card">
  <div class="card-head">
    <span class="card-icon accent">
      <Icon name="spark" size={18} />
    </span>
    <div>
      <h2>Idle mode</h2>
      <p>An ambient, full-screen attract screen for when you step away.</p>
    </div>
  </div>

  <div class="row">
    <div class="row-copy">
      <p>Enable idle mode</p>
      <span>Starts on the Home page after the delay below.</span>
    </div>
    <button
      type="button"
      class="toggle"
      class:on={value.enabled}
      role="switch"
      aria-checked={value.enabled}
      aria-label="Enable idle mode"
      onclick={() => set({ enabled: !value.enabled })}
    >
      <span class="knob"></span>
    </button>
  </div>

  <div class="field">
    <span class="label">Start after</span>
    <div class="segmented" role="group" aria-label="Idle delay">
      {#each IDLE_DELAYS as option (option.value)}
        <button
          type="button"
          class:active={value.delayMs === option.value}
          aria-pressed={value.delayMs === option.value}
          onclick={() => set({ delayMs: option.value })}
        >
          {option.label}
        </button>
      {/each}
    </div>
  </div>

  <div class="field">
    <span class="label">Art source</span>
    <div class="choices" role="group" aria-label="Idle art source">
      {#each ART_CHOICES as choice (choice.id)}
        <button
          type="button"
          class="choice"
          class:active={value.artSource === choice.id}
          aria-pressed={value.artSource === choice.id}
          onclick={() => set({ artSource: choice.id })}
        >
          <span class="choice-title">{choice.title}</span>
          <span class="choice-copy">{choice.copy}</span>
        </button>
      {/each}
    </div>
  </div>

  <div class="split">
    <div class="field">
      <span class="label">Crossfade</span>
      <div class="segmented" role="group" aria-label="Idle crossfade speed">
        <button
          type="button"
          class:active={value.crossfade === "slow"}
          aria-pressed={value.crossfade === "slow"}
          onclick={() => set({ crossfade: "slow" })}
        >
          Slow
        </button>
        <button
          type="button"
          class:active={value.crossfade === "fast"}
          aria-pressed={value.crossfade === "fast"}
          onclick={() => set({ crossfade: "fast" })}
        >
          Fast
        </button>
      </div>
    </div>

    <div class="field">
      <span class="label">Clock</span>
      <button
        type="button"
        class="toggle small"
        class:on={value.showClock}
        role="switch"
        aria-checked={value.showClock}
        aria-label="Show clock in idle mode"
        onclick={() => set({ showClock: !value.showClock })}
      >
        <span class="knob"></span>
      </button>
    </div>
  </div>

  <div class="foot">
    <button
      type="button"
      class="preview"
      onclick={() => onPreview?.()}
      disabled={!onPreview}
    >
      <Icon name="play" size={14} />
      Preview idle mode now
    </button>
    <button
      type="button"
      class="text-btn"
      onclick={() => onChange({ ...DEFAULT_IDLE })}
    >
      Reset
    </button>
  </div>

  <p class="note">
    Idle mode is a low-activity ambient display, not a screensaver: it keeps the
    window awake and does not blank or lock the screen. Use Windows power settings
    for display timeout.
  </p>
</section>

<style>
  .card {
    position: relative;
    grid-column: span 2;
    min-width: 0;
    padding: 24px;
    border-radius: var(--radius-lg);
    background:
      linear-gradient(160deg, rgba(255, 255, 255, 0.045), rgba(255, 255, 255, 0.014));
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.065);
    backdrop-filter: blur(18px);
  }

  .card-head {
    display: flex;
    align-items: flex-start;
    gap: 13px;
    margin-bottom: 18px;
  }

  .card-icon {
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

  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 22px;
    padding: 6px 0 16px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.06);
  }

  .row-copy p {
    margin: 0 0 4px;
    font-size: 13.5px;
    font-weight: 550;
    color: var(--fg-0);
  }

  .row-copy span {
    font-size: 11.5px;
    line-height: 1.5;
    color: var(--fg-2);
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding-top: 16px;
  }

  .label {
    font-size: 9.5px;
    font-weight: 650;
    letter-spacing: 0.16em;
    text-transform: uppercase;
    color: var(--fg-2);
  }

  .segmented {
    display: inline-flex;
    align-self: flex-start;
    gap: 3px;
    padding: 3px;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.04);
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.06);
  }

  .segmented button {
    min-width: 48px;
    padding: 6px 12px;
    border-radius: 999px;
    font-size: 11px;
    font-weight: 550;
    color: var(--fg-2);
    transition:
      background var(--dur-fast) var(--ease-out),
      color var(--dur-fast) var(--ease-out);
  }

  .segmented button:hover {
    color: var(--fg-0);
  }

  .segmented button.active {
    color: #fff;
    background: color-mix(in srgb, var(--accent) 78%, #0a0d16);
    box-shadow: 0 2px 10px color-mix(in srgb, var(--accent) 32%, transparent);
  }

  .choices {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 10px;
  }

  .choice {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 12px 13px;
    text-align: left;
    border-radius: var(--radius-md);
    color: var(--fg-2);
    background: rgba(255, 255, 255, 0.035);
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.055);
    transition:
      transform var(--dur-fast) var(--ease-out),
      background var(--dur-fast) var(--ease-out),
      box-shadow var(--dur-fast) var(--ease-out);
    will-change: transform;
  }

  .choice:hover {
    transform: translateY(-1px);
    background: rgba(255, 255, 255, 0.055);
  }

  .choice.active {
    background: color-mix(in srgb, var(--accent) 13%, transparent);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--accent) 38%, transparent);
  }

  .choice-title {
    font-size: 12.5px;
    font-weight: 600;
    color: var(--fg-0);
  }

  .choice-copy {
    font-size: 10.5px;
    line-height: 1.45;
  }

  .split {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    align-items: end;
    gap: 24px;
  }

  .split .field {
    padding-top: 16px;
  }

  .foot {
    display: flex;
    align-items: center;
    gap: 18px;
    margin-top: 22px;
    padding-top: 18px;
    border-top: 1px solid rgba(255, 255, 255, 0.06);
  }

  .preview {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    height: 38px;
    padding: 0 17px;
    border-radius: 999px;
    font-size: 12px;
    font-weight: 600;
    color: #fff;
    background: color-mix(in srgb, var(--accent) 72%, #0a0d16);
    box-shadow: 0 6px 22px color-mix(in srgb, var(--accent) 26%, transparent);
    transition: transform var(--dur-fast) var(--ease-out);
    will-change: transform;
  }

  .preview:hover {
    transform: translateY(-1px) scale(1.02);
  }

  .preview:disabled {
    opacity: 0.45;
    cursor: not-allowed;
    transform: none;
  }

  .text-btn {
    font-size: 11.5px;
    font-weight: 550;
    color: var(--fg-2);
    transition: color var(--dur-fast) var(--ease-out);
  }

  .text-btn:hover {
    color: var(--fg-0);
  }

  .note {
    margin: 16px 0 0;
    max-width: 92ch;
    font-size: 10.5px;
    line-height: 1.55;
    color: var(--fg-2);
  }

  /* toggle (same visual language as Settings' window toggle) */
  .toggle {
    position: relative;
    flex: 0 0 auto;
    width: 50px;
    height: 28px;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.12);
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.09);
  }

  .toggle.small {
    width: 46px;
    height: 26px;
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

  .knob {
    position: absolute;
    top: 3px;
    left: 3px;
    z-index: 1;
    width: 22px;
    height: 22px;
    border-radius: 50%;
    background: #f4f7ff;
    box-shadow: 0 2px 6px rgba(0, 0, 0, 0.35);
    transition: transform 220ms var(--ease-out);
  }

  .toggle.small .knob {
    width: 20px;
    height: 20px;
  }

  .toggle.on .knob {
    transform: translateX(22px);
  }

  .toggle.small.on .knob {
    transform: translateX(20px);
  }

  @media (max-width: 860px) {
    .card {
      grid-column: span 1;
    }
    .choices {
      grid-template-columns: 1fr;
    }
    .split {
      grid-template-columns: 1fr;
      gap: 0;
    }
  }
</style>
