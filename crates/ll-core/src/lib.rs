//! Device manager, print jobs, series/CSV, template storage. Print job
//! execution lands in M3; this crate currently provides device discovery,
//! status queries and the shared error type that wraps `ll-protocol`,
//! `ll-transport` and `ll-render` errors for the CLI and GUI.

pub mod device;
mod error;

pub use error::CoreError;
