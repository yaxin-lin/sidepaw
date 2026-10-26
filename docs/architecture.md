# Architecture

This document describes how sidepaw is put together and where the
boundaries are. Decisions with alternatives are recorded in
[adr/](adr/); interface definitions are in [contracts/](contracts/).
Anything marked **planned** does not exist yet; the order it will be
built in is in [roadmap.md](roadmap.md).

## Status

| Part | State |
| --- | --- |
| `sidepaw-core`: event, state machine, permission filter, pack manifest | built, tested |
| `sidepaw-daemon`: HTTP ingest, SQLite store, redactor, `hook` shim | built, tested |
| Claude Code adapter | built, hand-written fixtures |
| Other host adapters | planned |
| `config.toml`, per-host and per-project permission levels, `raw` size cap | built (0.3) |
| SSE state stream | not needed in-process; `GET /v1/state` snapshot stays for the headless path |
| System notifications (level 3+), level picker in the setup window | built |
| Allow / Deny from the bubble (level 4, per-host `act`), `approvals` audit | built |
| Chinese UI (system language): web views, tray, notifications | built |
| Idle behaviour: walks, dozing, click hop | built |
| Speech bubble with local lines from the pack | built (0.4) |
| Persona compiler (sliders, tags, free text to system prompt) | built (0.4), no UI yet |
| Brain: LLM client, line generation, chat, context, proactive, memory | planned |
| `apps/desktop`: transparent always-on-top window, spritesheet renderer, embedded server, TS types from Rust | built (0.1) |
| Click-through of transparent pixels, self-update, release workflow | built (0.2) |
| Setup window: install, update, remove Claude Code hooks | built (0.2) |
| Tray icon: session monitor, attention icon and count, setup, show/hide, quit | built |
| Daily report from counters (`GET /v1/report/today`, setup window) | built (0.3) |
| Bundled pet packs | `pets/placeholder`, a generated stand-in until a real pack lands |

## Overview

```
hosts            Claude Code   Codex   OpenCode   DSH   Kimi
                 http hook     hook    SSE        log   hook
                      \          |       |        tail   |
                       \         |       |         |     |
                        v        v       v         v     v
server          +---------------------------------------------------+
(Rust lib)      |  adapters  -> redactor -> permission gate -> store |
                |                                   |               |
                |                          state machine (per session)
                +---------------------------------------------------+
                                          |  HTTP on 127.0.0.1
                                          v
desktop         +---------------------------------------------------+
(Tauri)         |  renderer  chat panel  setup wizard  studio       |
                |  brain: persona  context  llm  proactive  memory  |
                +---------------------------------------------------+
                                          ^
pet packs       pet.json  spritesheet.webp  persona.*  lines.json
```

Two processes: the hosts (not ours) and the desktop app, which embeds
the server library ([ADR 6](adr/0006-embed-server-in-desktop-app.md),
proposed). The `sidepaw` binary runs the same library headless for
tests and servers, and doubles as the hook shim for hosts that only
support command hooks. The brain is planned for the desktop side
([ADR 7](adr/0007-brain-in-the-desktop-app.md), proposed).

## Data flow

1. A host emits a native event (hook stdin JSON, SSE message, log line).
2. The adapter for that host turns it into a `sidepaw_core::Event`
   (see `contracts/events.md`) and posts it to the daemon.
3. The redactor replaces secrets in any free-text field.
4. The permission gate resolves the level from the host and `cwd`
   (`config.toml`) and drops fields that level does not allow (see
   `contracts/permissions.md`).
5. The store appends the event to SQLite.
6. The state machine for that session consumes the event and may emit a
   state change.
7. The desktop app reads the state (snapshot today, SSE planned) and
   animates.
8. Planned: the brain reacts to state changes (proactive scheduler) and
   serves chat, building context from the store.

Everything after step 2 is host-independent. Adding a host means
writing one adapter and one fixture directory.

## Crates and apps

### sidepaw-core

Pure model, no I/O, no async. Anything here must be unit-testable with
plain values.

- `event`: the unified `Event` type and `Kind` enum
- `state`: per-session state machine and priority rules
- `pack`: pet pack manifest parsing and validation
- `permission`: level definitions and the field filter

### sidepaw-daemon

Library plus a thin `sidepaw` binary. Owns the listening socket,
SQLite and the adapters.

- `server`: HTTP ingest (`POST /v1/events`, `POST /v1/native/{host}`),
  `GET /v1/state` (snapshot) and `GET /v1/report/today`, bound to
  `127.0.0.1` with a bearer token
