# Permission levels

The level is a field filter applied before storage and a capability
gate for actions. Default is 1. Override per host in `config.toml`
(`[hosts.codex] level = 0`) and per project directory
(`[projects."/work/company-repo"] level = 0`). The lowest applicable
level wins.

| Level | Name | Stored from each event | Pet may |
| --- | --- | --- | --- |
| 0 | observe | `kind`, `ts`, `tool.category` | animate, local lines |
| 1 | record | level 0 plus `tool.name`, `error.type`, `tokens`, durations | daily report, counters, reminders |
| 2 | read | level 1 plus `cwd`, `text`, `error.message`, `raw` (all redacted) | answer questions about what happened |
| 3 | notify | same as 2 | system notifications, open a file or URL on click, write the report to a folder |
| 4 | act | same as 2 | send a message to an agent, per host, each confirmed unless allow-listed |

Always, at every level:

- secret redaction before storage
- a preview of what will be sent before any remote model call
- an audit row for every remote model call
- nothing is sent to a remote model from `raw`

Level 4 details:

- off by default for every host; enabled per host with
  `[hosts.<host>] act = true`. Today this covers answering permission
  prompts from the pet: the prompt is held for `act_window_s` seconds
  (default 15), a click is the confirmation, and every answer is written
  to the `approvals` table.
- each outgoing message shows a confirmation with the exact text unless
  it matches an entry in `[hosts.<host>].allow` (exact string match)
- voice input cannot trigger level 4 without a visible confirmation
