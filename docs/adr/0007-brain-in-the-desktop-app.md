# 7. The brain lives in the desktop app, in TypeScript

Status: proposed, 2026-10-26

## Context

The brain (persona compiler, context builder, LLM client, proactive
scheduler, memory) is the part of sidepaw that will change most
often: prompts are tuned, trigger rules are adjusted, providers are
added. It is also tightly coupled to the UI, which previews every
remote call and shows the lines the pet says. The ingest path, by
contrast, is stable and must be fast and small because the hook shim is
spawned on every tool call.

## Decision

- Rust owns the plumbing: events, state machine, redaction, permission
  filter, store. This is `sidepaw-core` and `sidepaw-daemon`.
- TypeScript in `apps/desktop` owns the brain. The server exposes
  read-only queries (recent events, today's counters) that respect the
  permission level; the brain never writes events.
- Types shared between the two sides (`Event`, `State`, `Level`, the
  pack manifest) are generated from the Rust definitions so the contract
  has one source. The tool for this is chosen when `apps/desktop` is
  scaffolded.
- No third language. Anything that does not fit Rust or TypeScript is
  a sign the design needs another look, not another runtime.

## Consequences

Prompt and rule changes ship without touching Rust. Contributors who
know only one of the two languages can still work on a whole feature
within their half. The audit log and the preview-before-send rule from
ADR 4 are enforced in the TypeScript LLM client, so that client must be
the only place that talks to a model.
