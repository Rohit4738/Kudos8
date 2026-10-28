# Contributing to Kudos

Thanks for helping. Kudos is a small, local-first Windows launcher: a Tauri 2
(Rust) shell around a Svelte 5 + TypeScript UI. It scans the Steam, Epic and GOG
libraries already on the machine, shows them in one controller-friendly library,
and can add non-Steam games to Steam.

Please read [`TARGET.md`](TARGET.md) first. It is the source of truth for the
shared contracts (`Game`, `Platform`, `LibrarySource`, Tauri commands), the task
board, known issues and the manual verification checklist. This guide covers the
practical workflow; `TARGET.md` wins if they disagree.

## Non-goals

Game downloads, store browsing, accounts, cloud sync and social features are out
of scope. Kudos does not run a background service, does not collect telemetry,
and makes no network request at startup.

## Prerequisites (Windows 10/11 x64)

| Tool | Version / notes |
| --- | --- |
| Node.js | 24 LTS (CI uses Node 24; 22 also works) |
| Rust | stable MSVC toolchain, `x86_64-pc-windows-msvc` |
| Visual Studio Build Tools | "Desktop development with C++" workload, or the Windows SDK import libraries |
| WebView2 | Evergreen runtime (preinstalled on Windows 11) |
| Git | any recent version |

Install the Rust toolchain if you do not have it:

```powershell
rustup toolchain install stable-x86_64-pc-windows-msvc
rustup default stable-x86_64-pc-windows-msvc
rustup component add rustfmt clippy
```

If Visual Studio is installed but the Windows SDK import libraries (`kernel32.lib`,
`rc.exe`) are missing, run:

```powershell
npm run setup:windows-sdk
```

That downloads the official Microsoft SDK NuGet packages into `.tools/`
(git-ignored) and wires them into the build through `scripts/msvc.mjs`.

## Build and run

```powershell
npm install
npm run tauri dev       # dev window with Vite HMR
npm run tauri build     # optimized exe + NSIS installer
```

Outputs:

- `src-tauri/target/release/kudos.exe` — portable executable
- `src-tauri/target/release/bundle/nsis/Kudos_<version>_x64-setup.exe` — per-user NSIS installer

**Always use the npm wrappers.** `scripts/msvc.mjs` loads the MSVC environment
(`vcvars64.bat`) before cargo or the Tauri CLI runs. A bare `cargo build` in a
normal shell fails with `linker link.exe not found`; that is expected and is not
something to "fix" in `Cargo.toml`.

### npm scripts

| Command | What it does |
| --- | --- |
| `npm run dev` | Vite dev server only (browser preview) |
| `npm run build` | Production frontend bundle into `dist/` |
| `npm run check` | `svelte-check` (TypeScript + Svelte + a11y) |
| `npm run rust:check` | `cargo check` in `src-tauri/` |
| `npm run rust:test` | `cargo test` (unit + registry integration tests) |
| `npm run rust -- <args>` | Any other cargo subcommand, with the MSVC environment loaded |
| `npm run tauri dev` | Tauri dev window |
| `npm run tauri build` | Release exe + NSIS installer |
| `npm run setup:windows-sdk` | Fetch Windows SDK import libraries into `.tools/` |

### Run all gates locally

CI and the release workflow run the same gates on a Windows runner. To run them
on your machine, use the standalone helper (it is intentionally not added to
`package.json`):

```powershell
node scripts/ci/verify-local.mjs            # every gate, including `tauri build`
node scripts/ci/verify-local.mjs --quick    # skip the slow packaging gate
node scripts/ci/verify-local.mjs --strict   # make advisory fmt/clippy failures fatal
node scripts/ci/verify-local.mjs --fail-fast
```

The script prints a pass/fail table and exits non-zero if a hard gate fails.
`--quick` is useful while iterating; run the full command before opening a PR.

### Preview without the desktop shell

```text
http://localhost:1420/?mock=1
```

Mock data is opt-in only. There is no dev-mock default and no localStorage
fallback in a real build.

## Branch and commit style

Branch from `main` with a short, descriptive name:

```text
feat/steam-owned-games
fix/add-to-steam-refusal
ci/windows-cache
docs/contributing
```

