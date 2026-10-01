//! Label model to 1-bit raster bitmap. Text/font/barcode/image rendering
//! lands in M5; this crate currently provides the shared `Bitmap` type used
//! by both live preview and the print path.

mod bitmap;
mod error;

pub use bitmap::Bitmap;
pub use error::RenderError;
