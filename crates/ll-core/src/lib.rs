//! Device manager, print jobs, series/CSV, template storage. Series/CSV
//! and template storage land in M7; this crate currently provides device
//! discovery, status queries, the label layout model / `.llabel` format,
//! print jobs and the shared error type
//! that wraps `ll-protocol`, `ll-transport` and `ll-render` errors for the
//! CLI and GUI.

pub mod device;
pub mod document;
mod error;
pub mod frames;
pub mod fusebox;
pub mod history;
pub mod iconsets;
pub mod label;
pub mod layouts;
pub mod pasted;
pub mod paths;
pub mod print;
pub mod series;
pub mod settings;

pub use error::CoreError;
