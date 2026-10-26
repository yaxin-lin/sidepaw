//! Host adapters. Each one turns a host's native payload into a
//! `sidepaw_core::Event`. Adapters are plain functions; add a trait
//! only if two of them ever need to be swapped at runtime.
//!
//! Adding a host: new module here, a row in the table in
//! `docs/architecture.md`, and real captured payloads under
//! `tests/fixtures/<host>/`.

pub mod claude_code;

use sidepaw_core::{Event, Host};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

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

pub fn now_rfc3339() -> String {
    OffsetDateTime::now_utc()
        .replace_nanosecond(OffsetDateTime::now_utc().millisecond() as u32 * 1_000_000)
        .unwrap_or_else(|_| OffsetDateTime::now_utc())
        .format(&Rfc3339)
        .unwrap_or_default()
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
