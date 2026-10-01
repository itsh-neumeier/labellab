//! Brother P-touch raster protocol: commands, status parsing, PackBits
//! encoding and the per-model constant table.
//!
//! Status parser, command builder and PackBits encoder land in M2. This
//! crate must stay free of GUI/transport dependencies so it is testable
//! without hardware (see `AGENTS.md`).

mod error;
pub mod model;

pub use error::ProtocolError;
