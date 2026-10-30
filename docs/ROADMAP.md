# Kudos — Roadmap

This roadmap separates **what shipped in v0.1.0**, **what is already on `main` but still in-flight**,
and **what comes next**. It is ordered by priority, not by date. There are no promised release dates
here; a version ships when its gates pass.

Status legend:

- **Shipped** — in the v0.1.0 release and covered by the release notes.
- **In-flight** — on `main`, usable in a development build, but not part of v0.1.0 and still subject
  to change.
- **Next** — not implemented yet. Ordered by what most improves trust in the current product.
- **Candidate** — deliberately parked. Not a commitment.

---

## 1. Shipped in v0.1.0

| Area | What works | Evidence |
| --- | --- | --- |
| One library across three stores | Steam from the registry, `libraryfolders.vdf` and `appmanifest_*.acf`; Epic from installed-app manifests; GOG from registry keys. Missing clients are empty sections. | Real Steam scan on the development machine; Epic/GOG fixture tests. |
| Launch through the real client | Steam URI, Epic launcher URI with executable fallback, direct GOG executable with arguments and working directory. | Source tests and manual Steam launches. |
| Offline-first cover art | Local Steam `librarycache` first, then Steam CDN, then a platform-tinted gradient with initials. GOG uses `GOG.ico` when present. | Real local covers; visual smoke screenshots. |
| Add to Steam | Standard non-Steam shortcut written to `shortcuts.vdf`, preserving unknown fields, creating a backup, idempotent, refusing to write while Steam runs. | Binary-VDF round-trip, backup, idempotency and refusal tests. Live write still needs a manual pass. |
| Local achievements | Steam achievement counts from `appcache/stats`; no Web API, no key, no network. | Parser tests and detail-page screenshots. |
| Recently played | Steam `LastPlayed` merged with Kudos' launch journal. | Source tests and row screenshots. |
| Controller-first UI | Xbox/PlayStation/Nintendo/generic glyphs, keyboard fallback, d-pad/stick navigation and a manual layout override. | Glyph and navigation smoke tests; no physical pad in the final pass. |
| Idle ambient mode | Timer, art source, crossfade, clock, hidden/unfocused pause and input exit. | Settings and idle screenshots; reduced-motion path. |
| Personalization | Accent presets/custom color, background modes, density, sorting, hidden games, crossfade, startup page and borderless fullscreen. | Settings screenshots and localStorage checks. |
| Windows packaging | Per-user NSIS installer, custom icon set, LZMA compression, portable `kudos.exe`. | `Kudos_0.1.0_x64-setup.exe`, ~1.43 MiB. |

---

## 2. In-flight on `main`

These features are already implemented on `main` and appear in development builds, but the changelog
keeps them under `[Unreleased]`. They are **not** part of the v0.1.0 release and their UI or command
names may still change.

| Work | State | What remains before it can ship |
| --- | --- | --- |
| **Owned Steam catalog** | Local `localconfig.vdf` catalog merged with installed games; owned-only entries get `steam://install/<appid>`. The development machine exposes 114 owned Steam apps. | Decide the release boundary, verify against a second Steam account, and document the local-data limits. |
| **Playtime** | Total minutes, last-two-weeks minutes and last-played timestamps, joined by game id. | Verify formatting across zero/unknown values and large libraries; decide whether the two-week value gets its own UI. |
| **Opt-in metadata sync** | User-triggered WinHTTP sync writes a filtered name cache; failed sync keeps the old cache. | Keep the core-library network guarantee (one explicit sync), add a clear first-run explanation, and verify the fallback endpoint over time. |
| **PS5-style Home** | Hero banner, independently focusable media rows, row tiles, refined logo/boot animation and empty-library onboarding. | Finish the release-level visual pass, confirm controller focus order, and keep the reduced-motion path. |
| **Search** | Dependency-free fuzzy overlay on `/` or `Ctrl+K`. | Verify large-library performance and keyboard/controller focus trapping. |
| **Favorites** | Persisted ids shared by rows, tiles, detail and Settings. | Confirm removal/clear behavior and the empty Favorites state. |
| **Steam library Settings card** | Catalog counts, cache age, Sync now and Rescan this PC. | Make the copy distinguish local catalog counts from the optional complete-library API. |
| **Complete Steam library** | Optional Steam Web API key, DPAPI vault, six-hour cache, Settings card and library merge; API entries are marked installed/not installed correctly. | Validate with a real key and public profile; confirm privacy copy and stale-cache refresh behaviour. |
| **SteamGridDB artwork** | Backend commands for search, validated download, local art cache and Steam `config/grid` writes; `ArtworkCard` is in Settings and `ArtPicker` opens from **Change artwork** on non-Steam detail pages. | Validate with a real key and cover the missing-shortcut, offline and rate-limit paths. |

The optional Steam Web API and SteamGridDB modules are now registered in the command surface. The
Steam key card is mounted and merged into the library; the **Artwork** card and **Change artwork**
picker are mounted too. Neither is part of v0.1.0, and both stay optional.

---

## 3. Next

### 3.1 Complete the Add-to-Steam verification

