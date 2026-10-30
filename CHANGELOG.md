# Changelog

All notable changes to Kudos are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.0] - 2026-10-30

A bigger library, a PS5-style home screen, optional Steam account access and
SteamGridDB artwork.

### Added

- **Full Steam library.** A Steam owned-games catalog reads the active account's
  `localconfig.vdf` so games that are owned but not installed appear alongside
  installed titles. Owned-only entries use a `steam://install/<appid>` install
  link. Additive commands: `get_steam_owned_games`, `get_steam_playtime`,
  `get_steam_sync_status` and `sync_steam_metadata`.
- **Optional Steam Web API key.** Paste a free key from
  `steamcommunity.com/dev/apikey` in Settings to load the account's *complete*
  owned library, including titles the local cache has never seen. The key is
  encrypted on disk with Windows DPAPI, is never logged, and the cache honours a
  six-hour freshness window. Without a key the app keeps using local files and
  makes no network calls.
- **Playtime.** Total minutes and last-two-weeks minutes per Steam app, with
  last-played timestamps, surfaced in the library rows and the detail page.
- **Opt-in Steam metadata sync.** A user-triggered name cache at
  `%APPDATA%\Kudos\steam-apps.json`; it is never refreshed while listing the
  library.
- **Favorites.** Persisted game ids (`kudos.favorites`) shared by rows, tiles,
  the detail page and Settings.
- **Search.** A dependency-free fuzzy search overlay opened with `/` or
  `Ctrl/Cmd+K`, matching titles and platforms.
- **PS5-style Home redesign.** Hero banner with crossfading art, independently
  focusable media rows, six library filters with live counts, a Continue Playing
  row, and a first-run onboarding screen for an empty library.
- **SteamGridDB artwork.** Optional free key, per-game artwork picker for grid,
  hero, logo and icon images, downloaded once and cached locally. Chosen art is
  also written into Steam's `config/grid/` folder so non-Steam shortcuts keep it.
  Community art is credited to SteamGridDB.
- **In-app key guides.** Both API keys ship with step-by-step instructions,
  limits and privacy notes inside Settings, so no external docs are required.
- **New branding and boot animation.** A refined mark and wordmark, a matching
  desktop/taskbar/installer icon set, and a PS5-style boot sequence (mark bloom,
  accent sweep, wordmark) that any input skips instantly and that collapses to a
  plain fade under `prefers-reduced-motion`.
- **Full-app colour schemes.** Choosing an accent (or a custom hex) now retints
  the whole app, not one gradient: rail, cards, borders, focus rings, scrollbar,
  selection, background wash and a derived second hue for gradients. Accent
  buttons pick a readable text colour automatically from the accent's luminance.
- **Controller navigation fixes.** The focused section is unmistakable (accent
  bar, "Browsing" badge, section counter), moving between sections scrolls the
  page so focus never disappears below the fold, and the left rail opens with
  Left or Start, navigates with the d-pad and closes with Back. Settings and the
  detail page move focus between controls and scroll with Up/Down.
- **Docs and landing page.** User guide, troubleshooting, architecture, roadmap
  and a dependency-free static site under `site/`, plus CI and a tag-triggered
  release workflow.

### Changed

- Home is reorganized around a hero banner and horizontal media rows instead of
  the single portrait rail; the existing keyboard, gamepad and reduced-motion
  behavior is retained.
- Settings gains the Steam library card, the Steam API key card, the artwork
  card and the new search/favorites entry points.
- The Steam source is extended without changing the frozen `Game`, `Platform`
  or `LibrarySource` shapes, so existing caches and settings keep working.

### Fixed

- Controller glyphs now match the detected pad (Xbox vs PlayStation) instead of
  always showing Xbox buttons.
- Idle ambient mode no longer leaves the window awake or the hero art animating
  when the library is empty.

## [0.1.0] - 2026-10-28

First public release: a lightweight, controller-first Windows launcher for the
games already installed through Steam, Epic and GOG.

### Added

- **One library across three stores.** Steam is discovered from the registry,
  `libraryfolders.vdf` and every `appmanifest_*.acf`; Epic from its launcher
  manifest files; GOG from the registry keys written by GOG Galaxy and GOG
  installers. A missing store client produces an empty section, not an error.
- **Launch through the real client.** Steam games open with
  `steam://rungameid/<appid>` (Cloud, overlay and playtime keep working); Epic
  uses its launcher URI with an executable fallback; GOG launches the installed
  executable with its working directory and launch arguments.
