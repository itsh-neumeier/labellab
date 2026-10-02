//! Symbols: the original bundled library (a curated set of Material
//! Symbols icons, Apache-2.0, see `assets/symbols/NOTICE.md`) as the
//! built-in icon set `material`, plus rendering of any icon from a
//! registered icon set (see [`crate::iconset`]) by name.
//!
//! Custom icons don't need this module — `labellab print --image icon.svg`
//! works independently of the icon sets.

use std::collections::BTreeMap;

use crate::iconset::{self, Category, Halftone, Icon, IconSet, Text};
use crate::picture::{render_gray_halftone, render_svg_to_gray};
use crate::{Bitmap, RenderError};

fn text(de: &str, en: &str) -> Text {
    Text::Localized(BTreeMap::from([
        ("de".to_owned(), de.to_owned()),
        ("en".to_owned(), en.to_owned()),
    ]))
}

macro_rules! symbols {
    ($($name:literal => $file:literal, $category:literal, $de:literal, $en:literal);+ $(;)?) => {
        /// Names of the icons in the built-in `material` set.
        pub const SYMBOL_NAMES: &[&str] = &[$($name),+];

        fn material_icons() -> Vec<Icon> {
            vec![$(Icon {
                id: $name.to_owned(),
                name: text($de, $en),
                category: Some($category.to_owned()),
                tags: Vec::new(),
                svg: include_str!(concat!("../assets/symbols/", $file)).to_owned(),
                license: None,
                author: None,
                source: None,
            }),+]
        }
    };
}

symbols! {
    "network" => "network.svg", "it", "Netzwerk", "Network";
    "wifi" => "wifi.svg", "it", "WLAN", "Wi-Fi";
    "power" => "power.svg", "electrical", "Strom", "Power";
    "warning" => "warning.svg", "safety", "Warnung", "Warning";
    "fire" => "fire.svg", "safety", "Feuer", "Fire";
    "fire-extinguisher" => "fire_extinguisher.svg", "safety", "Feuerlöscher", "Fire extinguisher";
    "arrow-up" => "arrow_up.svg", "arrows", "Pfeil hoch", "Arrow up";
    "arrow-down" => "arrow_down.svg", "arrows", "Pfeil runter", "Arrow down";
    "arrow-left" => "arrow_left.svg", "arrows", "Pfeil links", "Arrow left";
    "arrow-right" => "arrow_right.svg", "arrows", "Pfeil rechts", "Arrow right";
}

/// The original symbol library as the built-in icon set `material`.
pub fn material_set() -> IconSet {
    let mut set = IconSet::new("material", text("Basis-Symbole", "Basic symbols"));
    set.license = Some("Apache-2.0 (Material Symbols, Google)".to_owned());
    set.source = Some("https://fonts.google.com/icons".to_owned());
    set.categories = vec![
        Category {
            id: "it".into(),
            name: text("IT & Netzwerk", "IT & network"),
        },
        Category {
            id: "electrical".into(),
            name: text("Elektro", "Electrical"),
        },
        Category {
            id: "safety".into(),
            name: text("Sicherheit", "Safety"),
        },
        Category {
            id: "arrows".into(),
            name: text("Pfeile", "Arrows"),
        },
    ];
    set.icons = material_icons();
    set
}

/// SVG and halftone mode of a symbol by name (`set:icon` or `icon`).
fn lookup(name: &str) -> Result<(String, Halftone), RenderError> {
    let found = iconset::resolve(name).ok_or_else(|| iconset::unknown_symbol(name))?;
    Ok((found.icon().svg.clone(), found.set.halftone))
}

/// Renders a symbol by name (see [`crate::iconset::resolve`]) scaled to
/// fill `printable_pins` of tape height, like [`crate::render_image`]
/// renders an SVG file, with the set's halftone mode.
pub fn render_symbol(
    name: &str,
    head_pins: u16,
    printable_pins: u16,
    left_offset_pins: u16,
    invert: bool,
) -> Result<Bitmap, RenderError> {
    let (svg, halftone) = lookup(name)?;
    let gray = render_svg_to_gray(svg.as_bytes(), printable_pins)?;
    render_gray_halftone(
        &gray,
        head_pins,
        printable_pins,
        left_offset_pins,
        invert,
        halftone,
    )
}

/// SVG and halftone mode for [`crate::boxed::symbol_in_box`].
pub(crate) fn symbol_source(name: &str) -> Result<(String, Halftone), RenderError> {
    lookup(name)
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
