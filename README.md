# Project Kudos

A lightweight, PS5-style Windows game launcher for the games you already own on **Steam, Epic and
GOG** — one library, one Play button.

Built with **Tauri 2 (Rust)** + **Svelte 5** + **TypeScript**.

## Quick start

```bash
npm install
npm run tauri dev      # dev window with hot reload
npm run tauri build    # release exe + NSIS installer
```

Output: `src-tauri/target/release/kudos.exe` and
`src-tauri/target/release/bundle/nsis/Project Kudos_<version>_x64-setup.exe`.

## Checks

```bash
npm run check        # TypeScript + Svelte (0 errors expected)
npm run rust:check   # cargo check
npm run rust:test    # cargo test
```

Use the npm wrappers, not bare `cargo`: they load the MSVC compiler / Windows SDK environment. If the
Windows SDK is missing on a new machine, run `npm run setup:windows-sdk` once.

## Where things live

- `src/` — Svelte UI: `App.svelte` shell, `pages/` screens, `lib/api.ts` typed Tauri calls.
- `src-tauri/src/` — Rust: `commands.rs` (API), `models.rs` (shared `Game`), `sources/` (one file per store).
- **`TARGET.md` — the team contract: file ownership, interfaces, task board, rules. Read it before coding.**

## Status

Skeleton is complete and builds. Steam, Epic and GOG scanners are typed stubs returning empty
libraries; the UI is a functional placeholder. Work is split across sessions B (Steam), C (Epic +
GOG) and D (UI) — see the task board in `TARGET.md`.
