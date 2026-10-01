//! Device manager, print jobs, series/CSV, template storage. Device
//! discovery and job execution land in M2/M3; this crate currently
//! provides the shared error type that wraps `ll-protocol`, `ll-transport`
//! and `ll-render` errors for the CLI and GUI.

mod error;

pub use error::CoreError;
