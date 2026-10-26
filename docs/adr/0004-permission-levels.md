# 4. Five permission levels, per host and per project

Status: accepted, 2026-10-25

## Context

The pet is only useful if it can see what agents do, but sessions
contain file paths, commands and error output that users may not want
stored or sent to a model. Existing pets are all-or-nothing.

## Decision

Define levels 0 to 4 (observe, record, read, notify, act). The level
is a field filter applied before storage and a capability gate for
actions. Default is 1. The level can be overridden per host and per
project directory. Secret redaction runs at every level and cannot be
disabled. Remote model calls are previewed and logged at every level.
Details in `docs/contracts/permissions.md`.

## Consequences

Chat quality at the default level is limited to counts and tool names.
Users who want "why did it fail" must opt into level 2, and the pet asks
for that in character after a few days rather than through a settings
prompt on first run.
