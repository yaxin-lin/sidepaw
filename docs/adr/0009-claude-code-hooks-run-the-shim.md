# 9. Claude Code hooks run the shim, not an HTTP request

Status: accepted, 2026-10-29

## Context

Until now the Claude Code hooks were `http` handlers posting to
`127.0.0.1:<port>` with the bearer token written into
`settings.json`. Claude Code reports a failed HTTP hook as a
non-blocking error, so whenever the pet was not running (not started
yet, quit, crashed, being rebuilt) every prompt and every tool call
printed `connect ECONNREFUSED 127.0.0.1:<port>` into the transcript.
The same form also needed `env.NO_PROXY` in the user's settings,
because Claude Code sent the loopback request through a system proxy,
and could not hook `SessionStart`, which only takes command handlers.

The `sidepaw hook --host <host>` shim already existed for other hosts:
it reads the port and token from `state.json` and exits 0 whatever
happens. Claude Code's command hooks have an exec form (`command` plus
`args`) that spawns a program directly with no shell, which is also
what its documentation recommends on Windows, where the shell would be
PowerShell when Git Bash is missing.

## Decision

Claude Code's handlers are command hooks in exec form:
`{"type": "command", "command": "<this program>", "args": ["hook",
"--host", "claude-code"], "timeout": 5}` (60 for `PermissionRequest`),
on every event the adapter knows, `SessionStart` included.

The shim waits a second less than the hook's timeout, prints the
server's body when it answers 200 (a decision on a held permission
request, in the same JSON an HTTP hook would return) and exits 0 in
every case, printing nothing when the app is closed. It bypasses any
proxy itself.

`<this program>` is the path of the running app (or `sidepaw`
binary). Handlers are ours when their `args` are the shim's; the
earlier HTTP handlers are still recognised as ours, as stale, and
replaced at the next start. A path inside macOS's App Translocation
is refused rather than written.

## Consequences

- With the pet closed Claude Code shows nothing and loses about 10 ms
  per event to starting the shim.
- The token is no longer in `settings.json`; `state.json` holds it.
- Moving or deleting the app leaves hooks pointing at a missing
  program, which Claude Code reports. Starting the app from its new
  place rewrites them; Setup's Remove deletes them.
- A dev build and an installed app rewrite the hooks to themselves in
  turn, as they did with the port before.
- `env.NO_PROXY` is no longer added. One that an earlier version added
  stays; it is harmless.
- Claude Code versions without exec form hooks are not supported.
