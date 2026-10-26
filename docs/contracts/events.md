# Event contract

Every host adapter produces events of this shape and posts them to the
daemon as JSON. The Rust type is `sidepaw_core::Event`; the JSON Schema
is `event.schema.json` next to this file and is the source of truth.

```json
{
  "v": 1,
  "host": "claude-code",
  "session": "3f2a…",
  "cwd": "/home/me/project",
  "ts": "2026-10-25T13:04:05.123Z",
  "kind": "tool_start",
  "tool": { "name": "Bash", "category": "shell" },
  "text": "npm install",
  "error": null,
  "tokens": null,
  "raw": null
}
```

## Fields

| Field | Required | Notes |
| --- | --- | --- |
| `v` | yes | schema version, currently `1` |
| `host` | yes | `claude-code`, `codex`, `opencode`, `dsh`, `kimi`, `gemini`, `cursor`, `wrapped` |
| `session` | yes | host's session or thread id, opaque string |
| `cwd` | no | working directory; dropped below level 2 |
| `ts` | yes | RFC 3339 with milliseconds, UTC |
| `kind` | yes | see below |
| `tool` | for tool kinds | `name` as the host reports it, `category` as classified by the adapter |
| `text` | no | prompt, command, or last assistant message; dropped below level 2 |
| `error` | for error kinds | `type` and optional `message`; `message` dropped below level 2 |
| `tokens` | no | `{ "in": n, "out": n }` when the host reports usage |
| `raw` | no | the host's native payload; stored only at level 2 and above, never sent to a model |

## Kinds

| Kind | Meaning | State machine effect |
| --- | --- | --- |
| `session_start` | a session began or resumed | create session, `Idle` |
| `prompt` | user submitted a prompt | `Thinking` |
| `thinking` | host reports reasoning without a tool | `Thinking` |
| `tool_start` | tool call began | `Tool` |
| `tool_end` | tool call finished successfully | back to `Thinking` |
| `tool_fail` | tool call failed | `Error` |
| `approval_wait` | host is blocked on the user | `Waiting` |
| `approval_done` | user answered | clear `Waiting` |
| `turn_end` | assistant turn completed | `Done` |
| `error` | turn or session level failure | `Error` |
| `interrupt` | user interrupted | `Idle` |
| `session_end` | session closed | remove session |
| `note` | informational, no state effect | none |

Adapters must map every native event to exactly one kind or drop it. An
adapter that cannot observe `approval_wait` must document that in its
module doc comment.

## Tool categories

`shell`, `edit`, `read`, `search`, `web`, `other`. Classified by the
adapter from the native tool name with a per-host table; unknown names
are `other`.

## Redaction

Before storage the daemon replaces matches in `text`, `error.message`
and every string in `raw` with `[REDACTED:<type>]`. Types: `key`,
`token`, `password`, `email`, `ip`. The patterns live in
`sidepaw-daemon/src/redact.rs` with a fixture of positives and
negatives. Redaction cannot be disabled.
