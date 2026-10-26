## What

One sentence. Link the issue if there is one.

## Why

Only if the diff does not make it obvious.

## Checklist

- [ ] `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` pass locally
- [ ] Tests added or updated next to the code
- [ ] New host payloads captured into `crates/sidepaw-daemon/tests/fixtures/<host>/`
- [ ] `CHANGELOG.md` updated under Unreleased, if users will notice
- [ ] New dependency: justified below
- [ ] Changed a contract in `docs/contracts/`: linked ADR
