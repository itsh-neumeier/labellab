//! Label model to 1-bit raster bitmap: real text rendering via system
//! fonts (`text`/`fontsrc`), QR codes (`barcode`) and PNG export for
//! preview (`png`). Linear barcodes, frames, images and a bundled symbol
//! library are still open M5 scope.

pub mod barcode;
mod bitmap;
mod error;
pub mod fontsrc;
pub mod png;
pub mod text;

pub use barcode::{render_qr, QrErrorCorrection};
pub use bitmap::Bitmap;
pub use error::RenderError;
pub use text::{render_text, render_text_with_font};
