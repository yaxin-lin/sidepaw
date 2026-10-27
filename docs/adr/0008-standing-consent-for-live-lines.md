# 8. Standing consent for model-written proactive lines

Status: accepted, 2026-10-27

## Context

ADR 4 and `contracts/permissions.md` require a preview before every
remote model call. That works for chat and line generation, where the
user starts the call. Proactive lines are not started by the user: a
line about the agent that just failed twice has to be written within
seconds of the moment, and a confirmation dialog for each one would
interrupt more than the line itself. Without a model those lines come
from the pack and cannot mention what actually happened.

## Decision

Model-written lines are a separate opt-in, `[pet] live_lines`, off by
default. Turning it on shows the exact request one such line would
send right now (persona, the trigger, the context the current level
allows) and enables it only when the user accepts; that one preview is
the standing consent for calls of the same shape. It covers three
triggers whose value is the specifics: an agent waiting for minutes,
errors in a row, a long task finishing. Every call still writes an
`audit` row (purpose `nudge`), the context is built from the store
only, so the level filter and redaction have already run, calls are
capped per day, and a slow or failed call falls back to the pack line.
Local providers need no consent, as for every other call.

## Consequences

The preview rule now reads: a preview before any remote call the user
starts, and a one-time preview with explicit consent for the one kind
of call the pet starts itself. A change to what live lines send is a
change to this consent and must be shown again; the request is built
in one function (`brain/nudge.ts`) so that stays visible.
