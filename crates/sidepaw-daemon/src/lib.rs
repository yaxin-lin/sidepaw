//! Local ingest server, SQLite store and host adapters for sidepaw.
//!
//! This is a library so the desktop app can embed the server in its own
//! process; the `sidepaw` binary in `main.rs` is a thin wrapper that
//! exposes the same code as a headless daemon and as the hook shim.

pub mod adapters;
pub mod clock;
pub mod config;
pub mod home;
pub mod hook;
pub mod install;
pub mod origin;
pub mod pets;
pub mod redact;
pub mod registry;
pub mod secret;
pub mod server;
pub mod store;
