# TARGET — Kudos

> **Read this file before writing any code. Update your Status row before you finish.**
> Owner of this file: **Session A (architect)**. Sections may be appended to; contracts may not be
> changed silently. **Final integration state, known issues and the manual checklist are in §10–§13.**

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
| `get_achievements` | `(game_id: String) -> AchievementSummary` | `{ supported, unlocked, total, reason? }` | **Additive (Session F, §3.5).** Local Steam achievement counts only. Never rejects, never touches the network, never blocks `list_games`. |
| `get_recent_games` | `(limit: u32) -> Vec<RecentGame>` | `{ id, lastPlayedUnix, source }[]` | **Additive (Session G, §3.6).** Steam `LastPlayed` merged with Kudos' launch journal, filtered to the current library, newest first. Never rejects; an empty list hides the row. |
| `get_steam_owned_games` | `() -> Vec<Game>` | uninstalled Steam catalog | **Additive (Session H, §15).** Owned-but-uninstalled apps from the active account's `localconfig.vdf`; local-only, never rejects, no network. Merge with `list_games`; installed ids are omitted. |
| `get_steam_playtime` | `() -> Vec<SteamPlaytime>` | `{ appId, minutes, twoWeeksMinutes, lastPlayed? }[]` | **Additive (Session H, §15).** Local Steam playtime, sorted by minutes desc. Never rejects, no network. |
| `get_steam_sync_status` | `() -> SteamSyncStatus` | `{ cached, appCount, fetchedAt?, ownedAppCount }` | **Additive (Session H, §15).** Reads only the local name cache; never rejects, no network. |
| `sync_steam_metadata` | `() -> Result<SteamSyncStatus, String>` | status or a human-readable error string | **Additive (Session H, §15).** The only Session H network call; user-triggered only, never at startup. Async; failure leaves the cache unchanged. |

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

### 3.5 `AchievementSummary` — additive contract (Session F)

Rust: `src-tauri/src/models.rs` · TypeScript: `src/lib/types.ts` (must stay in sync)

```rust
pub struct AchievementSummary {
    pub supported: bool,   // false = no local data / no achievements / unreadable
    pub unlocked: u32,
    pub total: u32,
    pub reason: Option<String>, // omitted on the wire when None
}
```

Wire format (camelCase, `None` omitted):

```json
{ "supported": true, "unlocked": 12, "total": 40 }
```

`get_achievements(game_id)` reads only Steam's local `appcache/stats` files
(`UserGameStats_<accountid>_<appid>.bin` for unlocked flags and
`UserGameStatsSchema_<appid>.bin` for achievement totals). There is no Steam Web API,
API key, or network request. The command returns `supported:false` (never an error)
for Epic/GOG ids, missing Steam profiles, missing files, or any parse doubt.

### 3.6 `RecentGame` — additive contract (Session G)

Rust: `src-tauri/src/steam_recent.rs` · TypeScript: `src/lib/recent.ts`

```rust
pub struct RecentGame {
    pub id: String,             // "<platform>:<store-id>"
    pub last_played_unix: u64,  // Unix seconds
    pub source: RecentSource,   // "steam" | "kudos"
}
```

Wire format (camelCase):

```json
{ "id": "steam:730", "lastPlayedUnix": 1792855921, "source": "steam" }
```

