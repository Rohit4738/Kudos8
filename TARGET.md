# TARGET — Kudos

> **Read this file before writing any code. Update your Status row before you finish.**
> Owner of this file: **Session A (architect)**. Sections may be appended to; contracts may not be
> changed silently. **Final integration state, known issues and the manual checklist are in §10–§12.**

---

## 1. PROJECT GOAL

A **lightweight Windows game launcher with a PS5-style full-screen library**. It finds the games
already installed from **Steam, Epic and GOG**, shows them in one grid with cover art, and launches
any of them from a single "Play" button — no store client hopping. Anyone can also add a non-Steam
game to their Steam library from the game detail page.

Non-goals: game downloads, store browsing, accounts, cloud sync, social features.

Fixed stack (do not change): **Tauri 2 (Rust) + Svelte 5 + TypeScript**, Windows 10/11, x64.

---

## 2. FILE MAP — WHO OWNS WHAT

Only the owner edits a file. If you need a change in someone else's file, write it in the Notes
column of the task board (and, if urgent, leave a `// TODO(<session>):` comment at the exact line).

| Path | Owner | Purpose |
| --- | --- | --- |
| `TARGET.md` | **A** | This contract. Others may only update their own Status/Notes cells. |
| `package.json`, `vite.config.ts`, `tsconfig.json`, `svelte.config.js`, `index.html` | **A** | Build config. |
| `scripts/*.mjs` | **A** | Build wrappers (MSVC env, Windows SDK, cargo, tauri). |
| `src/main.ts`, `src/app.css`, `src/App.svelte` | **A** | App bootstrap, design tokens, shell + rail navigation. |
| `src/lib/types.ts`, `src/lib/api.ts`, `src/lib/router.svelte.ts` | **A** | Frontend contract: `Game` type, command wrappers, router. |
| `src-tauri/Cargo.toml`, `build.rs`, `tauri.conf.json`, `capabilities/`, `icons/` | **A** | Tauri config and dependencies. |
| `src-tauri/src/main.rs`, `lib.rs`, `models.rs`, `error.rs`, `state.rs`, `commands.rs`, `launch.rs` | **A** | Command layer + registry. |
| `src-tauri/src/sources/mod.rs` | **A** | `LibrarySource` trait + module list. |
| `src-tauri/src/sources/steam.rs` (+ any new `src-tauri/src/steam/**` files) | **B** | Steam scanning, launching, Steam shortcut creation, Steam status. |
| `src-tauri/src/sources/epic.rs` (+ `src-tauri/src/epic/**`) | **C** | Epic scanning + launching. |
| `src-tauri/src/sources/gog.rs` (+ `src-tauri/src/gog/**`) | **C** | GOG scanning + launching. |
| `src/pages/Home.svelte` | **D** | Library grid / hero. |
| `src/pages/GameDetail.svelte` | **D** | Detail + Play + Add to Steam. |
| `src/pages/Settings.svelte` | **D** | Steam status, about, future toggles. |
| `src/lib/components/**`, `src/styles/**` | **D** | New UI components and styles (create freely). |
| `src/lib/art.ts`, `src/lib/format.ts`, `src/lib/theme.ts`, `src/lib/windowPrefs.ts`, `src/lib/devMock.ts`, `src/lib/library.svelte.ts` | **D** | UI helpers: cover-art fallback, formatting, theme/window prefs, dev mock data, cached library store. |
| `public/fonts/**` | **D** | Bundled Inter Variable (SIL OFL) — no runtime font requests. |
| `README.md` | **A** | Human-facing quick start. |

Anything not listed: ask in Notes before creating it at the root of `src/` or `src-tauri/src/`.

---

## 3. SHARED CONTRACTS (do not break)

### 3.1 `Game` — the only shape the frontend sees

Rust: `src-tauri/src/models.rs` · TypeScript: `src/lib/types.ts` (must stay in sync)