- **Add to Steam.** GOG and other non-Steam games can be written into the active
  profile's `shortcuts.vdf` as a standard non-Steam shortcut. The writer
  preserves existing entries and unknown fields, creates a
  `.kudos-backup`, is idempotent, refuses unparseable files, and refuses to write
  while Steam is running.
- **Offline-first cover art.** Steam reads the local
  `appcache/librarycache` copy first, then the CDN `library_600x900.jpg`, then
  `header.jpg`, then a platform-tinted gradient with the game's initials. GOG
  uses its local `GOG.ico` when available. Epic intentionally falls back to the
  gradient.
- **PS5-style library UI.** Portrait rail with a focused tile, ambient cover
  crossfade, All/Steam/Epic/GOG filters, loading skeletons, empty and error
  states, and a detail page with cover hero, facts, Play and Add to Steam.
- **Keyboard and controller navigation.** Arrow keys, Enter/Space, Esc,
  d-pad/left stick with repeat, and face buttons. Controller layouts are
  detected for Xbox, PlayStation and Nintendo pads and can be overridden.
- **Controller glyphs.** Xbox, PlayStation and Nintendo button glyphs everywhere,
  including hand-written inline SVG PlayStation symbols (no font dependency);
  keyboard hints appear when no pad is connected.
- **Local achievements.** Steam achievement counts are read from the local
  `appcache/stats` files only. There is no Steam Web API, API key or network
  request; Epic/GOG and unreadable Steam data show "not available" instead of an
  error.
- **Recently played.** Steam's per-account `LastPlayed` timestamps are merged
  with Kudos' own launch journal (`%APPDATA%\Kudos\recent.json`), filtered to the
  current library and shown in a compact row.
- **Idle ambient mode.** Opt-in full-screen ambient screen after 30 s / 1 min /
  2 min / 5 min, with two-layer art crossfade, transform-only drift, clock,
  platform badge, title and relative play time. Any input exits after a short
  grace period; it pauses while the window is hidden or unfocused.
- **Personalization.** 12 accent presets and a custom color, ambient/solid/
  gradient backgrounds, compact/default/large tile density, sorting, hidden
  games with an unhide list, info-panel crossfade toggle, startup page choice
  and borderless fullscreen.
- **Settings.** Steam status (installed/running/path) with refresh, accent and
  background controls, idle preview, hidden games, and an About card.
- **Windows packaging.** Kudos product identity, custom icon set, per-user NSIS
  installer with LZMA compression and a portable `kudos.exe`.

### Changed

- Product, window title, installer, Start-menu folder and About card are branded
  **Kudos** (the original scaffold was "Project Kudos").
- The library shell renders before the first scan finishes; scans are I/O-bound
  and do not freeze the UI.
- Mock data is opt-in only (`?mock=1`); there is no dev-mock default and no
  localStorage fallback in a real build.

### Fixed

- Missing or unreadable store clients no longer produce error spam; they render
  an empty, friendly section.
- Broken or missing cover images fall back through the local/CDN chain to a
  gradient with initials instead of a broken-image icon.
- The Add-to-Steam flow refuses cleanly while Steam is running and tells the
  user to close Steam completely, instead of writing a change Steam would
  overwrite on exit.

### Security

- No telemetry, no accounts and no network calls at startup.
- Achievement and recently-played data are read locally only.
- Local cover access uses a narrow asset-protocol scope: default Steam
  `librarycache` globs plus exact files granted at runtime.
- The WebView runs with a restrictive Content Security Policy and no remote
  scripts.

### Known limitations

- The installer is unsigned; Windows SmartScreen may warn on first run.
- The live Add-to-Steam write was not verified end-to-end on the development
  machine because Steam was running. The writer is covered by fixture tests and
  the refusal path; the manual checklist in `TARGET.md` §12 remains.
- Steam listing is manifest-based: a never-installed title has no local
  `appmanifest_*.acf`. The owned-games catalog and the optional Steam Web API
  key cover this case; without either, offline owned-only titles stay hidden.
- Epic and GOG were fixture-only on the development machine; their scanners and
  launchers are covered by tests but were not exercised against a real client.
- Epic has no cover art by design; GOG is registry-based, so library-only GOG
  games do not appear.
- There is no filesystem watcher; library changes require Refresh or a restart.
  This is intentional (no polling, low idle RAM).
- Windows 10/11 x64 only. The WebView2 runtime sets a memory floor of roughly
  175 MB private even for an empty shell.

[Unreleased]: https://github.com/Rohit4738/Kudos8/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/Rohit4738/Kudos8/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/Rohit4738/Kudos8/releases/tag/v0.1.0
