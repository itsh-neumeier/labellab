//! Label model to 1-bit raster bitmap. Real typography (fonts, Unicode,
//! barcodes, images) lands in M5; `text` currently provides a crude
//! placeholder bitmap font just to prove the print pipeline end-to-end.

mod bitmap;
mod error;
pub mod font;
pub mod text;

pub use bitmap::Bitmap;
pub use error::RenderError;
pub use text::render_text;
