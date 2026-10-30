<!--
  BrandMark - one source of truth for the Kudos mark in the UI.
  OWNER: Session N.

  size:
    mark     -> square edge in px
    wordmark -> rendered width in px (aspect 1000:240)
    lockup   -> rendered width in px (aspect 600:480)

  The mark switches to the flat 16/24px artwork below 32px. The wordmark is
  inline so it inherits currentColor; `reveal` gives the boot splash its
  tracking ease using transform/opacity only (no letter-spacing layout work).
-->
<script lang="ts">
  interface Props {
    size?: number;
    variant?: "mark" | "wordmark" | "lockup";
    glow?: boolean;
    reveal?: boolean;
    /** Delay before the wordmark letter reveal, in ms. */
    delay?: number;
    /** Accessible name; omit for decorative use. */
    label?: string;
  }

  let {
    size = 32,
    variant = "mark",
    glow = false,
    reveal = false,
    delay = 0,
    label = "",
  }: Props = $props();

  const WORDMARK_W = 1000;
  const WORDMARK_H = 240;
  const LOCKUP_RATIO = 480 / 600;

  // Each letter's x-centre is derived from the 1000-unit wordmark. The shift
  // starts letters closer to the centre so they ease out into final tracking.
  const letters = [
    { d: "M96 50V190M96 120 216 50M96 120 216 190", shift: 41 },
    { d: "M266 50V130A60 60 0 0 0 386 130V50", shift: 21 },
    { d: "M436 50V190M436 50H516A55 70 0 0 1 516 190H436", shift: 0 },
    { d: "M612 120a65 70 0 1 0 130 0a65 70 0 1 0-130 0", shift: -21 },
    { d: "M889 50H829A35 35 0 0 0 829 120H869A35 35 0 0 1 869 190H809", shift: -41 },
  ];
</script>

{#if variant === "mark"}
  <img
    class="bm bm-mark"
    class:glow
    src={size < 32 ? "/brand/kudos-mark-small.svg" : "/brand/kudos-mark.svg"}
    width={size}
    height={size}
    alt={label}
    draggable="false"
    decoding="async"
  />
{:else if variant === "wordmark"}
  <svg
    class="bm bm-wordmark"
    class:glow
    width={size}
    height={(size * WORDMARK_H) / WORDMARK_W}
    viewBox={`0 0 ${WORDMARK_W} ${WORDMARK_H}`}
    fill="none"
    stroke="currentColor"
    stroke-width="24"
    stroke-linecap="round"
    stroke-linejoin="round"
    role={label ? "img" : undefined}
    aria-label={label || undefined}
    aria-hidden={label ? undefined : "true"}
    focusable="false"
  >
    {#if label}<title>{label}</title>{/if}
    {#each letters as letter, index (letter.d)}
      <g
        class="bm-letter"
        class:reveal
        style={`--i:${index};--shift:${letter.shift};--delay:${delay}ms`}
      >
        <path d={letter.d} />
      </g>
    {/each}
  </svg>
{:else}
  <img
    class="bm bm-lockup"
    class:glow
    src="/brand/kudos-lockup.svg"
    width={size}
    height={size * LOCKUP_RATIO}
    alt={label}
    draggable="false"
    decoding="async"
  />
{/if}

<style>
  .bm {
    display: block;
    flex: none;
  }

  .bm.glow {
    filter: drop-shadow(0 8px 22px var(--accent-glow))
      drop-shadow(0 0 34px rgba(var(--accent-rgb), 0.18));
  }

  .bm-wordmark {
    color: var(--fg-0);
    overflow: visible;
  }

  .bm-letter {
    opacity: 1;
    transform: translateX(0);
  }

  .bm-letter.reveal {
    opacity: 0;
    animation: bm-letter-in 360ms var(--ease-out) both;
    animation-delay: calc(var(--delay) + var(--i) * 20ms);
    will-change: transform, opacity;
  }

  @keyframes bm-letter-in {
    from {
      opacity: 0;
      transform: translateX(calc(var(--shift) * 1px));
    }
    to {
      opacity: 1;
      transform: translateX(0);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .bm-letter.reveal {
      animation: none !important;
      opacity: 1;
      transform: none;
    }
  }
</style>
