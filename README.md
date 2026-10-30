# sidepaw

[简体中文](README.zh-CN.md)

A desktop pet that watches your AI coding agents work.

sidepaw sits on your desktop and reacts to what Claude Code, Codex,
OpenCode, DeepSeek Harness and Kimi Code are doing: thinking, running a
tool, waiting for your approval, finishing, failing. You can give it any
face (it reads the Codex pet pack format) and any personality, and it
only sees as much of your sessions as you let it.

Status: pre-alpha. Nothing is released yet. See [docs/architecture.md](docs/architecture.md)
for the design, [docs/roadmap.md](docs/roadmap.md) for the order things
will be built in, and [docs/adr](docs/adr) for the decisions behind it.

## Why another one

There are already good desktop pets for coding agents (petdex, agentpet,
CoPet, dsh-pet). sidepaw differs in three ways:

- Your own character. One reference image and one sentence become a pet
  pack with a face and a personality. Works with every supported host,
  not just Codex.
- Context-aware chat. The pet knows what your agents did today, so
  "why did that last run fail?" gets a real answer.
- Permission levels. Five levels from "state only" to "may message the
  agent", per host and per project, with redaction and an audit log.

## Supported hosts

| Host | Integration | Status |
| --- | --- | --- |
| Claude Code | `http` hook | adapter built; hook config printed by `sidepaw install --host claude-code` |
| Codex | hooks via `sidepaw hook` | planned, 0.2 |
| DeepSeek Harness | session log tail | planned |
| OpenCode | `/event` SSE | planned |
| Kimi Code | hooks via `sidepaw hook` | planned |

## Layout

```
crates/sidepaw-core     event model, state machine, pack format (no I/O)
crates/sidepaw-daemon   library: local ingest server, SQLite store, host
                        adapters; plus the thin `sidepaw` binary
apps/desktop            Tauri 2 app: pet window and embedded server
docs/                   architecture, roadmap, ADRs, contracts
tools/                  stdlib-only scripts: placeholder spritesheet, app icon
pets/                   bundled pet packs (a generated placeholder so far)
```

## Building

Requires Rust 1.88+, Node 22 and pnpm. On Linux the Tauri system
dependencies are listed in `.github/workflows/ci.yml`.

```
pnpm install
pnpm gen:types                          # TypeScript types from the Rust model
cargo test --workspace
pnpm --filter sidepaw-desktop tauri dev # the pet window
```

The window starts with the placeholder pack from `pets/`; Setup > Look
installs others from a zip or from Codex's pets, has a store for browsing
the [petdex](https://petdex.dev) gallery by thumbnail (popular, newest,
by kind, by name) and adopting a pet in one click, and switches between
them. On first run, or on
right-click, a setup window offers to install the Claude Code hooks into
`~/.claude/settings.json` (a backup is written next to it). The same
thing from a terminal: `sidepaw install --host claude-code --apply`.

When an agent waits for you, click the pet and the terminal it runs
in comes to the front (the exact tab in Terminal.app or iTerm2, else
the app's window); the tray lists every session and clicking one does
the same.

It also speaks first, sparingly: when an agent has waited on you for a
while, after two errors in a row, when a long task finishes, at the end
of the day, and more as you turn up Tray > Chattiness. Close a line
with its cross and that kind comes less often. Setup > What it
remembers lists what it knows about you; chat uses it, and you can
edit or delete any of it. Click a line it said and the chat opens with
that line already in it; the chat keeps the last day of what was said,
and one click forgets it.

Tray > Usage History shows sessions, turns, tool calls, errors and
time spent waiting on you, per day and per project, for the last 7, 30
or 90 days.

Other tools: put `sidepaw run --` in front of any command, or post
events from a hook or plugin. See [docs/integrations.md](docs/integrations.md).

The pet talks from `lines.json` in its pack, so nothing needs a model.
To give it a personality of your own, the setup window has a persona
editor (sliders, tags, free text) and, once a model provider is
configured there (DeepSeek, OpenAI, Anthropic, Ollama or any
OpenAI-compatible URL, with the key in the system credential store; or
your own Claude Code sign-in, which runs the `claude` program on your
plan's usage with no key at all),
"Generate lines" writes a `lines.json` in that voice to
`~/.sidepaw/pets/<id>/`, and double-clicking the pet opens a chat
where it answers questions about what the agents did today. Every
remote call shows what it will send first and is listed under "Sent
to the model".

## Troubleshooting

- Claude Code prints `connect ECONNREFUSED 127.0.0.1:<port>` or
  `socket hang up` on every event: those are the HTTP hooks older
  versions wrote. Start sidepaw once and it replaces them with command
  hooks that stay quiet when the pet is closed; then start a new
  Claude Code session, since hooks are read at startup.
- Claude Code reports that a hook command was not found: the app was
  moved or deleted. Open it from where it is now (or click Remove in
  the setup window if you uninstalled it).

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

MIT. Pet assets under `pets/` carry their own licenses; see each pack's
`pet.json`.