New work uses [Conventional Commits](https://www.conventionalcommits.org/):

```text
feat(library): add an itch.io source
fix(steam): preserve unknown fields when rewriting shortcuts.vdf
ci: cache cargo registry on Windows
docs: document the Windows SDK fallback
```

- Types: `feat`, `fix`, `docs`, `style`, `refactor`, `perf`, `test`, `build`,
  `ci`, `chore`, `revert`.
- Use an optional scope (`steam`, `epic`, `gog`, `ui`, `library`, `ci`, `release`).
- Write the subject in the imperative mood, at most ~72 characters.
- Explain *why* in the body; link the issue or `TARGET.md` task in the footer.
- Keep commits small and focused on one concern.

The `Session X:` prefixes in the history are from the original parallel build.
They are historical; do not rewrite them and do not use them for new work. The
PR title becomes the squash-merge subject, so make it a valid Conventional
Commit.

## Pull requests

1. Keep the change focused. If it touches a shared contract, open a discussion
   first and update `TARGET.md` §3 in the same PR.
2. Fill in `.github/pull_request_template.md`, including the Definition of Done
   checklist.
3. Make sure CI is green. The Windows workflow runs `npm ci`, `npm run check`,
   `npm run rust:check`, rustfmt (advisory), clippy (warn-only), `npm run
   rust:test`, `npm run build`, `npm run tauri build`, and uploads the installer
   and portable exe as artifacts.
4. Attach before/after screenshots or a short recording for visible UI changes.
5. Update `CHANGELOG.md` under `[Unreleased]` for user-facing changes.
6. Do not commit `dist/`, `src-tauri/target/`, `.tools/`, logs, secrets, or a
   personal `shortcuts.vdf`.

### Current lint status

`rustfmt --check` and `clippy` are reported by CI but do not fail the build yet:
the existing tree has pre-existing formatting drift and a handful of clippy
lints. New code should still be formatted and lint-clean. Once the backlog is
cleared, CI can switch to `-D warnings` without changing contributor workflow.

## Adding a new store source

Read [`src-tauri/src/sources/mod.rs`](src-tauri/src/sources/mod.rs) and
`TARGET.md` §3.2 first. Every store is one implementation of the frozen
`LibrarySource` trait:

```rust
pub trait LibrarySource: Send + Sync {
    fn platform(&self) -> Platform;
    fn installed(&self) -> SourceResult<Vec<Game>>;
    fn launch(&self, game: &Game) -> SourceResult<()>;
}
```

Steps:

1. **Implement the trait** in a new module, for example
   `src-tauri/src/sources/itch.rs` (or a `src-tauri/src/itch/` directory for a
   larger parser). Keep parsing, caching and launch logic in that module.
2. **Follow the rules:**
   - Missing store client → `Ok(Vec::new())`. Return
     `Err(SourceError::Unavailable)` only when the client exists but its data
     cannot be read.
   - Never panic. A panic in a source can kill the app because commands run on
     the UI path.
   - `launch(&game)` must reject games from another platform with
     `SourceError::Invalid`, and should use `crate::launch::open` so `.exe` paths
     and protocol URIs both work.
   - IDs are stable and namespaced: `<platform>:<store-id>` (for example
     `steam:730`). The UI relies on them for selection, favorites and
     Add-to-Steam.
   - Return `Game` unchanged; do not add ad-hoc fields to the wire format.
   - Do not block `list_games` on network I/O. Cache local parses by file
     `(mtime, len)` where it matters.
3. **Register the source once** in `src-tauri/src/state.rs` (`AppState::new`).
   The frontend never sees source objects; it only calls the Tauri commands.
4. **Add tests with committed fixtures** under
   `src-tauri/tests/fixtures/<store>/`. Cover the happy path, a missing client,
   malformed files, duplicate entries and launch rejection. CI has no store
   clients installed, so tests must pass with fixtures alone.
5. **Cover art:** return either an `https://` URL or a local file path. Local
   paths are served through Tauri's asset protocol and must be inside a
   configured scope (or granted at runtime by `list_games`); widening that scope
   is a security decision, so explain it in the PR.
6. **New dependencies:** keep them minimal. Add a note in `TARGET.md` before
   adding a crate or npm package; do not add a UI framework, state library,
   router library or HTTP client crate.

Adding a **new platform** (not just another source for Steam/Epic/GOG) is a
shared contract change: `Platform` has to gain a variant in both
`src-tauri/src/models.rs` and `src/lib/types.ts`, the UI filters and badges need
updating, and `TARGET.md` §3 must be amended. Coordinate that with the
maintainers before writing code.

## Code style

- TypeScript is strict; prefer explicit types at module boundaries.
- Svelte 5 uses runes (`$state`, `$derived`, `$effect`); no stores or UI
  frameworks. Animate `transform` and `opacity` only, and respect
  `prefers-reduced-motion`.
- Rust is edition 2021. Prefer small functions, explicit error handling and
  `SourceResult` over `unwrap`/`expect` on user data.
- Windows paths come from environment variables or registry reads. Guard every
  file read so a missing folder returns an empty list rather than an error.
- Comments should explain *why* something is done, not restate the code.

## Security

- No secrets, tokens, telemetry, or network calls at startup.
- Local achievement and playtime reads stay local. The opt-in Steam metadata
  sync is the only user-triggered network call; keep it that way.
- Do not include personal data (Steam IDs, account names, full home paths) in
  issues, screenshots or fixtures.
- Report vulnerabilities privately as described in [`SECURITY.md`](SECURITY.md),
  not in a public issue.

## License

Kudos is MIT-licensed. By contributing you agree that your contribution is
provided under the same license.