**Why first:** this is the highest-risk shipped write path and the one known gap in the release
evidence. The writer is proven with fixtures, but no live write happened because Steam was running
during the final pass.

**Done when:**

- Steam is fully closed and a GOG/non-Steam shortcut is written to the active profile.
- The shortcut appears in Steam after restart and launches correctly.
- Re-adding the same game does not duplicate it.
- The `.kudos-backup` restores cleanly if the new file is removed.
- The result is recorded in the manual verification checklist.

### 3.2 Validate Epic and GOG on real machines

**Why:** both scanners are fixture-only on the development machine. A real client can expose
manifest, registry or launch-argument differences that fixtures do not.

**Done when:**

- At least one real Epic install and one real GOG install are scanned, launched and documented.
- Cover/icon fallbacks and missing-install paths are checked.
- Any parser difference becomes a fixture and a regression test.

### 3.3 Ship the in-flight library work

The owned catalog, playtime, PS5 Home, search, favorites, complete Steam library and Steam library
card are already on `main`. Shipping them is a release decision, not a rewrite.

**Done when:**

- The release notes and changelog move the agreed subset out of `[Unreleased]`.
- The command table and user guide describe only the commands that are actually registered.
- The full gate set passes: Svelte check, Rust tests, frontend build, Tauri build and a visual smoke.
- The local-catalog limits are stated in the release notes: never-installed Steam titles need the
  optional API key, and Epic/GOG still list installed games only.

### 3.4 Finish artwork integration and validate optional keys

The Steam Web API key path is wired: commands are registered, the **Complete Steam library** card is
mounted, and `hydrateSteam()` merges API-owned games. The SteamGridDB path is also wired: the
**Artwork** card is in Settings, and non-Steam detail pages open the picker with **Change artwork**.
Keep both labelled **optional** until the validation below is done.

- **Steam Web API key:** complete owned library, DPAPI-encrypted key, six-hour cache, privacy and
  rate-limit messages.
- **SteamGridDB artwork:** search, download to a cache, and write grid/hero/logo/icon files into
  Steam's grid folder only when the non-Steam shortcut already exists.

**Done when:**

- Commands are registered and appear in the command table. *(Done: 21 commands.)*
- A real key is used once for validation; no key ever reaches logs, localStorage or a command
  response.
- The artwork card/picker is keyboard/controller accessible and covered by screenshots.
- The privacy copy explains what leaves the machine and when, including the stale-cache refresh.
- Failure paths (invalid key, private profile, HTTP 429, offline, missing shortcut) are tested.

### 3.5 Make the installer trustworthy

The current NSIS installer is unsigned, so Windows SmartScreen may warn on first run.

**Done when:**

- A code-signing certificate is configured for the release build.
- The installer and portable executable have published checksums.
- The release page explains the SmartScreen status and how to verify the download.
- A clean Windows 10 and Windows 11 machine both install, launch and uninstall without leftovers.

### 3.6 Hardware and accessibility pass

**Why:** the final visual pass had no physical controller, and the app targets a living-room input
model.

**Done when:**

- Xbox, DualSense and Switch Pro pads are each tested wired and, where possible, Bluetooth.
- Focus order, focus visibility, reduced motion, high-DPI scaling and 1024×640 minimum window size
  are checked.
- The keyboard-only path can reach every action, including search, favorites, idle preview and
  settings.
- Any mismatch between detected glyphs and physical buttons is documented with the pad name.

---

## 4. Candidates (not committed)

These are parked until the items above are complete. They are listed so they are not mistaken for
shipped features.

- **Playtime insights:** a per-game history or simple charts. The data already exists, but a chart
  is new UI and a new performance surface.
- **Tray icon and optional autostart:** useful for a living-room launcher, but it adds background
  behavior and a second startup path that must be tested.
- **Library watch mode:** an opt-in, cheap refresh when Steam writes a new manifest. This conflicts
  with the current no-polling rule unless it is carefully bounded.
- **Additional stores:** any new store must fit the `LibrarySource` contract and bring fixtures.
- **Localization:** the UI is English-only today; a full pass needs more than string extraction
  because of the controller hint copy.
- **Portable settings export/import:** useful for backups, but it must not export secrets.

---

## 5. Explicit non-goals

Kudos is a launcher, not a store client. These are out of scope unless the product direction
changes:

- Downloading, patching or verifying game files. Kudos hands install/launch requests to the real
  store client.
- DRM bypass, account sharing or piracy.
- Cloud save sync, friends, chat or a social layer.
- Managing store accounts or switching Steam accounts inside Kudos.
- macOS, Linux or ARM builds.
- Telemetry, ads or a required online account.
- A bundled browser runtime. Kudos uses the system WebView2.

---

## 6. How work moves on this roadmap

A candidate becomes “next” when it has a clear user problem and an owner. A next item becomes
“shipped” only when:

1. The behavior is implemented without weakening an existing contract.
2. The relevant tests and the full gate set pass.
3. The user guide, troubleshooting page and changelog match the actual build.
4. Known validation gaps are written down instead of being hidden.

For the current architecture, see [Architecture](ARCHITECTURE.md). For user-facing steps, see the
[User Guide](USER-GUIDE.md).
