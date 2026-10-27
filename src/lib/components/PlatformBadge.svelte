<!-- Store badge with the platform's accent dot. OWNER: Session D. -->
<script lang="ts">
  import { platformDotColor } from "../art";
  import { platformShort } from "../format";
  import type { Platform } from "../types";

  let { platform, compact = false }: { platform: Platform; compact?: boolean } =
    $props();
</script>

<span class="badge" class:compact>
  <span class="dot" style:background={platformDotColor(platform)}></span>
  {platformShort(platform)}
</span>

<style>
  .badge {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    padding: 5px 10px 5px 8px;
    border-radius: 999px;
    font-size: 10.5px;
    font-weight: 600;
    letter-spacing: 0.09em;
    text-transform: uppercase;
    color: var(--fg-1);
    background: rgba(255, 255, 255, 0.05);
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.07);
    white-space: nowrap;
  }

  .compact {
    padding: 3px 8px 3px 6px;
    font-size: 9.5px;
  }

  .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    flex: 0 0 auto;
  }
</style>