```rust
pub struct Game {
    pub id: String,              // "<platform>:<store-id>", stable, e.g. "steam:730"
    pub platform: Platform,      // "steam" | "epic" | "gog"  (serialized lowercase)
    pub title: String,
    pub exe_path: Option<String>,
    pub launch_command: String,  // exe path or a protocol URI
    pub is_installed: bool,
    pub cover_image: Option<String>, // https URL or local path
    pub size_bytes: Option<u64>,
}
```

Wire format (camelCase, `None` omitted):

```json
{
  "id": "steam:730",
  "platform": "steam",
  "title": "Counter-Strike 2",
  "exePath": "D:\\Steam\\steamapps\\common\\Counter-Strike Global Offensive\\game\\bin\\win64\\cs2.exe",
  "launchCommand": "steam://rungameid/730",
  "isInstalled": true,
  "coverImage": "https://cdn.cloudflare.steamstatic.com/steam/apps/730/library_600x900.jpg",
  "sizeBytes": 38123456789
}
```

**ID rules (mandatory):** `steam:<appid>`, `epic:<AppName>`, `gog:<productId>`.
IDs must be stable across runs — the UI relies on them for selection and Add-to-Steam.

### 3.2 `LibrarySource` — one implementation per store

```rust
pub trait LibrarySource: Send + Sync {
    fn platform(&self) -> Platform;
    fn installed(&self) -> SourceResult<Vec<Game>>;
    fn launch(&self, game: &Game) -> SourceResult<()>;
}
```

`SourceResult<T> = Result<T, SourceError>` (`src-tauri/src/error.rs`). `SourceError` serializes to a
plain string for the frontend.

Rules:
- Store client not installed → `Ok(vec![])`. Only return `Err(Unavailable)` when it exists but is unreadable.
- Never panic in a source: commands run on the UI thread path and a panic kills the app.
- `launch(&game)` must reject games from another platform (`SourceError::Invalid`).
- Use `crate::launch::open(&game.launch_command)` to start things; it handles both `.exe` paths and
  shell URIs (`steam://`, `com.epicgames.launcher://`, `goggalaxy://`).

Sources are registered once in `src-tauri/src/state.rs` (`AppState::new`). Frontend never sees them.

### 3.3 Tauri commands — the whole frontend API

| Command | Rust signature | Returns | Behavior |
| --- | --- | --- | --- |
| `list_games` | `() -> Vec<Game>` | merged, title-sorted list | Never rejects. A broken source is logged and skipped. |
| `launch_game` | `(game_id: String) -> SourceResult<()>` | `null` or error string | Resolves the owning source by id, then launches. |
| `add_to_steam` | `(game_id: String) -> SourceResult<()>` | `null` or error string | Creates a non-Steam shortcut via Steam. |
| `get_steam_status` | `() -> SteamStatus` | `{ installed, running, steamPath? }` | Steam client detection. |

From JS always go through `src/lib/api.ts`:

```ts
import { listGames, launchGame, addToSteam, getSteamStatus } from "../lib/api";
```

Tauri v2 camelCases arguments: Rust `game_id` ↔ JS `{ gameId }`. Do not rename either side.

### 3.4 Steam-only helpers (Session B surface)

`src-tauri/src/sources/steam.rs` must keep these free functions — `commands.rs` calls them by name:

```rust
pub fn add_to_steam_shortcut(game: &Game) -> SourceResult<()>;
pub fn steam_status() -> SteamStatus;
```

---

## 4. CURRENT STATE (Session A baseline — see §10 for the final build)

- Tauri 2.12 + Svelte 5.56 + Vite 8, plain Svelte (no SvelteKit). Bundle ~48 KB JS.
- Shell: left rail (Library / Settings), in-memory router, page fade transitions, design tokens in
  `src/app.css`, Esc = back.
- At baseline, pages were functional placeholders and sources were typed stubs. Sessions B–E
  replaced the source bodies and visuals; the contracts in §3 were never changed.
- Registry, commands, error type, launch helper: complete and frozen (change via Notes).

Verified build evidence (Session A, 2026-10-27): `npm run tauri dev` opened a window titled
"Project Kudos"; `npm run tauri build` produced
`src-tauri/target/release/kudos.exe` (3.1 MB) and an NSIS installer (1.14 MB).

