<!--
  API KEY GUIDE (Session M) - one component, two providers.

  Shows exactly how to get the key, without sending the user hunting through
  docs. The link opens in the default browser through the Tauri opener plugin;
  in a browser preview it falls back to a normal anchor.
-->
<script lang="ts">
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { isTauriRuntime } from "../art";

  interface Props {
    provider: "steam" | "steamgriddb";
    /** Render expanded from the start (used by the no-key empty state). */
    open?: boolean;
  }

  let { provider, open = false }: Props = $props();
  let expanded = $state(false);

  // `open` is only a starting hint (used by the no-key empty state); the user
  // can still collapse the guide afterwards.
  $effect(() => {
    if (open) expanded = true;
  });

  const CONTENT = {
    steam: {
      label: "Steam Web API",
      url: "https://steamcommunity.com/dev/apikey",
      linkText: "steamcommunity.com/dev/apikey",
      free: "Free. No purchase, no card.",
      steps: [
        "Open the link below and sign in with your Steam account.",
        'In "Domain Name" type localhost - Valve does not verify it.',
        "Tick the agreement, press Register, then copy the 32-character key.",
        "Paste it above and press Save. Kudos stores it encrypted on this PC.",
      ],
      notes: [
        "For the full library, set Steam > Profile > Edit Profile > Privacy > Game details to Public.",
        "Leaked it? Revoke or regenerate on the same page - it is instant and free.",
      ],
    },
    steamgriddb: {
      label: "SteamGridDB",
      url: "https://www.steamgriddb.com/profile/preferences/api",
      linkText: "steamgriddb.com/profile/preferences/api",
      free: "Free. Needs a SteamGridDB account (Steam login).",
      steps: [
        "Sign in to SteamGridDB with Steam.",
        "Open Profile > Preferences > API and generate a personal key.",
        "Copy the key, paste it above and press Save.",
        "Open any GOG or non-Steam game and choose Change artwork.",
      ],
      notes: [
        "Artwork is community-made and provided by SteamGridDB.",
        "Kudos downloads each image once and caches it locally.",
      ],
    },
  } as const;

  const info = $derived(CONTENT[provider]);

  async function follow(event: MouseEvent) {
    if (!isTauriRuntime()) return; // normal anchor behaviour in a browser
    event.preventDefault();
    try {
      await openUrl(info.url);
    } catch {
      /* opening a browser is best-effort; the link text is still visible */
    }
  }
</script>

<div class="guide" class:expanded>
  <button
    type="button"
    class="toggle"
    aria-expanded={expanded}
    onclick={() => (expanded = !expanded)}
  >
    <span class="chev" aria-hidden="true">›</span>
    How do I get this key?
    <span class="free">{info.free}</span>
  </button>

  {#if expanded}
    <div class="body">
      <ol>
        {#each info.steps as step, index}
          <li><span class="num">{index + 1}</span>{step}</li>
        {/each}
      </ol>
      <a class="link" href={info.url} onclick={follow} rel="noreferrer">
        {info.linkText}
      </a>
      <ul class="notes">
        {#each info.notes as note}
          <li>{note}</li>
        {/each}
      </ul>
    </div>
  {/if}
</div>

<style>
  .guide {
    margin-top: 10px;
    border: 1px solid var(--line, rgba(255, 255, 255, 0.08));
    border-radius: var(--radius-sm);
    background: rgba(255, 255, 255, 0.02);
    overflow: hidden;
  }

  .toggle {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 10px 12px;
    border: 0;
    background: transparent;
    color: var(--fg-1);
    font: inherit;
    font-size: 12.5px;
    text-align: left;
    cursor: pointer;
  }

  .toggle:hover {
    color: var(--fg-0);
  }

  .toggle:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }

  .chev {
    display: inline-block;
    font-size: 15px;
    line-height: 1;
    transition: transform var(--dur-fast, 140ms) var(--ease-out, ease);
  }

  .expanded .chev {
    transform: rotate(90deg);
  }

  .free {
    margin-left: auto;
    font-size: 11px;
    color: var(--fg-2);
  }

  .body {
    padding: 4px 14px 14px 34px;
    animation: reveal 180ms var(--ease-out, ease);
  }

  ol,
  ul {
    margin: 0;
    padding: 0;
    list-style: none;
  }

  li {
    position: relative;
    padding: 4px 0;
    font-size: 12px;
    line-height: 1.55;
    color: var(--fg-1);
  }

  ol li {
    display: flex;
    gap: 8px;
  }

  .num {
    flex: 0 0 16px;
    height: 16px;
    margin-top: 2px;
    border-radius: 50%;
    background: rgba(var(--accent-rgb), 0.18);
    color: var(--accent);
    font-size: 10px;
    font-weight: 700;
    line-height: 16px;
    text-align: center;
  }

  .link {
    display: inline-block;
    margin: 8px 0 4px;
    color: var(--accent);
    font-size: 12px;
    text-decoration: none;
    border-bottom: 1px solid rgba(var(--accent-rgb), 0.35);
  }

  .link:hover {
    border-bottom-color: var(--accent);
  }

  .notes {
    margin-top: 6px;
    padding-top: 6px;
    border-top: 1px solid rgba(255, 255, 255, 0.06);
  }

  .notes li {
    color: var(--fg-2);
    font-size: 11.5px;
  }

  .notes li::before {
    content: "- ";
  }

  @keyframes reveal {
    from {
      opacity: 0;
      transform: translateY(-4px);
    }
    to {
      opacity: 1;
      transform: none;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .body {
      animation: none;
    }
    .chev {
      transition: none;
    }
  }
</style>
