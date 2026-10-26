# Configuration

One file: `~/.sidepaw/config.toml`. Every key has a default; a missing
file is valid. The setup wizard writes `[model]` and `[hosts]`; the rest
is for people who like editing files.

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

[hosts]
claude-code = true
codex       = true
dsh         = true
opencode    = false
kimi        = false

[hosts.codex]
level = 1

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
