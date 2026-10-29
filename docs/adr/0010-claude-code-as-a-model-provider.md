# 10. The user's own Claude Code as a model provider

Status: accepted, 2026-10-29

## Context

Everything the pet says beyond its pack lines needs a model (ADR 5,
ADR 7), and until now that meant an API key and a bill, or a local
Ollama. Most sidepaw users already pay for a Claude subscription and
have `claude` signed in on the same machine. The pet's calls are
small: a line is about 600 input tokens and a handful out.

Anthropic's terms draw a line we can respect: an application may not
collect, store or route requests through a user's claude.ai
credentials, but nothing prevents "an end user from signing in to the
unmodified Claude Code binary with their own Claude subscription"
(Claude Code legal and compliance page, 2026-10). Running the user's
own `claude` program, as they could from a terminal, stays on the
right side of that line; reading its OAuth token out of the keychain
would not.

Measured on Claude Code 2.1.312 with a Max sign-in:

- `claude -p` with `--setting-sources ""` loads no settings, so none
  of sidepaw's own hooks fire and the pet does not react to its own
  calls, and no CLAUDE.md is read. `--bare` would also do that but
  ignores subscription sign-ins.
- Auto memory is not covered by `--setting-sources`; a memory
  directory for the working folder was loaded. `CLAUDE_CODE_DISABLE_AUTO_MEMORY=1`
  stops that, as does a working folder Claude Code has never seen.
- A separate `CLAUDE_CONFIG_DIR` loses the sign-in; the user's real
  `~/.claude` has to be used.
- `--output-format stream-json` carries text deltas, a
  `rate_limit_event` with the five-hour and seven-day utilization, and
  a final `result`. Nothing lands in `~/.claude/history.jsonl`.
- About 1.4 s of start-up before the model answers; the `claude`
  program is not on a GUI app's PATH.

## Decision

A provider `claude-code` in `brain/llm.ts` and a module `claude_cli`
in the daemon. The desktop command `claude_cli_run` runs the `claude`
program found on PATH, in the usual install folders or by asking the
login shell, as:

```
claude -p --model <model> --effort low --max-turns 1
  --output-format stream-json --verbose --include-partial-messages
  --tools "" --strict-mcp-config --no-session-persistence
  --setting-sources "" --system-prompt <persona>
```

with the prompt on stdin, `CLAUDE_CODE_DISABLE_AUTO_MEMORY=1`, and a
fresh empty temporary folder as the working directory, removed after.
Earlier chat turns ride inside the one prompt. Deltas stream to the
web view over a channel; the reply carries the window utilization.

The default model is `haiku`. Proactive lines (`nudge`) are not sent
when the five-hour window is at 90% or more, so the pet does not spend
what the user is about to need. Preview and audit apply as to any
remote provider. The key field is hidden; Setup shows the program's
path, sign-in and plan.

## Consequences

- No API cost for subscribers; usage counts against the plan's
  windows, a small fraction of a coding session.
- The pet works only where `claude` is installed and signed in; the
  other providers stay.
- Each call pays Claude Code's start-up, acceptable for lines in the
  background and for a streamed chat.
- The sign-in is never read or stored by sidepaw. If Anthropic changes
  the rule for this use, the provider is one setting to switch away
  from.
- `claude.cmd` on Windows needs `cmd /C`; not yet tried on Windows.
- Codex (`codex exec`) could be added the same way for ChatGPT plans.
