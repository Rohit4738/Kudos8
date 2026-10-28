## Summary

<!-- What changed, and why? Link the issue or TARGET.md task if there is one. -->

## Related issue / task

<!-- e.g. Closes #12, or "TARGET.md D7" -->

## Type of change

- [ ] Bug fix
- [ ] New feature
- [ ] Refactor / cleanup (no behavior change)
- [ ] Documentation
- [ ] CI, build or release tooling
- [ ] Dependency update

## How was this tested?

<!--
List the commands you ran and the result. For UI changes, say which flow you
clicked through in `npm run tauri dev` and attach screenshots below.
-->

```text
npm run check
npm run rust:check
npm run rust:test
npm run build
```

## Screenshots / recordings

<!-- Required for visible UI changes. Before/after is ideal. Remove personal data. -->

## Definition of Done (TARGET.md §9)

- [ ] `npm run check` — 0 errors, 0 warnings
- [ ] `npm run rust:check` — clean
- [ ] `npm run rust:test` — all tests pass
- [ ] `npm run build` — production frontend bundle builds
- [ ] `npm run tauri build` — green (required when Rust, Tauri config, icons or packaging changed)
- [ ] `npm run tauri dev` — the changed flow works and the console has no panic
- [ ] My task-board row in `TARGET.md` is updated with the command and result
- [ ] No files outside my ownership changed without a Notes entry
- [ ] Shared contracts (`Game`, `Platform`, `LibrarySource`, command names/arguments) are unchanged, or the change is documented and approved
- [ ] No new npm or Cargo dependency without a Notes entry
- [ ] No secrets, telemetry, or network calls at startup
- [ ] Screenshots/GIF attached for visible UI changes
- [ ] `CHANGELOG.md` updated under `[Unreleased]` for user-facing changes
- [ ] PR title follows Conventional Commits (it becomes the squash-merge subject)

## Notes for reviewers

<!-- Anything risky, unverified, or intentionally left out of scope. -->
