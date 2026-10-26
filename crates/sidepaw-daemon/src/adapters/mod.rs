//! Host adapters. Each one turns a host's native payload into a
//! `sidepaw_core::Event`. Adapters are plain functions; add a trait
//! only if two of them ever need to be swapped at runtime.
//!
//! Adding a host: new module here, a row in the table in
//! `docs/architecture.md`, and real captured payloads under
//! `tests/fixtures/<host>/`.

pub mod claude_code;

use sidepaw_core::{Event, Host};

/// Convert a native payload for `host`. `None` means the payload is
/// one we deliberately ignore, not an error.
pub fn from_native(host: Host, payload: &serde_json::Value) -> Option<Event> {
    match host {
        Host::ClaudeCode => claude_code::from_hook(payload),
        // The remaining hosts are mapped in docs/architecture.md and
        // land here one at a time, each with fixtures.
        _ => None,
    }
}
