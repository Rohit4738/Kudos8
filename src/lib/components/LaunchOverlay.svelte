<!--
  Fade-to-black launching state. Games take a moment to create their window,
  so we hold the user's hand instead of leaving a frozen-looking UI.
  OWNER: Session D.
-->
<script lang="ts">
  import { fade } from "svelte/transition";
  import Spinner from "./Spinner.svelte";

  interface Props {
    active: boolean;
    title?: string;
    /** Eyebrow over the title: "Launching" or "Opening Steam". */
    label?: string;
    /** Supporting line under the title. */
    hint?: string;
  }

  let {
    active,
    title = "",
    label = "Launching",
    hint = "Handing over to the game window",
  }: Props = $props();
</script>

{#if active}
  <div
    class="launch"
    transition:fade={{ duration: 300 }}
    role="status"
    aria-live="assertive"
  >
    <div class="core">
      <Spinner size={30} />
      <p class="eyebrow">{label}</p>
      {#if title}<h2>{title}</h2>{/if}
      <p class="hint">{hint}</p>
    </div>
  </div>
{/if}

<style>
  .launch {
    position: fixed;
    inset: 0;
    z-index: 90;
    display: grid;
    place-items: center;
    background: #000;
  }

  .core {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 12px;
    text-align: center;
    color: var(--fg-0);
    padding: 24px;
  }

  .eyebrow {
    margin: 10px 0 0;
    font-size: 10px;
    font-weight: 650;
    letter-spacing: 0.3em;
    text-transform: uppercase;
    color: var(--fg-2);
  }

  h2 {
    margin: 0;
    font-family: var(--font-display);
    font-size: clamp(22px, 3vw, 34px);
    font-weight: 600;
    letter-spacing: -0.03em;
    max-width: 24ch;
  }

  .hint {
    margin: 2px 0 0;
    font-size: 12px;
    color: var(--fg-2);
  }
</style>
