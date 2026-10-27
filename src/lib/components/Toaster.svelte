<!-- Global toast stack for command feedback. OWNER: Session D. -->
<script lang="ts">
  import { fade, fly } from "svelte/transition";
  import { dismissToast, toasts } from "../toast.svelte";
  import Icon from "./Icon.svelte";

  const ICONS = {
    error: "alert",
    success: "check",
    info: "spark",
  } as const;
</script>

{#if toasts.length > 0}
  <div class="toaster" aria-live="polite">
    {#each toasts as toast (toast.id)}
      <div
        class="toast {toast.kind}"
        role={toast.kind === "error" ? "alert" : "status"}
        transition:fly={{ y: 16, duration: 260, opacity: 0 }}
        out:fade={{ duration: 160 }}
      >
        <span class="glyph" aria-hidden="true">
          <Icon name={ICONS[toast.kind]} size={17} />
        </span>

        <div class="copy">
          <p class="title">{toast.title}</p>
          {#if toast.message}<p class="message">{toast.message}</p>{/if}
        </div>

        <button
          type="button"
          class="close"
          aria-label="Dismiss"
          onclick={() => dismissToast(toast.id)}
        >
          <Icon name="close" size={14} />
        </button>
      </div>
    {/each}
  </div>
{/if}

<style>
  .toaster {
    position: fixed;
    right: 24px;
    bottom: 24px;
    z-index: 120;
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 10px;
    width: min(360px, calc(100vw - 48px));
    pointer-events: none;
  }

  .toast {
    pointer-events: auto;
    display: flex;
    align-items: flex-start;
    gap: 11px;
    width: 100%;
    padding: 13px 12px 13px 14px;
    border-radius: 14px;
    color: var(--fg-0);
    background: rgba(14, 18, 28, 0.88);
    backdrop-filter: blur(22px) saturate(1.2);
    box-shadow:
      inset 0 0 0 1px rgba(255, 255, 255, 0.09),
      0 22px 55px rgba(0, 0, 0, 0.55);
    will-change: transform, opacity;
  }

  .glyph {
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    border-radius: 9px;
    flex: 0 0 auto;
    background: rgba(255, 255, 255, 0.07);
  }

  .toast.error .glyph {
    color: var(--danger);
    background: rgba(255, 95, 109, 0.13);
  }

  .toast.success .glyph {
    color: var(--ok);
    background: rgba(61, 220, 151, 0.12);
  }

  .toast.info .glyph {
    color: var(--accent);
    background: color-mix(in srgb, var(--accent) 16%, transparent);
  }

  .copy {
    min-width: 0;
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 3px;
    padding-top: 2px;
  }

  .title {
    margin: 0;
    font-size: 13px;
    font-weight: 600;
    letter-spacing: -0.005em;
  }

  .message {
    margin: 0;
    font-size: 11.5px;
    line-height: 1.45;
    color: var(--fg-1);
    overflow-wrap: anywhere;
  }

  .close {
    flex: 0 0 auto;
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    border-radius: 7px;
    color: var(--fg-2);
    transition: transform var(--dur-fast) var(--ease-out);
  }

  .close:hover {
    transform: scale(1.1);
    color: var(--fg-0);
  }
</style>
