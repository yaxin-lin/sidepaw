# Roadmap

Each step ends with a tagged release that works on its own. The riskiest
unknown is validated first. Versions are 0.x; minor versions may break
compatibility until 1.0, but the event contract's `v` field is bumped
whenever its shape changes. GitHub milestones mirror this list.

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

Codex adapter with real captured fixtures, `sidepaw hook` shim in
use, a minimal setup wizard that writes hook configs, Tauri updater
wired. Exit criterion: adding a host really was one adapter module and
one fixture directory; if not, fix the abstraction now.

## 0.3 Trustworthy records

Permission level resolved per host and per project before the filter
runs; `raw` capped or off by default; `migrations/` directory and first
migration; daily report from counters alone, no model. macOS signing
and notarization in CI. Exit criterion: fully usable with no API key.

## 0.4 It talks

Brain in TypeScript (ADR 7): one provider, streamed chat, persona
compiler producing `lines.json`, preview before send, audit rows.
Server gains read-only query endpoints.

## 0.5 It speaks first

Proactive triggers, budget, cooldowns, quiet hours, memory list.

## Later

Studio (image to spritesheet, sentence to persona), Pack v2 animation
layers (webm, Live2D), voice, multi-pet, level 4 actions, remaining
hosts (OpenCode, DSH, Kimi Code, Gemini CLI, Cursor, wrapped commands),
community pack gallery.
