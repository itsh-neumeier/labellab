//! Label model to 1-bit raster bitmap: real text rendering via system
//! fonts (`text`/`fontsrc`), QR codes and Code128 barcodes (`barcode`/
//! `linear_barcode`), imported images (`picture`), a whole-label border
//! (`frame`) and PNG export for preview (`png`). A bundled symbol library
//! is still open M5 scope.

pub mod barcode;
mod bitmap;
mod error;
pub mod fontsrc;
pub mod frame;
pub mod linear_barcode;
pub mod picture;
pub mod png;
pub mod text;

pub use barcode::{render_qr, QrErrorCorrection};
pub use bitmap::Bitmap;
pub use error::RenderError;
pub use frame::draw_border;
pub use linear_barcode::render_code128;
pub use picture::render_image;
pub use text::{render_text, render_text_with_font};
