<!--
  Filter chips: All / Installed / Not installed + Steam / Epic / GOG, with live
  counts. Empty stores stay visible at 0 and show a friendly empty state.
  Active pill is an opacity layer, never a paint animation.
  OWNER: Session D / I.
-->
<script lang="ts">
  import { platformDotColor } from "../art";
  import {
    visibleFilters,
    type Filter,
    type FilterCounts,
  } from "../filter";

  interface Props {
    value: Filter;
    counts: FilterCounts;
    onChange: (value: Filter) => void;
  }

  let { value, counts, onChange }: Props = $props();

  const defs = $derived(visibleFilters(counts));

  function dotFor(id: Filter): string | null {
    if (id === "steam" || id === "epic" || id === "gog") {
      return platformDotColor(id);
    }
    return null;
  }
</script>

<div class="filters" role="group" aria-label="Filter library">
  {#each defs as filter (filter.id)}
    <button
      type="button"
      class="chip"
      class:active={value === filter.id}
      aria-pressed={value === filter.id}
      onclick={() => onChange(filter.id)}
    >
      <span class="chip-bg" aria-hidden="true"></span>
      {#if dotFor(filter.id)}
        <span class="dot" style:background={dotFor(filter.id)} aria-hidden="true"></span>
      {/if}
      <span class="label">{filter.label}</span>
      <span class="count" class:empty={counts[filter.id] === 0}>
        {counts[filter.id]}
      </span>
    </button>
  {/each}
</div>

<style>
  .filters {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 7px;
  }

  .chip {
    position: relative;
    display: inline-flex;
    align-items: center;
    gap: 7px;
    height: 32px;
    padding: 0 12px;
    border-radius: 999px;
    color: var(--fg-2);
    font-size: 11.5px;
    font-weight: 550;
    letter-spacing: 0.01em;
    white-space: nowrap;
    isolation: isolate;
    transition: transform 160ms var(--ease-out);
    will-change: transform;
  }

  .chip:hover {
    transform: translateY(-1px);
    color: var(--fg-1);
  }

  .chip:active {
    transform: scale(0.97);
  }

  .chip.active {
    color: #fff;
  }

  .chip-bg {
    position: absolute;
    inset: 0;
    z-index: -1;
    border-radius: inherit;
    background: rgba(255, 255, 255, 0.055);
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.07);
    opacity: 0;
    transition: opacity 220ms var(--ease-out);
  }

  .chip:hover .chip-bg {
    opacity: 0.6;
  }

  .chip.active .chip-bg {
    opacity: 1;
    background:
      linear-gradient(
        180deg,
        color-mix(in srgb, var(--accent) 32%, transparent) 0%,
        color-mix(in srgb, var(--accent) 15%, transparent) 100%
      );
    box-shadow:
      inset 0 0 0 1px color-mix(in srgb, var(--accent) 52%, transparent),
      0 6px 20px color-mix(in srgb, var(--accent) 16%, transparent);
  }

  .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    opacity: 0.9;
  }

  .count {
    font-size: 9.5px;
    font-variant-numeric: tabular-nums;
    color: var(--fg-2);
    background: rgba(255, 255, 255, 0.055);
    border-radius: 999px;
    padding: 1px 5px;
    min-width: 18px;
    text-align: center;
  }

  .chip.active .count {
    color: rgba(255, 255, 255, 0.86);
    background: rgba(255, 255, 255, 0.12);
  }

  .count.empty {
    opacity: 0.45;
  }
</style>
