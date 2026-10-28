# Changelog

All notable changes to this project will be documented in this file.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and the project uses [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- A welcome window on first run, in place of the full setup window:
  connect Claude Code in one click, pick level 1, 2 or 3, then start a
  session and watch the first event arrive. Finishing or skipping it
  sets `[ui] welcomed`; Setup has a link to open it again.
- The pet knows when you are away from the OS idle counter (macOS and
  Windows, no permission; Linux still watches the cursor), so typing
  without touching the mouse counts as being there. After five minutes
  without input and nothing running it dozes; when you are back after
  fifteen minutes it waves and says what finished, failed or is
  waiting for you (`back.waiting`, `back.errors`, `back.done` lines,
  with `{minutes}`, `{waiting}`, `{errors}`, `{done}`). The welcome
  back now speaks from chattiness 2.

### Changed

- The database is upgraded by numbered migrations
  (`crates/sidepaw-daemon/migrations/`), each applied once and recorded,
  instead of re-running the whole schema with `IF NOT EXISTS`. Before
  an upgrade the old database is copied to `sidepaw.sqlite.v<n>.bak`,
  and a database written by a newer sidepaw is refused rather than
  changed. Existing databases are already at the latest version.
- Minimum Rust is 1.88 (was 1.80), for zip 9: older zip releases have
  extraction advisories, and pet packs are untrusted downloads.

### Fixed

- The pet could end up off every screen (found after opening and
  closing chat in mini mode, at about 1980,1571 on a 1728x1080 screen)
  and seem gone. A check every three seconds now brings it back to the
  bottom right of the nearest screen when less than 30 % of it is
  visible, and logs where it was so the cause can be traced.

- After a restart the server waits up to three seconds for its previous
  port instead of moving to a new one, and rewrites its own Claude Code
  hooks when they still point at an old port, so hooks no longer fail
  with `ECONNREFUSED` after the app comes back.

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
- Allow / Deny from the pet (level 4, per host): with `act = true` for
  a host, the server holds its PermissionRequest hook open for
  `act_window_s` seconds (default 15) while the bubble shows the tool,
  the command and two buttons. A click answers Claude Code through the
  hook response; no click returns an empty response so the terminal
  prompt appears as usual. Every answer is written to the new
  `approvals` table (schema version 2). The PermissionRequest handler's
  timeout is now 60 s, so existing installs read as stale until updated.
- Chinese UI: the setup window, the pet's status label, the tray menu
  and notifications follow the system language (zh-* gives 简体中文, anything else English). Packs may
  ship `lines.<lang>.json` beside `lines.json`; the placeholder has
  Chinese lines.
- Idle behaviour: every 20 to 60 s of idleness the pet walks a short
  way on the running rows, staying inside the monitor's work area and
  stopping the moment an agent does anything; it dozes (half speed,
  slightly faded) while asleep; a click gets a hop on the jumping row.
- Tray session monitor: the menu lists every live session with host,
  project folder (when the level keeps `cwd`), state and duration,
  sessions needing input first; the icon turns orange and shows a count
  while any session waits for approval.
- On macOS, notifications go through `osascript` because the plugin's
  NSUserNotificationCenter backend is dropped silently by current macOS
  for an ad-hoc signed bundle; the sender shows as Script Editor until
  code signing lands.
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
- Desktop life: throw the pet and it flies, bounces off the edges of
  the work area and lands on the nearest thing under it: another app's
  window or the bottom of the screen. When that window moves or closes
  it falls. It steps away from a cursor that lingers next to it, sizes
  up a cursor that hovers, says a line (`drop`, falling back to
  `click`) when a file is dropped on it, and hides while the app in
  front is full screen. macOS and Windows; no OS permission is asked.
- More life in every pet: it breathes at rest (slower when asleep),
  crouches before a hop and squashes on landing, leans into a walk,
  and swings like a charm while you drag it. The bundled blob is
  redrawn: smooth edges, shading, a face with pupils that look where
  it is going, blinks, happy eyes when waving, crossed eyes and a
  sweat drop on errors, a question mark when it waits, circling dots
  while thinking, a magnifying glass while reading, and a full crouch,
  stretch and squash jump.
- The pet moves on its own: it rides the window it stands on when you
  drag it, strolls only along the visible part of its top edge, does a
  small hop now and then with a line that invites you to click or
  chat, and hops once when a session starts waiting. It knows the
  kind of app in front by name: quiet in Zoom, Teams, Tencent Meeting
  and FaceTime, a dance when music comes up, and now and then a line
  when you switch apps (Tray > Chattiness 3 and up).
- Live lines (Setup > Model, off by default): when an agent waits for
  minutes, fails twice or finishes a long task, the model writes the
  pet's line from what it can see. Turning it on shows the exact
  request once and needs a yes (ADR 8); at most 20 a day, audited,
  falling back to the pack's lines when slow or off.
- After three days of use at the default level the pet asks once, in
  character, whether it may read error messages (level 2); clicking
  opens Setup at the permission levels. It asks at most twice.
- The pet says which project: the label reads "sidepaw · waiting",
  permission bubbles and notifications name the project, and lines
  about a waiting or failing agent carry it when several sessions run.
  Works from level 1; only the folder name is kept, in memory.
- Pet packs: Setup > Look shows every pet with a preview and switches
  between them at once. Install from a zip or folder, bring over pets
  from Codex's or petdex's folders, search and install from the petdex
  gallery (5,000+ pets), drop a zip on the pet, or run
  `sidepaw pack <zip|folder>`. petdex v2 sheets work. Packs without
  lines use the default lines until you generate their own.
- The pet speaks first: nine moments (an agent waiting over three
  minutes, two errors in a row, a long task done, the day's report, a
  morning greeting, late night, two hours without a break, welcome
  back, small talk), gated by Tray > Chattiness 0-5, quiet hours,
  do-not-disturb and per-moment cooldowns that double when you close a
  line with its cross and halve when you click it. Lines come from the
  pack; no model call.
- Memory: Setup > What it remembers lists everything, each editable,
  deletable and kept forever, a month or a week. Chat uses the list,
  the pet can note a fact itself (kept a month, announced in chat), and
  "记住…" / "remember …" adds one without a model.
- Permission prompts held by the pet queue oldest first: the bubble
  shows a count of the rest and a countdown to the terminal's own
  prompt. Control+Option+Y / Control+Option+N (Ctrl+Alt on Windows and
  Linux, `[keys]` in config.toml) answer from any app; they are
  registered only while a prompt waits.
- Usage history (Tray > Usage History): totals, tool calls per day and
  a per-project table for the last 7, 30 or 90 days, from the local
  store only. Days are local days; projects are folder names, so
  sessions stored below level 2 show as one unrecorded row.
- `sidepaw run -- <command>`: the pet shows any command as busy, then
  done, failed or interrupted; the exit status is passed through. The
  installed app is the CLI (it runs it when started with a
  subcommand), and Setup > Command line puts it on PATH.
  `docs/integrations.md` explains posting events from any tool.
- Tray toggles: Do Not Disturb (no notifications or unprompted lines;
  clicks and permission prompts still work), Mini Mode (the pet parks
  at the right screen edge and slides out on hover or when an agent
  waits), Size (small, medium, large) and Start at Login. The first
  three are saved in `[pet]`.
- Jump to the terminal: click the pet while an agent waits, or a
  session in the tray, and its terminal comes to the front. The daemon
  finds the host process through the hook's loopback connection, once
  per session; on macOS the exact Terminal.app or iTerm2 tab is
  selected, otherwise the host app's window (Windows: the window of
  the nearest ancestor process).
- Language setting (`[ui] language`: follow the system, 中文, English)
  in the setup window, applied at once to every window, the tray and
  notifications. A persona without a `language` now speaks the UI
  language instead of English, and chat answers in the language the
  user writes.
- Chat: double-click the pet (or Tray > Chat) to ask it what the
  agents did. Each message carries the current sessions, today's
  counters and the last twelve stored events; the line under the log
  says how much and opens to the exact text. Replies stream in.
- Model provider: `[model]` in `config.toml` (DeepSeek, OpenAI,
  Anthropic, Ollama or any OpenAI-compatible URL, one at a time per
  ADR 5) with the API key in the OS credential store or
  `SIDEPAW_API_KEY`. The brain's LLM client is the only code that talks
  to a model; every remote call shows a preview first and writes an
  `audit` row (size and digest, never the text).
- Persona editor in the setup window: name, species, how it calls you,
  six sliders, strength and flaw tags, catchphrases, taboo topics and
  free text, saved under `~/.sidepaw/pets/<id>/` on top of the pack.
  "Try a line" and "Generate lines" write `lines.json` from the saved
  persona; the pet picks the new lines up without a restart.

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
