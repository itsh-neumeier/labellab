//! Label model to 1-bit raster bitmap: real text rendering via system
//! fonts (`text`/`fontsrc`), QR codes (`barcode`) and linear barcodes —
//! Code128/EAN-13/EAN-8/UPC-A/Code39/ITF (`linear_barcode`), imported
//! images including SVG (`picture`), a bundled symbol library
//! (`symbols`), a whole-label border (`frame`), PNG export for preview
//! (`png`), system fonts with bold/italic (`fonts`) and box-fitted
//! rendering of every element type for the free-layout editor (`boxed`).

pub mod barcode;
mod bitmap;
pub mod boxed;
pub mod decor;
mod error;
pub mod fonts;
pub mod fontsrc;
pub mod frame;
pub mod iconset;
pub mod image_edit;
pub mod linear_barcode;
pub mod picture;
pub mod png;
pub mod richtext;
pub mod shape;
pub mod symbols;
pub mod text;

pub use barcode::{render_qr, QrErrorCorrection};
pub use bitmap::Bitmap;
pub use boxed::{align_content, TextAlign, VAlign};
pub use error::RenderError;
pub use fonts::{Face, FaceSet};
pub use frame::{draw_border, draw_border_styled, Border, BorderSides, BorderStyle, Insets};
pub use iconset::{Halftone, IconSet, ICONSET_EXTENSION};
pub use image_edit::ImageEdit;
pub use linear_barcode::{render_barcode, render_barcode_with_module, render_code128, Symbology};
pub use picture::{render_image, render_image_edited, render_svg_bytes, ImageAdjust};
pub use shape::ShapeKind;
pub use symbols::{render_symbol, SYMBOL_NAMES};
pub use text::{render_text, render_text_with_font};
