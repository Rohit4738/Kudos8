<!--
  First-run / truly-empty library. Explains what Kudos does and gives the one
  meaningful action: sync the Steam library. OWNER: Session I.
-->
<script lang="ts">
  import Icon from "./Icon.svelte";
  import Spinner from "./Spinner.svelte";

  interface Props {
    syncing?: boolean;
    note?: string;
    scanning?: boolean;
    onSync: () => void;
    onRefresh: () => void;
  }

  let { syncing = false, note = "", scanning = false, onSync, onRefresh }: Props = $props();
</script>

<section class="onboarding" aria-label="Welcome to Kudos">
  <div class="glow" aria-hidden="true"></div>

  <div class="copy">
    <p class="eyebrow">Welcome to Kudos</p>
    <h1>Your library,<br />one quiet place.</h1>
    <p class="lede">
      Kudos gathers the games already on this PC — Steam today, Epic and GOG when their
      clients are installed — and launches them from one controller-first screen. No
      store hopping, no new account.
    </p>

    <div class="actions">
      <button type="button" class="primary" disabled={syncing} onclick={onSync}>
        {#if syncing}
          <Spinner size={16} />
          Reading your Steam library…
        {:else}
          <span class="icon"><Icon name="refresh" size={16} /></span>
          Sync Steam library
        {/if}
      </button>

      <button type="button" class="secondary" disabled={scanning} onclick={onRefresh}>
        {#if scanning}
          <Spinner size={15} />
        {:else}
          <Icon name="library" size={15} />
        {/if}
        Scan this PC
      </button>
    </div>

    {#if note}
      <p class="note">{note}</p>
    {/if}
  </div>

  <div class="features" aria-label="What Kudos does">
    <article>
      <span class="feature-icon"><Icon name="library" size={17} /></span>
      <div>
        <h2>One library</h2>
        <p>Installed games and the rest of your Steam account, side by side.</p>
      </div>
    </article>

    <article>
      <span class="feature-icon"><Icon name="gamepad" size={17} /></span>
      <div>
        <h2>Made for the couch</h2>
        <p>Arrow keys or a controller: focus a row, pick a game, press play.</p>
      </div>
    </article>

    <article>
      <span class="feature-icon"><Icon name="spark" size={17} /></span>
      <div>
        <h2>Stays local</h2>
        <p>No account, no telemetry. Only the metadata sync you start yourself.</p>
      </div>
    </article>
  </div>

  <p class="foot">
    Epic and GOG games appear automatically when their launchers are installed. Nothing
    to configure.
  </p>
</section>

<style>
  .onboarding {
    position: relative;
    overflow: hidden;
    isolation: isolate;
    min-height: min(680px, calc(100vh - 140px));
    display: flex;
    flex-direction: column;
    justify-content: center;
    gap: clamp(28px, 4vh, 46px);
    padding: clamp(30px, 5vw, 72px);
    border-radius: clamp(22px, 2.2vw, 32px);
    background:
      radial-gradient(90% 90% at 84% 8%, color-mix(in srgb, var(--accent) 17%, transparent), transparent 62%),
      linear-gradient(160deg, rgba(255, 255, 255, 0.05), rgba(255, 255, 255, 0.012));
    box-shadow:
      inset 0 0 0 1px rgba(255, 255, 255, 0.07),
      0 34px 90px rgba(0, 0, 0, 0.42);
  }

  .glow {
    position: absolute;
    z-index: -1;
    inset: auto -10% -35% auto;
    width: 70%;
    aspect-ratio: 1;
    border-radius: 50%;
    background: radial-gradient(
      circle,
      color-mix(in srgb, var(--accent) 22%, transparent) 0%,
      transparent 66%
    );
    filter: blur(40px);
    pointer-events: none;
  }

  .copy {
    max-width: 660px;
  }

  .eyebrow {
    margin: 0 0 12px;
    font-size: 10px;
    font-weight: 650;
    letter-spacing: 0.24em;
    text-transform: uppercase;
    color: var(--accent);
  }

  h1 {
    margin: 0;
    font-family: var(--font-display);
    font-size: clamp(40px, 6.4vh, 72px);
    line-height: 0.96;
    font-weight: 600;
    letter-spacing: -0.05em;
  }

  .lede {
    margin: 20px 0 0;
    max-width: 56ch;
    font-size: 14px;
    line-height: 1.65;
    color: var(--fg-1);
  }

  .actions {
    display: flex;
    align-items: center;
    gap: 11px;
    flex-wrap: wrap;
    margin-top: 28px;
  }

  .primary {
    display: inline-flex;
    align-items: center;
    gap: 10px;
    height: 52px;
    padding: 0 24px;
    border-radius: 999px;
    font-size: 14px;
    font-weight: 650;
    color: #05070c;
    background: #f7f9ff;
    box-shadow:
      0 16px 40px rgba(0, 0, 0, 0.38),
      inset 0 0 0 1px rgba(255, 255, 255, 0.5);
    transition: transform 240ms var(--ease-out);
    will-change: transform;
  }

  .primary:hover:not(:disabled) {
    transform: translateY(-2px) scale(1.02);
  }

  .primary:disabled {
    opacity: 0.7;
    cursor: default;
  }

  .icon {
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    border-radius: 50%;
    color: #fff;
    background: #05070c;
  }

  .secondary {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    height: 46px;
    padding: 0 18px;
    border-radius: 999px;
    font-size: 12.5px;
    font-weight: 600;
    color: var(--fg-0);
    background: rgba(9, 13, 22, 0.4);
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.14);
    transition: transform 220ms var(--ease-out);
  }

  .secondary:hover:not(:disabled) {
    transform: translateY(-2px);
  }

  .secondary:disabled {
    opacity: 0.6;
    cursor: default;
  }

  .note {
    margin: 14px 0 0;
    font-size: 11.5px;
    line-height: 1.55;
    color: var(--fg-2);
    max-width: 58ch;
  }

  .features {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 12px;
    max-width: 980px;
  }

  .features article {
    display: flex;
    align-items: flex-start;
    gap: 11px;
    padding: 16px;
    border-radius: 16px;
    background: rgba(255, 255, 255, 0.032);
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.055);
    backdrop-filter: blur(12px);
  }

  .feature-icon {
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    border-radius: 10px;
    flex: 0 0 auto;
    color: var(--accent);
    background: color-mix(in srgb, var(--accent) 14%, transparent);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--accent) 26%, transparent);
  }

  .features h2 {
    margin: 0 0 4px;
    font-size: 12.5px;
    font-weight: 600;
  }

  .features p {
    margin: 0;
    font-size: 11px;
    line-height: 1.5;
    color: var(--fg-2);
  }

  .foot {
    margin: 0;
    font-size: 10.5px;
    color: var(--fg-2);
    opacity: 0.8;
  }

  @media (max-width: 900px) {
    .features {
      grid-template-columns: 1fr;
    }
  }
</style>
