# 3. Use the Codex pet pack format as the baseline

Status: accepted, 2026-10-25

## Context

OpenAI's Codex ships a pet feature with a documented package format:
`pet.json` plus `spritesheet.webp`, 1536 by 1872, 8 columns by 9 rows of
192 by 208 cells, one row per state (idle, running-right, running-left,
waving, jumping, failed, waiting, running, review). petdex, agentpet and
CoPet all read this format, and there are public galleries of packs.
dsh-pet uses a richer but heavier format (one VP9-alpha webm per action,
100 or more files).

## Decision

Read the Codex format unchanged as the baseline. Put sidepaw's
additions (personality files, optional animation layers, attribution)
in optional files and under an `x-sidepaw` key in `pet.json` so other
readers ignore them.

## Consequences

Hundreds of existing packs work on day one, and packs made with
sidepaw's studio can be submitted to existing galleries. Expressiveness
is limited to nine states until the optional animation layer exists.