---

## 5. BUILD & RUN (this machine)

Node, Rust and the Windows SDK are already installed — but not via system installers:

- Node 24: `.tools/node-v24.22.1-win-x64` (on the user PATH; `.tools/` is git-ignored)
- Rust 1.99 MSVC: `C:\Users\shubham\.cargo\bin` (on the user PATH)
- Windows SDK import libs/headers + `rc.exe`: `.tools/winsdk*`, wired in automatically by
  `scripts/msvc.mjs`. Re-create with `npm run setup:windows-sdk` if `.tools/` is wiped.

**Always use the npm scripts — they load the MSVC/SDK environment for you:**

```bash
npm install                 # once
npm run tauri dev           # dev window with HMR
npm run tauri build         # release exe + NSIS installer
npm run check               # svelte-check (TS + a11y)
npm run rust:check          # cargo check (in src-tauri)
npm run rust:test           # cargo test
npm run rust -- <args>      # any other cargo subcommand
```

A bare `cargo build` in a normal shell fails with "linker `link.exe` not found" — expected, use
`npm run rust:check` instead. Do not "fix" `Cargo.toml` for this.

Dependencies: if you need a Rust crate or npm package, note it in the task board and let **A** add it
to the shared `Cargo.toml` / `package.json`, unless A says otherwise. Keep dependencies minimal —
every crate is startup time and RAM.

---

## 6. PERFORMANCE BUDGET

- 60 fps: animate only `transform` / `opacity`; no layout thrash; no JS animation libraries.
- UI must render before the first scan finishes: show cached/empty state immediately, never block on
  `list_games`.
- Scans are I/O-bound and must not freeze the UI. No `sleep` loops. If a source needs a background
  watcher (Steam library changes), keep it opt-in and cheap.
- Keep JS under ~200 KB uncompressed and avoid UI frameworks.
- **Measured RAM (release build, idle):** native `kudos.exe` **~6.3 MB** private; WebView2
  subprocesses **~175 MB** private for the empty/placeholder shell and **~244 MB** in the final build
  with three local 600×900 Steam covers decoded on this machine. That is the WebView2/Edge runtime and
  GPU floor, not our code. Budget rule: our code must not *grow* it meaningfully (no polling, no big
  JS libs, no unbounded image cache). If a strict <100 MB total is required, that conflicts with the
  fixed WebView2 stack — raise it with the team lead instead of shaving features.

---

## 7. TASK BOARD

Status values: `NOT STARTED` · `IN PROGRESS` · `DONE`.
Update **your** row(s) after every milestone; add technical findings/blockers to Notes.

