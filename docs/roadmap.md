# Roadmap

Each step ends with a tagged release that works on its own. The riskiest
unknown is validated first. Versions are 0.x; minor versions may break
compatibility until 1.0, but the event contract's `v` field is bumped
whenever its shape changes. GitHub milestones mirror this list.

## Next up, in order

Decided 2026-10-26 after comparing agentpet, clawd-on-desk, UniPet,
agent-pet and pets-driven. Each item ships on its own.

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
6. Model provider, persona editor, generated `lines.json`.
7. Codex adapter and `sidepaw run -- <cmd>`.
8. One pet per project.
9. Signing, notarization, Homebrew.
10. Proactive chat.

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
persona compiler. Remaining: one provider (config and key), line
generation into `lines.json` with the digest as cache key, persona
editor in the setup window, streamed chat, context builder, preview
before send, audit rows. Server gains read-only query endpoints.

## 0.5 It speaks first

Proactive triggers, budget, cooldowns, quiet hours, memory list.

## Later

Studio (image to spritesheet, sentence to persona), Pack v2 animation
layers (webm, Live2D), voice, multi-pet, level 4 actions, remaining
hosts (OpenCode, DSH, Kimi Code, Gemini CLI, Cursor, wrapped commands),
community pack gallery.
