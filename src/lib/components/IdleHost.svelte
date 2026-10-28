<!--
  App-level host for the idle overlay. OWNER: Session G.
  Mount once near the root of the app; it renders nothing until idle fires.
  Keeping it separate means App.svelte only needs one line of integration.
-->
<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { loadRecent, recent } from "../history.svelte";
  import { exitIdle, idlePrefs, idleState, setIdleGate } from "../idle.svelte";
  import { library } from "../library.svelte";
  import IdleAmbient from "./IdleAmbient.svelte";

  onMount(() => {
    // Idle never opens onto an empty library; the watcher retries later.
    setIdleGate(() => library.games.length > 0);
    return () => setIdleGate(null);
  });

  $effect(() => {
    if (!idleState.active) return;
    if (library.games.length === 0) {
      exitIdle();
      return;
    }
    // Refresh timestamps when the screen opens. untrack keeps the effect from
    // re-running on every status/entry change inside the store.
    untrack(() => {
      void loadRecent();
    });
  });
</script>

{#if idleState.active}
  <IdleAmbient
    games={library.games}
    recent={recent.entries}
    settings={idlePrefs}
    onExit={exitIdle}
  />
{/if}
