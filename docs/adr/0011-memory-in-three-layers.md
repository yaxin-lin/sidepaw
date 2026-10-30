# 11. Memory in three layers: facts, diary, follow-ups

Status: accepted, 2026-10-30 (chat persistence and bubbles that open
the chat shipped; follow-ups, diary and fact kinds to come)

## Context

The pet has one memory: a flat list of facts (`memories` table),
added when the model marks `[remember: ...]` in chat or the user types
"记住 ...", kept forever, 30 days or 7 days, shown and edited in
Setup, and sent (newest 30) only with chat requests. It cannot update
or drop a fact, does not separate "about the user" from "about this
project", knows nothing of yesterday, and never reaches the proactive
lines, so the pet opens every day as a stranger. The goal behind the
memory is not recall for its own sake but the feeling of talking to
someone who was there: continuity, callbacks, and coming back to ask.

What others do (surveyed 2026-10-29):

- Claude Code auto memory: one directory per project; `MEMORY.md`
  index loaded at start (200 lines / 25 KB), topic files on demand;
  four kinds (user, feedback, project, reference); `modified` stamps.
- Codex memories: asynchronous two-phase pipeline, per-session raw
  memory and summary, then a serialised consolidation into
  `MEMORY.md`, ranked by use and recency, forgetting what went unused.
- mem0: extract candidate facts, find the similar ones, let a model
  pick add / update / delete / nothing.
- OpenClaw: append-only daily notes plus a curated long-term file;
  today's and yesterday's notes loaded at start.
- Desktop pets (clawd-on-desk, OpenPets, agentpet, CoPet): none keeps
  facts about the user; clawd-on-desk keeps activity counts only and
  stores no text at all.

## Decision

Three layers, each visible and editable in Setup, each gated by the
permission level and the redactor like everything else that reaches
a model.

1. **Facts** (`memories`, as now, plus `kind` and `project`). Kinds
   follow Claude Code: `user`, `preference` (how the pet should
   behave), `project` (ongoing work in one folder), `reference`.
   Project facts go out only when that project is in the context.
   The prompt numbers the facts; the model may answer with
   `[remember: ...]`, `[update #id: ...]` or `[forget #id]`, which the
   client applies after a near-duplicate check. No embeddings: the
   whole list fits in the prompt at this size. Pinned facts (the
   column exists) go into proactive lines as well as chat.

2. **Diary** (`diary`, one row per local day and project). Written
   when the day ends or on the first chat of the next day, from the
   stored events and counters: a template at level 1 (counts only),
   the model at level 2 and above. Chat gets today and yesterday in
   full and one line for each of the last seven days; proactive lines
   get yesterday's line. This is what no other memory system has: what
   the pet actually watched happen.

3. **Follow-ups** (`followups`: due time, topic, source turn). When
   the pet asks something, or the user says "later", the model marks
   `[followup: <when>: <topic>]`; the scheduler fires a one-shot
   `followup` trigger at the due time with the topic in the prompt.
   Listed and cancellable in Setup; never fired in quiet hours or
   do-not-disturb.

Alongside, the chat keeps its own short-term memory: turns are stored
(`chat_turns`), the last 24 hours reload when the window opens, and
every line the pet said in a bubble that day is part of the history,
so it does not repeat itself and can refer back. Each new fact shows
as a note in the chat with an undo; a "don't remember this chat"
switch exists.

Order of work: chat persistence and bubbles that open the chat first,
then follow-ups, then the diary, then fact kinds and updates.

Two choices made while building the first step: only unprompted
(scheduled) lines are stored, not the reactions to clicks, drags and
state changes, which are frequent and canned; and bubble lines reach
the model as a dated list in the system prompt rather than as history
turns, since providers want the user to speak first and the list
keeps the "do not repeat yourself" use in one place.

## Consequences

- More model calls per day (one diary entry, follow-ups); affordable
  with the subscription provider (ADR 10) and still bounded by
  chattiness and the daily cap.
- Three tables and a migration; the pet-pack contract is untouched.
- Everything remembered is on this machine, in SQLite, listed in
  Setup, and deletable; nothing is written without showing up there.
- Project facts and diary rows name the project folder, which is
  level-1 information already shown in the bubble.
