<!-- Inline SVG icon set (no icon-font, no network). OWNER: Session D. -->
<script lang="ts">
  interface Props {
    name:
      | "play"
      | "arrow-left"
      | "arrow-right"
      | "plus"
      | "check"
      | "alert"
      | "close"
      | "gamepad"
      | "keyboard"
      | "library"
      | "sliders"
      | "spark"
      | "refresh"
      | "trophy"
      | "hide"
      | "eye";
    size?: number;
    stroke?: number;
  }

  let { name, size = 18, stroke = 1.8 }: Props = $props();
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width={stroke}
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
  focusable="false"
>
  {#if name === "play"}
    <path d="M8 5.2v13.6L19.2 12 8 5.2Z" fill="currentColor" stroke="none" />
  {:else if name === "arrow-left"}
    <path d="M19 12H5" />
    <path d="m12 19-7-7 7-7" />
  {:else if name === "arrow-right"}
    <path d="M5 12h14" />
    <path d="m12 5 7 7-7 7" />
  {:else if name === "plus"}
    <path d="M12 5v14M5 12h14" />
  {:else if name === "check"}
    <path d="M20 6 9 17l-5-5" />
  {:else if name === "alert"}
    <circle cx="12" cy="12" r="9" />
    <path d="M12 7.5v5.5" />
    <path d="M12 16.5h.01" />
  {:else if name === "close"}
    <path d="M18 6 6 18M6 6l12 12" />
  {:else if name === "gamepad"}
    <path d="M7.5 8h9a4.5 4.5 0 0 1 4.4 5.4l-.6 2.8a2.4 2.4 0 0 1-4.2 1.1L14.5 15h-5l-1.6 2.3a2.4 2.4 0 0 1-4.2-1.1l-.6-2.8A4.5 4.5 0 0 1 7.5 8Z" />
    <path d="M8 10.5v3M6.5 12h3" />
    <path d="M15.5 11.2h.01M17.3 13h.01" />
  {:else if name === "keyboard"}
    <rect x="3" y="6" width="18" height="12" rx="2.5" />
    <path d="M7 10h.01M11 10h.01M15 10h.01M17 10h.01M7 14h.01M10 14h4M17 14h.01" />
  {:else if name === "library"}
    <rect x="3.5" y="3.5" width="7" height="7" rx="1.6" />
    <rect x="13.5" y="3.5" width="7" height="7" rx="1.6" />
    <rect x="3.5" y="13.5" width="7" height="7" rx="1.6" />
    <rect x="13.5" y="13.5" width="7" height="7" rx="1.6" />
  {:else if name === "sliders"}
    <path d="M4 7h10M18 7h2M4 17h4M12 17h8" />
    <circle cx="16" cy="7" r="2" />
    <circle cx="10" cy="17" r="2" />
  {:else if name === "spark"}
    <path d="M12 3.5 13.9 9l5.6 1.9-5.6 1.9L12 18.4l-1.9-5.6-5.6-1.9L10.1 9 12 3.5Z" />
  {:else if name === "refresh"}
    <path d="M20 11a8 8 0 1 0-2.3 6.1" />
    <path d="M20 4.5V11h-6.5" />
  {:else if name === "trophy"}
    <path d="M8 4.5h8v5a4 4 0 0 1-8 0v-5Z" />
    <path d="M8 5.5H5.5v1.8A3.2 3.2 0 0 0 8.7 10.5M16 5.5h2.5v1.8a3.2 3.2 0 0 1-3.2 3.2" />
    <path d="M12 13.5v3.2M8.5 19.5h7" />
  {:else if name === "hide"}
    <path d="M3 3l18 18" />
    <path d="M10.6 6.3A9.7 9.7 0 0 1 12 6.2c4.4 0 7.5 3 9 5.8a13 13 0 0 1-2.2 2.8" />
    <path d="M6.3 7.4A13.4 13.4 0 0 0 3 12c1.5 2.8 4.6 5.8 9 5.8 1.2 0 2.3-.2 3.3-.6" />
    <path d="M9.9 9.9a3 3 0 0 0 4.2 4.2" />
  {:else if name === "eye"}
    <path d="M3 12c1.5-2.8 4.6-5.8 9-5.8s7.5 3 9 5.8c-1.5 2.8-4.6 5.8-9 5.8S4.5 14.8 3 12Z" />
    <circle cx="12" cy="12" r="2.6" />
  {/if}
</svg>
