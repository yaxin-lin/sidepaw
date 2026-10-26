# 1. Record decisions as ADRs

Status: accepted, 2026-10-25

## Context

The project borrows from several existing pets (petdex, agentpet,
CoPet, dsh-pet) and from the Codex pet format. Future contributors will
ask why a given choice was made and whether it still holds.

## Decision

Keep architecture decision records in `docs/adr/`, numbered, in the
Nygard format. A decision is superseded by a new record, not edited.

## Consequences

Slightly more writing per structural change. Much less re-litigating of
settled questions in issues.
