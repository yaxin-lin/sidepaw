# Roadmap

Each step ends with a tagged release that works on its own. The riskiest
unknown is validated first. Versions are 0.x; minor versions may break
compatibility until 1.0, but the event contract's `v` field is bumped
whenever its shape changes. GitHub milestones mirror this list.

## Next up, in order

Decided 2026-10-26 after comparing agentpet, clawd-on-desk, UniPet,
agent-pet and pets-driven, revisited 2026-10-27 after a wider survey
(openpets, Clyde, CoPet, claude-pet, vibe-pet). Each item ships on its
own. Done since that survey: jump to the session's terminal; tray
toggles for do-not-disturb, mini mode, size and start at login.

1. Done: system notifications with sound on waiting, error and done
   (L3), with a permission level picker in the setup window.
2. Done: tray session monitor: host, project, state and duration per
   session; sessions that need input first, icon tinted.
3. Done: Allow / Deny in the bubble while waiting, answered through
   the hook response (L4, per host, every answer recorded).
4. Done: idle behaviour: short walks, dozing, a hop on click. Looking
   at the cursor needs art the Codex sheet does not have; revisit with
   Pack v2.
5. Done: Chinese UI (web views, tray, notifications) following the
   system language; packs may ship `lines.<lang>.json`.
6. Done: model provider, persona editor, generated `lines.json`, with
   preview before send and audit rows.
7. Codex adapter. Done: `sidepaw run -- <cmd>`, the installed app as
   the CLI with a setup button to put it on PATH, and
   docs/integrations.md for tools that post events themselves.
8. One pet per project.
9. Desktop life. Done: throw it (fling physics, bounces off the work
   area), it steps away from a cursor that lingers beside it and sizes
   up one that hovers, files dropped on it get a line; it lands on
   other apps' windows and falls when they move or close, and hides
   while the app in front is full screen (macOS and Windows, no
   permission needed). Next, still without OS permissions: system
   idle time (lands with 0.5, where it is used). Last, opt-in and
   gated by level: global input and screen reading.
10. Signing, notarization, Homebrew.
11. Done: proactive chat and the memory list (0.5).

Not planned: token feeding, levels, achievements, leaderboards.

## 0.1 It moves

One vertical slice. Tauri 2 app with a transparent, borderless,
always-on-top window; spritesheet renderer for a Codex-layout pack
(a generated placeholder until a licensed pack is chosen); server
embedded (ADR 6); Claude Code `http` hook posting straight in. Hook
config pasted by hand. No chat, no wizard. Shared types generated from
Rust. Click-through of transparent pixels moved to 0.2: it needs
per-pixel hit testing because a fully click-through window cannot be
dragged. Exit criterion: the pet changes rows on both macOS and Windows
while Claude Code works.

## 0.2 Second host

Pack choice is done: install from a zip, a folder, Codex's pets or the
petdex gallery, and switch in Setup > Look.

Codex adapter with real captured fixtures and the `sidepaw hook` shim
in use. Done already: the setup window for Claude Code, click-through
of transparent pixels, Tauri updater and the release workflow. Exit criterion: adding a host really was one adapter module
and one fixture directory; if not, fix the abstraction now.

## 0.3 Trustworthy records

Done: permission level resolved per host and per project before the
filter runs, `raw` capped at 64 KB, `config.toml` loaded, tray icon,
daily report from counters. Remaining: `migrations/` directory and
first migration; macOS signing and
notarization in CI. Exit criterion: fully usable with no API key.

## 0.4 It talks

Brain in TypeScript (ADR 7). Done: speech bubble with pack lines,
persona compiler, one provider (config and key), line generation into
`lines.json` with the digest as cache key, persona editor in the setup
window, preview before send, audit rows, streamed chat window with a
context builder over the server's read-only queries (in-process
commands rather than HTTP routes, since the brain is in the same
process). Exit criterion met: the pet can be asked what the agents did
today and answers in character.

## 0.5 It speaks first

Done: nine proactive triggers from local lines, chattiness 0-5 in the
tray, quiet hours, cooldowns that learn from dismissals and answers,
and a memory list the user sees and edits, used in chat. Remaining:
lines written by the model for event-specific moments (opt-in, since
it costs a call each time), and the pet offering a higher permission
level itself after a few days, as the design notes suggest.

## 0.6 It lives on the desktop

The permission-gated tier of item 9 (global input, screen reading),
each behind its own switch, and Pack v2 art for looking at the cursor.

## Later

Studio (image to spritesheet, sentence to persona), Pack v2 animation
layers (webm, Live2D), voice, multi-pet, level 4 actions, remaining
hosts (OpenCode, DSH, Kimi Code, Gemini CLI, Cursor, wrapped commands),
community pack gallery.