`get_recent_games(limit)` merges Steam's per-account `LastPlayed` from
`userdata/<accountid>/config/localconfig.vdf` with Kudos' own launch journal
(`%APPDATA%\Kudos\recent.json`, capped at 100 entries, atomic replace). Steam wins
for the same id; both lists are filtered to ids in the current library and ordered
newest first. `limit` is clamped to `1..=50`. The command never rejects. `Game`,
`Platform`, `LibrarySource` and every pre-existing command signature stay unchanged.

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
| F1 | Controller detection, live connect/disconnect, persisted layout override | F | DONE | New `src/lib/controller.svelte.ts`. Classifies `Gamepad.id` once per event (Xbox/XInput/Microsoft/045e, DualSense/DualShock/Sony/PS3-5/054c, Nintendo/Switch/Joy-Con/057e, else generic). Listens to `gamepadconnected` / `gamepaddisconnected` plus focus/visibility refresh — no polling and **no per-frame classification**; `gamepad.ts` keeps its single existing input rAF loop. No pad ⇒ keyboard hints. Override persisted at **`kudos.padLayout`**. Settings shows the raw pad name, detected layout and Auto/Xbox/PlayStation/Nintendo/Keyboard. Evidence: `?mock=1&pad=…` renders keyboard plus all pad families; Keyboard override survived reload; 13 smoke screenshots, 0 console errors. |
| F2 | Controller-aware button glyphs everywhere | F | DONE | New `PadGlyph.svelte` / `ControlHint.svelte`: full face set A/B/X/Y (Xbox coloured; Nintendo letters), PlayStation **✕/○/□/△ as four hand-written inline SVG paths** (no font dependency), generic shapes, keyboard keycaps. Nintendo physical mapping in `gamepad.ts`: confirm = button 1, back = button 0, secondary = button 3. Settings shows the full four-glyph face set for the active layout. Evidence: PlayStation footer = 4 SVGs / 0 `<kbd>`; keyboard footer = 0 SVGs / 4 `<kbd>`; 0 console errors. |
| F3 | Personalization: accents, background, density, sorting, hidden games | F | DONE | Extends `theme.ts`: 12 accent presets, background `ambient\|solid\|gradient`, tile density `compact\|default\|large` via HTML data-attributes + CSS vars, info-panel crossfade toggle. `windowPrefs.ts` adds startup page `home\|settings`; `library.svelte.ts` adds title/platform/installed-first sorting. Gradient uses an oversized `.backdrop` with a 26 s **transform-only** drift and `animation:none` under `prefers-reduced-motion`. Evidence: compact+gradient screenshot; startup reload lands on Settings; crossfade off leaves one panel; no console errors. |
| F4 | Hide-from-library UX + Settings unhide list | F | DONE | Right-click or 650 ms press on a Home tile hides it; detail page has Hide/Unhide; ids persist in **`kudos.hiddenGames`**; Settings lists hidden games with Unhide / Unhide all; Home header shows “n hidden”. No library contract change — hidden games stay in `library.games`. Evidence: hide took the rail from 10 → 9 tiles and showed “1 hidden”; Unhide all cleared the key. |
| F5 | `get_achievements` command + local Steam parser | F | DONE | **Additive contract change**, documented in §3.5. New `src-tauri/src/achievements.rs` parses `UserGameStats_<account>_<appid>.bin` (case-sensitive magic `cache`) + `UserGameStatsSchema_<appid>.bin` for totals; unlocked = `max(bitfield popcount, AchievementTimes keys)` per achievement group, clipped to total. Reuses B's `detect_steam_root` / `active_userdata_dir` (made `pub(crate)`). Cache per `(root, account, appid)` with both files' `(mtime,len)`; never called by `list_games`. Real machine: **`steam:730` 1/1 supported; `steam:12150` supported:false, “Steam has no local achievement data for this game.”; `steam:413150` 0/49 supported.** |
| F6 | Achievements UI on GameDetail | F | DONE | Steam: “Achievements — n / total unlocked” + subtle progress bar (mock: **18/45, 40%**); Epic/GOG: “Achievements not available for this platform”; Steam `supported:false`: renders nothing (no toast, no error card). Local read runs after paint and never blocks the library. Evidence: `f-detail-steam` (Cyberpunk 2077), `f-detail-epic`, `f-detail-gog`; 0 console errors. |
| F7 | Achievement parser tests + committed fixtures | F | DONE | 9 achievement tests + 5 committed binaries under `src-tauri/tests/fixtures/achievements/` (`schema-424242`, `stats-424242`, `stats-empty`, `stats-corrupt`, `stats-wrong-magic`). Covers totals, bitfield/**AchievementTimes** unlocked counts, absent file, wrong magic/case, trailing END markers, schema appid mismatch and cache invalidation. Full suite: **52 unit + 3 registry integration, 0 failed**; real-machine smoke output recorded in §13. |
| F8 | Final gates, visual smoke, documentation, push | F | DONE | `npm run check` 0/0; `npm run rust:check` clean; `npm run rust:test` 52+3 pass; `npm run build` **124.75 KB JS / 42.19 KB gzip** (CSS 39.52 / 8.12 KB); `npm run tauri build` green (exe + `Kudos_0.1.0_x64-setup.exe`, 1.40 MiB). Headless Chrome 1600×1000 with `?mock=1` + `?pad=` preview modes: 13 screenshots in `%TEMP%\opencode\kudos-shots`, 0 console errors. Committed with `Session F:` subjects and pushed to `origin main`. |
| G1 | Idle/ambient state: prefs, activity watcher, gate | G | DONE | New `src/lib/idle.svelte.ts`. Prefs at **`kudos.idle`** (enabled, 30 s/1 m/2 m/5 m/Never, art source, crossfade, clock); pointer/keyboard/wheel/touch plus 2 Hz gamepad activity only reschedules one timeout; pauses while hidden/unfocused; reference-counted `armIdleWatcher`; `setIdleGate` refuses an empty library. Settings preview forces entry but still respects the gate. Evidence: a seeded 30 s delay auto-started on Control Ultimate Edition with two layers mid-crossfade, 0 console errors. |
| G2 | PS5-style ambient screen | G | DONE | New `IdleAmbient.svelte`. Full-bleed art with two-layer crossfade (15 s/2 s slow, 8 s/1 s fast), transform-only Ken Burns drift, clock/date, platform + title + relative play time, input/gamepad exits after an 800 ms grace period, body scroll locked, reduced motion keeps opacity-only. Steam `library_hero.jpg` with local 600×900 fallback; GOG `.ico` skipped for gradients. Evidence: `sessionG-idle-1600x1000.png`, `sessionG-idle-crossfade-1600x1000.png`. |
| G3 | Recently Played backend: Steam LastPlayed + Kudos journal | G | DONE | New `steam_recent.rs` parses `localconfig.vdf` `apps/<appid>/LastPlayed` (cached by mtime+len), merges with `%APPDATA%\Kudos\recent.json` (cap 100, atomic replace), filters to the current library, clamps limit to 1..=50. `launch_game` journals only successful launches, best-effort. Reuses B's `detect_steam_root`/`active_userdata_dir` via visibility-only edits. 8 tests incl. real-machine smoke; real data `steam:730` (1793157398), `steam:12150` (1793116823), `steam:413150` (1792425299). |
| G4 | Recently Played row + local preview mirror | G | DONE | New `recent.ts`, `history.svelte.ts`, `RecentlyPlayedRow.svelte`. Backend list is authoritative; `localStorage` **`kudos.recentLaunches`** mirrors launches for an instant update before the next round-trip and for plain-browser previews. Steam wins per id; unknown/uninstalled ids are skipped; the row hides when empty. Six mock entries and ten rail tiles at 1600×1000; empty-library safe. |
| G5 | Idle settings card + preview | G | DONE | New `IdleSettings.svelte` (controlled) mounted in Settings: enable toggle, delay, art source, crossfade, clock, Reset, “Preview idle mode now”. Preview calls `enterIdle(true)` after `loadLibrary`; it bypasses enabled/delay but still refuses an empty library with friendly copy. Values live in `kudos.idle` and survive reload. Evidence: `sessionG-integrated-settings-idle-1600x1000.png`. |
| G6 | App integration: Home row/watcher, App host, detail mirror | G | DONE | Home imports `RecentlyPlayedRow` above the rail and arms the idle watcher in `onMount` (cleanup on leave); App mounts `<IdleHost />`; GameDetail mirrors a successful real launch via `recordLaunch`. Home spacing/rail padding tuned so the full 1600×1000 page is exactly 1000 px with no content clipped. No changes to `Game`/`LibrarySource` or existing command args. |
| G7 | Dev harness + visual smoke | G | DONE | `?idle=1` lazily mounts `IdleHarness.svelte` from `main.ts` (normal bundle unaffected); `?idle=1&mock=1` opens ambient, `?idle=1&mock=1&shell=1` shows row + settings. Headless Chrome at 1600×1000: integrated home, settings card, preview overlay, 30 s auto-start, mid-crossfade (2 layers), 0 console errors/exceptions. Screenshots `sessionG-*` in `%TEMP%\opencode\kudos-shots`. |
| G8 | Final gates, docs, push | G | DONE | `npm run check` 0 errors/0 warnings; `npm run rust:check` clean; `npm run rust:test` **52 unit + 3 registry, 0 failed**; `npm run build` **143.52 KB JS / 48.20 KB gzip** (CSS 50.18 / 9.81 KB); `npm run tauri build` green (`kudos.exe` + `Kudos_0.1.0_x64-setup.exe`, 1.40 MiB). TARGET.md §3.6/§14 added; committed with `Session G:` subjects and pushed to `origin main`. |
| H1 | Steam owned catalog from `localconfig.vdf` | H | DONE | New `src-tauri/src/steam_owned.rs`. Parses the active account's `Apps` table (`Playtime`, `Playtime2wks`, `LastPlayed`), cached by `(path,len,mtime)`; excludes installed ids, 228980, Steam client 7 / Steam Cloud 760 and shortcut ids. `steam:<appid>`, `is_installed:false`, `launch_command:"steam://install/<appid>"`, cached title else `"Steam App <appid>"`, CDN cover only when named. Real machine: **114 uninstalled owned apps**; cold 26.8 ms / warm 4.6 ms (debug), no network, never panics. |
| H2 | `get_steam_playtime` | H | DONE | Sorted by minutes desc then app id; camelCase wire `{appId, minutes, twoWeeksMinutes, lastPlayed}` with `lastPlayed` omitted when absent. Real machine: **119 rows, 95 with minutes, 395.0 total hours**; top five `steam:730` 2044, `steam:447040` 2039, `steam:298110` 1634, `steam:1687950` 1558, `steam:2698940` 1153 min. Includes 2 zero-minute Steam client/Cloud rows; owned/status count excludes them. |
| H3 | Opt-in metadata sync + status | H | DONE | `sync_steam_metadata` GETs the frozen `ISteamApps/GetAppList/v2/` with a 10 s timeout and one retry; Valve retired the keyless endpoint (HTTP 404), so it falls back to keyless `IStoreBrowseService/GetItems/v1` in batches of 100. Only known ids are cached at **`%APPDATA%\Kudos\steam-apps.json`** as `{fetchedAt, apps}` via tmp+rename; never called at startup. Real run: `cached=true appCount=109 fetchedAt=1793176743 ownedAppCount=117`, 1.82 s. `get_steam_sync_status` is local-only. |
| H4 | Command registration + install fallback | H | DONE | Registered the four additive commands in `lib.rs`/`commands.rs`; wrappers are sync except the async network sync. `launch_game` now opens `steam://install/<appid>` when an owned, uninstalled Steam id is not in `list_games`; the install is **not** journaled as a play. Unknown/non-Steam ids still return the normal error. No frontend files touched. |
| H5 | Tests + committed fixture | H | DONE | 9 new tests (8 normal + 1 `#[ignore]` real-network sync) + `src-tauri/tests/fixtures/steam/localconfig-owned.vdf`. Covers quoted keys, shortcut/junk/duplicate-table filtering, installed/redistributable exclusion, atomic cache round-trip, app-list/store fallback parsing, exact camelCase JSON, and real-machine smoke (<100 ms warm). Full suite: **60 unit + 3 registry, 0 failed, 1 ignored**. |
| H6 | Final gates, docs, push | H | DONE | `npm run check` 0 errors/0 warnings; `npm run rust:check` clean; `npm run rust:test` 60+3 pass; `npm run build` green; `npm run tauri build` green (`kudos.exe` + `Kudos_0.1.0_x64-setup.exe`, 1.43 MiB). TARGET.md §3.3/§15 added; committed with `Session H:` subjects and pushed to `origin main`. |

> **Session F file-ownership note (approved by the user's task):** F edits `src-tauri/src/sources/steam.rs`
> only to expose two existing helpers (`detect_steam_root`, `active_userdata_dir`) as `pub(crate)`, and
> edits Session D's UI files (`Home`, `GameDetail`, `Settings`, `gamepad.ts`, `theme.ts`, `windowPrefs.ts`,
> `library.svelte.ts`, `App.svelte`) for the three requested features. No frozen shape changes; the only
> contract addition is `get_achievements` in §3.5.

> **Session G file-ownership note (approved by the user's task):** G edits Session A's `src/main.ts`
> (lazy `?idle=1` harness import only), `src/lib/api.ts` (additive `getRecentGames` wrapper),
> `commands.rs`/`lib.rs` (additive command + launch journal hook) and Session D's `Home.svelte`
> (recent row + idle arming + spacing), `App.svelte` (`<IdleHost />`), `Settings.svelte` (idle card +
> preview) and `GameDetail.svelte` (local recent mirror). `sources/steam.rs` changes are visibility-only
> (`pub(crate)` on seven existing helpers). No frozen shape changed; the only contract addition is
> `get_recent_games` in §3.6.

> **Session H file-ownership note (approved by the user's task):** H edits Session A's
> `src-tauri/src/commands.rs` (four additive command wrappers + install fallback in `launch_game`)
> and `src-tauri/src/lib.rs` (module + `invoke_handler` registration). `src-tauri/src/sources/steam.rs`
> changes are visibility-only (`pub(crate)` on `REDISTRIBUTABLE_APPID` and `steam_cover_url`). All new
> logic lives in `src-tauri/src/steam_owned.rs`. No `src/**` file, frozen `Game`/`LibrarySource`
> shape or existing command argument changed; the only contract addition is the four commands in
> §3.3/§15. No new Cargo or npm dependency.

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

---

## 13. SESSION F — CONTROLLER, PERSONALIZATION, ACHIEVEMENTS

Final pass: **2026-10-28**. All work is additive; `get_achievements` is the only contract
addition (§3.5). No new npm or Cargo dependencies.

**Gates (all green):**

| Command | Result |
| --- | --- |
| `npm run check` | 0 errors, 0 warnings |
| `npm run rust:check` | clean |
| `npm run rust:test` | **52 passed, 0 failed** (9 achievement tests) + 3 registry integration |
| `npm run build` | **124.75 KB JS / 42.19 KB gzip**; 39.52 KB CSS / 8.12 KB gzip |
| `npm run tauri build` | `kudos.exe` + `Kudos_0.1.0_x64-setup.exe` (1.40 MiB) |
| Real achievement smoke | `steam:730` 1/1; `steam:12150` unsupported; `steam:413150` 0/49 |

**Visual smoke:** headless Chrome at 1600×1000 against `?mock=1`, with
`?mock=1&pad=xbox|playstation|nintendo|generic|keyboard` for each glyph family. Thirteen PNGs in
`%TEMP%\opencode\kudos-shots` (`f-home-*`, `f-detail-*`, `f-settings-*`, `f-startup-settings`),
**0 console errors**. Verified in the DOM/screenshots:

- No pad ⇒ keyboard keycaps; PlayStation footer uses inline SVG glyphs, not a font; Keyboard
  override persists across reload.
- Hide drops the rail from 10 to 9 tiles and shows “1 hidden”; Unhide all clears `kudos.hiddenGames`.
- Gradient mode runs a 26 s transform-only drift, and `prefers-reduced-motion: reduce` computes
  `animation: none`; compact density and the startup page (Settings) both apply.
- Crossfade off leaves one info panel; Steam mock detail shows **18/45 (40%)**; Epic/GOG show
  “Achievements not available for this platform”.
- Achievement reads are local-only. `steam:730` parses **1/1** (this account's local CS2 stats),
  `steam:413150` **0/49**, and `steam:12150` returns `supported:false` with “Steam has no local
  achievement data for this game.”

**Not verified on this machine:**

- No physical Xbox/PlayStation/Nintendo pad was available. Detection patterns and live
  connect/disconnect events are wired, but only the `?pad=` previews were exercised visually.
- Epic/GOG achievements are UI-only by design; neither client is installed, so no real non-Steam
  achievement provider was tested. The command returns `supported:false` for non-Steam ids.
- Achievement counts reflect Steam's local cache and are re-read on detail-page visits (cache keyed
  by both files' `mtime` + length), not pushed while Steam writes. A title with no local stats or
  schema legitimately shows `supported:false`.
- Steam stayed running for the whole pass, so the E6 live Add-to-Steam write remains blocked (§11).

---

## 14. SESSION G — IDLE MODE + RECENTLY PLAYED

Final pass: **2026-10-28**. All work is additive; `get_recent_games` is the only contract
addition (§3.6). No new npm or Cargo dependencies. Steam was left running throughout.

**Gates (all green):**

| Command | Result |
| --- | --- |
| `npm run check` | 0 errors, 0 warnings |
| `npm run rust:check` | clean |
| `npm run rust:test` | **52 unit + 3 registry, 0 failed** (8 recent tests) |
| `npm run build` | **143.52 KB JS / 48.20 KB gzip**; 50.18 KB CSS / 9.81 KB gzip |
| `npm run tauri build` | `kudos.exe` + `Kudos_0.1.0_x64-setup.exe` (1.40 MiB) |
| Real recent smoke | `steam:730`, `steam:12150`, `steam:413150` from this machine |

**What was built:**

- **Idle engine** (`idle.svelte.ts`): opt-in ambient mode after 30 s / 1 m / 2 m / 5 m /
  Never. One timer, reset by real input (pointer, keyboard, wheel, touch, 2 Hz gamepad
  poll); paused while the window is hidden or unfocused, so alt-tabbing never opens the
  screen behind another app. The gate refuses an empty library and retries later.
- **Ambient screen** (`IdleAmbient.svelte`): PS5-style full-bleed art, two-layer
  crossfade, slow transform-only drift, clock/date, platform badge, title and relative
  play time. Any input exits after an 800 ms grace period; body scroll is locked and
  focus/scroll are restored. `prefers-reduced-motion` removes the pan/zoom but keeps the
  fade. Steam uses `library_hero.jpg` with the local portrait cover as fallback.
- **Recent backend** (`steam_recent.rs`): parses Steam's per-account `LastPlayed` from
  `localconfig.vdf`, merges it with Kudos' own launch journal
  (`%APPDATA%\Kudos\recent.json`, capped at 100, atomic replace), filters to the current
  library and clamps the request to 1..=50. `launch_game` appends only after a successful
  launch. Steam wins over the journal for the same id.
- **Recent row** (`RecentlyPlayedRow.svelte`): compact art strip above the Home rail,
  resolving ids against the live library and hiding itself when there is nothing recent.
  A `localStorage` mirror (`kudos.recentLaunches`) updates the row instantly after a launch
  and keeps plain-browser previews useful; the backend result always wins in the desktop app.
- **Settings** (`IdleSettings.svelte`): enable, delay, art source, crossfade, clock, Reset
  and **Preview idle mode now**. Preview bypasses the enabled/delay check but still needs a
  game; an empty library shows friendly copy instead of a blank overlay.
- **Dev harness**: `?idle=1` lazily mounts `IdleHarness.svelte` from `main.ts`, so the
  normal bundle is unchanged. `?idle=1&mock=1` opens ambient; `&shell=1` shows the row and
  settings card.

**Visual smoke:** headless Chrome at 1600×1000 against the integrated app (`?mock=1`),
**0 console errors / exceptions**. Screenshots in `%TEMP%\opencode\kudos-shots`:

- `sessionG-integrated-home-1600x1000.png` — six recent entries, ten rail tiles, footer
  hints visible; the whole page is exactly 1000 px (no scroll).
- `sessionG-integrated-settings-idle-1600x1000.png` — idle card in the Settings grid.
- `sessionG-integrated-idle-1600x1000.png` — preview from Settings, art + clock + meta.
- `sessionG-idle-autostart-1600x1000.png` — seeded 30 s delay fired on its own; two layers
  mid-crossfade, `body { overflow: hidden }`.
- `sessionG-idle-1600x1000.png`, `sessionG-idle-crossfade-1600x1000.png`,
  `sessionG-shell-1600x1000.png` — harness ambient, crossfade and shell modes.

**Not verified on this machine:**

- No physical controller was available for the ambient wake path (400 ms gamepad poll);
  keyboard/pointer exit and the Settings preview were exercised.
- Long-duration burn-in and display sleep were not tested. Idle mode is explicitly not a
  screensaver: it keeps the window awake and does not blank or lock the screen.
- Epic/GOG have no local play history, so their recent timestamps come from Kudos'
  journal after a launch; neither client is installed here.
- The idle auto-start was verified in headless Chrome with focus emulation. A live Tauri
  window follows the same suspended-when-unfocused rule by design.

---

## 15. SESSION H — STEAM OWNED CATALOG + PLAYTIME

Final pass: **2026-10-28**. Additive only: four commands in §3.3, no frozen shape change, no
new npm or Cargo dependency, no `src/**` edit. Steam files are read-only and Steam was left
running throughout.

**Gates (all green):**

| Command | Result |
| --- | --- |
| `npm run check` | 0 errors, 0 warnings |
| `npm run rust:check` | clean |
| `npm run rust:test` | **60 unit + 3 registry, 0 failed, 1 ignored** (9 new Session H tests) |
| `npm run build` | green; 178.13 KB JS / 58.09 KB gzip, 67.56 KB CSS / 12.35 KB gzip. The shared worktree also contained Session I's in-progress UI; Session H adds no frontend code. |
| `npm run tauri build` | `kudos.exe` + `Kudos_0.1.0_x64-setup.exe` (1.43 MiB) |
| Real owned/playtime smoke | **114 uninstalled apps**, **119 playtime rows**, **95 with minutes**, **395.0 total hours** |
| Real opt-in sync | `cached=true appCount=109 fetchedAt=1793176743 ownedAppCount=117`, 1.82 s |

**Commands** (registered in `lib.rs`, wrappers in `commands.rs`, all logic in `steam_owned.rs`):

| Command | Signature | Notes |
| --- | --- | --- |
| `get_steam_owned_games` | `() -> Vec<Game>` | Owned-but-uninstalled catalog. Local-only, never rejects, no network. |
| `get_steam_playtime` | `() -> Vec<SteamPlaytime>` | Local playtime, sorted by `minutes` desc then `appId`. Never rejects. |
| `get_steam_sync_status` | `() -> SteamSyncStatus` | Reads only `%APPDATA%\Kudos\steam-apps.json`. Local-only. |
| `sync_steam_metadata` | `() -> Result<SteamSyncStatus, String>` | Async; the only network call. User-triggered only, never at startup. |

Exact wire shapes:

```json
{"id":"steam:447040","platform":"steam","title":"Watch_Dogs® 2","launchCommand":"steam://install/447040","isInstalled":false,"coverImage":"https://cdn.cloudflare.steamstatic.com/steam/apps/447040/library_600x900.jpg"}
```

```json
{"appId":"steam:730","minutes":2044,"twoWeeksMinutes":845,"lastPlayed":1793162961}
```

```json
{"cached":true,"appCount":109,"fetchedAt":1793176743,"ownedAppCount":117}
```

`lastPlayed` and `fetchedAt` are omitted when unknown. `Game` keeps its existing camelCase
shape; `exePath` and `sizeBytes` are omitted for catalog entries.

**Owned catalog rules.** The source is the active account's
`userdata/<accountid>/config/localconfig.vdf` → `UserLocalConfigStore → Software → Valve →
Steam → Apps`. Each numeric store id becomes `steam:<appid>` with
`platform:"steam"`, `is_installed:false` and `launch_command:"steam://install/<appid>"`.
Installed ids (already returned by `list_games`), 228980, Steam client 7, Steam Cloud 760
and non-Steam shortcut ids (`>= 0x8000_0000`) are excluded. The title comes from the name
cache; without a name it is `"Steam App <appid>"` and **no** cover is advertised. With a
name, `coverImage` is the CDN `library_600x900.jpg`. The list is title-sorted
case-insensitively. Parsing is cached by `(path, len, mtime)` and a corrupt/mid-write file
yields an empty list rather than a panic.

**Playtime.** Every numeric store id in `Apps` becomes
`{appId, minutes, twoWeeksMinutes, lastPlayed?}`. On this machine that is 119 rows, 95 with
non-zero minutes, 395.0 total hours. The two zero-minute Steam client/Cloud rows are included
here but excluded from owned/status counts. Top five: `steam:730` Counter-Strike 2 2044 min,
`steam:447040` Watch_Dogs® 2 2039, `steam:298110` Far Cry® 4 1634, `steam:1687950` Persona 5
Royal 1558, `steam:2698940` The Crew Motorfest 1153.

**Opt-in metadata sync.** The frozen primary is
`GET https://api.steampowered.com/ISteamApps/GetAppList/v2/` with a 10 s timeout and one
retry. Valve has retired the keyless form: this machine returns HTTP 404
`Method 'GetAppList' not found in interface 'ISteamApps'` on both attempts. The fallback is
the store's keyless `IStoreBrowseService/GetItems/v1` on the same host, batched 100 ids per
request. Only ids in the local catalog are retained. The cache is
`%APPDATA%\Kudos\steam-apps.json`:

```json
{ "fetchedAt": 1793176743, "apps": { "730": "Counter-Strike 2", "447040": "Watch_Dogs® 2" } }
```

It is written through `<file>.kudos-tmp` + rename; a failed sync leaves the previous cache
untouched and returns a human-readable error string. `cached` means a non-empty name cache
exists, `appCount` is its pair count, and `ownedAppCount` is the distinct non-game store ids
known locally (installed included). Nothing calls sync during startup or `list_games`.

**Install fallback.** `list_games` never includes catalog entries, so `find_game` cannot
resolve them. `launch_game` therefore checks `steam_owned::install_command` after a failed
lookup and opens `steam://install/<appid>` for an owned, uninstalled Steam id. This is an
install action, not a play, and is deliberately **not** journaled in `recent.json`. Unknown
ids and non-Steam ids still return the normal `Invalid` error.

**Tests.** 9 new tests in `steam_owned.rs` plus the committed fixture
`src-tauri/tests/fixtures/steam/localconfig-owned.vdf`:

- `parses_playtime_two_weeks_and_last_played_from_quoted_keys`
- `ignores_shortcuts_junk_and_the_second_apps_table`
- `owned_games_exclude_installed_and_redistributables`
- `name_cache_roundtrip_is_atomic_and_filters_junk`
- `app_list_filter_keeps_only_known_non_empty_names`
- `store_fallback_keeps_known_names_and_builds_batches`
- `wire_shapes_are_camel_case`
- `real_machine_owned_smoke` (read-only; asserts warm <100 ms)
- `real_machine_metadata_sync_opt_in` (`#[ignore]`; the only network test)

**Measured on this machine (debug build).** 114 uninstalled owned apps, 119 playtime rows,
109 cached names; cold 26.8 ms, warm 4.6 ms for the owned catalog (earlier warm runs
1.3–1.9 ms), both well under the 100 ms budget. Sync named 109 of 117 ids in 1.82 s.

**Known limits.**

- `GetAppList` v2 is retired by Valve. The keyless store fallback is not part of the frozen
  endpoint contract and could change; if both paths fail, sync returns an error and the old
  cache is kept. No Steam Web API key is used.
- 8 of 117 ids are hidden/region-restricted in the store response, so they stay
  `"Steam App <id>"` with no cover until a future sync names them.
- The catalog is account-local and only as complete as `localconfig.vdf`. A fresh account,
  family sharing, or a never-started Steam client yields `[]`, not an error. The 119 local
  rows are not a claim about the entire Steam store.
- Playtime includes 2 zero-minute Steam client/Cloud rows; owned/status counts exclude them.
- Catalog entries are not in `list_games`, so the frontend must merge both lists. Installed
  ids appear only once, from `list_games`.
- Covers for uninstalled named apps are CDN URLs (no local `librarycache` lookup); unnamed
  apps get no cover and rely on the gradient fallback.
- Sync is manual with no TTL or background refresh. `steam://install` opens Steam's install
  flow and depends on the OS protocol handler; it does not verify or wait for the install.
- Windows-only, like the rest of the app: WinHTTP via FFI, `%APPDATA%` and Steam's registry
  detection.

---

## 16. SESSION I — PS5 LIBRARY SHELL, SEARCH, FAVORITES, STEAM MERGE

Final pass: **2026-10-30**. Frontend-only: every edit is under `src/**` plus this section;
`src-tauri/**` was not touched. Session H's four commands are consumed exactly as frozen in
§3.3/§15, and both H commits (`737e5e8`, `7548bcf`) are already on `main`. No new npm or
Cargo dependency. Steam was left running throughout.

**Gates (all green):**

| Command | Result |
| --- | --- |
| `npm run check` | 0 errors, 0 warnings |
| `npm run build` | **178.53 KB JS / 58.24 KB gzip**; 67.56 KB CSS / 12.35 KB gzip |
| `npm run tauri build` | `kudos.exe` + `Kudos_0.1.0_x64-setup.exe` (1.43 MiB) |
| Visual smoke | 12 PNGs at 1600×1000; latest re-run **PASS** — 0 console errors, 0 exceptions, 0 log errors |
| JS budget | 58.24 KB gzip ≤ 150 KB |

**Task board:**

| ID | Change | Evidence |
| --- | --- | --- |
| I1 | **Merged library.** `get_steam_owned_games` + `list_games` are merged client-side, deduped by `id`, installed wins. Owned Steam entries get a “Not installed” badge and an Install action. | `library.svelte.ts`, `steam.ts`; mock = 22 games (8 installed + 14 owned), 13 Steam owned/not-installed; `sessionI-home-1600x1000.png` |
| I2 | **Filters.** All six filters (All / Installed / Not installed / Steam / Epic / GOG) always visible with live counts; a store with 0 games shows `0` and a friendly empty state; choice persisted in `kudos.libraryFilter`. | `FilterBar.svelte`, `filter.ts`; DOM `All 22`, `Installed 8`, `Not installed 14`, `Steam 16`, `Epic 3`, `GOG 3`; `sessionI-home-not-installed-1600x1000.png` |
| I3 | **Playtime.** Steam minutes map to tiles and detail (“22 h”, “120 h played · last played yesterday”, “75 h played · last played 2 days ago”); zero-minute rows render no time, never “0 h”. | `library.svelte.ts`, `RowTile.svelte`, `GameDetail.svelte`; DOM zero-hours regex check; installed/owned detail shots |
| I4 | **PS5 home shell.** Fixed 390 px hero with focus parallax, kicker follows the active row, rows ordered Favorites → Continue playing → Recently played → Your library → In your Steam account; Up/Down rows, Left/Right tiles, Enter opens, Esc backs. | `Home.svelte`, `HeroBanner.svelte`, `MediaRow.svelte`; ArrowDown changed row, ArrowRight changed tile; `sessionI-home-focus-1600x1000.png` |
| I5 | **Boot splash.** 600 ms vignette, fade at 330 ms, skippable by pointer/keyboard/wheel; reduced-motion removes animation. | `BootSplash.svelte`, `App.svelte`; painted capture `sessionI-boot-1600x1000.png` on the normal 600 ms path (smoke overrides `prefers-reduced-motion`), opacity 1, element gone after a normal reload; reduced-motion exits at 80 ms by design |
| I6 | **Search.** `/` or Ctrl+K opens a native `dialog`, fuzzy scoring without new deps, arrows/Enter/Esc; Enter opens the detail page. | `SearchOverlay.svelte`, `search.svelte.ts`; typing “eld” returned Elden Ring / Skyrim / Witcher 3; `sessionI-search-1600x1000.png` |
| I7 | **Favorites.** `F` toggles the focused tile, star button on detail, hold/right-click a tile; persisted in `kudos.favorites`; Favorites row sits first; Settings card removes one or clears all. | `favorites.svelte.ts`, `RowTile.svelte`, `GameDetail.svelte`, `Settings.svelte`; stored `["epic:ControlUltimate"]`; `sessionI-home-favorites-1600x1000.png`, `sessionI-settings-favorites-1600x1000.png` |
| I8 | **Detail page.** Landscape Steam hero with portrait fallback, platform/size/playtime/achievements facts, favorite toggle, Play for installed, Install for owned, “Install through Steam” copy, Add to Steam/hide retained. | `GameDetail.svelte`, `art.ts`; 120 h + 11/42 on Elden Ring, 75 h + Install on Baldur's Gate 3; `sessionI-detail-installed-1600x1000.png`, `sessionI-detail-owned-1600x1000.png`, `sessionI-install-overlay-1600x1000.png` |
| I9 | **Steam library card.** Settings shows status pill, apps in Steam, owned-not-installed, installed, cache age, and a user-triggered Sync now with progress, success toast and error text. | `SteamLibraryCard.svelte`, `Settings.svelte`, `library.svelte.ts`; 72 / 13 / 8 / “2h ago”, Syncing… → Fresh / Just now; `sessionI-settings-steam-1600x1000.png` |
| I10 | **Onboarding + resilience.** True-zero library replaces rows/filters with “Sync Steam library” and the Epic/GOG note; every new command is feature-detected and a missing command silently falls back to installed-only; all four commands are mocked under `?mock=1`. | `Onboarding.svelte`, `api.ts`, `devMock.ts`; `?mock=1&empty=1` showed 0 rows/0 chips; `sessionI-onboarding-1600x1000.png` |

**Mocked vs real.** All screenshots and DOM assertions use `?mock=1` with deterministic data
(22 games, 8 installed, 72 Steam apps, 119 playtime rows). The production path invokes the
four H commands through `api.ts`; they are registered in `lib.rs` and the `tauri build`
above contains them. Real-machine command behaviour was verified by Session H (114 owned
apps, 119 playtime rows, 395.0 h, 109-name sync); the real UI path was not re-screenshotted
in this pass.

**Screenshots** (`%TEMP%\opencode\kudos-shots`, all `sessionI-*`):
`boot`, `home`, `home-focus`, `home-favorites`, `home-not-installed`, `search`,
`detail-installed`, `detail-owned`, `install-overlay`, `settings-steam`,
`settings-favorites`, `onboarding` — each `-1600x1000.png`.

**Ownership note.** Session I edited frontend files previously owned by Sessions A/D
(`App.svelte`, `Home.svelte`, `GameDetail.svelte`, `Settings.svelte`, `api.ts`, `art.ts`,
`FilterBar.svelte`, `Icon.svelte`, `LaunchOverlay.svelte`, `devMock.ts`, `filter.ts`,
`library.svelte.ts`) and added new `src/lib` modules/components. Those edits were made in
the shared worktree as agreed for F/G; A/D owners should treat them as reviewed frontend
work, not frozen contracts. No `src-tauri/**` file was changed.

**Known limits.**

- No physical controller was available; the preserved Session F keyboard/gamepad glyph
  system was exercised with `?pad=keyboard` and the footer hints only.
- Install is a mock overlay in the screenshots (`Opening Steam`). The real action opens
  `steam://install/<appid>` and cannot be completed headlessly while Steam is running.
- “Your library” and “In your Steam account” intentionally overlap in the All view because
  the second row is the owned-but-uninstalled subset. Under Not installed the same subset
  repeats; this is the fixed row model, not duplicate data.
- Steam sync is manual with no TTL, matching H. A failed sync keeps the previous cache and
  surfaces the backend error string in the card.
- Boot capture required emulated `prefers-reduced-motion: no-preference`; on a machine with
  reduced motion the splash removes itself at 80 ms by design.

---

## v0.2.0 - artwork, Steam account access, branding, docs and controller polish

Shipped after the v0.1.0 tag. All work landed on `main` and the `v0.2.0` tag.

| Task | Scope | Evidence |
| --- | --- | --- |
| M | SteamGridDB artwork: secure key, search/apply for grid/hero/logo/icon, local cache, write into Steam `config/grid/` | `artProvider.ts`, `ApiKeyGuide.svelte`, `ArtworkCard.svelte`, `ArtPicker.svelte`; `npm run check` 0/0 |
| L | Optional Steam Web API key: complete owned library, DPAPI vault, 6 h cache, privacy/401/429 copy | `steam_api.rs` (11 tests), `steamKey.ts`, `SteamKeyCard.svelte`; `rust:test` 106 + 3 |
| N | Refined mark/wordmark, PS5 boot sequence, 54 regenerated icons, favicon | `BrandMark.svelte`, `BootSplash.svelte`, `src-tauri/icons/**`; boot frames `sessionN-boot-*` |
| K | User guide, troubleshooting, architecture, roadmap, deploy notes, static `site/` | `docs/**`, `site/**`; no absolute local paths |
| O | Whole-app colour schemes, section awareness, controller scrolling, rail focus | `theme.ts`, `app.css`, `App.svelte`, `Home.svelte`, `MediaRow.svelte`, `shellFocus.svelte.ts`, `padNav.ts` |

**Colour scheme contract.** `applyAccent()` writes `--accent`, `--accent-rgb`,
`--accent-2`, `--accent-2-rgb`, `--accent-soft`, `--accent-glow`,
`--accent-contrast`, `--line-accent` and accent-tinted `--bg-0..3`. Components use
those tokens only; no hardcoded accent blue remains outside the colour picker.

**Controller contract.** Home owns the page zone (Up/Down sections, Left/Right
tiles, Left at column 0 opens the rail, Start toggles it, A opens, B clears the
filter). The shell owns the rail zone (Up/Down, A selects, B closes). Settings and
GameDetail move DOM focus with Up/Down and scroll it into view.

**Manual checks still open.**

- Live SteamGridDB key: search, apply and the real `config/grid/` write.
- Live Steam Web API key: real owned count, 401/429 and private-profile paths.
- Real controller: Xbox and PlayStation pads, rail entry, section scroll feel.
- Live Add-to-Steam with Steam closed, then auto-art.
- Epic/GOG against real clients (fixture-only here).
