use thiserror::Error;

/// Errors raised while rendering a label into a raster bitmap.
#[derive(Debug, Error)]
pub enum RenderError {
    #[error("label height {height} exceeds tape geometry ({max_height} pins)")]
    HeightExceedsTape { height: u16, max_height: u16 },

    #[error("font error: {0}")]
    Font(String),

    #[error("no usable system font found; pass a font file explicitly")]
    NoSystemFont,

    #[error("image encoding failed: {0}")]
    Encode(String),

    #[error("barcode error: {0}")]
    Barcode(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}
