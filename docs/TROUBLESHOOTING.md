# Kudos — Troubleshooting

This page covers the failure modes that actually exist in Kudos today. If a problem is not listed
here, include the app version, your Windows version, the store involved and the exact message when
you [open an issue](https://github.com/Rohit4738/Kudos8/issues).

> **Before anything else:** use the desktop app, not a browser preview. A plain browser tab cannot
> read Steam/Epic/GOG data or launch games; it shows "desktop app required" copy by design.

---

## No games found

**Symptom:** the library is empty, or a store shows `0` games.

**Why it happens**

- Kudos lists **installed** games from local files. It does not browse a store's online catalog.
- **Steam:** a game needs an `appmanifest_*.acf` file in a Steam library's `steamapps` folder. A
  never-installed Steam game has no manifest and is not in `list_games` unless the optional Steam Web
  API key has cached it.
- **Epic:** a game needs an installed-app `.item` manifest and its install folder must still exist.
- **GOG:** a game needs a registry entry written by GOG Galaxy or a GOG installer, and its files must
  still be on disk.
- A missing store client is treated as "no games", not an error. All six filter chips stay visible,
  and an empty store shows `0` with a friendly empty state.

**What to try**

1. Open **Settings → Steam library** and check the Steam status line. It should say Steam is
   installed and show its path.
2. Install at least one game through the store client and launch it once. Some clients only finish
   writing metadata after the first launch.
3. Choose **Rescan this PC** in **Settings → Steam library**, or restart Kudos. There is no
   filesystem watcher; new installs are picked up on refresh, not instantly.
4. Make sure the **All** filter is selected and no games are hidden under **Settings → Hidden
   games**.
5. If you are on a fresh Steam account, Steam's local catalog may genuinely be empty until you sign
   in and start the client once.

**If you own Steam games that were never installed:** the local catalog only knows what Steam has
written on this PC. The optional Steam Web API key adds the complete owned list when the profile's
game details are public. See
[Optional keys and artwork](USER-GUIDE.md#10-optional-keys-and-artwork).

---

## Steam is installed on a non-default drive

**Symptom:** Steam works, but Kudos shows no Steam games (or only some of them).

**Why it happens**

Kudos finds the Steam root from the registry (`SteamPath` / `InstallPath`) and then reads every
library listed in `libraryfolders.vdf`. A normal Steam install on `D:` or another drive is supported.
The scan breaks down when:

- the registry points at an old or removed Steam folder;
- `libraryfolders.vdf` is missing or does not list a library;
- a library is on a removable or disconnected drive;
- the library was moved manually without telling Steam.

**What to try**

1. In Steam, open **Settings → Storage** and confirm the drive/library is listed. Add it there if it
   is missing; Steam rewrites `libraryfolders.vdf` for you.
2. Reconnect any removable drive and confirm the game folders are readable.
3. Restart Steam once, then restart Kudos and choose **Rescan this PC**.
4. If Steam itself was moved, reinstall/repair Steam so its registry entries point at the new
   location. Kudos also checks the two default Steam folders as a last resort, but it does not guess
   arbitrary drives.
5. Local cover art is only read from known `appcache/librarycache` locations. A cover in an unusual
   layout may fall back to the Steam CDN or the gradient placeholder; the game itself should still
   appear.

---

## Epic or GOG not detected

**Symptom:** the Epic or GOG chip never appears, or the store is empty.

**Be aware of the validation status:** on the development machine **neither Epic Games Launcher nor
GOG Galaxy is installed**. Their scanners and launchers are covered by unit tests and committed
fixtures, but they have **not** been exercised against a real Epic/GOG installation. Treat a
real-client mismatch as a bug worth reporting, not as something the UI can fix.

**Epic**

- Kudos reads installed-app manifests from Epic's default manifest folder under
  `%PROGRAMDATA%\Epic\EpicGamesLauncher\Data\Manifests`.
- Only `.item` manifests with a launch executable/command and an existing install folder count.
  Incomplete installs, DLC, plugins and engine assets are skipped.
- Install the game through Epic, launch it once, then rescan. A library-only (not installed) Epic
  game is not shown.

**GOG**

- Kudos reads the registry keys written by GOG Galaxy and GOG installers under
  `HKLM\SOFTWARE\WOW6432Node\GOG.com\Games`, `HKLM\SOFTWARE\GOG.com\Games` and
  `HKCU\Software\GOG.com\Games`.
- A registry key alone is not enough: the install folder must still exist. Library-only GOG games do
  not appear.
- Install the game through GOG Galaxy or its installer, launch it once, then rescan.
- If you use a portable GOG install that never wrote registry keys, Kudos cannot see it. Add it to
  Steam manually, or use a store install that registers itself.

If the client is installed and a game still does not appear, include the manifest/registry details
in an issue. Do not edit the registry unless you are comfortable doing so.

---

## "Close Steam first" when adding a game to Steam

**Symptom:** **Add to Steam** fails with a message saying Steam is running and to close it
completely.

**Why it happens**

Steam keeps `shortcuts.vdf` in memory and rewrites it when it exits. If Kudos wrote the file while
Steam was running, Steam's exit would silently discard the new shortcut. Kudos refuses instead of
risking your library.

**What to try**

1. Exit Steam from **Steam → Exit**, not just the window's close button.
2. Open the Windows tray, right-click the Steam icon and choose **Exit**.
3. Confirm `steam.exe` is gone in **Task Manager → Details**. Wait a few seconds for it to fully
   exit.
4. In Kudos, open the GOG/non-Steam game and choose **Add to Steam** again.
5. On success, **reopen Steam** and look under **Library** for the new non-Steam shortcut.

If it still refuses, reboot or sign out and back in, then try once with Steam never started. Kudos
checks for a running `steam.exe`; background Steam helper processes alone are not the same thing, but
a stuck `steam.exe` will block the write.

**Other Add-to-Steam failures**

- **"No signed-in Steam account found"** — sign in to Steam once, close it, then try again.
- **"No local .exe path"** — Steam shortcuts need a local executable. A game that only has a
  protocol URI, or whose files were removed, cannot be added.
- **"Already a Steam game"** — Steam titles are already in Steam; the button is only for non-Steam
  games.
- **Unparseable `shortcuts.vdf`** — Kudos refuses to overwrite a file it cannot read. Restore Steam's
  own file or remove the damaged copy before retrying. A `shortcuts.vdf.kudos-backup` from an earlier
  successful write is safe to keep or delete manually.

---

## Achievements show nothing

**Symptom:** the detail page has no achievements card, or says achievements are not available.

**Why it happens**

Kudos reads **local Steam files only**:

- `appcache/stats/UserGameStats_<accountid>_<appid>.bin` for unlocked flags;
- `appcache/stats/UserGameStatsSchema_<appid>.bin` for the achievement total.

There is no Steam Web API call and no API key involved. If either file is missing, locked,
half-written or uses an unexpected layout, Kudos reports `supported: false` instead of guessing. Epic
and GOG have no local achievement provider in Kudos.

**What to try**

1. Launch the game once through Steam with Steam running, then give Steam a moment to write its
   stats cache.
2. Reopen the game's detail page in Kudos. Counts are re-read on each visit (cached by file
   modification time and length), not pushed live.
3. Check that you are looking at the **active** Steam account. Kudos uses the most recent
   `loginusers.vdf` profile and its `userdata` folder.
4. Some games genuinely have no achievements, or store them only online. In that case there is
   nothing local to show.
5. Epic/GOG titles show "Achievements not available for this platform" by design.

If a game shows achievements in Steam but not in Kudos, include the game name and the two file names
from `appcache/stats` in an issue.

---

## Controller not detected

**Symptom:** **Settings → Controller** says **No controller detected**, or the glyphs do not match
the pad.

**Why it happens**

Kudos uses the browser **Gamepad API** exposed by WebView2. It only sees pads that Windows and
WebView2 expose with a gamepad mapping, and it only updates while the Kudos window has focus. Some
pads, especially in Bluetooth mode, report a generic name or do not appear until a button is pressed.

**What to try**

1. Connect the pad **before** opening Kudos, or connect it and press a button after the window has
   focus.
2. Click the Kudos window once, then press **A / Cross** or move the left stick. The Gamepad API
   often hides a pad until first input.
3. Try a **wired** connection. Bluetooth pads sometimes expose a different device name or a
   non-standard mapping; wired/XInput mode is the most predictable.
4. Open **Settings → Controller** and check the reported pad name. If it is wrong, force the layout:
   **Xbox**, **PlayStation**, **Nintendo**, or **Keyboard**.
5. Close other apps that grab the controller exclusively (some launchers and remappers do), then
   reconnect.
6. If Windows itself does not see the pad, fix that first: open **Set up USB game controllers**
   (`joy.cpl`) and confirm the device appears and responds.

**Limits to know**

- Kudos cannot add a driver or bypass the Gamepad API. If a pad works in games but not in Kudos, the
  likely cause is its WebView2/Gamepad API mapping.
- No physical Xbox/PlayStation/Nintendo pad was available on the development machine. Detection
  patterns and the layout override are covered by preview fixtures, not a hardware matrix.
- Multiple pads: Kudos prefers a pad with the standard mapping and otherwise uses the first
  connected one. Disconnect the extra pad if the wrong one is picked.

---

## High memory use

**Symptom:** Task Manager shows Kudos using a few hundred MB, even when idle.

**Why it happens**

Kudos is a small native Rust shell around **Microsoft Edge WebView2**. The WebView2 runtime and its
GPU process set the memory floor; the Rust executable itself is only a few MB.

Measured on the development machine (Windows 11, release build):

| Part | Private memory |
| --- | --- |
| `kudos.exe` | ~6.3 MB |
| WebView2, empty/placeholder shell | ~175 MB |
| WebView2, final build with three local 600×900 covers decoded | ~244 MB |

That is the fixed WebView2/Edge runtime and GPU floor, not unbounded growth in Kudos. A strict
sub-100 MB total is not achievable with this stack.

**What to try**

1. Check **Task Manager → Details** and group by app. The `msedgewebview2.exe` children belong to
   WebView2; they are expected.
2. Close other WebView2-based apps; each app has its own process group and GPU cache.
3. Update the **Microsoft Edge WebView2 Runtime** through Windows Update or Microsoft's installer.
4. Restart Kudos after a very long session if you want to release decoded cover memory. There is no
   polling loop or background scan that keeps growing.
5. If memory is far above the numbers above at idle, or keeps climbing without interaction, report
   it with a Task Manager screenshot and the number of games in the library.

Do not disable WebView2 or the GPU process to chase a lower number; that breaks rendering and is not
a supported configuration.

---

## SmartScreen warns about the installer

**Symptom:** Windows shows "Windows protected your PC" or "Unknown publisher" when running
`Kudos_0.1.0_x64-setup.exe`.

**Why it happens**

The installer is **unsigned**. There is no code-signing certificate configured for this project, so
SmartScreen cannot verify the publisher and may warn on first run, especially for a new download.

**What to try**

1. Confirm you downloaded the installer from the official
   [GitHub releases page](https://github.com/Rohit4738/Kudos8/releases/latest).
2. Choose **More info → Run anyway** if you trust the download.
3. Do **not** disable SmartScreen globally or add broad exclusions. If you need a managed
   deployment, sign the build with your organization's certificate.

A signed installer is a release-engineering task, not a bug in the app. See the
[roadmap](ROADMAP.md#35-make-the-installer-trustworthy).

---

## Resetting preferences

**Symptom:** you want a clean slate — accent, background, favorites, hidden games, filter, idle
settings, recent list or saved API keys.

**What to try first (targeted)**

- **Accent colour → Reset** returns the accent to the default.
- **Idle mode → Reset** returns idle settings to their defaults.
- **Settings → Favorites** removes one favorite or clears all.
- **Settings → Hidden games → Unhide all** restores hidden titles.
- Change the filter back to **All** on the library screen.

**Full reset**

Preferences such as accent, background, density, sort, filter, favorites, hidden games and idle
settings live in the WebView's `localStorage`. Recent launches, the Steam name cache, the optional
complete-library cache, the artwork cache and the DPAPI key vault live under `%APPDATA%\Kudos`. The
WebView profile lives under `%LOCALAPPDATA%\com.kudos.launcher`.

1. Close Kudos completely (check Task Manager for `kudos.exe`).
2. To reset app data, delete `%APPDATA%\Kudos`. This removes `recent.json`, `steam-apps.json`,
   `steam-owned-api.json`, `art\` and `secrets.bin` (including any saved API keys).
3. To reset the UI profile and `localStorage`, delete `%LOCALAPPDATA%\com.kudos.launcher`.
4. Reopen Kudos. It recreates both folders and performs a fresh scan.

The uninstaller intentionally keeps user data, so uninstalling and reinstalling alone will **not**
reset preferences. Deleting these folders is permanent; there is no cloud backup.

---

## Steam API key returns an empty list

**Status:** optional. The **Complete Steam library** card is in **Settings** in this build.

**Symptom:** a 32-character Steam Web API key is saved, but the owned-games list is empty or the card
shows an error.

**Why it happens**

Steam's `IPlayerService/GetOwnedGames` returns `{"response":{}}` when the profile's game details are
private. Kudos treats that as the privacy path and shows an actionable message instead of pretending
the account owns nothing.

**What to try**

1. In Steam, open your profile → **Edit Profile → Privacy Settings**.
2. Set **Game details** to **Public**. (Private or Friends-only can produce an empty response for
   this API.)
3. Save the change in Steam, wait a moment, then press **Test** in Kudos.
4. Confirm the key is the full 32 hexadecimal characters from
   [steamcommunity.com/dev/apikey](https://steamcommunity.com/dev/apikey), with no spaces.
5. Make sure the active Steam account on this PC is the same account the key belongs to. Kudos scopes
   the cache to the local account.
6. If Steam returns HTTP 429, wait and try again. Kudos does not retry rate limits automatically.
7. If the key is rejected (HTTP 401/403), revoke it and create a new one.

A failed sync keeps the previous cache. Nothing is deleted, and the local installed library keeps
working without a key. A saved key with a cache older than six hours refreshes once when **Settings**
is opened; this is not a background poll.

---

## Artwork does not appear after applying it

**Status:** optional. The **Artwork** card is in **Settings**, and non-Steam game detail pages have a
**Change artwork** button that opens the picker.

**Symptom:** you choose a grid, hero, logo or icon, but the tile or Steam library still shows the old
image, or the card reports that Steam was not updated.

**Why it happens**

- SteamGridDB needs its own API key; a Steam Web API key does not work for artwork.
- Downloads are restricted to SteamGridDB's HTTPS hosts and must pass an image check. A proxy,
  firewall or malformed URL fails closed.
- Kudos writes the chosen image to `%APPDATA%\Kudos\art\`. Steam's `config/grid` is updated only when
  the game already has a non-Steam shortcut; Steam titles use Steam's own artwork.
- If Steam is running, it may not notice a new grid file until it restarts.

**What to try**

1. Open **Settings → Artwork** and confirm a SteamGridDB key is saved. Create one at
   [steamgriddb.com/profile/preferences/api](https://www.steamgriddb.com/profile/preferences/api).
2. Open the game's detail page and choose **Change artwork**. Apply it **after** **Add to Steam** has
   succeeded. If the game has no shortcut, the local cache is still saved but Steam cannot show it.
3. Close Steam completely, apply the art again, then reopen Steam.
4. Check the reported error. A `401`/`403` means the key is invalid; `429` means SteamGridDB is rate
   limiting; a timeout usually means the network or a proxy is blocking the request.
5. If the image is still wrong, remove the cached file under `%APPDATA%\Kudos\art\` and apply it
   again. Deleting that folder is safe; it only removes downloaded artwork overrides.

The built-in cover chain (local Steam cache → Steam CDN → gradient) still works without a key.
