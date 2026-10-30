# Publishing the Kudos landing page

`site/` is a standalone static landing page: `index.html`, `styles.css` and local files under
`site/images/`. There is no build step, no package install, no CDN, no JavaScript and no tracking.
It works from `file://` and from any static host.

The download button points at
[github.com/Rohit4738/Kudos8/releases/latest](https://github.com/Rohit4738/Kudos8/releases/latest),
so publishing the page does not publish a new app build.

> **CI ownership:** workflows under `.github/` belong to another session. Do **not** add or edit a
> GitHub Actions workflow to deploy this page. Use GitHub's built-in Pages publishing instead.

---

## Recommended: publish `site/` from a `gh-pages` branch

This keeps `main` clean and publishes only the landing page. GitHub Pages serves the branch root.

### 1. Preview locally

From the repository root:

```powershell
python -m http.server 8000 --directory site
```

Open <http://localhost:8000>. Opening `site/index.html` directly in a browser is also supported.

### 2. Publish the branch

```powershell
git switch main
git pull --rebase origin main
git subtree push --prefix site origin gh-pages
```

The first push creates the `gh-pages` branch with the contents of `site/` at its root. Later pushes
update it.

If the branch already exists and the push is rejected because the histories diverged, replace the
published site deliberately:

```powershell
git push origin "$(git subtree split --prefix site main):gh-pages" --force
```

Only use `--force` when you intend to replace the published branch. The source of truth remains
`site/` on `main`.

### 3. Enable GitHub Pages

1. Open the repository on GitHub: **Settings → Pages**.
2. Under **Build and deployment**, choose **Deploy from a branch**.
3. Set **Branch** to `gh-pages` and the folder to **/ (root)**.
4. Save. GitHub publishes at `https://rohit4738.github.io/Kudos8/` unless a custom domain is set.
5. Wait for the Pages deployment to finish, then hard-refresh the page.

No workflow file is needed. GitHub Pages serves the branch contents directly.

### 4. Verify the published page

- The page loads at the Pages URL with the dark theme, all five screenshots and the logo.
- **Download for Windows** opens the latest GitHub release.
- The footer links open the repository, releases and issues pages.
- The page works at a narrow width (360 px) and at 1600×1000.
- Browser console has no errors.
- `site/` contains no absolute local paths such as `C:\Users\...`.

---

## Alternative: publish from `/docs`

GitHub Pages can also serve the `/docs` folder on the default branch. This repository already uses
`docs/` for Markdown documentation, so this option requires a deliberate layout decision:

1. Copy the landing page into `docs/` as `docs/index.html` plus `docs/styles.css` and
   `docs/images/` (or change the page to reference `site/` paths).
2. In **Settings → Pages**, choose **Deploy from a branch**, branch `main`, folder **/docs**.
3. Keep the Markdown documentation files alongside it, or move them to a separate docs site.

This mixes the product page and the engineering docs in one published folder. The `gh-pages` branch
is cleaner for Kudos and is the recommended option.

---

## Custom domain (optional)

If you use a custom domain:

1. Add the domain in **Settings → Pages → Custom domain**.
2. Add a `CNAME` file at the root of the published branch (not in `site/` unless you also want it in
   the source tree).
3. Configure the DNS records GitHub shows, then enable **Enforce HTTPS**.

Do not add a `CNAME` file for the default `github.io` URL.

---

## Rollback

- **Revert the page content:** restore the previous `site/` commit on `main`, then run the subtree
  push again.
- **Roll back the published branch:** force-push the previous `gh-pages` commit, or switch Pages
  back to the previous branch/folder.
- **Remove the site:** disable Pages in **Settings → Pages**. The `site/` source files can stay in
  the repository.

---

## What is intentionally not included

- No GitHub Actions deployment workflow.
- No build step or generated bundle.
- No analytics, cookies, fonts or scripts from third parties.
- No automatic upload of the installer. The download button links to GitHub Releases.

See [Architecture](ARCHITECTURE.md) for the app, [User Guide](USER-GUIDE.md) for using Kudos, and
[Troubleshooting](TROUBLESHOOTING.md) for common issues.
