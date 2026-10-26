//! Pure model for sidepaw: events, the per-session state machine,
//! permission levels and the pet pack manifest. No I/O, no async.

pub mod event;
pub mod pack;
pub mod permission;
pub mod state;

pub use event::{Event, Host, Kind, Tool, ToolCategory};
pub use permission::Level;
pub use state::{Session, State};
