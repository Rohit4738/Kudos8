<!-- Minimal ring spinner; rotates with transform only. OWNER: Session D. -->
<script lang="ts">
  let { size = 22 }: { size?: number } = $props();
</script>

<span class="spinner" style:--size={`${size}px`} aria-hidden="true"></span>

<style>
  .spinner {
    width: var(--size);
    height: var(--size);
    border-radius: 50%;
    border: max(2px, calc(var(--size) / 12)) solid rgba(255, 255, 255, 0.16);
    border-top-color: currentColor;
    display: inline-block;
    animation: spin 720ms linear infinite;
    will-change: transform;
    flex: 0 0 auto;
  }

  @keyframes spin {
    to {
      transform: rotate(1turn);
    }
  }
</style>
