# Changelog

All notable changes to this project will be documented in this file.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and the project uses [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- Workspace skeleton: `sidepaw-core` (event model, state machine) and
  `sidepaw-daemon` (local ingest server).
- Architecture document, contracts and initial ADRs.
- `sidepaw-daemon` is now a library with a thin `sidepaw` binary, so
  the desktop app can embed the server (ADR 6, proposed).
- ADR 7 (proposed): the brain lives in the desktop app in TypeScript.
- `docs/roadmap.md`, a status table in `docs/architecture.md`, and the
  standard community files (code of conduct, security policy, issue and
  pull request templates).
- Chinese README (`README.zh-CN.md`).

### Fixed

- Approving a Claude Code permission prompt left the pet in `Waiting`
  until the turn ended, because Claude Code sends no event on approve.
  A tool finishing or failing now clears `Waiting`.
