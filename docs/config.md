# Configuration

One file: `~/.sidepaw/config.toml`. Every key has a default; a missing
file is valid; an unknown key is an error so typos do not pass silently.
The setup wizard will write `[model]` and `[hosts]`; the rest is for
people who like editing files.

Read today: `[ui]`, `[model]` (the setup window writes both), `[pet]`
`level`, `size`, `dnd`, `mini` (the tray writes the last three),
`[hosts.*]`, `[projects.*]`, `[daemon] port`. The effective level for an
event is the lowest of the global level, the host's `level`, and the
`level` of the longest `[projects."..."]` path that contains the event's
`cwd` (matched on path segments, so `/work` covers `/work/x` but not
`/workshop`). `[quiet]` and `chatty` are parsed but nothing acts on them
yet.

```toml
[ui]
language = "system"          # system | en | zh-CN: windows, tray, notifications

[model]
provider = "deepseek"        # deepseek | openai | anthropic | ollama | custom
model    = ""                # empty picks the provider default:
                             # deepseek-chat, gpt-5-mini, claude-opus-5-5
# base_url = "http://127.0.0.1:11434/v1"   # ollama / custom only
# The API key is never stored here. It lives in the OS credential store
# (the setup window puts it there), or in the SIDEPAW_API_KEY
# environment variable, which wins when set.

[pet]
active = "whale-maid"
level  = 1                   # 0..4, see docs/contracts/permissions.md
chatty = 2                   # 0..5
size   = "m"                 # s | m | l (tray > Size)
dnd    = false               # do not disturb: no notifications, no unprompted lines
mini   = false               # parked at the right screen edge

[quiet]
from = "23:00"
to   = "08:00"

[hosts.codex]
enabled = true               # default true for every host
level   = 1                  # optional cap for this host

[hosts.claude-code]
act          = true          # level 4 only: answer permission prompts from the pet
act_window_s = 15            # seconds the bubble gets before the terminal prompt

[hosts.kimi]
enabled = false

[projects."/work/company-repo"]
level = 0

[daemon]
port = 0                     # 0 picks a free port; recorded in state.json
```

Data directory layout:

```
~/.sidepaw/
  config.toml
  state.json        port and token of the running server
  sidepaw.sqlite    events, state changes, approvals, audit rows
  pets/<id>/        persona.json, persona.md, lines.json overriding the pack's
```

Override the directory with `SIDEPAW_HOME`.
