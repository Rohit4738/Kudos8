# Kudos

A lightweight, controller-first Windows launcher for the games you already own on **Steam, Epic and
GOG** — one PS5-style library, one Play button.

Built with **Tauri 2 (Rust)** + **Svelte 5** + **TypeScript**. No Electron, no bundled browser, no
account, no telemetry.

![Kudos icon](src-tauri/icons/128x128.png)

## What it does

- **One library.** Steam is discovered from the registry, `libraryfolders.vdf` and every
  `appmanifest_*.acf`; Epic from its launcher manifests; GOG from the registry keys written by GOG
  Galaxy/installers. Missing store clients simply produce an empty section.
- **Instant covers.** Steam cover art is read from the local `appcache/librarycache` first (works
  offline), then the Steam CDN `library_600x900.jpg`, then `header.jpg`, then a platform-tinted
  gradient with the game's initials. GOG uses its local `GOG.ico` when present.
- **Launch the real client.** Steam games open through `steam://rungameid/<appid>` so Cloud, overlay
  and playtime keep working. Epic uses its launcher URI, GOG launches the installed executable.
- **Add to Steam.** GOG/non-Steam games can be written into the active profile's `shortcuts.vdf` as
  a standard non-Steam shortcut. The writer preserves every existing entry and unknown field, makes
  a `.kudos-backup`, refuses unparseable files, is idempotent, and **refuses to write while Steam is
  running** (Steam would overwrite the change on exit).
- **Living-room UI.** PS5-style portrait rail, ambient cover crossfade, keyboard/gamepad navigation
  (arrows/stick + Enter + Esc), reduced-motion support, accent color and borderless fullscreen.
- **Small shell.** ~102 KB of JS (35 KB gzip), ~3.4 MB native executable, no HTTP client or polling
  in the app.

## Status — real vs fixture-only

| Area | State on this machine | Evidence |
| --- | --- | --- |
| Steam scan + launch | **Real.** 3 games found (730, 12150, 413150), local covers resolved. | `registry.rs` integration test: `3 local, 0 remote, 3 games total`; `rust:test` 35/35. |
| Steam Add-to-Steam writer | **Verified with fixtures**, live write **pending**. Steam was running during the final pass, so Kudos correctly refused instead of killing it. | Binary-VDF round-trip, backup, idempotency and running-refusal tests. |
| Epic scan + launch | **Fixture-only.** Epic Games Launcher is not installed on this machine. | Manifest parser tests + `epic-fortnite.item` fixture. |
| GOG scan + launch | **Fixture-only.** GOG Galaxy / GOG games are not installed on this machine. | Registry record mapping, `GOG.ico` and launch-argument tests. |
| Steam local cover asset scope | **Real.** Exact `librarycache` files are granted at runtime. | `list_games` integration test asserts absolute, existing paths inside `librarycache`. |
| NSIS installer | **Real.** `Kudos_0.1.0_x64-setup.exe`. | Built and inspected 2026-10-28. |

**No live `shortcuts.vdf` write happened in this pass.** To verify manually: close Steam completely,
launch Kudos, open a GOG/non-Steam game, choose **Add to Steam**, confirm the “Restart Steam…” toast,
then open Steam → Library. The new shortcut should be there. Re-adding the same game must not create
a duplicate.

## Build

Use the npm wrappers. They load the MSVC compiler and Windows SDK environment; a bare `cargo build`
will fail with `linker link.exe not found` on a clean shell.

```bash
npm install
npm run tauri dev      # dev window with hot reload
npm run tauri build    # optimized exe + NSIS installer
```

Outputs:

- `src-tauri/target/release/kudos.exe` — ~3.4 MB
- `src-tauri/target/release/bundle/nsis/Kudos_0.1.0_x64-setup.exe` — ~1.4 MiB
- Product name / publisher / description: **Kudos**, version **0.1.0**, per-user install.

First build needs Rust (MSVC), Node 24+, and the Windows SDK. On a machine without the SDK, run
`npm run setup:windows-sdk` once.

## Checks

```bash
npm run check        # Svelte + TypeScript (0 errors / 0 warnings)
npm run rust:check   # cargo check
npm run rust:test    # unit + real-machine registry integration tests
npm run build        # production frontend bundle
```

## Preview without the desktop shell

The browser build shows honest “desktop app required” copy for Steam detection and launching. To
preview the full library with fixture-like sample data, open the dev server with an explicit query
parameter:

```text
http://localhost:1420/?mock=1
```

Mock mode is **opt-in only** — there is no dev-mock default and no localStorage fallback. Real builds
call the Tauri commands through `src/lib/api.ts`.

## RAM reality

Measured release build, idle on Windows 11, 2026-10-28:

- Native `kudos.exe`: **~6.3 MB** private.
- WebView2 subprocesses: **~175 MB** private for the empty/placeholder shell; **~244 MB** in the
  final build with three 600×900 local Steam covers decoded. That is the Edge WebView2 runtime and
  GPU floor, not Rust app growth.

Budget rule: no polling, no big JS libraries, no unbounded image cache. A strict sub-100 MB total is
not achievable with the fixed WebView2 stack; that is an intentional product trade-off, not a bug.

## Project map

- `src/` — Svelte 5 UI: `App.svelte` shell, `pages/` screens, `lib/api.ts` typed Tauri calls,
  `lib/components/` art, toasts and icons.
- `src-tauri/src/` — Rust: `commands.rs` (four-command API), `models.rs` (shared `Game`),
  `state.rs` (source registry), `sources/` (Steam / Epic / GOG).
- `src-tauri/tests/` — fixtures plus the real-machine registry integration test.
- **`TARGET.md` — the frozen contracts, final status, known issues and manual verification checklist.**

## Known limitations

- Steam games with no local `appmanifest_*.acf` cannot be listed offline (never-installed titles).
- Local Steam cover scope is limited to the default and known library `appcache/librarycache` paths;
  unusual Steam layouts fall back to the CDN.
- Epic cover art is intentionally `null` (gradient fallback); the manifest has no stable cover URL.
- GOG is registry-based, so library-only (not installed) GOG games do not appear.
- Live Add-to-Steam verification still needs a manual pass with Steam closed.
- Windows 10/11 x64 only.

## License

MIT. Bundled Inter font is licensed under the SIL Open Font License.
