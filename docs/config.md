# Configuration

One file: `~/.sidepaw/config.toml`. Every key has a default; a missing
file is valid; an unknown key is an error so typos do not pass silently.
The setup wizard will write `[model]` and `[hosts]`; the rest is for
people who like editing files.

Read by the server today: `[pet] level`, `[hosts.*]`, `[projects.*]`,
`[daemon] port`. The effective level for an event is the lowest of the
global level, the host's `level`, and the `level` of the longest
`[projects."..."]` path that contains the event's `cwd` (matched on path
segments, so `/work` covers `/work/x` but not `/workshop`). `[model]`,
`[quiet]` and `chatty` are parsed but nothing acts on them yet.

```toml
[model]
provider = "deepseek"        # deepseek | openai | anthropic | ollama | custom
model    = ""                # empty picks the provider default
# base_url = "http://127.0.0.1:11434"   # ollama / custom only
# The API key is never stored here. It lives in the OS keyring, or in
# the SIDEPAW_API_KEY environment variable.

[pet]
active = "whale-maid"
level  = 1                   # 0..4, see docs/contracts/permissions.md
chatty = 2                   # 0..5

[quiet]
from = "23:00"
to   = "08:00"

[hosts.codex]
enabled = true               # default true for every host
level   = 1                  # optional cap for this host

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
  state.json        port, token id, first-run flag
  sidepaw.sqlite
  audit.log
  pets/<id>/
  logs/
```

Override the directory with `SIDEPAW_HOME`.
