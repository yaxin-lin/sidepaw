# Contributing

Thanks for taking a look. This file is short on purpose; the rules that
matter are the ones people actually read.

## Workflow

1. Open an issue before a large change so we can agree on the shape.
2. Branch from `main`. Keep pull requests small; one concern per PR.
3. Run `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`
   and `cargo test --workspace` before pushing. CI runs the same, plus
   `cargo deny check` for licenses and advisories.
4. Update `CHANGELOG.md` under "Unreleased" if users will notice the change.

## Commit messages

Conventional Commits, short imperative subject, body only when the diff
does not explain itself.

```
feat(daemon): tail DSH session logs
fix(core): treat Interrupt as a terminal state
docs: add ADR for permission levels
```

## Code style

- Rust: `rustfmt` defaults, clippy clean. Prefer plain structs and
  functions over traits until there is a second implementation.
- Comments explain why, not what. Doc comments on public items only.
- Names are short and ordinary: `Event`, `Session`, `State`. No
  `Manager`, `Helper`, `Service` suffixes unless the thing is actually one.
- No speculative abstraction. If a feature needs a plugin system, add it
  when the second plugin arrives.
- Tests live next to the code they test (`#[cfg(test)] mod tests`).
  Integration tests go in `tests/`.
- Errors: `thiserror` in libraries, `anyhow` only at the binary boundary.

## Adding a host adapter

Each adapter turns one host's native events into `sidepaw_core::Event`
and posts them to the daemon. Put it in
`crates/sidepaw-daemon/src/adapters/<host>.rs`, add a fixture under
`tests/fixtures/<host>/` with real captured payloads, and document the
mapping in `docs/architecture.md`. An adapter that cannot detect
"waiting for approval" should say so in its doc comment.

## Adding a pet pack

Packs are plain folders. See `docs/contracts/pet-pack.md`. Packs
submitted to this repository must include `license` and `attribution`
in `pet.json`.

## Roadmap and scope

`docs/roadmap.md` lists what is being built and in which order. A PR for
something further down the list is welcome but may wait; open an issue
first so we can agree it fits. Good first contributions: a host adapter
with real fixtures, or a pet pack with a clear license.

## Releases

Maintainers only; see `docs/releasing.md`.

## Design decisions

Non-trivial decisions are recorded as ADRs in `docs/adr/`. If your PR
changes an existing decision, add a new ADR that supersedes the old one
rather than editing it.
