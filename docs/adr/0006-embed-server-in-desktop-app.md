# 6. Embed the server in the desktop app

Status: accepted, 2026-10-26. Partially supersedes ADR 2, which described
a separate daemon process.

## Context

ADR 2 made the daemon a separate process so it could run without the
desktop app. That buys a headless path for tests and servers, at the
cost of a second process, a port and token handoff through
`state.json`, and an HTTP plus SSE layer between the two halves of our
own code. A desktop pet is a long-running app by nature; when it is not
running, nobody is looking at the pet, and the daily report can be
rebuilt from events that arrive after it starts.

## Decision

- `sidepaw-daemon` is a library crate. The desktop app links it and
  runs the server inside its own process.
- The `sidepaw` binary stays as a thin wrapper: `serve` for headless
  use and tests, `hook` as the stdin-to-HTTP shim, `install` to print
  hook snippets.
- The desktop app reads state through an in-process channel. The HTTP
  `GET /v1/state` endpoint stays for the headless path and for tools.

## Consequences

One process for users, one fewer moving part to debug, and no SSE layer
to build before the first release. If a real need for a standalone
daemon appears (a shared machine, a remote host), the library split
makes it a packaging change rather than a rewrite.
