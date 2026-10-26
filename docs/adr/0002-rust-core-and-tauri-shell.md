# 2. Rust daemon with a Tauri desktop shell

Status: accepted, 2026-10-25

## Context

The pet needs a transparent always-on-top window on Windows and macOS,
a small background process that tails logs and serves a local socket,
and a distributable that does not require the user to install Node or
Python. Existing projects: CoPet and pets-driven use Tauri; agentpet
uses native Swift on macOS and Tauri elsewhere; dsh-pet and deepfish
use Electron.

## Decision

- `sidepaw-core` and `sidepaw-daemon` in Rust. The daemon is the
  single binary users install; it also serves as the hook shim
  (`sidepaw --hook`) so no interpreter is needed.
- The desktop app is Tauri 2. Rendering is a web view, which lets the
  renderer reuse browser animation for spritesheets and later webm or
  Live2D without native code per platform.
- No native Swift build for now. One codebase, three platforms.

## Consequences

Binary size and memory are small compared with Electron. We depend on
the system web view, which means testing WebView2 on Windows and WKWebView
on macOS for transparency and click-through. A native macOS build can be
added later if the web view proves limiting.
