# Security policy

Kudos is a local Windows desktop app. It has no accounts, no telemetry, no
cloud service, and makes no network request at startup. It reads local store
data (Steam, Epic, GOG), launches games through the installed clients, and can
write a non-Steam shortcut into Steam's `shortcuts.vdf` when the user asks it to.

## Supported versions

| Version | Supported |
| --- | --- |
| 0.1.x | Yes |
| < 0.1 | No |

Pre-1.0 releases are supported on a best-effort basis. Security fixes are made
on `main` and included in the next release.

## Reporting a vulnerability

**Please do not open a public issue for a security problem.**

Use GitHub's private vulnerability reporting:

<https://github.com/Rohit4738/Kudos8/security/advisories/new>

If that page is unavailable, open a minimal public issue that only asks for a
private contact channel; do not include exploit details, personal data, or
proof-of-concept files in it.

Please include:

- Kudos version and how it was installed (NSIS or portable exe).
- Windows version and build (`winver`).
- A description of the impact and the conditions required to trigger it.
- Reproduction steps or a minimal proof of concept.
- Any relevant logs or screenshots with personal information removed.

Maintainers aim to acknowledge a report within 7 days and to agree on a
disclosure timeline with the reporter. Please give us a reasonable window to
ship a fix before publishing details.

## Security model and attack surface

Kudos is a Tauri 2 app. The Rust side owns filesystem, registry and process
access; the WebView only calls the commands exposed in
`src-tauri/src/commands.rs`. The main areas to review are:

- **Local parsers** for Steam VDF/ACF, Epic JSON manifests and GOG registry
  records. Malformed input must return an error or an empty list, never panic.
- **Steam shortcut writer**: reads and rewrites the binary `shortcuts.vdf`,
  preserves unknown fields, makes a `.kudos-backup`, refuses unparseable files,
  and refuses to write while Steam is running.
- **Process launching**: launch commands come from parsed store data or a stable
  `steam://` / `com.epicgames.launcher://` URI. `LibrarySource::launch` rejects
  games from another platform.
- **Asset protocol scope**: local cover art is limited to Steam's
  `appcache/librarycache`; exact files are granted at runtime. Widening the
  static scope in `tauri.conf.json` increases the local file exposure and needs
  a security review.
- **Content Security Policy**: the WebView CSP allows only local scripts and
  images from `self`, `asset:`, `https:` and `data:`/`blob:`; there is no remote
  script execution.

## Out of scope

- The installer is unsigned. Windows SmartScreen warnings are a known release
  engineering limitation, not a vulnerability.
- Issues that require administrator access, physical access, or an already
  compromised machine.
- Vulnerabilities in Steam, Epic Games Launcher, GOG Galaxy, WebView2, or
  Windows itself. Report those to the respective vendor.
- Social engineering and denial-of-service against a local single-user app with
  no server component.
- Missing hardening that does not cross a trust boundary (for example, a local
  user reading their own game library files).

## Dependencies

Kudos keeps its dependency surface small. Dependency advisories can be checked
with:

```powershell
npm audit
cargo audit        # if cargo-audit is installed
```

Please report a vulnerable dependency through the private channel above if it
is exploitable in Kudos' usage; otherwise an issue is fine.

## No bug bounty

This is a community project. There is no paid bug bounty, but responsible
reports are credited in the release notes if the reporter wants to be named.
