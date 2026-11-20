# Publish Mnemosyne through GitHub Pages

The app uses the same deployment architecture as the owner's other apps: GitHub repository → Actions → GitHub Pages. Vercel is not used.

1. Repository: `marcusdhaynes/Mnemosyne` (created). Initial source is restored from a bootstrapped archive on first commit.
2. Commit the contents of the Mnemosyne source project directly to the repository's default `main` branch. The `.github/workflows/pages.yml` file **must** be included.
3. Open the repository on GitHub. Go to **Settings → Pages → Build and deployment → Source** and select **GitHub Actions**.
4. Go to **Actions → Publish Mnemosyne Web Preview** and observe the run triggered by the source commit, or select **Run workflow** after configuring Pages if a run needs to be retriggered.
5. When the deployment completes successfully, visit `https://marcusdhaynes.github.io/Mnemosyne/` (unless a custom domain is configured). This URL is not live until deployment completes.

The Pages workflow runs five organizer tests, produces the static `dist/` build, checks expected assets and relative paths, archives a copy of that revision for 90 days, and skips deployment if the commit is no longer the current `main`. Git history permanently preserves committed source revisions, subject to repository retention. Historical Actions artifacts expire after 90 days.

The site is a functional early preview. It is local-first and operates without Supabase. Do not upload sensitive photographs to an untrusted device or shared browser: browser IndexedDB is per-origin and not end-to-end encrypted. A dedicated Supabase project can be added separately after account/organization approval.
