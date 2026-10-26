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

Requires Rust 1.80+, Node 22 and pnpm. On Linux the Tauri system
dependencies are listed in `.github/workflows/ci.yml`.

```
pnpm install
pnpm gen:types                          # TypeScript types from the Rust model
cargo test --workspace
pnpm --filter sidepaw-desktop tauri dev # the pet window
```

The window shows the placeholder pack from `pets/`. On first run, or on
right-click, a setup window offers to install the Claude Code hooks into
`~/.claude/settings.json` (a backup is written next to it). The same
thing from a terminal: `sidepaw install --host claude-code --apply`.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

MIT. Pet assets under `pets/` carry their own licenses; see each pack's
`pet.json`.
