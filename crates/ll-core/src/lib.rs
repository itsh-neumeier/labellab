//! Device manager, print jobs, series/CSV, template storage. Series/CSV
//! and template storage land in M7; this crate currently provides device
//! discovery, status queries, the label layout model / `.llabel` format,
//! print jobs and the shared error type
//! that wraps `ll-protocol`, `ll-transport` and `ll-render` errors for the
//! CLI and GUI.

pub mod device;
mod error;
pub mod label;
pub mod print;

pub use error::CoreError;
