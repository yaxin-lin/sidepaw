# Changelog

All notable changes to this project will be documented in this file.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and the project uses [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- Workspace skeleton: `sidepaw-core` (event model, state machine) and
  `sidepaw-daemon` (local ingest server).
- Architecture document, contracts and initial ADRs.
- `sidepaw-daemon` is now a library with a thin `sidepaw` binary, so
  the desktop app can embed the server (ADR 6, proposed).
- ADR 7 (proposed): the brain lives in the desktop app in TypeScript.
- `docs/roadmap.md`, a status table in `docs/architecture.md`, and the
  standard community files (code of conduct, security policy, issue and
  pull request templates).
- Chinese README (`README.zh-CN.md`).

- `apps/desktop`: Tauri 2 pet window (transparent, borderless, always
  on top, draggable) with a canvas spritesheet renderer for Codex-layout
  packs, polling the embedded server through a Tauri command.
- `sidepaw-daemon`: `server::start` returns a handle so the desktop app
  can embed the server; `server::run` stays for headless use.
- TypeScript bindings generated from the Rust model with ts-rs
  (`pnpm gen:types`), checked for staleness in CI.
- `pets/placeholder`: a generated stand-in pack and the stdlib-only
  scripts that produce it and the app icon (`tools/`).
- CI builds the frontend and the Tauri crate on all three platforms.
- Click-through: the renderer sends a coarse opacity mask of each frame
  to Rust, which polls the cursor and toggles `set_ignore_cursor_events`,
  so the transparent parts of the window do not block the desktop while
  the pet stays draggable.
- Self-update: release builds check the GitHub release feed at startup
  and install silently (`tauri-plugin-updater`). A tag-triggered
  `release` workflow builds installers for macOS (both architectures),
  Linux and Windows into a draft release. See `docs/releasing.md`.

### Fixed

- Approving a Claude Code permission prompt left the pet in `Waiting`
  until the turn ended, because Claude Code sends no event on approve.
  A tool finishing or failing now clears `Waiting`.
- The hook shim no longer routes its loopback request through
  `HTTP_PROXY`; a local proxy that rejects localhost traffic would have
  dropped every event.
