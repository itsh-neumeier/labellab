use thiserror::Error;

/// Errors raised while building or parsing Brother raster protocol data.
#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("status block has wrong length: expected 32 bytes, got {0}")]
    InvalidStatusLength(usize),

    #[error("unknown model byte combination: series {series:#04x}, model {model:#04x}")]
    UnknownModel { series: u8, model: u8 },

    #[error("tape width {0} mm is not supported by this model")]
    UnsupportedTapeWidth(u8),
}