| # | Task | Owner | Status | Notes |
| --- | --- | --- | --- | --- |
| A1 | Scaffold Tauri 2 + Svelte 5 + TS, working `tauri dev` and `tauri build` | A | DONE | Window verified; exe + NSIS installer produced. |
| A2 | Shared `Game` / `Platform` models + TS mirror | A | DONE | `models.rs` ↔ `lib/types.ts`. |
| A3 | `LibrarySource` trait, registry, error type, launch helper | A | DONE | `sources/mod.rs`, `state.rs`, `error.rs`, `launch.rs`. |
| A4 | Tauri commands + typed frontend API | A | DONE | `list_games`, `launch_game`, `add_to_steam`, `get_steam_status`. |
| A5 | Stub modules + app shell + 3 placeholder pages | A | DONE | B/C/D replace stub bodies only. |
| A6 | TARGET.md, build wrappers, commit | A | DONE | Baseline commit includes Session D's early `src/lib/*` helpers and `public/fonts/**` that were on disk at commit time; D keeps ownership. |
| B1 | Parse Steam `libraryfolders.vdf` + `appmanifest_*.acf`; return real games | B | DONE | `steam.rs`: modern + legacy `libraryfolders.vdf`; every `appmanifest_*.acf` in every library; `steam:<appid>` ids; skips 228980. `StateFlags & 4` ⇒ installed; other flags returned with `is_installed=false` and a valid `steam://rungameid/<appid>`. Cache per manifest `(mtime,len)` + libraryfolders stamp, pruned on removal. Real machine: 3 games (730/12150/413150), cold 2.5 ms, warm 0.35 ms (debug). Limit: a never-installed title has no local manifest and cannot be listed offline; “not downloaded” = manifest exists but `StateFlags&4==0`. |
| B2 | Steam covers: `library_600x900.jpg` URL pattern, local `librarycache` fallback | B | DONE | **Updated by E:** returns the local `appcache/librarycache/<appid>/library_600x900.jpg` (or flat `_library_600x900` layout) when present, else the CDN URL. Listing still does zero network I/O. Tauri asset protocol is enabled and `list_games` grants each exact local cover at runtime (`asset_protocol_scope().allow_file`); `GameArt.svelte` retries CDN 600x900 → `header.jpg` → gradient. Real machine: 3/3 games use local covers (integration test). |
| B3 | Steam launch via `steam://rungameid/<appid>` with exe fallback | B | DONE | Prefers `steam://rungameid/<appid>` derived from the stable id (Steam's canonical form; `steam://run/<appid>` also works). Falls back to `exe_path` then `launch_command` for hand-built `Game` values; rejects other platforms with `Invalid`. ACF has no exe field (only `installdir`), so Steam games legitimately have `exePath=None`. |
| B4 | `steam_status()`: registry `HKCU\Software\Valve\Steam\SteamPath` + running process | B | DONE | `winreg` (Session C's existing dep; no new crate): HKCU `SteamPath`, then HKLM WOW6432Node + 64-bit `InstallPath`, then default dirs; path normalised to `\`. Running check = `tasklist /FI "IMAGENAME eq steam.exe" /FO CSV` with `CREATE_NO_WINDOW`. Real machine: `installed=true running=true steamPath="C:\Program Files (x86)\Steam"`. |
| B5 | `add_to_steam_shortcut`: write `shortcuts.vdf`, restart notice | B | DONE | Full binary-VDF tree read-modify-write: existing entries + unknown fields preserved byte-for-byte; old short and new long entry formats supported; only missing standard fields are added, so icon/LaunchOptions/tags are never clobbered. Backup `shortcuts.vdf.kudos-backup`, atomic tmp+rename, refuses to touch an unparseable file. Active profile = `loginusers.vdf` MostRecent/newest `timestamp` → `userdata/<accountid>`, fallback newest profile. appid = `CRC32(exe+AppName) OR 0x80000000` (2.1B+, deterministic, collision-bumped); re-add is idempotent. Steam running → clear `Invalid` error (“Close Steam completely…”). Quirks: file ends `08 08`; appid is int32 (prints negative); strings UTF-8 + NUL; types 0x00–0x08 + 0x0A supported, unknown type refuses write; this Steam build has no `MostRecent`, only `timestamp`. Ok path is `null` (contract), so restart hint is stderr only — UI must say “Restart Steam”. |
| B6 | Unit tests for VDF/ACF parsing with committed fixtures | B | DONE | 17 Steam tests + 8 fixtures under `src-tauri/tests/fixtures/steam/` (modern/legacy libraryfolders, installed/not-downloaded/redistributable ACFs, loginusers, 13-byte empty shortcuts.vdf, 556-byte 2-entry old+new-format shortcuts.vdf). Covers exact binary round-trip, unknown-field preservation, idempotent add, backup, unparseable refusal, appid range, running refusal, real-machine smoke. Evidence: `npm run rust:test` 34/34 pass (17 Steam); `npm run rust:check` clean; `npm run check` 0 errors/0 warnings; `npm run build` passes (101 KB JS); `cargo build` links. |
| C1 | Parse Epic manifests `%ProgramData%\Epic\EpicGamesLauncher\Data\Manifests\*.item` | C | DONE | `epic.rs`: BOM-tolerant JSON; skips `bIsIncomplete`/`bIsIncompleteInstall`, `bIsApplication:false`, DLC/plugin/asset/UE categories, missing launch target, stale install dirs. Quirks handled: both incomplete flags, UTF-8 BOM, `InstallSize` as number or numeric string, `LaunchExecutable` relative or absolute. Duplicate AppName → newest BuildVersion (fallback AppVersionString) wins. Cache key = manifest count/size/mtime. Measured 60 manifests: cold 23.6 ms, cache hit 0.55 ms (test asserts <300 ms). `Game` has no build-version field, so BuildVersion is used only for dedupe. |
| C2 | Epic launch via `com.epicgames.launcher://apps/<AppName>?action=launch&silent=true` | C | DONE | URI preferred; when the launcher is absent, spawns `exePath` with the install dir as working directory. `coverImage` is null (UI placeholder), per approved scope. |
| C3 | GOG discovery via Galaxy DB or registry uninstall entries | C | DONE | Approved scope uses registry, not Galaxy DB: HKLM `SOFTWARE\WOW6432Node\GOG.com\Games`, then HKLM 64-bit view, then HKCU `Software\GOG.com\Games`. Ids `gog:<gameId>` (subkey name fallback). Quirks handled: `gameId`/`size` as REG_SZ or DWORD/QWORD, `exe` relative or absolute, `launchParam` with quotes, `workingDir` separate from install `path`. 5 s TTL cache; stale entries filtered by install dir/exe existence; `GOG.ico` local path when usable else null. Library-only (not installed) GOG games are not covered. Added `winreg = "0.56.0"` to Cargo.toml (rule 6 dependency note). |
| C4 | GOG launch via `goggalaxy://openGameView/<id>` with exe fallback | C | DONE | Per approved scope, launches the exe directly with install/working dir and honours `launchParam` (double-quote aware); registry is re-read at launch. `goggalaxy://` intentionally not used. |
| C5 | Missing-client behavior: `Ok(vec![])`, no error spam | C | DONE | Epic: missing manifest dir → `Ok([])`; only a present-but-unreadable dir returns `Unavailable`. GOG: missing/denied registry keys → `Ok([])` plus one stderr warning, never panics. `npm run rust:check` clean. |
| C6 | Unit tests for manifest/registry parsing | C | DONE | 17/17 tests pass via `npm run rust:test`, including scan speed/cache invalidation and duplicate-manifest resolution; Epic fixture at `src-tauri/tests/fixtures/epic-fortnite.item`. Final gates: `npm run rust:check` clean, `npm run check` 0 errors/0 warnings, `npm run build` passes, `npm run tauri build --no-bundle` produced `src-tauri/target/release/kudos.exe`. |
| D1 | PS5-style Home: hero row for recently played + grid, hover/focus states, 60 fps | D | DONE | PS5 rail instead of grid: 2:3 portrait tiles, active scale + static glow ring, dimmed neighbours, ambient cover crossfade, All/Steam/Epic/GOG filters. `Game` has no play-history field, so no "recently played" hero row. `npm run build` passes. |
| D2 | GameDetail redesign: cover hero, Play (launch), Add to Steam, size/platform facts | D | DONE | Cover hero + facts. PLAY -> launch overlay; GOG Add to Steam idle/loading/success/"Close Steam first" toast (matches B5's real "Close Steam completely." error); success hints a restart. Enter opens, Esc backs. Browser `?mock=1` simulates both flows; real calls go through `api.ts`. |
| D3 | Settings: Steam status card, source availability, about, version | D | DONE | Accent presets + custom hex + Reset (`localStorage kudos.accent`); borderless fullscreen via Tauri Window API (`kudos.fullscreen`); Steam status/refresh with honest browser fallback; about/version. Verified in browser preview; fullscreen no-ops outside Tauri by design. |
| D4 | Loading / empty / error states for "no games found" | D | DONE | Loading skeleton; error card + Retry; filtered "No games found" with Refresh / Show all games; refresh invalidates cache. Missing IPC bridge in a plain browser is shown as human copy, not a raw TypeError. |
| D5 | Cover-art fallback (gradient + initial) and broken-image handling | D | DONE | Platform-tinted gradient + initials placeholder, image fade-in and `onerror` fallback (no broken icons). `art.ts` uses `convertFileSrc` for local paths. **Updated by E:** `app.security.assetProtocol` is enabled with a minimal static Steam `librarycache` scope; `list_games` additionally grants exact GOG.ico / non-default Steam cover files at runtime. Final chain: local file → CDN `library_600x900` → CDN `header` → gradient. |
| D6 | Keyboard/gamepad-style navigation (arrows + Enter + Esc) | D | DONE | Home arrows/A-D, Up-Down/W-S filter, Home/End, Enter/Space; detail Enter/X/B; Esc via shell. Gamepad: d-pad/left stick with repeat + face buttons, one rAF loop. Mouse hover browses without rail glide. |
| D7 | Polish pass: transitions, reduced-motion, a11y labels, focus rings | D | DONE | Reduced-motion, focus-visible rings, aria labels, transform/opacity only, `will-change` on animated nodes. Browser smoke 1600x1000 (`?mock=1`): 10 tiles, store filters, 2-layer info crossfade with stable height, accent persists across navigation, 0 console errors. `npm run check` 0 errors / 0 warnings; `npm run build` passes. New npm dep `@fontsource-variable/inter@5.2.8` vendors Inter woff2 into `public/fonts/` (no runtime font requests). |
| E1 | Close Rust↔Svelte wiring gaps; audit frozen API/Game contracts | E | DONE | Audited `api.ts`, all pages and Rust commands. Removed the dev-mock default and localStorage fallback: mock data is **only** `?mock=1`. `list_games` still never rejects; success toast now says “Restart Steam to see the new shortcut in your Library.” `npm run check` 0/0. |
| E2 | Local Steam/GOG cover art via asset protocol | E | DONE | `protocol-asset` feature + minimal static scope; runtime `allow_file` for exact files; `steam_local_cover()` prefers `librarycache`; `GameArt` chain local → CDN → gradient. Integration test: 3 local covers, 0 remote, 3 Steam games. |
| E3 | Packaging: “Kudos” product identity, custom icon, NSIS metadata | E | DONE | Generated full Windows/iOS/Android icon set from `src-tauri/icons/app-icon.svg`; copied 128px to `public/favicon.png`. `productName`/window title/publisher/description/start-menu folder = **Kudos**; version 0.1.0; per-user NSIS, English, LZMA. Built `Kudos_0.1.0_x64-setup.exe` (1.36 MiB); exe 3.39 MB, metadata ProductName/CompanyName/FileVersion correct. |
| E4 | Real-machine registry integration test | E | DONE | `src-tauri/tests/registry.rs`: all 3 platforms registered once; ids namespaced/unique/resolvable through `find_game`; every local cover absolute, existing and (for Steam) inside `librarycache`. Evidence: `3 local, 0 remote, 3 games total`; 38 total Rust tests pass. |
| E5 | Final gates + documentation | E | DONE | `npm run check` 0 errors/0 warnings; `npm run rust:check` clean; `npm run rust:test` 38 pass; `npm run build` 101.77 KB JS (35.08 KB gzip); `npm run tauri build` green. README rewritten; §10–§12 record final status, known issues and the manual checklist. |
| E6 | Add-to-Steam live verification | E | **BLOCKED** | Steam was running (PID 7696) during the final pass. Per the rule “do not kill Steam / do not write live shortcuts while it runs”, the app was only verified to refuse (test `refuses_to_add_while_steam_is_running`) and the writer verified against fixtures (round-trip, backup, idempotency). Manual check required — see §12. |

---

## 8. TEAM RULES

1. **Read TARGET.md before you start.** Re-read section 3 before touching Rust/frontend boundaries.
2. **Update your Status column after each milestone** (`NOT STARTED` → `IN PROGRESS` → `DONE`).
   A task is only `DONE` when its verification command passed.
3. **Never edit another session's files** (see FILE MAP) without writing the reason, the file and the
   exact change in Notes first. If it is urgent, add `// TODO(<owner>):` at the change site.
4. **Do not change shared contracts** — `Game`, `Platform`, `LibrarySource`, command names/args,
   `src/lib/api.ts` signatures. They are frozen. If a contract must change, write it in Notes and wait
   for A to update both sides.
5. **Keep the build green.** Before finishing a session run:
   `npm run check` and `npm run rust:check` (plus `npm run tauri build` if you touched Rust).
   A stub that compiles beats a feature that doesn't.
6. **No new dependencies without a Notes entry.** No UI framework, no state library, no router
   library, no HTTP client crate.
7. **No secrets, no telemetry, no network calls at startup.** Cover images are loaded by the WebView
   from returned URLs only.
8. **Windows-only paths.** Use `std::env::var("ProgramData")` / `dirs`-free resolution helpers where
   possible; guard every file read so a missing folder returns `Ok(vec![])`.
9. **Commit small, one concern per commit.** Include `Session X:` in the commit subject.
10. Stuck for more than ~20 minutes on someone else's contract? Write the blocker in Notes and move to
    a task you own.

---

## 9. DEFINITION OF DONE (per session)

- `npm run check` → `0 errors, 0 warnings`.
- `npm run rust:check` → clean.
- `npm run tauri dev` opens the window and the changed flow works (no console panic).
- Your row(s) on the task board updated with evidence (command + result).
- No files outside your ownership changed without a Notes entry.

---

## 10. SESSION E — FINAL INTEGRATION STATUS

Final pass: **2026-10-28**. Product name **Kudos**, version **0.1.0**, Windows 10/11 x64.

**Gates (all green):**

| Command | Result |
| --- | --- |
| `npm run check` | 0 errors, 0 warnings |
| `npm run rust:check` | clean |
| `npm run rust:test` | **38 passed, 0 failed** (35 unit + 3 registry integration) |
| `npm run build` | 101.77 KB JS / 35.08 KB gzip; 34.02 KB CSS / 7.24 KB gzip |
| `npm run tauri build` | `kudos.exe` 3.39 MB; `Kudos_0.1.0_x64-setup.exe` 1.36 MiB |
| Registry integration | `3 local, 0 remote, 3 games total` |

**Wiring closed in this pass:**

- `list_games(app, state)` now grants local cover files through the asset protocol; the static scope
  in `tauri.conf.json` is limited to default Steam `librarycache` globs, and non-default files are
  allowed one-by-one at runtime.
- Steam scanning prefers `librarycache` local art (offline-first), then the frontend retries the CDN
  and finally the gradient.
- Mock data is explicit only (`?mock=1`). No `import.meta.env.DEV` default, no localStorage fallback.
- Add-to-Steam success copy explicitly tells the user to **restart Steam**; the backend still returns
  `null` per the frozen contract.
- Missing store clients render an empty, friendly section — no error cards, no console spam.
- Packaging: custom icon set, `Kudos` product/window/publisher metadata, per-user NSIS with LZMA,
  English-only installer, `Kudos` start-menu folder.
- README rewritten; this file records the final evidence, known issues and manual checklist.

**Commits:** `2dd03dc Session E: wire local Steam/GOG cover art through the asset protocol`, followed by
the packaging/docs commit(s) from this pass.

---

## 11. KNOWN ISSUES

1. **Live Add-to-Steam write is not verified end-to-end.** Steam was running (PID 7696) throughout the
   final pass. Per the safety rule, Kudos did not kill it and did not write a live `shortcuts.vdf`. The
   writer is proven by fixtures (round-trip, backup, idempotency, unknown-field preservation) and the
   running-refusal path is proven by test. Manual step in §12 is required.
2. **Epic and GOG are fixture-only on this machine.** Neither client is installed. Their scanners,
   launchers, cover/ico handling and error paths are covered by unit tests and committed fixtures, but
   no real Epic/GOG game was launched or added to Steam here.
3. **Steam library listing is manifest-based.** A never-installed title has no local
   `appmanifest_*.acf` and cannot be listed offline.
4. **Local cover scope is deliberately narrow.** Default Steam library folders are in the static
   asset-protocol scope; covers in unusual Steam layouts are granted individually at runtime and can
   fall back to the CDN if the file is unreadable.
5. **Epic has no cover art.** `coverImage` is `null` by design; the UI shows the gradient/initials
   placeholder.
6. **GOG is registry-based.** Library-only (not installed) GOG games do not appear.
7. **Installer is unsigned.** Windows SmartScreen may warn on first run; there is no code-signing
   certificate configured. This is a release-engineering decision, not a bug.
8. **No filesystem watcher.** Library changes require Refresh (Home) or a restart; this is intentional
   (no polling, low RAM).
9. **RAM floor.** ~6.3 MB native, ~175 MB WebView2 private minimum, ~244 MB measured with three local
   covers decoded. A sub-100 MB target is incompatible with the fixed WebView2 stack.
10. **Backup file remains.** A successful write leaves `shortcuts.vdf.kudos-backup` next to the original
    by design; it is safe to delete manually.

---

## 12. MANUAL VERIFICATION CHECKLIST

Run on a Windows 10/11 x64 machine with Steam installed. Tick each item and note the date/build.

**A. Install and identity**

- [ ] Run `src-tauri/target/release/bundle/nsis/Kudos_0.1.0_x64-setup.exe`.
- [ ] Installer, Start-menu entry, window title and About card all say **Kudos** (not “Project Kudos”).
- [ ] Custom icon appears in the installer, Start menu, taskbar and window.
- [ ] App starts without a console panic; no error dialog.
- [ ] Uninstall removes the app and keeps user data (accent/fullscreen preferences).

**B. Steam scan and launch**

- [ ] Library shows the installed Steam games with local cover art (this machine: 730, 12150, 413150).
- [ ] Covers render with the network disconnected (local `librarycache` path works).
- [ ] Click **Play** on a Steam game → correct game opens through Steam.
- [ ] Refresh picks up a newly installed/removed Steam game.
- [ ] Filters (All/Steam/Epic/GOG), keyboard arrows/Enter/Esc and gamepad d-pad/face buttons work.
- [ ] Empty filter state shows friendly copy, not a crash.

**C. Missing clients**

- [ ] With Epic absent, the Epic filter shows an empty section with friendly copy (no error toast).
- [ ] With GOG absent, same for GOG.
- [ ] Browser preview without Tauri shows “desktop app required” copy, not a raw TypeError.
- [ ] `?mock=1` shows the sample library; omitting it never shows mock data.

**D. Add to Steam — refusal path (Steam running)**

- [ ] Start Steam, then launch Kudos.
- [ ] Open a non-Steam/GOG game and choose **Add to Steam**.
- [ ] Error explains “Steam is running… close Steam completely”; no success toast.
- [ ] `shortcuts.vdf` hash and mtime are unchanged; no `.kudos-tmp` file appears.

**E. Add to Steam — live write (Steam fully closed)**

- [ ] Exit Steam completely (tray included); confirm no `steam.exe` in Task Manager.
- [ ] Launch Kudos, open a GOG/non-Steam game, choose **Add to Steam**.
- [ ] Success toast says the shortcut was added and to **restart Steam**.
- [ ] `userdata/<account>/config/shortcuts.vdf.kudos-backup` exists.
- [ ] Re-add the same game → no duplicate entry; existing icon/LaunchOptions/tags preserved.
- [ ] Open Steam → Library → the new non-Steam shortcut is present and launches the game.
- [ ] Invalid/unparseable `shortcuts.vdf` (test copy) is refused, not overwritten.

**F. Settings and performance**

- [ ] Accent presets/custom color persist across restarts; Reset works.
- [ ] Borderless fullscreen toggles, and the preference is restored on next start.
- [ ] Steam status shows installed/running/path and Refresh updates it.
- [ ] Idle private RAM: native ~6 MB; WebView2 within the documented ~175–250 MB floor.
- [ ] No console errors in the browser preview and no repeated background work/polling.
