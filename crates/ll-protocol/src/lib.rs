//! Brother P-touch raster protocol: commands, status parsing, PackBits
//! encoding and the per-model constant table.
//!
//! This crate stays free of GUI/transport dependencies so it is testable
//! without hardware (see `AGENTS.md`).

pub mod command;
mod error;
pub mod media;
pub mod model;
pub mod packbits;
pub mod status;

pub use error::ProtocolError;
