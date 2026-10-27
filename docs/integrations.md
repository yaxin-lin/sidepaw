# Connecting a tool

sidepaw has a built-in adapter for Claude Code. Anything else can talk
to it in one of two ways, without changing sidepaw.

## Wrap a command

```
sidepaw run -- npm test
sidepaw run -- aider --model sonnet
```

The pet shows a shell tool while the command runs, then done, failed
(non-zero exit) or interrupted (killed by a signal, usually Ctrl+C).
The command keeps the terminal, and `sidepaw run` exits with its exit
status, so it can go in front of anything in scripts and aliases. The
same command in the same folder is one session, so running it again
updates one row in the tray. Nothing happens to the command when the
app is not running.

`sidepaw` is the app itself. Setup > Command line > Add to PATH links
it as `~/.local/bin/sidepaw` (macOS, Linux) or `sidepaw.cmd` in the
per-user WindowsApps folder (Windows). From a source checkout,
`cargo run -p sidepaw-daemon -- run -- <command>` does the same.

## Post events

A tool with hooks, a plugin system or a log to tail can post
[unified events](contracts/events.md) itself:

```
POST http://127.0.0.1:<port>/v1/events
Authorization: Bearer <token>
Content-Type: application/json
```

The port and token are in `~/.sidepaw/state.json` while the app runs;
the port stays the same across restarts. The request must come from
the same machine. A minimal session:

```sh
state=~/.sidepaw/state.json
port=$(sed -n 's/.*"port": *\([0-9]*\).*/\1/p' "$state")
token=$(sed -n 's/.*"token": *"\([^"]*\)".*/\1/p' "$state")
post() {
  curl -s --noproxy '*' "http://127.0.0.1:$port/v1/events" \
    -H "authorization: Bearer $token" -H 'content-type: application/json' \
    -d "{\"v\":1,\"host\":\"wrapped\",\"session\":\"my-tool-1\",\"ts\":\"$(date -u +%Y-%m-%dT%H:%M:%S.000Z)\",$1}"
}
post '"kind":"prompt","text":"refactor the parser"'
post '"kind":"tool_start","tool":{"name":"edit","category":"edit"}'
post '"kind":"approval_wait"'
post '"kind":"approval_done"'
post '"kind":"turn_end"'
```

Use `host: "wrapped"` for tools sidepaw has no adapter for. `--noproxy`
matters on machines with a system proxy: a proxy that answers for
`127.0.0.1` swallows the request.

What the pet does with each `kind` is in the
[event contract](contracts/events.md#kinds). Everything that arrives
goes through the same redaction and permission level as Claude Code's
events: below level 2, `cwd`, `text` and error messages are dropped
before they are stored.

To jump to the tool's terminal from the pet, nothing extra is needed:
sidepaw finds the process behind the request, so the session's tray
row is clickable once the first event has arrived from it.

## Adding a built-in adapter

When a host has a stable native format worth supporting out of the
box, see "Adding a host adapter" in [CONTRIBUTING.md](../CONTRIBUTING.md).
