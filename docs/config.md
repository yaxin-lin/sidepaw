# Configuration

One file: `~/.sidepaw/config.toml`. Every key has a default; a missing
file is valid; an unknown key is an error so typos do not pass silently.
The setup wizard will write `[model]` and `[hosts]`; the rest is for
people who like editing files.

Read today: `[ui]`, `[keys]`, `[model]` (the setup window writes both), `[pet]`
`level`, `size`, `dnd`, `mini` (the tray writes the last three),
`[hosts.*]`, `[projects.*]`, `[daemon] port`. The effective level for an
event is the lowest of the global level, the host's `level`, and the
`level` of the longest `[projects."..."]` path that contains the event's
`cwd` (matched on path segments, so `/work` covers `/work/x` but not
`/workshop`). `[quiet]` and `chatty` drive
the proactive lines.

```toml
[ui]
language = "system"          # system | en | zh-CN: windows, tray, notifications
welcomed = false             # set once the first-run welcome is finished or skipped

[keys]                       # level 4 with act: answer the oldest held prompt
allow = "Control+Alt+Y"      # registered only while a prompt waits; "" turns it off
deny  = "Control+Alt+N"

[model]
provider = "deepseek"        # deepseek | openai | anthropic | claude-code | ollama | custom
model    = ""                # empty picks the provider default:
                             # deepseek-chat, gpt-5-mini, claude-opus-5-5, haiku
# base_url = "http://127.0.0.1:11434/v1"   # ollama / custom only
# claude-code runs the `claude` you are signed in to (ADR 10): no key,
# no base_url, usage counts against your plan.
# The API key is never stored here. It lives in the OS credential store
# (the setup window puts it there), or in the SIDEPAW_API_KEY
# environment variable, which wins when set.

[pet]
active = "placeholder"       # Setup > Look; a bundled or installed pack id
level  = 1                   # 0..4, see docs/contracts/permissions.md
chatty = 2                   # 0..5, tray > Chattiness: 0 only when clicked,
                             # 1 needs you, 2 + finished tasks and the day's report,
                             # 3 + greetings and breaks, 4 + small talk (3/hour), 5 chatterbox
size   = "m"                 # s | m | l (tray > Size)
dnd    = false               # do not disturb: no notifications, no unprompted lines
mini   = false               # parked at the right screen edge
live_lines = false           # the model writes the pet's unprompted lines (ADR 8)
live_shape = 0               # which preview was accepted; a changed request asks again

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
  packs/<id>/       installed pet packs (Setup > Look, `sidepaw pack <zip>`)
  pets/<id>/        persona.json, persona.md, lines.json overriding the pack's
```

Override the directory with `SIDEPAW_HOME`.
