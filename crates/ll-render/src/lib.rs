//! Label model to 1-bit raster bitmap: real text rendering via system
//! fonts (`text`/`fontsrc`), QR codes (`barcode`) and linear barcodes —
//! Code128/EAN-13/EAN-8/UPC-A/Code39/ITF (`linear_barcode`), imported
//! images including SVG (`picture`), a bundled symbol library
//! (`symbols`), a whole-label border (`frame`) and PNG export for
//! preview (`png`). M5 is now feature-complete at this scope (a
//! composable multi-element editor is M6).

pub mod barcode;
mod bitmap;
mod error;
pub mod fontsrc;
pub mod frame;
pub mod linear_barcode;
pub mod picture;
pub mod png;
pub mod symbols;
pub mod text;

pub use barcode::{render_qr, QrErrorCorrection};
pub use bitmap::Bitmap;
pub use error::RenderError;
pub use frame::draw_border;
pub use linear_barcode::{render_barcode, render_code128, Symbology};
pub use picture::{render_image, render_svg_bytes};
pub use symbols::{render_symbol, SYMBOL_NAMES};
pub use text::{render_text, render_text_with_font};
