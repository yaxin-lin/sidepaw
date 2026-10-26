# Security policy

sidepaw reads what your coding agents do, so a bug here can leak
things you did not mean to store. Please report security issues
privately.

## Reporting

Use GitHub's private vulnerability reporting on this repository
("Security" tab, "Report a vulnerability"). If that is unavailable,
email the maintainer listed in `Cargo.toml` and put "sidepaw security"
in the subject.

Please include the version or commit, the host (Claude Code, Codex, ...)
and a minimal payload or steps. You should hear back within 7 days.

## Scope

In scope:

- secrets reaching SQLite, logs or a remote model despite redaction
- the permission level storing more than `docs/contracts/permissions.md`
  allows
- the local server accepting requests without the bearer token, or
  binding to anything other than `127.0.0.1`
- pet packs or hook payloads that crash or escape the loader (path
  traversal, oversized images)

Out of scope: vulnerabilities in the hosts themselves, or in a model
provider's API.

## Supported versions

Pre-1.0, only the latest release gets fixes.
