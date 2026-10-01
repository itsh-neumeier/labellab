//! Bundled symbol library: a small curated set of Material Symbols icons
//! (Apache-2.0, see `assets/symbols/NOTICE.md`), embedded at compile time
//! and rendered through the same SVG path as `picture::render_image()`.
//!
//! Custom icons don't need this module — `labellab print --image icon.svg`
//! works independently of the bundled set.

use crate::picture::render_svg_bytes;
use crate::{Bitmap, RenderError};

macro_rules! symbols {
    ($($name:literal => $file:literal),+ $(,)?) => {
        /// Bundled symbol names, see [`symbol_svg`].
        pub const SYMBOL_NAMES: &[&str] = &[$($name),+];

        /// Returns the raw SVG bytes for a bundled symbol name, or `None`
        /// if `name` isn't in [`SYMBOL_NAMES`].
        pub fn symbol_svg(name: &str) -> Option<&'static [u8]> {
            match name {
                $($name => Some(include_bytes!(concat!("../assets/symbols/", $file)))),+,
                _ => None,
            }
        }
    };
}

symbols! {
    "network" => "network.svg",
    "wifi" => "wifi.svg",
    "power" => "power.svg",
    "warning" => "warning.svg",
    "arrow-up" => "arrow_up.svg",
    "arrow-down" => "arrow_down.svg",
    "arrow-left" => "arrow_left.svg",
    "arrow-right" => "arrow_right.svg",
    "fire" => "fire.svg",
    "fire-extinguisher" => "fire_extinguisher.svg",
}

/// Renders a bundled symbol by name (see [`SYMBOL_NAMES`]) the same way
/// [`crate::render_image`] renders an SVG file: scaled to fill
/// `printable_pins` of tape height, Floyd-Steinberg dithered.
pub fn render_symbol(
    name: &str,
    head_pins: u16,
    printable_pins: u16,
    left_offset_pins: u16,
    invert: bool,
) -> Result<Bitmap, RenderError> {
    let data = symbol_svg(name).ok_or_else(|| {
        RenderError::Image(format!(
            "unknown symbol '{name}', available: {}",
            SYMBOL_NAMES.join(", ")
        ))
    })?;
    render_svg_bytes(data, head_pins, printable_pins, left_offset_pins, invert)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_bundled_symbol_renders_ink() {
        for &name in SYMBOL_NAMES {
            let bmp = render_symbol(name, 128, 50, 39, false)
                .unwrap_or_else(|e| panic!("symbol '{name}' failed to render: {e}"));
            let has_ink = (0..bmp.height_dots()).any(|y| bmp.row(y).iter().any(|&b| b != 0));
            assert!(has_ink, "symbol '{name}' rendered no ink");
        }
    }

    #[test]
    fn unknown_symbol_name_errors() {
        let err = render_symbol("does-not-exist", 128, 50, 39, false).unwrap_err();
        assert!(matches!(err, RenderError::Image(_)));
    }
}
