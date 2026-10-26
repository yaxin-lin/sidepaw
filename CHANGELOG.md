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
- Setup window (first run, or right-click the pet) that installs,
  updates or removes the Claude Code hooks in `settings.json` with a
  backup; `sidepaw install --host claude-code --apply` does the same
  from a terminal. Handlers are identified by URL, so install is
  idempotent and remove leaves other hooks alone.
- The server reuses its previous port across restarts so installed hook
  configs stay valid.
- `~/.sidepaw/config.toml` is read: global level, per-host `enabled`
  and `level`, per-project `level` by `cwd` prefix, and the daemon port.
  Unknown keys are rejected. Events from disabled hosts are dropped.
- `raw` payloads above 64 KB are not stored.
- Menu bar / tray icon with Setup, Show/Hide pet and Quit. On macOS the
  app runs as an accessory, so no dock icon.
- System notifications with sound when an agent waits for approval,
  errors or finishes a turn, at permission level 3 and up, at most once
  per session and state per minute. The setup window gains a permission
  level picker that writes `[pet] level` back to `config.toml` without
  touching the rest of the file, and a test button.
- Speech bubble: the pet says a line from the pack's `lines.json` on
  state changes and clicks, picking uniformly while avoiding the last
  three, throttled so tool calls and thinking do not become a ticker.
  `pets/placeholder` gained hand-written lines and a `persona.json`.
- Persona compiler (TypeScript, ADR 7): sliders, trait tags and free
  text become one system prompt; tags map to behaviour rules. No model
  is called yet; this is the input for chat and line generation.
- Vitest for the desktop app, run in CI.
- Daily report from counters alone: sessions, turns, tool calls,
  errors, tokens, time spent waiting on the user, most used tools. In
  the setup window and at `GET /v1/report/today`. Works at every
  permission level and needs no model.
- Installing Claude Code hooks also exempts loopback from proxies via
  `env.NO_PROXY` in `settings.json`; a global-mode system proxy would
  otherwise swallow every hook request ("socket hang up").

### Fixed

- A session that went silent while `Thinking`, in a `Tool` or in `Error`
  (interrupted turn, closed terminal) kept the pet busy forever, since
  hosts send no event for that. Those states now expire to `Idle`.
- With several terminals open, the pet showed the highest-priority
  state across all sessions, so an interrupted terminal masked the one
  being used. It now follows the most recently active session, except
  that a session waiting for approval always wins.

- Approving a Claude Code permission prompt left the pet in `Waiting`
  until the turn ended, because Claude Code sends no event on approve.
  A tool finishing or failing now clears `Waiting`.
- `StopFailure` payloads were read with the wrong field names; `error`
  is the type and `error_details` the message.
- The Claude Code hook block no longer sets `async` (command-only) and
  no longer registers `SessionStart`, which rejects `http` handlers.
- The hook shim no longer routes its loopback request through
  `HTTP_PROXY`; a local proxy that rejects localhost traffic would have
  dropped every event.
