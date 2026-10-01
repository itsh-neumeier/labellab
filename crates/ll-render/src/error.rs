use thiserror::Error;

/// Errors raised while rendering a label into a raster bitmap.
#[derive(Debug, Error)]
pub enum RenderError {
    #[error("label height {height} exceeds tape geometry ({max_height} pins)")]
    HeightExceedsTape { height: u16, max_height: u16 },
}
