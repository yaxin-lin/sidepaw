# Releasing

Releases are built by `.github/workflows/release.yml` when a `v*` tag is
pushed. Nothing is published automatically: the workflow creates a
draft GitHub release with the installers and `latest.json`, and a
person publishes it.

## One-time setup

- Updater signing key. Generate with
  `pnpm --filter sidepaw-desktop tauri signer generate -w ~/.tauri/sidepaw.key`
  and keep the private key out of the repository. The public key lives
  in `apps/desktop/src-tauri/tauri.conf.json` under `plugins.updater`.
- Repository secrets: `TAURI_SIGNING_PRIVATE_KEY` (the private key file
  contents) and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` (empty if the key
  has none). Without them the release build fails at the signing step.
- Later, for signed installers: Apple Developer ID certificate and
  notarization credentials, and a Windows code-signing certificate.
  Until then macOS users must right-click, Open on first launch and
  Windows shows a SmartScreen warning; say so in the release notes.

## Cutting a release

1. Bump the version in the three places that carry it:
   `Cargo.toml` (`workspace.package.version`),
   `apps/desktop/src-tauri/tauri.conf.json` (`version`),
   `apps/desktop/package.json` (`version`).
2. Move the `Unreleased` section of `CHANGELOG.md` under the new
   version with today's date.
3. Commit as `release: vX.Y.Z`, tag `vX.Y.Z`, push both.
4. Wait for the `release` workflow, open the draft release, check the
   assets (one bundle per platform plus `latest.json` and `.sig` files),
   paste the changelog section into the body, publish.

Installed apps check `latest.json` on the next launch and update
themselves. Pre-1.0, only the latest release is supported.
