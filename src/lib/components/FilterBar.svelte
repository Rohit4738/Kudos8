<!-- Store filter row. Active pill is an opacity layer, never a paint animation. -->
<script lang="ts">
  import { platformDotColor } from "../art";
  import { FILTERS, type Filter, type FilterCounts } from "../filter";

  interface Props {
    value: Filter;
    counts: FilterCounts;
    onChange: (value: Filter) => void;
  }

  let { value, counts, onChange }: Props = $props();
</script>

<div class="filters" role="group" aria-label="Filter by store">
  {#each FILTERS as filter (filter.id)}
    <button
      class="chip"
      class:active={value === filter.id}
      aria-pressed={value === filter.id}
      onclick={() => onChange(filter.id)}
    >
      <span class="chip-bg" aria-hidden="true"></span>
      {#if filter.id !== "all"}
        <span
          class="dot"
          style:background={platformDotColor(filter.id)}
          aria-hidden="true"
        ></span>
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
    gap: 8px;
  }

  .chip {
    position: relative;
    display: inline-flex;
    align-items: center;
    gap: 8px;
    height: 34px;
    padding: 0 14px;
    border-radius: 999px;
    color: var(--fg-2);
    font-size: 12.5px;
    font-weight: 550;
    letter-spacing: 0.01em;
    white-space: nowrap;
    isolation: isolate;
    transition: transform var(--dur-fast) var(--ease-out);
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
    background: rgba(255, 255, 255, 0.07);
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.08);
    opacity: 0;
    transition: opacity var(--dur-med) var(--ease-out);
  }

  .chip:hover .chip-bg {
    opacity: 0.55;
  }

  .chip.active .chip-bg {
    opacity: 1;
    background:
      linear-gradient(
        180deg,
        color-mix(in srgb, var(--accent) 34%, transparent) 0%,
        color-mix(in srgb, var(--accent) 18%, transparent) 100%
      );
    box-shadow:
      inset 0 0 0 1px color-mix(in srgb, var(--accent) 55%, transparent),
      0 6px 20px color-mix(in srgb, var(--accent) 18%, transparent);
  }

  .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    opacity: 0.9;
  }

  .count {
    font-size: 10.5px;
    font-variant-numeric: tabular-nums;
    color: var(--fg-2);
    background: rgba(255, 255, 255, 0.06);
    border-radius: 999px;
    padding: 1px 6px;
    min-width: 20px;
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