- `config`: `config.toml` and the effective permission level per event
- `install`: writes, checks and removes hook configuration in a host's
  settings file
- `clock`: the only place that asks what time it is
- `store`: SQLite via rusqlite, schema in `contracts/schema.sql`
- `adapters/`: one module per host
- `home`: the data directory and `state.json` (port and token)
- `hook`: `sidepaw hook --host <h>` reads one native payload from
  stdin and posts it, so users never install a script;
  `sidepaw install --host <h>` prints the hook snippet

### apps/desktop

Tauri 2 app, vanilla TypeScript. A 192 by 240 transparent, borderless,
always-on-top window draws the active pack's spritesheet on a canvas and
polls the embedded server's snapshot twice a second through a Tauri
command. The pet is its own drag handle. `src-tauri` embeds
`sidepaw-daemon` as a library (ADR 6) and starts it on a free port in
`setup`; hook payloads still arrive over HTTP from the hosts.

Shared types (`State`, `Snapshot`, `Event`, ...) are generated from the
Rust definitions with ts-rs into `src/generated` (`pnpm gen:types`), so
the renderer cannot drift from the model.

Click-through works per pixel: after each drawn frame the renderer
sends a 4 by 4 logical-pixel opacity mask to Rust (`hit.rs`), and a
poller reads the OS cursor position every 40 ms and toggles
`set_ignore_cursor_events`. The cursor has to come from the OS because
the web view receives no mouse events while the window is ignoring
them.

Release builds check the GitHub release feed once at startup and update
silently (`update.rs`); dev builds never do. `docs/releasing.md` has the
release procedure.

Right-click on the pet, or the first run with no hooks installed, opens
the setup window (`setup.html`), which calls the `host_*` commands.

The tray icon (`tray.rs`) is the way back in once the pet is hidden;
the app has no dock icon on macOS.

`idle.ts` moves the window for short walks on the running rows while
idle, slows and fades the loop while asleep, and plays the jumping row
on click; any agent activity cancels a walk.

`lines.ts` picks pack lines (uniform, avoiding the last three) and
`bubble.ts` decides when to speak: states that need the user or close a
turn always do, thinking and tool calls at most once per 30 and 60 s.
`brain/persona.ts` compiles `persona.json` plus `persona.md` into the
system prompt that chat and line generation will share; `personaDigest`
is the `generatedFrom` cache key for `lines.json`.

Planned here: the chat panel, the studio, and the brain (ADR 7).

## Host adapters

| Host | Mechanism | Thinking | Tool | Waiting | Done | Error |
| --- | --- | --- | --- | --- | --- | --- |
| Claude Code | `http` hooks in `~/.claude/settings.json`, written by the setup window | `UserPromptSubmit` | `PreToolUse` / `PostToolUse` | `PermissionRequest` | `Stop` | `StopFailure`, `PostToolUseFailure` |
| Codex | `~/.codex/hooks.json`, shim via `--hook` | `UserPromptSubmit` | `PreToolUse` / `PostToolUse` | `PermissionRequest` | `Stop` | `PostToolUse` with error |
| OpenCode | `GET /event` SSE on the local server | `session.status: busy` | `session.next.tool.*` | `permission.asked` | `session.idle` | `session.error` |
| DSH | tail `~/.dsh/sessions/**/session.v*.jsonl.zstd` | `turn/start` | `tool/call` / `tool/result` | tool awaiting approval | `turn/end` | `isError` |
| Kimi Code | `[[hooks]]` in `~/.kimi-code/config.toml`, shim | `TurnStarted` | `PreToolUse` / `PostToolUse` | `PermissionRequest` | `Stop` | `StopFailure` |

Only the Claude Code row is implemented. The others are the intended
mapping and will land one at a time, each with real captured fixtures.

Claude Code sends no event when the user approves a permission prompt
(the order is `PreToolUse`, `PermissionRequest`, the tool runs,
`PostToolUse`), so the state machine treats the tool finishing as the
answer. `PermissionDenied` is the only explicit answer.

Claude Code does not allow `http` handlers on `SessionStart`, so it is
not registered; the session is created by its first other event. The
setup window (`install.rs`) merges one `http` handler per event into the
user's settings, recognises its own handlers by URL, and backs the file
up before every write.

