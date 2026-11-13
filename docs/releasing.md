# Releasing WireCraft

Every push to the `release` branch runs [`.github/workflows/release.yml`](../.github/workflows/release.yml). It
builds the installers for Linux, macOS and Windows and creates (or updates) a **draft** GitHub Release named
`WireCraft v<version>`. Nobody sees a draft until a maintainer publishes it.

## Versions

WireCraft follows [SemVer](https://semver.org). The version lives in one place, `[workspace.package] version`
in the root `Cargo.toml`; the app, the Tauri bundle and `package.json` all take it from there.

| Change | Bump | Example |
|---|---|---|
| A batch of features and fixes | minor | `0.2.0 → 0.3.0` |
| Fixes only, or a packaging fix | patch | `0.3.0 → 0.3.1` |
| A release candidate to try before the real one | pre-release suffix | `0.4.0-rc.1` |
| Breaking change to rules, settings or saved files (after 1.0) | major | `1.4.2 → 2.0.0` |

Before 1.0, breaking changes go in a minor release and are called out in its notes.

```sh
cargo xtask version              # prints the current version, e.g. 0.2.0
cargo xtask version set 0.3.0    # or 0.3.0-rc.1: updates Cargo.toml, Cargo.lock, package.json, package-lock.json
```

## Cutting a release

1. **Bump the version** on `main` with `cargo xtask version set <x.y.z>` and commit the four files it touched
   (`chore: release 0.3.0`).
2. **Fast-forward `release` to `main`** and push. The workflow starts by itself.
   ```sh
   git push origin main:release
   ```
3. **Wait for the draft** (`WireCraft v0.3.0`, tag `v0.3.0` on the pushed commit) with every installer and
   `SHA256SUMS.txt`. Notes are generated from the merged PRs, grouped by label ([`.github/release.yml`](../.github/release.yml)).
4. **Check it.** A `::warning::` in the job summary means a signing secret was missing and that installer is
   unsigned. Install at least the Linux build and run through onboarding and a captured request.
5. **Publish** the draft. Versions with a pre-release suffix (`-rc.1`) are marked pre-release, so they don't
   become "Latest".

Pushing to `release` again before publishing rebuilds the same draft and replaces its assets. After
publishing, the workflow refuses to touch that version again, so bump first.

## Cadence

- **Minor releases** when a set of features is ready, typically every one to two weeks while WireCraft is
  pre-1.0.
- **Patch releases** whenever a fix shouldn't wait for the next minor.
- **Release candidates** (`-rc.N`) before a minor release that changes the proxy core, certificates or the
  settings format.
- **Nightly dry run:** at 01:40 UTC the workflow builds every platform from `main` without signing or
  releasing, and keeps the bundles as workflow artifacts for 7 days. It finds release-only breakage before
  release day.

## Test runs

Build from any branch without creating a release (artifacts are kept on the run):

```sh
gh workflow run release.yml --ref <branch> -f version=0.3.0-rc.1
```

## What gets built

| Platform | Runner | Installers |
|---|---|---|
| Linux x64 | ubuntu-22.04 | `.AppImage`, `.deb`, `.rpm` |
| macOS (Apple silicon and Intel) | macos-latest | universal `.dmg`, `.app.tar.gz` |
| Windows x64 | windows-latest | `.msi`, `-setup.exe` (NSIS) |

Bundle identifier: `dev.wirecraft.app`. File names are Tauri's (`WireCraft_0.3.0_amd64.deb`, …).

## Signing secrets

Set these as repository (or `release` environment) secrets. Each is optional: without it the installer is
built unsigned and the run says so.

| Secret | Used for |
|---|---|
| `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY` | macOS code signing (base64 `.p12`) |
| `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID` | macOS notarization (app-specific password) |

Windows and Linux installers are currently unsigned.

## Checks

- [`ci.yml`](../.github/workflows/ci.yml) runs on every pull request and push to `main`:
  `cargo xtask ci` (clippy with `-D warnings`, `cargo test --workspace`, `svelte-check`, the frontend build).
- Run the same locally before bumping: `cargo xtask ci`.
