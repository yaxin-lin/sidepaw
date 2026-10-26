# sidepaw

Desktop pet that reacts to AI coding agents (Claude Code, Codex,
OpenCode, DeepSeek Harness, Kimi Code). Rust workspace plus a Tauri app.

Read before changing anything:

- `docs/architecture.md` for the data flow and module boundaries
- `docs/adr/` for why things are the way they are
- `docs/contracts/` for the event schema, SQLite schema and pet pack
  format. These are stable interfaces; change them only with a new ADR.

## Commands

```
cargo test --workspace                                 # run everything
cargo clippy --workspace --all-targets -- -D warnings  # must be clean
cargo fmt                                              # before every commit
cargo deny check                                       # licenses and advisories
cargo run -p sidepaw-daemon -- serve --port 0          # headless server, random port
pnpm install && pnpm gen:types                         # desktop deps and TS types from Rust
pnpm --filter sidepaw-desktop tauri dev                # run the pet window
```

`apps/desktop/src/generated` is produced by ts-rs; never edit it by
hand, rerun `pnpm gen:types` after changing a shared Rust type. CI
fails if it is stale.

## Conventions

- Follow `CONTRIBUTING.md`. It is short; read it in full.
- Comments explain why. Do not restate the code in a comment.
- Doc comments only on public items. No doc comment that just repeats
  the item name.
- Plain names. No `Manager`, `Helper`, `Utils`, `Service` unless the
  thing really is one.
- Do not add abstractions for hypothetical future needs. Two concrete
  implementations before a trait.
- Do not add dependencies without saying why in the PR.
- No emoji in code, comments, commit messages or docs.
- Tests next to the code. Fixtures of real host payloads live in
  `crates/sidepaw-daemon/tests/fixtures/<host>/`.
- Commit messages follow Conventional Commits. Subject only unless the
  body adds something the diff does not show.
- `README.md` describes what exists today. Do not write about planned
  features there; use `docs/architecture.md` and label them as planned.

## Boundaries

- `sidepaw-core` has no I/O and no async. It is the model: events,
  state machine, pack format, permission levels.
- `sidepaw-daemon` is a library that owns the socket, SQLite and host
  adapters; `main.rs` is a thin wrapper. Keep logic out of `main.rs`.
- `apps/desktop` embeds the server library and renders. It does not
  read host files directly. The brain (prompts, triggers, LLM client)
  belongs there in TypeScript, not in Rust (ADR 7). `src-tauri/src/lib.rs`
  only wires commands to the library; no logic there.
- `docs/architecture.md` describes what exists; anything else is marked
  planned and ordered in `docs/roadmap.md`. Keep the status table there
  current.
- Secrets never go into SQLite or logs. The redactor runs before the
  store, see `docs/contracts/events.md`.

## When unsure

Ask, or open a draft PR with the question in the description. Do not
guess at host payload shapes; capture a real one into fixtures first.