Candidates not yet mapped: Gemini CLI (`~/.gemini/settings.json` hooks),
Cursor (`~/.cursor/hooks.json`), and hosts without hooks (MiniMax Code),
which would be wrapped with a planned `sidepaw run -- <command>` that
reports working while the command runs and done when it exits.

## State machine

One instance per session. Priorities, highest first:

```
Waiting > Error > Tool > Thinking > Done > Idle > Sleeping
```

- `Done` decays to `Idle` after 10 s.
- `Idle` decays to `Sleeping` after 120 s without events.
- Busy states expire when the host goes silent, because interrupting a
  turn or closing a terminal sends no event: `Thinking` after 3 min,
  `Tool` after 15 min (a Bash call can run 10), `Error` after 2 min, all
  to `Idle`. `Waiting` never expires.
- `Waiting` is set by `approval_wait` and cleared by `approval_done`,
  by the tool finishing or failing, or by the turn ending. A new tool
  starting while a prompt is open does not clear it.
- `Error` survives `tool_end` but yields to any new activity:
  `tool_start`, `prompt`, `thinking` or `turn_end`.
- `Tool` carries a category (shell, edit, read, web, other) derived from
  the tool name so the renderer can pick a sub-animation.
- A multi-turn goal task only emits `Done` on its final turn.

When several sessions are active, the snapshot's `overall` is `Waiting`
if any session waits for the user, otherwise the state of the session
the host reported on most recently. Recency beats priority because
several terminals are usually one person switching between them, and a
turn interrupted with Esc in one terminal (no event for that) must not
mask the live one. One pet per session remains a possible renderer
choice.

## Brain (planned)

Lives in the desktop app (ADR 7). The server exposes read-only queries
over the store; the brain never writes events.

- Persona compiler: `persona.json` sliders and tags plus `persona.md`
  free text compile into a system prompt, and once per change into
  `lines.json` (local lines per state) through the configured model.
- Context builder: the last N events for the active sessions plus
  today's counters, summarised to under 1k tokens. Respects the
  permission level: at L0 and L1 it contains no file paths or commands.
- LLM client: one provider at a time (official API, Ollama, or any
  OpenAI-compatible URL). Chat is streamed; lines generation is batched.
- Proactive scheduler: triggers (agent events, time, user presence,
  persona) pass through a budget (`chatty` 0 to 5, quiet hours,
  fullscreen detection, 24 h topic dedup) and a feedback loop (dismissed
  triggers cool down, answered triggers warm up).
- Memory: a list the user can see and edit. Entries have a source
  (user said, pet observed) and a retention (forever, 30 d, 7 d).

## Pet packs

A pack is a folder. The baseline is the Codex pet format (`pet.json` plus
a 1536 by 1872 `spritesheet.webp`, 8 columns by 9 rows of 192 by 208
cells, one row per state). sidepaw reads that format unchanged and
adds optional files for animation layers and personality. See
`contracts/pet-pack.md`.

## Security

- The server listens on `127.0.0.1` only and requires a bearer token
  generated on first run and kept in `~/.sidepaw/state.json`. The
  token also has to appear in plain text in each host's hook config, so
  a keyring would add nothing; its job is to stop web pages from
  posting to localhost, not to hide the token from local processes.
- Pet packs and sound packs are untrusted input; manifests are validated
  and images are re-encoded before use.
- The redactor runs before anything is stored. Patterns: API keys,
  bearer tokens, passwords in URLs, email addresses, private IPs.
- Planned: every request to a remote model is logged to the `audit`
  table with the byte count and a hash of the payload. The chat panel
  shows what will be sent before it is sent.
- Level 3 and up: `notify.rs` in the desktop app watches session states
  and posts a system notification for waiting, error and done, naming
  only the host. The level is changed from the setup window and written
  to `config.toml` with comments preserved (`toml_edit`).
- Level 4 with `[hosts.<host>] act = true`: `post_native` holds a
  PermissionRequest hook open for `act_window_s` seconds and the desktop
  bubble offers Allow and Deny; the click is the confirmation, the
  answer goes back in the hook response and is written to `approvals`.
  Unanswered prompts fall through to the host's own prompt. Sending
  free-form messages to an agent and allow lists are still planned.

## Known gaps

- `raw` above 64 KB is dropped rather than stored (Claude Code's `Edit`
  and `Write` payloads carry whole files). The rest of the event stays.
- `schema.sql` mentions a `migrations/` directory that does not exist
  yet; the schema is applied once with `IF NOT EXISTS`.
